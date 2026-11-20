use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, DataClass, ModuleExecutionContext, ModuleId,
    PayloadEncoding, ResourceRef, SchemaId, SchemaVersion, TypedPayload,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRisk {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayloadContract {
    pub owner: ModuleId,
    pub schema_id: SchemaId,
    pub schema_version: SchemaVersion,
    pub descriptor_hash: [u8; 32],
    pub allowed_data_classes: Vec<DataClass>,
    pub allowed_encodings: Vec<PayloadEncoding>,
    pub maximum_size_bytes: u64,
}

impl PayloadContract {
    pub fn matches(&self, payload: &TypedPayload) -> bool {
        self.owner == payload.owner
            && self.schema_id == payload.schema_id
            && self.schema_version == payload.schema_version
            && self.descriptor_hash == payload.descriptor_hash
            && self.allowed_data_classes.contains(&payload.data_class)
            && self.allowed_encodings.contains(&payload.encoding)
            && payload.bytes.len() as u64 <= self.maximum_size_bytes
            && payload.maximum_size_bytes <= self.maximum_size_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDefinition {
    pub capability_id: CapabilityId,
    pub capability_version: CapabilityVersion,
    pub owner_module_id: ModuleId,
    pub input_contract: PayloadContract,
    pub output_contract: Option<PayloadContract>,
    pub risk: CapabilityRisk,
    pub mutation: bool,
    pub requires_idempotency: bool,
    pub requires_approval: bool,
    pub authorization_policy_id: String,
    pub rate_limit_policy_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalEvidence {
    pub approval_id: String,
    pub actor_id: ActorId,
    pub capability_id: CapabilityId,
    pub capability_version: CapabilityVersion,
    pub input_hash: [u8; 32],
    pub policy_version: String,
    pub expires_at_unix_nanos: i64,
    pub opaque_proof: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityRequest {
    pub context: ModuleExecutionContext,
    pub input: TypedPayload,
    pub input_hash: [u8; 32],
    pub approval: Option<ApprovalEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityExecutionResult {
    pub output: Option<TypedPayload>,
    pub affected_resources: Vec<ResourceRef>,
    pub replayed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RateLimitDecision {
    pub allowed: bool,
    pub decision_id: String,
    pub retry_after_millis: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationDecision {
    pub allowed: bool,
    pub decision_id: String,
    pub reason_code: String,
    pub policy_version: String,
}
