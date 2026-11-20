use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_privacy::{CustomerDataLegalHold, LegalHoldScope, LegalHoldStatus, MODULE_ID};
use crm_customer_privacy_persistence_adapter::{
    legal_hold_persisted_payload, legal_hold_record_ref,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordId, RecordSnapshot,
    SchemaVersion, SdkError,
};
use crm_proto_contracts::crm::{customer::v1 as customer, customer_privacy::v1 as wire};
use sha2::{Digest, Sha256};

pub const PLACE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY: &str = "customer_privacy.legal_hold.place";
pub const PLACE_CUSTOMER_DATA_LEGAL_HOLD_REQUEST_SCHEMA: &str =
    "crm.customer_privacy.v1.PlaceCustomerDataLegalHoldRequest";
pub const PLACE_CUSTOMER_DATA_LEGAL_HOLD_RESPONSE_SCHEMA: &str =
    "crm.customer_privacy.v1.PlaceCustomerDataLegalHoldResponse";
pub const CUSTOMER_DATA_LEGAL_HOLD_PLACED_EVENT_TYPE: &str = "customer_privacy.legal_hold.placed";
pub const CUSTOMER_DATA_LEGAL_HOLD_PLACED_EVENT_SCHEMA: &str =
    "crm.customer_privacy.v1.CustomerDataLegalHoldPlacedEvent";

const LEGAL_HOLD_ID_DOMAIN: &[u8] = b"crm.customer-privacy.legal-hold/v1";
const LEGAL_HOLD_ID_PREFIX: &str = "customer-data-legal-hold-";
const NANOS_PER_MILLISECOND: i64 = 1_000_000;

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerPrivacyLegalHoldPlaceCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerPrivacyLegalHoldPlaceCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let hold = customer_data_legal_hold_from_place_request(request)?;
        Ok(AggregateTarget {
            reference: legal_hold_record_ref(&hold)?,
            presence: AggregatePresence::MustBeAbsent,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_definition(definition, request)?;
        if current.is_some() {
            return Err(plan_invalid(
                "deterministic customer-data legal hold already exists",
            ));
        }
        let command = decode_place_request(request)?;
        let effective_from_unix_nanos = millis_to_nanos(
            command.effective_from_unix_ms,
            "customer_privacy.legal_hold.effective_from_unix_ms",
        )?;
        let effective_until_unix_nanos = command
            .effective_until_unix_ms
            .map(|value| {
                millis_to_nanos(value, "customer_privacy.legal_hold.effective_until_unix_ms")
            })
            .transpose()?;
        let hold = legal_hold_from_command(
            request,
            &command,
            effective_from_unix_nanos,
            effective_until_unix_nanos,
        )?;
        plan_legal_hold_place(
            definition,
            request,
            hold,
            effective_from_unix_nanos,
            effective_until_unix_nanos,
        )
    }
}

pub fn place_customer_data_legal_hold_capability_definition()
-> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(
            PLACE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY,
        ))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            PLACE_CUSTOMER_DATA_LEGAL_HOLD_REQUEST_SCHEMA,
            vec![DataClass::Personal],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            PLACE_CUSTOMER_DATA_LEGAL_HOLD_RESPONSE_SCHEMA,
            vec![DataClass::Personal],
        )?),
        risk: CapabilityRisk::High,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: PLACE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

pub fn deterministic_customer_data_legal_hold_id(
    tenant_id: &str,
    idempotency_key: &str,
) -> Result<RecordId, SdkError> {
    let mut hasher = Sha256::new();
    hasher.update(LEGAL_HOLD_ID_DOMAIN);
    update_length_framed(&mut hasher, tenant_id.as_bytes());
    update_length_framed(&mut hasher, idempotency_key.as_bytes());
    let digest = hasher.finalize();
    RecordId::try_new(format!("{LEGAL_HOLD_ID_PREFIX}{}", hex(&digest)))
        .map_err(configuration_error)
}

pub fn customer_data_legal_hold_canonical_party_id_from_request(
    request: &CapabilityRequest,
) -> Result<RecordId, SdkError> {
    request.context.validate()?;
    canonical_party_id_from_wire(
        decode_place_request(request)?.canonical_party_ref,
        "customer_privacy.legal_hold.canonical_party_ref",
    )
}

