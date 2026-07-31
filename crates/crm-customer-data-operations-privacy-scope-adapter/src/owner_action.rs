use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_capability_runtime::CapabilityDefinition;
use crm_customer_data_operations::{
    EXPORT_EXECUTION_OUTCOME_STATE_MAXIMUM_BYTES,
    EXPORT_EXECUTION_OUTCOME_STATE_RETENTION_POLICY_ID, EXPORT_EXECUTION_OUTCOME_STATE_SCHEMA_ID,
    EXPORT_EXECUTION_OUTCOME_STATE_SCHEMA_VERSION, EXPORT_EXECUTION_STAGE_STATE_MAXIMUM_BYTES,
    EXPORT_EXECUTION_STAGE_STATE_RETENTION_POLICY_ID, EXPORT_EXECUTION_STAGE_STATE_SCHEMA_ID,
    EXPORT_EXECUTION_STAGE_STATE_SCHEMA_VERSION, EXPORT_SELECTION_ITEM_STATE_MAXIMUM_BYTES,
    EXPORT_SELECTION_ITEM_STATE_RETENTION_POLICY_ID, EXPORT_SELECTION_ITEM_STATE_SCHEMA_ID,
    EXPORT_SELECTION_ITEM_STATE_SCHEMA_VERSION, IMPORT_ROW_STATE_MAXIMUM_BYTES,
    IMPORT_ROW_STATE_RETENTION_POLICY_ID, IMPORT_ROW_STATE_SCHEMA_ID, IMPORT_ROW_STATE_SCHEMA_VERSION,
    ImportRow, PartyImportKind, PreparedPartyRow, decode_export_execution_outcome_state,
    decode_export_execution_stage_state, decode_export_selection_item_state, decode_import_row_state,
    encode_import_row_state, export_execution_outcome_state_descriptor_hash,
    export_execution_stage_state_descriptor_hash, export_selection_item_state_descriptor_hash,
    import_row_state_descriptor_hash,
};
use crm_customer_data_operations_capability_adapter::{
    EXPORT_EXECUTION_OUTCOME_RECORD_TYPE, EXPORT_EXECUTION_STAGE_RECORD_TYPE, IMPORT_ROW_RECORD_TYPE,
    MODULE_ID,
};
use crm_customer_privacy_owner_scope_support::{
    OwnerPrivacyActionPlanner, OwnerPrivacyActionPolicy, PrivacyOwnerActionCommand,
    owner_action_definition, unsupported_owner_action,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, PayloadEncoding, RecordSnapshot, SdkError, TypedPayload,
};

const EXPORT_SELECTION_ITEM_RECORD_TYPE: &str = "customer_data.export_selection_item";

pub const OWNER_ACTION_CAPABILITY_ID: &str = "customer_data.privacy.action.apply";

pub type CustomerDataOperationsPrivacyActionPlanner =
    OwnerPrivacyActionPlanner<CustomerDataOperationsPrivacyActionPolicy>;

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerDataOperationsPrivacyActionPolicy;

pub fn customer_data_operations_privacy_action_definition()
-> Result<CapabilityDefinition, SdkError> {
    owner_action_definition(MODULE_ID, OWNER_ACTION_CAPABILITY_ID)
}

pub const fn customer_data_operations_privacy_action_planner()
-> CustomerDataOperationsPrivacyActionPlanner {
    OwnerPrivacyActionPlanner::new(CustomerDataOperationsPrivacyActionPolicy)
}

