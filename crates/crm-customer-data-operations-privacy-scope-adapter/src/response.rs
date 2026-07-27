use crate::contract::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION, OUTPUT_MAXIMUM_BYTES,
    OUTPUT_RETENTION_POLICY_ID, OUTPUT_SCHEMA_ID, module_id, output_descriptor_hash, schema_id,
    schema_version,
};
use crate::errors::{association_state_invalid, configured};
use crate::request::{CursorState, ResourceFamily, ValidatedRequest, encode_cursor};
use crm_customer_data_operations::{
    EXPORT_EXECUTION_OUTCOME_STATE_RETENTION_POLICY_ID,
    EXPORT_EXECUTION_STAGE_STATE_RETENTION_POLICY_ID,
    EXPORT_SELECTION_ITEM_STATE_RETENTION_POLICY_ID, IMPORT_ROW_STATE_RETENTION_POLICY_ID,
};
use crm_customer_data_operations_capability_adapter::{
    EXPORT_EXECUTION_OUTCOME_RECORD_TYPE, EXPORT_EXECUTION_STAGE_RECORD_TYPE,
    IMPORT_ROW_RECORD_TYPE, MODULE_ID,
};
use crm_customer_privacy_owner_scope_support::{append_frame, framed_digest};
use crm_module_sdk::{
    DataClass, PayloadEncoding, RecordId, RetentionPolicyId, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::customer_privacy::v1 as privacy;
use sha2::{Digest, Sha256};

const EXPORT_SELECTION_ITEM_RECORD_TYPE: &str = "customer_data.export_selection_item";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedCustomerDataResource {
    pub family: ResourceFamily,
    pub record_id: RecordId,
    pub resource_version: u64,
}

impl VerifiedCustomerDataResource {
    fn resource_type(&self) -> &'static str {
        match self.family {
            ResourceFamily::ImportRow => IMPORT_ROW_RECORD_TYPE,
            ResourceFamily::ExportSelectionItem => EXPORT_SELECTION_ITEM_RECORD_TYPE,
            ResourceFamily::ExportExecutionStage => EXPORT_EXECUTION_STAGE_RECORD_TYPE,
            ResourceFamily::ExportExecutionOutcome => EXPORT_EXECUTION_OUTCOME_RECORD_TYPE,
        }
    }

    fn retention_policy_id(&self) -> &'static str {
        match self.family {
            ResourceFamily::ImportRow => IMPORT_ROW_STATE_RETENTION_POLICY_ID,
            ResourceFamily::ExportSelectionItem => EXPORT_SELECTION_ITEM_STATE_RETENTION_POLICY_ID,
            ResourceFamily::ExportExecutionStage => {
                EXPORT_EXECUTION_STAGE_STATE_RETENTION_POLICY_ID
            }
            ResourceFamily::ExportExecutionOutcome => {
                EXPORT_EXECUTION_OUTCOME_STATE_RETENTION_POLICY_ID
            }
        }
    }
}

pub(crate) fn build_response(
    request: &ValidatedRequest,
    resources: &[VerifiedCustomerDataResource],
    scanned_resource_count: u64,
    next_state: Option<&CursorState>,
) -> Result<privacy::CustomerDataPrivacyScopeContributionResponse, SdkError> {
    let next_cursor = match next_state {
        Some(state) => encode_cursor(
            request,
            request.page_number.checked_add(1).ok_or_else(|| {
                association_state_invalid("Customer Data privacy scope page number overflowed")
            })?,
            state,
        )?,
        None => String::new(),
    };
    let cursor_digest = framed_digest(
        b"crm.customer-data-operations.privacy.scope.cursor-evidence/v1",
        &[
            request.lineage.tenant_id.as_bytes(),
            request.lineage.privacy_case_id.as_bytes(),
            request.canonical_party_id.as_str().as_bytes(),
            request.page_number.to_string().as_bytes(),
            request.cursor_state.family.token().as_bytes(),
            request
                .cursor_state
                .after_record_id
                .as_ref()
                .map(RecordId::as_str)
                .unwrap_or("origin")
                .as_bytes(),
            next_cursor.as_bytes(),
        ],
    );
    let page_digest = page_digest(request, resources, scanned_resource_count, &cursor_digest);

    Ok(privacy::CustomerDataPrivacyScopeContributionResponse {
        contribution: Some(privacy::PrivacyScopeContributionResponseEnvelope {
            owner_module_id: MODULE_ID.to_owned(),
            capability_id: CAPABILITY_ID.to_owned(),
            capability_version: CAPABILITY_VERSION.to_owned(),
            lineage: Some(request.lineage.clone()),
            resources: resources
                .iter()
                .map(|resource| privacy::PrivacyScopeResourceReference {
                    resource_type: resource.resource_type().to_owned(),
                    resource_id: resource.record_id.as_str().to_owned(),
                    resource_version: resource.resource_version,
                    data_class: privacy::CustomerDataClass::Personal as i32,
                    evidence_class: privacy::PrivacyScopeEvidenceClass::RetainMinimizedEvidence
                        as i32,
                    retention_policy_id: resource.retention_policy_id().to_owned(),
                })
                .collect(),
            page_evidence: Some(privacy::PrivacyScopeContributionPageEvidence {
                page_number: request.page_number,
                scanned_resource_count,
                emitted_resource_count: u64::try_from(resources.len()).map_err(|_| {
                    association_state_invalid(
                        "Customer Data privacy scope emitted count does not fit in u64",
                    )
                })?,
                next_cursor,
                terminal_complete: next_state.is_none(),
                cursor_digest_sha256: cursor_digest.to_vec(),
                page_digest_sha256: page_digest.to_vec(),
            }),
        }),
    })
}

pub(crate) fn typed_output(bytes: Vec<u8>) -> Result<TypedPayload, SdkError> {
    let output = TypedPayload {
        owner: module_id()?,
        schema_id: schema_id(OUTPUT_SCHEMA_ID)?,
        schema_version: schema_version(CONTRACT_SCHEMA_VERSION)?,
        descriptor_hash: output_descriptor_hash(),
        data_class: DataClass::Confidential,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: OUTPUT_MAXIMUM_BYTES,
        retention_policy_id: configured(RetentionPolicyId::try_new(OUTPUT_RETENTION_POLICY_ID))?,
        bytes,
    };
    output.validate()?;
    Ok(output)
}

fn page_digest(
    request: &ValidatedRequest,
    resources: &[VerifiedCustomerDataResource],
    scanned_resource_count: u64,
    cursor_digest: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    append_frame(
        &mut hasher,
        b"crm.customer-data-operations.privacy.scope.page/v1",
    );
    append_frame(&mut hasher, request.lineage.privacy_case_id.as_bytes());
    append_frame(&mut hasher, request.canonical_party_id.as_str().as_bytes());
    append_frame(&mut hasher, request.page_number.to_string().as_bytes());
    append_frame(&mut hasher, scanned_resource_count.to_string().as_bytes());
    for resource in resources {
        append_frame(&mut hasher, resource.resource_type().as_bytes());
        append_frame(&mut hasher, resource.record_id.as_str().as_bytes());
        append_frame(
            &mut hasher,
            resource.resource_version.to_string().as_bytes(),
        );
        append_frame(&mut hasher, b"personal");
        append_frame(&mut hasher, b"retain_minimized_evidence");
        append_frame(&mut hasher, resource.retention_policy_id().as_bytes());
    }
    append_frame(&mut hasher, cursor_digest);
    hasher.finalize().into()
}