pub fn customer_data_legal_hold_from_place_request(
    request: &CapabilityRequest,
) -> Result<CustomerDataLegalHold, SdkError> {
    request.context.validate()?;
    let command = decode_place_request(request)?;
    let effective_from_unix_nanos = millis_to_nanos(
        command.effective_from_unix_ms,
        "customer_privacy.legal_hold.effective_from_unix_ms",
    )?;
    let effective_until_unix_nanos = command
        .effective_until_unix_ms
        .map(|value| millis_to_nanos(value, "customer_privacy.legal_hold.effective_until_unix_ms"))
        .transpose()?;
    legal_hold_from_command(
        request,
        &command,
        effective_from_unix_nanos,
        effective_until_unix_nanos,
    )
}

fn legal_hold_from_command(
    request: &CapabilityRequest,
    command: &wire::PlaceCustomerDataLegalHoldRequest,
    effective_from_unix_nanos: i64,
    effective_until_unix_nanos: Option<i64>,
) -> Result<CustomerDataLegalHold, SdkError> {
    CustomerDataLegalHold::place(
        deterministic_customer_data_legal_hold_id(
            request.context.execution.tenant_id.as_str(),
            request.context.execution.idempotency_key.as_str(),
        )?,
        request.context.execution.tenant_id.clone(),
        canonical_party_id_from_wire(
            command.canonical_party_ref.clone(),
            "customer_privacy.legal_hold.canonical_party_ref",
        )?,
        legal_hold_scope_from_wire(command.scope.clone())?,
        RecordId::try_new(command.authority_reference_id.clone()).map_err(|error| {
            SdkError::invalid_argument(
                "customer_privacy.legal_hold.authority_reference_id",
                error.to_string(),
            )
        })?,
        command.reason_code.clone(),
        policy_version(command.policy_version.clone())?,
        request.context.execution.actor_id.clone(),
        effective_from_unix_nanos,
        effective_until_unix_nanos,
    )
    .map_err(domain_error)
}

fn plan_legal_hold_place(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    hold: CustomerDataLegalHold,
    effective_from_unix_nanos: i64,
    effective_until_unix_nanos: Option<i64>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let aggregate = legal_hold_record_ref(&hold)?;
    let public_hold =
        legal_hold_to_wire(&hold, effective_from_unix_nanos, effective_until_unix_nanos)?;
    let output = support::protobuf_payload(
        MODULE_ID,
        PLACE_CUSTOMER_DATA_LEGAL_HOLD_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::PlaceCustomerDataLegalHoldResponse {
            customer_data_legal_hold: Some(public_hold.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: CUSTOMER_DATA_LEGAL_HOLD_PLACED_EVENT_TYPE,
            event_schema_id: CUSTOMER_DATA_LEGAL_HOLD_PLACED_EVENT_SCHEMA,
            aggregate_version: 1,
            previous_version: None,
        },
        DataClass::Personal,
        &wire::CustomerDataLegalHoldPlacedEvent {
            customer_data_legal_hold: Some(public_hold),
        },
    )?;
    let audit = support::audit_intent(
        request,
        &aggregate,
        1,
        definition.capability_id.as_str(),
        &output.bytes,
    )?;

    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records: vec![RecordMutation::Create {
                reference: aggregate,
                payload: legal_hold_persisted_payload(&hold)?,
            }],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(definition, request)?,
            audits: vec![audit],
        },
        output: Some(output),
    })
}

fn legal_hold_to_wire(
    hold: &CustomerDataLegalHold,
    effective_from_unix_nanos: i64,
    effective_until_unix_nanos: Option<i64>,
) -> Result<wire::CustomerDataLegalHold, SdkError> {
    if hold.status() != LegalHoldStatus::Active || hold.version() != 1 {
        return Err(plan_invalid(
            "legal_hold.place output must contain an active version-1 legal hold",
        ));
    }
    Ok(wire::CustomerDataLegalHold {
        customer_data_legal_hold_ref: Some(wire::CustomerDataLegalHoldRef {
            customer_data_legal_hold_id: hold.hold_id().as_str().to_owned(),
        }),
        canonical_party_ref: Some(customer::PartyRef {
            party_id: hold.canonical_party_id().as_str().to_owned(),
        }),
        scope: Some(legal_hold_scope_to_wire(hold.scope())),
        authority_reference_id: hold.authority_reference().as_str().to_owned(),
        reason_code: hold.reason_code().to_owned(),
        policy_version: hold.policy_version().as_str().to_owned(),
        status: wire::CustomerDataLegalHoldStatus::Active as i32,
        version: i64::try_from(hold.version())
            .map_err(|_| plan_invalid("legal-hold version exceeds the wire range"))?,
        placed_by_actor_id: hold.placed_by().as_str().to_owned(),
        effective_from_unix_ms: nanos_to_millis(
            effective_from_unix_nanos,
            "customer_privacy.legal_hold.effective_from_unix_ms",
        )?,
        effective_until_unix_ms: effective_until_unix_nanos
            .map(|value| {
                nanos_to_millis(value, "customer_privacy.legal_hold.effective_until_unix_ms")
            })
            .transpose()?,
        released_by_actor_id: None,
        released_at_unix_ms: None,
    })
}

