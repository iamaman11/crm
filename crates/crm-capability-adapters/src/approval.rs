use crm_capability_runtime::{
    ApprovalEvidence, CapabilityApprovalVerifier, CapabilityDefinition, CapabilityRequest,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, ErrorCategory, PortFuture, SdkError,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRecord {
    pub approval_id: String,
    pub actor_id: ActorId,
    pub capability_id: CapabilityId,
    pub capability_version: CapabilityVersion,
    pub input_hash: [u8; 32],
    pub policy_version: String,
    pub expires_at_unix_nanos: i64,
    pub proof_sha256: [u8; 32],
}

impl ApprovalRecord {
    pub fn from_evidence(evidence: &ApprovalEvidence) -> Result<Self, ApprovalStoreError> {
        if evidence.opaque_proof.is_empty() {
            return Err(ApprovalStoreError::InvalidRecord(
                "approval proof must not be empty",
            ));
        }
        Ok(Self {
            approval_id: evidence.approval_id.clone(),
            actor_id: evidence.actor_id.clone(),
            capability_id: evidence.capability_id.clone(),
            capability_version: evidence.capability_version.clone(),
            input_hash: evidence.input_hash,
            policy_version: evidence.policy_version.clone(),
            expires_at_unix_nanos: evidence.expires_at_unix_nanos,
            proof_sha256: sha256(&evidence.opaque_proof),
        })
    }

    fn validate(&self) -> Result<(), ApprovalStoreError> {
        if self.approval_id.is_empty() || self.policy_version.is_empty() {
            return Err(ApprovalStoreError::InvalidRecord(
                "approval ID and policy version must not be empty",
            ));
        }
        if self.input_hash.iter().all(|byte| *byte == 0)
            || self.proof_sha256.iter().all(|byte| *byte == 0)
            || self.expires_at_unix_nanos <= 0
        {
            return Err(ApprovalStoreError::InvalidRecord(
                "approval hashes and expiry must be valid",
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum ApprovalStoreError {
    InvalidRecord(&'static str),
    Duplicate(String),
    Poisoned,
}

impl fmt::Display for ApprovalStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRecord(message) => formatter.write_str(message),
            Self::Duplicate(approval_id) => {
                write!(formatter, "approval {approval_id} already exists")
            }
            Self::Poisoned => formatter.write_str("approval store lock is poisoned"),
        }
    }
}

impl Error for ApprovalStoreError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredApproval {
    record: ApprovalRecord,
    revoked: bool,
}

#[derive(Debug, Default)]
struct ApprovalState {
    revision: u64,
    records: BTreeMap<String, StoredApproval>,
}

#[derive(Debug, Clone, Default)]
pub struct ApprovalStore {
    state: Arc<RwLock<ApprovalState>>,
}

impl ApprovalStore {
    pub fn issue(&self, record: ApprovalRecord) -> Result<u64, ApprovalStoreError> {
        record.validate()?;
        let mut state = self
            .state
            .write()
            .map_err(|_| ApprovalStoreError::Poisoned)?;
        if state.records.contains_key(&record.approval_id) {
            return Err(ApprovalStoreError::Duplicate(record.approval_id));
        }
        state.revision = state.revision.saturating_add(1);
        state.records.insert(
            record.approval_id.clone(),
            StoredApproval {
                record,
                revoked: false,
            },
        );
        Ok(state.revision)
    }

    pub fn revoke(&self, approval_id: &str) -> Result<bool, ApprovalStoreError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| ApprovalStoreError::Poisoned)?;
        let Some(stored) = state.records.get_mut(approval_id) else {
            return Ok(false);
        };
        if stored.revoked {
            return Ok(false);
        }
        stored.revoked = true;
        state.revision = state.revision.saturating_add(1);
        Ok(true)
    }
}

#[derive(Debug, Clone)]
pub struct StoredApprovalVerifier {
    store: ApprovalStore,
}

impl StoredApprovalVerifier {
    pub fn new(store: ApprovalStore) -> Self {
        Self { store }
    }
}

impl CapabilityApprovalVerifier for StoredApprovalVerifier {
    fn verify<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
        approval: &'a ApprovalEvidence,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let state = self.store.state.read().map_err(|_| dependency_error())?;
            let stored = state
                .records
                .get(&approval.approval_id)
                .ok_or_else(|| invalid_approval("approval_not_found"))?;
            if stored.revoked {
                return Err(invalid_approval("approval_revoked"));
            }
            let record = &stored.record;
            if record.actor_id != request.context.execution.actor_id
                || record.capability_id != definition.capability_id
                || record.capability_version != definition.capability_version
                || record.input_hash != request.input_hash
                || record.policy_version != approval.policy_version
                || record.expires_at_unix_nanos != approval.expires_at_unix_nanos
            {
                return Err(invalid_approval("approval_binding_mismatch"));
            }
            if !constant_time_equal(&record.proof_sha256, &sha256(&approval.opaque_proof)) {
                return Err(invalid_approval("approval_proof_invalid"));
            }
            Ok(())
        })
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn constant_time_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right.iter())
        .fold(0_u8, |difference, (left, right)| difference | (left ^ right))
        == 0
}