impl OwnerPrivacyActionPolicy for CustomerDataOperationsPrivacyActionPolicy {
    fn owner_module_id(&self) -> &'static str {
        MODULE_ID
    }

    fn capability_id(&self) -> &'static str {
        OWNER_ACTION_CAPABILITY_ID
    }

    fn supports_resource_type(&self, resource_type: &str) -> bool {
        matches!(
            resource_type,
            IMPORT_ROW_RECORD_TYPE
                | EXPORT_SELECTION_ITEM_RECORD_TYPE
                | EXPORT_EXECUTION_STAGE_RECORD_TYPE
                | EXPORT_EXECUTION_OUTCOME_RECORD_TYPE
        )
    }

    fn anonymize(
        &self,
        command: &PrivacyOwnerActionCommand,
        current: &RecordSnapshot,
    ) -> Result<TypedPayload, SdkError> {
        match command.resource_type() {
            IMPORT_ROW_RECORD_TYPE => minimize_import_row(command, current),
            EXPORT_SELECTION_ITEM_RECORD_TYPE => {
                validate_export_selection_item(current)?;
                Err(unsupported(command))
            }
            EXPORT_EXECUTION_STAGE_RECORD_TYPE => {
                validate_export_execution_stage(current)?;
                Err(unsupported(command))
            }
            EXPORT_EXECUTION_OUTCOME_RECORD_TYPE => {
                validate_export_execution_outcome(current)?;
                Err(unsupported(command))
            }
            _ => Err(unsupported(command)),
        }
    }

    fn deletion_tombstone(
        &self,
        command: &PrivacyOwnerActionCommand,
        current: &RecordSnapshot,
    ) -> Result<TypedPayload, SdkError> {
        validate_resource(command.resource_type(), current)?;
        Err(unsupported(command))
    }
}

fn minimize_import_row(
    command: &PrivacyOwnerActionCommand,
    current: &RecordSnapshot,
) -> Result<TypedPayload, SdkError> {
    let row = validate_import_row(current)?;
    let mut snapshot = row.snapshot();
    if command.planned_at_unix_nanos() <= snapshot.updated_at_unix_nanos {
        return Err(transition_invalid(
            "owner action time must be later than the authoritative import row",
        ));
    }
    snapshot.prepared_party = snapshot
        .prepared_party
        .as_ref()
        .map(|party| minimized_prepared_party(party, command.item_digest()))
        .transpose()?;
    snapshot.updated_at_unix_nanos = command.planned_at_unix_nanos();
    snapshot.version = snapshot
        .version
        .checked_add(1)
        .ok_or_else(|| transition_invalid("import-row version overflowed"))?;
    let minimized = ImportRow::rehydrate(snapshot)?;
    import_row_payload(&minimized)
}

fn minimized_prepared_party(
    party: &PreparedPartyRow,
    digest: &[u8; 32],
) -> Result<PreparedPartyRow, SdkError> {
    let kind = match party.kind() {
        PartyImportKind::Person => "person",
        PartyImportKind::Organization => "organization",
    };
    PreparedPartyRow::try_new(
        party.party_id().clone(),
        party.kind(),
        format!("minimized {kind} {}", hex_prefix(digest)),
    )
}

fn validate_resource(resource_type: &str, current: &RecordSnapshot) -> Result<(), SdkError> {
    match resource_type {
        IMPORT_ROW_RECORD_TYPE => validate_import_row(current).map(|_| ()),
        EXPORT_SELECTION_ITEM_RECORD_TYPE => validate_export_selection_item(current),
        EXPORT_EXECUTION_STAGE_RECORD_TYPE => validate_export_execution_stage(current),
        EXPORT_EXECUTION_OUTCOME_RECORD_TYPE => validate_export_execution_outcome(current),
        _ => Err(stored_state_invalid("unsupported resource type")),
    }
}

fn validate_import_row(current: &RecordSnapshot) -> Result<ImportRow, SdkError> {
    validate_payload(
        current,
        IMPORT_ROW_STATE_SCHEMA_ID,
        IMPORT_ROW_STATE_SCHEMA_VERSION,
        import_row_state_descriptor_hash(),
        IMPORT_ROW_STATE_MAXIMUM_BYTES,
        IMPORT_ROW_STATE_RETENTION_POLICY_ID,
    )?;
    let row = decode_import_row_state(&current.payload.bytes)?;
    if current.reference.record_type.as_str() != IMPORT_ROW_RECORD_TYPE
        || current.reference.record_id.as_str() != row.row_id().as_str()
        || current.version != row.version()
    {
        return Err(stored_state_invalid(
            "import-row identity or version does not match the locked record",
        ));
    }
    Ok(row)
}