fn decode_place_request(
    request: &CapabilityRequest,
) -> Result<wire::PlaceCustomerDataLegalHoldRequest, SdkError> {
    support::decode_request_with_data_class(
        request,
        MODULE_ID,
        PLACE_CUSTOMER_DATA_LEGAL_HOLD_REQUEST_SCHEMA,
        DataClass::Personal,
    )
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != PLACE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY
        || definition.capability_version.as_str() != support::CONTRACT_VERSION
        || request.context.module_id.as_str() != MODULE_ID
        || request.context.execution.capability_id.as_str()
            != PLACE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY
        || request.context.execution.capability_version.as_str() != support::CONTRACT_VERSION
    {
        return Err(plan_invalid(
            "capability definition does not match the exact legal_hold.place request coordinate",
        ));
    }
    Ok(())
}

fn canonical_party_id_from_wire(
    reference: Option<customer::PartyRef>,
    field: &'static str,
) -> Result<RecordId, SdkError> {
    let reference = reference.ok_or_else(|| {
        SdkError::invalid_argument(field, "Canonical Party reference is required.")
    })?;
    RecordId::try_new(reference.party_id)
        .map_err(|error| SdkError::invalid_argument(field, error.to_string()))
}

fn legal_hold_scope_from_wire(
    value: Option<wire::CustomerDataLegalHoldScope>,
) -> Result<LegalHoldScope, SdkError> {
    let scope = value.and_then(|value| value.scope).ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_privacy.legal_hold.scope",
            "Legal-hold scope is required.",
        )
    })?;
    match scope {
        wire::customer_data_legal_hold_scope::Scope::AllCustomerData(true) => {
            Ok(LegalHoldScope::AllCustomerData)
        }
        wire::customer_data_legal_hold_scope::Scope::AllCustomerData(false) => {
            Err(SdkError::invalid_argument(
                "customer_privacy.legal_hold.scope.all_customer_data",
                "All-customer-data scope must be true when selected.",
            ))
        }
        wire::customer_data_legal_hold_scope::Scope::DataClass(value) => {
            Ok(LegalHoldScope::DataClass(data_class_from_wire(value)?))
        }
        wire::customer_data_legal_hold_scope::Scope::OwnerModuleId(value) => {
            ModuleId::try_new(value)
                .map(LegalHoldScope::Owner)
                .map_err(|error| {
                    SdkError::invalid_argument(
                        "customer_privacy.legal_hold.scope.owner_module_id",
                        error.to_string(),
                    )
                })
        }
    }
}

fn legal_hold_scope_to_wire(value: &LegalHoldScope) -> wire::CustomerDataLegalHoldScope {
    let scope = match value {
        LegalHoldScope::AllCustomerData => {
            wire::customer_data_legal_hold_scope::Scope::AllCustomerData(true)
        }
        LegalHoldScope::DataClass(value) => {
            wire::customer_data_legal_hold_scope::Scope::DataClass(data_class_to_wire(*value))
        }
        LegalHoldScope::Owner(value) => {
            wire::customer_data_legal_hold_scope::Scope::OwnerModuleId(value.as_str().to_owned())
        }
    };
    wire::CustomerDataLegalHoldScope { scope: Some(scope) }
}

fn data_class_from_wire(value: i32) -> Result<DataClass, SdkError> {
    match wire::CustomerDataClass::try_from(value) {
        Ok(wire::CustomerDataClass::Public) => Ok(DataClass::Public),
        Ok(wire::CustomerDataClass::Internal) => Ok(DataClass::Internal),
        Ok(wire::CustomerDataClass::Confidential) => Ok(DataClass::Confidential),
        Ok(wire::CustomerDataClass::Personal) => Ok(DataClass::Personal),
        Ok(wire::CustomerDataClass::SensitivePersonal) => Ok(DataClass::SensitivePersonal),
        Ok(wire::CustomerDataClass::Biometric) => Ok(DataClass::Biometric),
        Ok(wire::CustomerDataClass::Financial) => Ok(DataClass::Financial),
        Ok(wire::CustomerDataClass::Credential) => Ok(DataClass::Credential),
        Ok(wire::CustomerDataClass::Restricted) => Ok(DataClass::Restricted),
        Ok(wire::CustomerDataClass::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "customer_privacy.legal_hold.scope.data_class",
            "Legal-hold data class is unsupported.",
        )),
    }
}