fn invalid_approval(reference: &str) -> SdkError {
    SdkError::new(
        "CAPABILITY_APPROVAL_INVALID",
        ErrorCategory::Authorization,
        false,
        "The supplied approval is invalid.",
    )
    .with_internal_reference(reference)
}

fn dependency_error() -> SdkError {
    SdkError::new(
        "APPROVAL_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Approval verification is temporarily unavailable.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_capability_runtime::{CapabilityRisk, PayloadContract};
    use crm_module_sdk::{
        BusinessTransactionId, CausationId, CorrelationId, DataClass, ExecutionContext,
        IdempotencyKey, ModuleExecutionContext, ModuleId, PayloadEncoding, RequestId,
        RetentionPolicyId, SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
    };

    #[tokio::test]
    async fn verifies_exact_stored_proof_and_binding() {
        let approval = approval(b"high-entropy-proof");
        let store = ApprovalStore::default();
        store
            .issue(ApprovalRecord::from_evidence(&approval).unwrap())
            .unwrap();
        let verifier = StoredApprovalVerifier::new(store);

        verifier
            .verify(&definition(), &request(), &approval)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn revocation_and_wrong_proof_are_rejected() {
        let approval = approval(b"high-entropy-proof");
        let store = ApprovalStore::default();
        store
            .issue(ApprovalRecord::from_evidence(&approval).unwrap())
            .unwrap();
        let verifier = StoredApprovalVerifier::new(store.clone());
        let mut wrong = approval.clone();
        wrong.opaque_proof = b"wrong".to_vec();
        assert_eq!(
            verifier
                .verify(&definition(), &request(), &wrong)
                .await
                .unwrap_err()
                .code,
            "CAPABILITY_APPROVAL_INVALID"
        );

        store.revoke(&approval.approval_id).unwrap();
        assert!(
            verifier
                .verify(&definition(), &request(), &approval)
                .await
                .is_err()
        );
    }

    fn approval(proof: &[u8]) -> ApprovalEvidence {
        ApprovalEvidence {
            approval_id: "approval-1".to_owned(),
            actor_id: ActorId::try_new("actor-1").unwrap(),
            capability_id: CapabilityId::try_new("crm.sales.deal.create").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            input_hash: [2; 32],
            policy_version: "approval-policy-3".to_owned(),
            expires_at_unix_nanos: 1_000,
            opaque_proof: proof.to_vec(),
        }
    }

    fn definition() -> CapabilityDefinition {
        CapabilityDefinition {
            capability_id: CapabilityId::try_new("crm.sales.deal.create").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
            input_contract: PayloadContract {
                owner: ModuleId::try_new("crm.sales").unwrap(),
                schema_id: SchemaId::try_new("crm.sales.deal.create").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [1; 32],
                allowed_data_classes: vec![DataClass::Internal],
                allowed_encodings: vec![PayloadEncoding::Json],
                maximum_size_bytes: 4096,
            },
            output_contract: None,
            risk: CapabilityRisk::High,
            mutation: true,
            requires_idempotency: true,
            requires_approval: true,
            authorization_policy_id: "sales.deal.create".to_owned(),
            rate_limit_policy_id: None,
        }
    }

    fn request() -> CapabilityRequest {
        CapabilityRequest {
            context: ModuleExecutionContext {
                module_id: ModuleId::try_new("crm.sales").unwrap(),
                execution: ExecutionContext {
                    tenant_id: TenantId::try_new("tenant-1").unwrap(),
                    actor_id: ActorId::try_new("actor-1").unwrap(),
                    request_id: RequestId::try_new("request-1").unwrap(),
                    correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
                    causation_id: CausationId::try_new("causation-1").unwrap(),
                    trace_id: TraceId::try_new("trace-1").unwrap(),
                    capability_id: CapabilityId::try_new("crm.sales.deal.create").unwrap(),
                    capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                    idempotency_key: IdempotencyKey::try_new("idem-1").unwrap(),
                    business_transaction_id: BusinessTransactionId::try_new("txn-1").unwrap(),
                    schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                    request_started_at_unix_nanos: 100,
                },
            },
            input: TypedPayload {
                owner: ModuleId::try_new("crm.sales").unwrap(),
                schema_id: SchemaId::try_new("crm.sales.deal.create").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [1; 32],
                data_class: DataClass::Internal,
                encoding: PayloadEncoding::Json,
                maximum_size_bytes: 4096,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes: b"{}".to_vec(),
            },
            input_hash: [2; 32],
            approval: None,
        }
    }
}