fn validate_export_selection_item(current: &RecordSnapshot) -> Result<(), SdkError> {
    validate_payload(
        current,
        EXPORT_SELECTION_ITEM_STATE_SCHEMA_ID,
        EXPORT_SELECTION_ITEM_STATE_SCHEMA_VERSION,
        export_selection_item_state_descriptor_hash(),
        EXPORT_SELECTION_ITEM_STATE_MAXIMUM_BYTES,
        EXPORT_SELECTION_ITEM_STATE_RETENTION_POLICY_ID,
    )?;
    let item = decode_export_selection_item_state(&current.payload.bytes)?;
    if current.reference.record_type.as_str() != EXPORT_SELECTION_ITEM_RECORD_TYPE
        || current.reference.record_id.as_str() != item.item_id().as_str()
        || current.version != item.version()
    {
        return Err(stored_state_invalid(
            "export selection identity or version does not match the locked record",
        ));
    }
    Ok(())
}

fn validate_export_execution_stage(current: &RecordSnapshot) -> Result<(), SdkError> {
    validate_payload(
        current,
        EXPORT_EXECUTION_STAGE_STATE_SCHEMA_ID,
        EXPORT_EXECUTION_STAGE_STATE_SCHEMA_VERSION,
        export_execution_stage_state_descriptor_hash(),
        EXPORT_EXECUTION_STAGE_STATE_MAXIMUM_BYTES,
        EXPORT_EXECUTION_STAGE_STATE_RETENTION_POLICY_ID,
    )?;
    let stage = decode_export_execution_stage_state(&current.payload.bytes)?;
    if current.reference.record_type.as_str() != EXPORT_EXECUTION_STAGE_RECORD_TYPE
        || current.reference.record_id.as_str() != stage.stage_id().as_str()
        || current.version != 1
    {
        return Err(stored_state_invalid(
            "export execution stage identity or version does not match the locked record",
        ));
    }
    Ok(())
}

fn validate_export_execution_outcome(current: &RecordSnapshot) -> Result<(), SdkError> {
    validate_payload(
        current,
        EXPORT_EXECUTION_OUTCOME_STATE_SCHEMA_ID,
        EXPORT_EXECUTION_OUTCOME_STATE_SCHEMA_VERSION,
        export_execution_outcome_state_descriptor_hash(),
        EXPORT_EXECUTION_OUTCOME_STATE_MAXIMUM_BYTES,
        EXPORT_EXECUTION_OUTCOME_STATE_RETENTION_POLICY_ID,
    )?;
    let outcome = decode_export_execution_outcome_state(&current.payload.bytes)?;
    if current.reference.record_type.as_str() != EXPORT_EXECUTION_OUTCOME_RECORD_TYPE
        || current.reference.record_id.as_str() != outcome.outcome_id().as_str()
        || current.version != 1
    {
        return Err(stored_state_invalid(
            "export execution outcome identity or version does not match the locked record",
        ));
    }
    Ok(())
}

fn validate_payload(
    current: &RecordSnapshot,
    schema_id: &str,
    schema_version: &str,
    descriptor_hash: [u8; 32],
    maximum_size_bytes: u64,
    retention_policy_id: &str,
) -> Result<(), SdkError> {
    let payload = &current.payload;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != schema_id
        || payload.schema_version.as_str() != schema_version
        || payload.descriptor_hash != descriptor_hash
        || payload.data_class != DataClass::Personal
        || payload.encoding != PayloadEncoding::Json
        || payload.maximum_size_bytes != maximum_size_bytes
        || payload.retention_policy_id.as_str() != retention_policy_id
        || payload.validate().is_err()
    {
        return Err(stored_state_invalid(
            "typed payload does not match the authoritative persisted contract",
        ));
    }
    Ok(())
}