fn data_class_to_wire(value: DataClass) -> i32 {
    match value {
        DataClass::Public => wire::CustomerDataClass::Public as i32,
        DataClass::Internal => wire::CustomerDataClass::Internal as i32,
        DataClass::Confidential => wire::CustomerDataClass::Confidential as i32,
        DataClass::Restricted => wire::CustomerDataClass::Restricted as i32,
        DataClass::Personal => wire::CustomerDataClass::Personal as i32,
        DataClass::SensitivePersonal => wire::CustomerDataClass::SensitivePersonal as i32,
        DataClass::Biometric => wire::CustomerDataClass::Biometric as i32,
        DataClass::Financial => wire::CustomerDataClass::Financial as i32,
        DataClass::Credential => wire::CustomerDataClass::Credential as i32,
    }
}

fn policy_version(value: String) -> Result<SchemaVersion, SdkError> {
    if value.trim().is_empty() {
        return Err(SdkError::invalid_argument(
            "customer_privacy.legal_hold.policy_version",
            "Policy version is required.",
        ));
    }
    SchemaVersion::try_new(value).map_err(|error| {
        SdkError::invalid_argument(
            "customer_privacy.legal_hold.policy_version",
            error.to_string(),
        )
    })
}

fn millis_to_nanos(value: i64, field: &'static str) -> Result<i64, SdkError> {
    if value < 0 {
        return Err(SdkError::invalid_argument(
            field,
            "Unix timestamp must not be negative.",
        ));
    }
    value.checked_mul(NANOS_PER_MILLISECOND).ok_or_else(|| {
        SdkError::invalid_argument(field, "Unix timestamp exceeds the supported range.")
    })
}

fn nanos_to_millis(value: i64, field: &'static str) -> Result<i64, SdkError> {
    if value < 0 {
        return Err(plan_invalid(format!("{field} is negative")));
    }
    Ok(value / NANOS_PER_MILLISECOND)
}

fn update_length_framed(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(configuration_error)
}

fn configuration_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_LEGAL_HOLD_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The customer-data legal-hold capability is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

fn domain_error(error: crm_customer_privacy::PrivacyDomainError) -> SdkError {
    let category = match error {
        crm_customer_privacy::PrivacyDomainError::VersionConflict { .. }
        | crm_customer_privacy::PrivacyDomainError::InvalidTransition { .. } => {
            ErrorCategory::Conflict
        }
        crm_customer_privacy::PrivacyDomainError::InvalidArgument { .. } => {
            ErrorCategory::InvalidArgument
        }
    };
    SdkError::new(
        error.code(),
        category,
        error.retryable(),
        "The customer-data legal-hold request is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn plan_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_LEGAL_HOLD_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The customer-data legal-hold mutation plan is invalid.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definition_is_exact_high_risk_personal_mutation() {
        let definition = place_customer_data_legal_hold_capability_definition().unwrap();
        assert_eq!(
            definition.capability_id.as_str(),
            PLACE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY
        );
        assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
        assert_eq!(definition.capability_version.as_str(), "1.0.0");
        assert_eq!(
            definition.input_contract.allowed_data_classes,
            vec![DataClass::Personal]
        );
        assert_eq!(definition.risk, CapabilityRisk::High);
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
        assert!(!definition.requires_approval);
    }

    #[test]
    fn deterministic_identity_is_tenant_and_idempotency_bound() {
        let first = deterministic_customer_data_legal_hold_id("tenant-a", "same-key").unwrap();
        let replay = deterministic_customer_data_legal_hold_id("tenant-a", "same-key").unwrap();
        let other_tenant =
            deterministic_customer_data_legal_hold_id("tenant-b", "same-key").unwrap();
        assert_eq!(first, replay);
        assert_ne!(first, other_tenant);
        assert!(first.as_str().starts_with(LEGAL_HOLD_ID_PREFIX));
    }

    #[test]
    fn nanosecond_values_are_truncated_to_wire_milliseconds() {
        assert_eq!(
            nanos_to_millis(
                1_999_999,
                "customer_privacy.legal_hold.effective_from_unix_ms"
            )
            .unwrap(),
            1
        );
        assert!(nanos_to_millis(-1, "field").is_err());
    }
}
