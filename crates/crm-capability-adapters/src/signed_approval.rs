use crm_capability_runtime::{
    ApprovalEvidence, CapabilityApprovalVerifier, CapabilityDefinition, CapabilityRequest,
};
use crm_module_sdk::{ErrorCategory, PortFuture, SdkError};
use sha2::{Digest, Sha256};
use std::sync::Arc;

const APPROVAL_SIGNATURE_NAMESPACE: &[u8] = b"crm.capability.approval.hmac-sha256/v1";
const HMAC_SHA256_BLOCK_BYTES: usize = 64;
const HMAC_SHA256_OUTPUT_BYTES: usize = 32;
const MINIMUM_SIGNING_KEY_BYTES: usize = 32;

/// Stateless verifier for externally issued, request-bound approval evidence.
///
/// The signing key belongs to the trusted approval issuer/runtime boundary. The
/// signed envelope binds the approval ID, actor, capability coordinate, exact
/// semantic input hash, policy version and expiry. The opaque proof is a raw
/// 32-byte HMAC-SHA-256 tag and is compared in constant time.
#[derive(Clone)]
pub struct HmacSha256ApprovalVerifier {
    signing_key: Arc<[u8]>,
}

impl std::fmt::Debug for HmacSha256ApprovalVerifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HmacSha256ApprovalVerifier")
            .field("signing_key", &"<redacted>")
            .finish()
    }
}

impl HmacSha256ApprovalVerifier {
    pub fn try_new(signing_key: impl Into<Vec<u8>>) -> Result<Self, SdkError> {
        let signing_key = signing_key.into();
        if signing_key.len() < MINIMUM_SIGNING_KEY_BYTES {
            return Err(configuration_error(
                "approval signing key must contain at least 32 bytes",
            ));
        }
        Ok(Self {
            signing_key: Arc::from(signing_key),
        })
    }

    /// Signs an already-bound approval envelope. The existing `opaque_proof`
    /// value is ignored so callers can safely re-sign a cloned envelope.
    pub fn sign(&self, evidence: &ApprovalEvidence) -> Vec<u8> {
        hmac_sha256(&self.signing_key, &canonical_approval_bytes(evidence)).to_vec()
    }
}

impl CapabilityApprovalVerifier for HmacSha256ApprovalVerifier {
    fn verify<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
        approval: &'a ApprovalEvidence,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            if approval.actor_id != request.context.execution.actor_id
                || approval.capability_id != definition.capability_id
                || approval.capability_version != definition.capability_version
                || approval.input_hash != request.input_hash
            {
                return Err(invalid_approval("approval_binding_mismatch"));
            }
            let expected = hmac_sha256(
                &self.signing_key,
                &canonical_approval_bytes(approval),
            );
            let supplied: [u8; HMAC_SHA256_OUTPUT_BYTES] = approval
                .opaque_proof
                .as_slice()
                .try_into()
                .map_err(|_| invalid_approval("approval_proof_length_invalid"))?;
            if !constant_time_equal(&expected, &supplied) {
                return Err(invalid_approval("approval_proof_invalid"));
            }
            Ok(())
        })
    }
}

fn canonical_approval_bytes(evidence: &ApprovalEvidence) -> Vec<u8> {
    let mut output = Vec::with_capacity(256);
    append_field(&mut output, APPROVAL_SIGNATURE_NAMESPACE);
    append_field(&mut output, evidence.approval_id.as_bytes());
    append_field(&mut output, evidence.actor_id.as_str().as_bytes());
    append_field(&mut output, evidence.capability_id.as_str().as_bytes());
    append_field(
        &mut output,
        evidence.capability_version.as_str().as_bytes(),
    );
    append_field(&mut output, &evidence.input_hash);
    append_field(&mut output, evidence.policy_version.as_bytes());
    append_field(
        &mut output,
        &evidence.expires_at_unix_nanos.to_be_bytes(),
    );
    output
}

fn append_field(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value);
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; HMAC_SHA256_OUTPUT_BYTES] {
    let mut key_block = [0_u8; HMAC_SHA256_BLOCK_BYTES];
    if key.len() > HMAC_SHA256_BLOCK_BYTES {
        let digest = Sha256::digest(key);
        key_block[..HMAC_SHA256_OUTPUT_BYTES].copy_from_slice(&digest);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut inner_pad = [0x36_u8; HMAC_SHA256_BLOCK_BYTES];
    let mut outer_pad = [0x5c_u8; HMAC_SHA256_BLOCK_BYTES];
    for index in 0..HMAC_SHA256_BLOCK_BYTES {
        inner_pad[index] ^= key_block[index];
        outer_pad[index] ^= key_block[index];
    }

    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);
    let inner_digest = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_digest);
    outer.finalize().into()
}