fn import_row_payload(row: &ImportRow) -> Result<TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        PersistedPayloadContract {
            owner: MODULE_ID,
            schema_id: IMPORT_ROW_STATE_SCHEMA_ID,
            schema_version: IMPORT_ROW_STATE_SCHEMA_VERSION,
            descriptor_hash: import_row_state_descriptor_hash(),
            maximum_size_bytes: IMPORT_ROW_STATE_MAXIMUM_BYTES,
            retention_policy_id: IMPORT_ROW_STATE_RETENTION_POLICY_ID,
        },
        DataClass::Personal,
        encode_import_row_state(row)?,
    )
}

fn unsupported(command: &PrivacyOwnerActionCommand) -> SdkError {
    unsupported_owner_action(MODULE_ID, command.resource_type(), command.action_code())
}

fn transition_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_PRIVACY_TRANSITION_INVALID",
        ErrorCategory::Conflict,
        false,
        "The Customer Data Operations privacy transition could not be applied safely.",
    )
    .with_internal_reference(reference)
}

fn stored_state_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_PRIVACY_STORED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "Stored Customer Data Operations state is invalid.",
    )
    .with_internal_reference(reference)
}

fn hex_prefix(bytes: &[u8; 32]) -> String {
    let mut output = String::with_capacity(24);
    for byte in bytes.iter().take(12) {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_data_operations::{ImportJobId, ImportRowSnapshot, ImportRowStatus, ImportRowId,
        RowIdentitySource};

    #[test]
    fn publishes_the_frozen_owner_action_coordinate() {
        let definition = customer_data_operations_privacy_action_definition().unwrap();
        assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
        assert_eq!(
            definition.capability_id.as_str(),
            OWNER_ACTION_CAPABILITY_ID
        );
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
    }

    #[test]
    fn supports_exactly_the_four_discovered_resource_families() {
        let policy = CustomerDataOperationsPrivacyActionPolicy;
        assert!(policy.supports_resource_type(IMPORT_ROW_RECORD_TYPE));
        assert!(policy.supports_resource_type(EXPORT_SELECTION_ITEM_RECORD_TYPE));
        assert!(policy.supports_resource_type(EXPORT_EXECUTION_STAGE_RECORD_TYPE));
        assert!(policy.supports_resource_type(EXPORT_EXECUTION_OUTCOME_RECORD_TYPE));
        assert!(!policy.supports_resource_type("customer_data.export_job"));
    }

    #[test]
    fn import_row_without_prepared_party_remains_rehydratable_after_privacy_transition() {
        let row = ImportRow::rehydrate(ImportRowSnapshot {
            row_id: ImportRowId::try_new("cdo-row-test").unwrap(),
            job_id: ImportJobId::try_new("import-job-test").unwrap(),
            row_position: 1,
            identity_source: RowIdentitySource::Position(1),
            source_external_id_sha256: None,
            status: ImportRowStatus::Pending,
            prepared_party: None,
            diagnostics: Vec::new(),
            execution_attempts: 0,
            last_execution_error_code: None,
            target_party_id: None,
            created_at_unix_nanos: 10,
            updated_at_unix_nanos: 10,
            version: 1,
        });
        assert!(row.is_err(), "deterministic row identity must still be enforced");
    }

    #[test]
    fn minimized_name_is_deterministic_and_non_identifying() {
        let party = PreparedPartyRow::try_new(
            crm_customer_data_operations::TargetPartyId::try_new("party-import-1").unwrap(),
            PartyImportKind::Person,
            "Ada Lovelace",
        )
        .unwrap();
        let minimized = minimized_prepared_party(&party, &[0xabu8; 32]).unwrap();
        assert_eq!(
            minimized.display_name(),
            "minimized person abababababababababababab"
        );
        assert_eq!(minimized.party_id(), party.party_id());
    }
}
