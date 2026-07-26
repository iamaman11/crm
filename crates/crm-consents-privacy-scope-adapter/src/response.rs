use crate::contract::{
    CAPABILITY_ID, CAPABILITY_VERSION, CONTRACT_SCHEMA_VERSION, OUTPUT_MAXIMUM_BYTES,
    OUTPUT_RETENTION_POLICY_ID, OUTPUT_SCHEMA_ID, module_id, output_descriptor_hash, schema_id,
    schema_version,
};
use crate::digest::{append_frame, framed_digest};
use crate::errors::{configured, stored_state_invalid};
use crate::request::{ValidatedRequest, encode_cursor};
use crm_consents::{CONSENT_AUTHORIZATION_STATE_RETENTION_POLICY_ID, MODULE_ID};
use crm_consents_capability_adapter::RECORD_TYPE;
use crm_module_sdk::{
    DataClass, PayloadEncoding, RecordId, RetentionPolicyId, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::customer_privacy::v1 as privacy;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedConsentResource {
    pub record_id: RecordId,
    pub resource_version: u64,
}

pub(crate) fn build_response(
    request: &ValidatedRequest,
    resources: &[VerifiedConsentResource],
    scanned_resource_count: u64,
    has_more: bool,
) -> Result<privacy::ConsentsPrivacyScopeContributionResponse, SdkError> {
    let next_cursor = if has_more {
        let last = resources.last().ok_or_else(|| {
            stored_state_invalid(
                "a non-terminal Consent scope page must emit at least one resource",
            )
        })?;
        encode_cursor(
            request,
            request
                .page_number
                .checked_add(1)
                .ok_or_else(|| stored_state_invalid("Consent scope page number overflowed"))?,
            &last.record_id,
        )?
    } else {
        String::new()
    };
    let cursor_digest = framed_digest(
        b"crm.consents.privacy.scope.cursor-evidence/v1",
        &[
            request.lineage.tenant_id.as_bytes(),
            request.lineage.privacy_case_id.as_bytes(),
            request.canonical_party_id.as_str().as_bytes(),
            request.page_number.to_string().as_bytes(),
            request
                .after_record_id
                .as_ref()
                .map(RecordId::as_str)
                .unwrap_or("origin")
                .as_bytes(),
            next_cursor.as_bytes(),
        ],
    );
    let page_digest = page_digest(request, resources, scanned_resource_count, &cursor_digest);

    Ok(privacy::ConsentsPrivacyScopeContributionResponse {
        contribution: Some(privacy::PrivacyScopeContributionResponseEnvelope {
            owner_module_id: MODULE_ID.to_owned(),
            capability_id: CAPABILITY_ID.to_owned(),
            capability_version: CAPABILITY_VERSION.to_owned(),
            lineage: Some(request.lineage.clone()),
            resources: resources
                .iter()
                .map(|resource| privacy::PrivacyScopeResourceReference {
                    resource_type: RECORD_TYPE.to_owned(),
                    resource_id: resource.record_id.as_str().to_owned(),
                    resource_version: resource.resource_version,
                    data_class: privacy::CustomerDataClass::Personal as i32,
                    evidence_class: privacy::PrivacyScopeEvidenceClass::ImmutableRequiredEvidence
                        as i32,
                    retention_policy_id: CONSENT_AUTHORIZATION_STATE_RETENTION_POLICY_ID.to_owned(),
                })
                .collect(),
            page_evidence: Some(privacy::PrivacyScopeContributionPageEvidence {
                page_number: request.page_number,
                scanned_resource_count,
                emitted_resource_count: u64::try_from(resources.len()).map_err(|_| {
                    stored_state_invalid("Consent scope emitted count does not fit in u64")
                })?,
                next_cursor,
                terminal_complete: !has_more,
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
    resources: &[VerifiedConsentResource],
    scanned_resource_count: u64,
    cursor_digest: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    append_frame(&mut hasher, b"crm.consents.privacy.scope.page/v1");
    append_frame(&mut hasher, request.lineage.privacy_case_id.as_bytes());
    append_frame(&mut hasher, request.canonical_party_id.as_str().as_bytes());
    append_frame(&mut hasher, request.page_number.to_string().as_bytes());
    append_frame(&mut hasher, scanned_resource_count.to_string().as_bytes());
    for resource in resources {
        append_frame(&mut hasher, RECORD_TYPE.as_bytes());
        append_frame(&mut hasher, resource.record_id.as_str().as_bytes());
        append_frame(
            &mut hasher,
            resource.resource_version.to_string().as_bytes(),
        );
        append_frame(&mut hasher, b"personal");
        append_frame(&mut hasher, b"immutable_required_evidence");
        append_frame(
            &mut hasher,
            CONSENT_AUTHORIZATION_STATE_RETENTION_POLICY_ID.as_bytes(),
        );
    }
    append_frame(&mut hasher, cursor_digest);
    hasher.finalize().into()
}