fn constant_time_equal(
    left: &[u8; HMAC_SHA256_OUTPUT_BYTES],
    right: &[u8; HMAC_SHA256_OUTPUT_BYTES],
) -> bool {
    left.iter()
        .zip(right.iter())
        .fold(0_u8, |difference, (left, right)| {
            difference | (*left ^ *right)
        })
        == 0
}

fn configuration_error(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "APPROVAL_SIGNING_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The approval verification boundary is not configured safely.",
    )
    .with_internal_reference(internal.into())
}

fn invalid_approval(reference: &'static str) -> SdkError {
    SdkError::new(
        "CAPABILITY_APPROVAL_INVALID",
        ErrorCategory::Authorization,
        false,
        "The supplied approval is invalid.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_capability_runtime::{CapabilityRisk, PayloadContract};
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId,
        CorrelationId, DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext,
        ModuleId, PayloadEncoding, RequestId, RetentionPolicyId, SchemaId, SchemaVersion, TenantId,
        TraceId, TypedPayload,
    };

    #[tokio::test]
    async fn verifies_exact_signed_approval_and_rejects_tampering() {
        let verifier = HmacSha256ApprovalVerifier::try_new(vec![0x41; 32]).unwrap();
        let definition = definition();
        let request = request();
        let mut approval = approval(&definition, &request);
        approval.opaque_proof = verifier.sign(&approval);

        verifier
            .verify(&definition, &request, &approval)
            .await
            .unwrap();

        let mut tampered_proof = approval.clone();
        tampered_proof.opaque_proof[0] ^= 0xff;
        assert_eq!(
            verifier
                .verify(&definition, &request, &tampered_proof)
                .await
                .unwrap_err()
                .code,
            "CAPABILITY_APPROVAL_INVALID"
        );

        let mut tampered_input = approval;
        tampered_input.input_hash[0] ^= 0xff;
        assert_eq!(
            verifier
                .verify(&definition, &request, &tampered_input)
                .await
                .unwrap_err()
                .code,
            "CAPABILITY_APPROVAL_INVALID"
        );
    }

    #[test]
    fn rejects_short_signing_key() {
        assert_eq!(
            HmacSha256ApprovalVerifier::try_new(vec![0; 31])
                .unwrap_err()
                .code,
            "APPROVAL_SIGNING_CONFIGURATION_INVALID"
        );
    }

    fn approval(
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> ApprovalEvidence {
        ApprovalEvidence {
            approval_id: "approval-1".to_owned(),
            actor_id: request.context.execution.actor_id.clone(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            input_hash: request.input_hash,
            policy_version: "identity-resolution-merge-approval/v1".to_owned(),
            expires_at_unix_nanos: 10_000,
            opaque_proof: Vec::new(),
        }
    }

    fn definition() -> CapabilityDefinition {
        CapabilityDefinition {
            capability_id: CapabilityId::try_new("identity_resolution.merge.execute").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            owner_module_id: ModuleId::try_new("crm.identity-resolution").unwrap(),
            input_contract: PayloadContract {
                owner: ModuleId::try_new("crm.identity-resolution").unwrap(),
                schema_id: SchemaId::try_new("crm.identity_resolution.v1.MergePartyRequest").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [1; 32],
                allowed_data_classes: vec![DataClass::Personal],
                allowed_encodings: vec![PayloadEncoding::Protobuf],
                maximum_size_bytes: 4096,
            },
            output_contract: None,
            risk: CapabilityRisk::High,
            mutation: true,
            requires_idempotency: true,
            requires_approval: true,
            authorization_policy_id: "identity_resolution.merge.execute".to_owned(),
            rate_limit_policy_id: None,
        }
    }

    fn request() -> CapabilityRequest {
        CapabilityRequest {
            context: ModuleExecutionContext {
                module_id: ModuleId::try_new("crm.identity-resolution").unwrap(),
                execution: ExecutionContext {
                    tenant_id: TenantId::try_new("tenant-1").unwrap(),
                    actor_id: ActorId::try_new("actor-1").unwrap(),
                    request_id: RequestId::try_new("request-1").unwrap(),
                    correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
                    causation_id: CausationId::try_new("causation-1").unwrap(),
                    trace_id: TraceId::try_new("trace-1").unwrap(),
                    capability_id: CapabilityId::try_new("identity_resolution.merge.execute").unwrap(),
                    capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                    idempotency_key: IdempotencyKey::try_new("idem-1").unwrap(),
                    business_transaction_id: BusinessTransactionId::try_new("txn-1").unwrap(),
                    schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                    request_started_at_unix_nanos: 100,
                },
            },
            input: TypedPayload {
                owner: ModuleId::try_new("crm.identity-resolution").unwrap(),
                schema_id: SchemaId::try_new("crm.identity_resolution.v1.MergePartyRequest").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [1; 32],
                data_class: DataClass::Personal,
                encoding: PayloadEncoding::Protobuf,
                maximum_size_bytes: 4096,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes: b"merge".to_vec(),
            },
            input_hash: [2; 32],
            approval: None,
        }
    }
}
