use crate::contract::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION, OUTPUT_MAXIMUM_BYTES,
    OUTPUT_RETENTION_POLICY_ID, OUTPUT_SCHEMA_ID, module_id, output_descriptor_hash, schema_id,
    schema_version,
};
use crate::errors::configured;
use crate::request::ValidatedRequest;
use crm_customer_privacy_owner_scope_support::framed_digest;
use crm_module_sdk::{DataClass, PayloadEncoding, RetentionPolicyId, SdkError, TypedPayload};
use crm_parties::{MODULE_ID, PARTY_STATE_RETENTION_POLICY_ID};
use crm_parties_capability_adapter::RECORD_TYPE as PARTY_RECORD_TYPE;
use crm_proto_contracts::crm::customer_privacy::v1 as privacy;

pub(crate) fn build_response(
    request: &ValidatedRequest,
    resource_version: u64,
) -> privacy::PartiesPrivacyScopeContributionResponse {
    let cursor_digest = framed_digest(
        b"crm.parties.privacy.scope.cursor/v1",
        &[
            request.lineage.tenant_id.as_bytes(),
            request.canonical_party_id.as_str().as_bytes(),
            request
                .identity_resolution_generation
                .to_string()
                .as_bytes(),
            request.lineage.registry_digest_sha256.as_slice(),
            request.page_size.to_string().as_bytes(),
            b"terminal",
        ],
    );
    let page_digest = framed_digest(
        b"crm.parties.privacy.scope.page/v1",
        &[
            request.lineage.privacy_case_id.as_bytes(),
            request.canonical_party_id.as_str().as_bytes(),
            resource_version.to_string().as_bytes(),
            b"personal",
            b"retain_minimized_evidence",
            PARTY_STATE_RETENTION_POLICY_ID.as_bytes(),
            cursor_digest.as_slice(),
        ],
    );

    privacy::PartiesPrivacyScopeContributionResponse {
        contribution: Some(privacy::PrivacyScopeContributionResponseEnvelope {
            owner_module_id: MODULE_ID.to_owned(),
            capability_id: CAPABILITY_ID.to_owned(),
            capability_version: CAPABILITY_VERSION.to_owned(),
            lineage: Some(request.lineage.clone()),
            resources: vec![privacy::PrivacyScopeResourceReference {
                resource_type: PARTY_RECORD_TYPE.to_owned(),
                resource_id: request.canonical_party_id.as_str().to_owned(),
                resource_version,
                data_class: privacy::CustomerDataClass::Personal as i32,
                evidence_class: privacy::PrivacyScopeEvidenceClass::RetainMinimizedEvidence as i32,
                retention_policy_id: PARTY_STATE_RETENTION_POLICY_ID.to_owned(),
            }],
            page_evidence: Some(privacy::PrivacyScopeContributionPageEvidence {
                page_number: 1,
                scanned_resource_count: 1,
                emitted_resource_count: 1,
                next_cursor: String::new(),
                terminal_complete: true,
                cursor_digest_sha256: cursor_digest.to_vec(),
                page_digest_sha256: page_digest.to_vec(),
            }),
        }),
    }
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
