use super::{
    ApprovalEvidence, CustomerDataLegalHold, LegalHoldScope, LegalHoldStatus, PrivacyCase,
    PrivacyCaseKind, PrivacyCaseStatus, PrivacyDomainError, ProcessingRestriction,
    RescopeRequirement, RestrictionScope, RestrictionStatus, ResumeStage, SubjectBinding,
    SubjectVerificationMethod,
};
use crm_module_sdk::{
    ActorId, DataClass, ErrorCategory, ModuleId, RecordId, SchemaVersion, SdkError, TenantId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PRIVACY_CASE_STATE_SCHEMA_ID: &str = "crm.customer-privacy.case.state";
pub const PRIVACY_CASE_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const PRIVACY_CASE_STATE_MAXIMUM_BYTES: u64 = 64 * 1024;
pub const PRIVACY_CASE_STATE_RETENTION_POLICY_ID: &str = "crm.customer_privacy.case";

pub const PROCESSING_RESTRICTION_STATE_SCHEMA_ID: &str =
    "crm.customer-privacy.processing_restriction.state";
pub const PROCESSING_RESTRICTION_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const PROCESSING_RESTRICTION_STATE_MAXIMUM_BYTES: u64 = 16 * 1024;
pub const PROCESSING_RESTRICTION_STATE_RETENTION_POLICY_ID: &str =
    "crm.customer_privacy.restriction";

pub const LEGAL_HOLD_STATE_SCHEMA_ID: &str = "crm.customer-privacy.legal_hold.state";
pub const LEGAL_HOLD_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const LEGAL_HOLD_STATE_MAXIMUM_BYTES: u64 = 16 * 1024;
pub const LEGAL_HOLD_STATE_RETENTION_POLICY_ID: &str = "crm.customer_privacy.legal_hold";

const PRIVACY_CASE_STATE_DESCRIPTOR: &[u8] = b"crm.customer-privacy.case.state/v1:case_id,tenant_id,kind,status,version_decimal,policy_version,created_at_decimal,last_transition_at_decimal,previous_case_id,subject_binding,pending_rescope,scope_snapshot_id,action_plan_id,approval";
const PROCESSING_RESTRICTION_STATE_DESCRIPTOR: &[u8] = b"crm.customer-privacy.processing_restriction.state/v1:restriction_id,tenant_id,canonical_party_id,scope,status,version_decimal,policy_version,placed_by,placed_at_decimal,effective_from_decimal,expires_at_decimal,released_by,released_at_decimal";
const LEGAL_HOLD_STATE_DESCRIPTOR: &[u8] = b"crm.customer-privacy.legal_hold.state/v1:hold_id,tenant_id,canonical_party_id,scope,authority_reference,reason_code,policy_version,status,version_decimal,placed_by,effective_from_decimal,effective_until_decimal,released_by,released_at_decimal";

pub fn privacy_case_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(PRIVACY_CASE_STATE_DESCRIPTOR).into()
}

pub fn processing_restriction_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(PROCESSING_RESTRICTION_STATE_DESCRIPTOR).into()
}

pub fn legal_hold_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(LEGAL_HOLD_STATE_DESCRIPTOR).into()
}

pub fn encode_privacy_case_state(case: &PrivacyCase) -> Result<Vec<u8>, SdkError> {
    encode_state(
        &PrivacyCaseStateV1::from(case),
        PRIVACY_CASE_STATE_MAXIMUM_BYTES,
        "privacy case",
    )
}

pub fn decode_privacy_case_state(bytes: &[u8]) -> Result<PrivacyCase, SdkError> {
    validate_size(bytes, PRIVACY_CASE_STATE_MAXIMUM_BYTES, "privacy case")?;
    let state: PrivacyCaseStateV1 = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("privacy-case JSON is invalid: {error}")))?;
    let case = state.into_domain()?;
    if encode_privacy_case_state(&case)? != bytes {
        return Err(persisted_error(
            "persisted privacy case is not the strict canonical v1 encoding",
        ));
    }
    Ok(case)
}

pub fn encode_processing_restriction_state(
    restriction: &ProcessingRestriction,
) -> Result<Vec<u8>, SdkError> {
    encode_state(
        &ProcessingRestrictionStateV1::from(restriction),
        PROCESSING_RESTRICTION_STATE_MAXIMUM_BYTES,
        "processing restriction",
    )
}

pub fn decode_processing_restriction_state(
    bytes: &[u8],
) -> Result<ProcessingRestriction, SdkError> {
    validate_size(
        bytes,
        PROCESSING_RESTRICTION_STATE_MAXIMUM_BYTES,
        "processing restriction",
    )?;
    let state: ProcessingRestrictionStateV1 = serde_json::from_slice(bytes).map_err(|error| {
        persisted_error(format!("processing-restriction JSON is invalid: {error}"))
    })?;
    let restriction = state.into_domain()?;
    if encode_processing_restriction_state(&restriction)? != bytes {
        return Err(persisted_error(
            "persisted processing restriction is not the strict canonical v1 encoding",
        ));
    }
    Ok(restriction)
}

pub fn encode_legal_hold_state(hold: &CustomerDataLegalHold) -> Result<Vec<u8>, SdkError> {
    encode_state(
        &LegalHoldStateV1::from(hold),
        LEGAL_HOLD_STATE_MAXIMUM_BYTES,
        "customer-data legal hold",
    )
}

pub fn decode_legal_hold_state(bytes: &[u8]) -> Result<CustomerDataLegalHold, SdkError> {
    validate_size(bytes, LEGAL_HOLD_STATE_MAXIMUM_BYTES, "customer-data legal hold")?;
    let state: LegalHoldStateV1 = serde_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("legal-hold JSON is invalid: {error}")))?;
    let hold = state.into_domain()?;
    if encode_legal_hold_state(&hold)? != bytes {
        return Err(persisted_error(
            "persisted customer-data legal hold is not the strict canonical v1 encoding",
        ));
    }
    Ok(hold)
}

fn encode_state<T: Serialize>(
    state: &T,
    maximum_bytes: u64,
    label: &str,
) -> Result<Vec<u8>, SdkError> {
    let bytes = serde_json::to_vec(state)
        .map_err(|error| persisted_error(format!("{label} serialization failed: {error}")))?;
    validate_size(&bytes, maximum_bytes, label)?;
    Ok(bytes)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrivacyCaseStateV1 {
    case_id: String,
    tenant_id: String,
    kind: PrivacyCaseKindState,
    status: PrivacyCaseStatusState,
    version: String,
    policy_version: String,
    created_at_unix_nanos: String,
    last_transition_at_unix_nanos: String,
    previous_case_id: Option<String>,
    subject_binding: Option<SubjectBindingStateV1>,
    pending_rescope: Option<RescopeRequirementStateV1>,
    scope_snapshot_id: Option<String>,
    action_plan_id: Option<String>,
    approval: Option<ApprovalEvidenceStateV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PrivacyCaseKindState {
    Access,
    PortabilityExport,
    RestrictProcessing,
    Erasure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SubjectVerificationMethodState {
    AuthenticatedPortal,
    StaffAssisted,
    VerifiedDocument,
    ExistingHighAssuranceIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ResumeStageState {
    Scoping,
    Planning,
    Executing,
    Converging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", content = "resume_stage", rename_all = "snake_case")]
enum PrivacyCaseStatusState {
    Draft,
    Submitted,
    SubjectVerified,
    Scoping,
    Scoped,
    Planned,
    AwaitingApproval,
    Executing,
    Converging,
    RescopeRequired,
    FailedRetryable(ResumeStageState),
    Completed,
    PartiallyCompleted,
    Denied,
    Cancelled,
    FailedTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubjectBindingStateV1 {
    submitted_party_id: String,
    canonical_party_id: String,
    identity_resolution_generation: String,
    verification_method: SubjectVerificationMethodState,
    verified_by: String,
    verified_at_unix_nanos: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RescopeRequirementStateV1 {
    previous_canonical_party_id: String,
    proposed_canonical_party_id: String,
    previous_identity_resolution_generation: String,
    proposed_identity_resolution_generation: String,
    detected_at_unix_nanos: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApprovalEvidenceStateV1 {
    approved_by: String,
    approved_at_unix_nanos: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProcessingRestrictionStateV1 {
    restriction_id: String,
    tenant_id: String,
    canonical_party_id: String,
    scope: RestrictionScopeState,
    status: RestrictionStatusState,
    version: String,
    policy_version: String,
    placed_by: String,
    placed_at_unix_nanos: String,
    effective_from_unix_nanos: String,
    expires_at_unix_nanos: Option<String>,
    released_by: Option<String>,
    released_at_unix_nanos: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RestrictionScopeState {
    Processing,
    Communication,
    ProcessingAndCommunication,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RestrictionStatusState {
    Active,
    Released,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegalHoldStateV1 {
    hold_id: String,
    tenant_id: String,
    canonical_party_id: String,
    scope: LegalHoldScopeState,
    authority_reference: String,
    reason_code: String,
    policy_version: String,
    status: LegalHoldStatusState,
    version: String,
    placed_by: String,
    effective_from_unix_nanos: String,
    effective_until_unix_nanos: Option<String>,
    released_by: Option<String>,
    released_at_unix_nanos: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum LegalHoldScopeState {
    AllCustomerData,
    DataClass(DataClass),
    Owner(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LegalHoldStatusState {
    Active,
    Released,
}

impl From<&PrivacyCase> for PrivacyCaseStateV1 {
    fn from(case: &PrivacyCase) -> Self {
        Self {
            case_id: case.case_id.as_str().to_owned(),
            tenant_id: case.tenant_id.as_str().to_owned(),
            kind: case.kind.into(),
            status: case.status.into(),
            version: case.version.to_string(),
            policy_version: case.policy_version.as_str().to_owned(),
            created_at_unix_nanos: case.created_at_unix_nanos.to_string(),
            last_transition_at_unix_nanos: case.last_transition_at_unix_nanos.to_string(),
            previous_case_id: case
                .previous_case_id
                .as_ref()
                .map(|value| value.as_str().to_owned()),
            subject_binding: case.subject_binding.as_ref().map(SubjectBindingStateV1::from),
            pending_rescope: case
                .pending_rescope
                .as_ref()
                .map(RescopeRequirementStateV1::from),
            scope_snapshot_id: case
                .scope_snapshot_id
                .as_ref()
                .map(|value| value.as_str().to_owned()),
            action_plan_id: case
                .action_plan_id
                .as_ref()
                .map(|value| value.as_str().to_owned()),
            approval: case.approval.as_ref().map(ApprovalEvidenceStateV1::from),
        }
    }
}

impl PrivacyCaseStateV1 {
    fn into_domain(self) -> Result<PrivacyCase, SdkError> {
        let case = PrivacyCase {
            case_id: record_id(self.case_id, "privacy case")?,
            tenant_id: tenant_id(self.tenant_id)?,
            kind: self.kind.into(),
            status: self.status.into(),
            version: decimal_u64(self.version, "version")?,
            policy_version: schema_version(self.policy_version)?,
            created_at_unix_nanos: decimal_i64(
                self.created_at_unix_nanos,
                "created_at_unix_nanos",
            )?,
            last_transition_at_unix_nanos: decimal_i64(
                self.last_transition_at_unix_nanos,
                "last_transition_at_unix_nanos",
            )?,
            previous_case_id: optional_record_id(self.previous_case_id, "previous privacy case")?,
            subject_binding: self
                .subject_binding
                .map(SubjectBindingStateV1::into_domain)
                .transpose()?,
            pending_rescope: self
                .pending_rescope
                .map(RescopeRequirementStateV1::into_domain)
                .transpose()?,
            scope_snapshot_id: optional_record_id(self.scope_snapshot_id, "scope snapshot")?,
            action_plan_id: optional_record_id(self.action_plan_id, "action plan")?,
            approval: self
                .approval
                .map(ApprovalEvidenceStateV1::into_domain)
                .transpose()?,
        };
        validate_privacy_case(&case)?;
        Ok(case)
    }
}

impl From<&SubjectBinding> for SubjectBindingStateV1 {
    fn from(binding: &SubjectBinding) -> Self {
        Self {
            submitted_party_id: binding.submitted_party_id.as_str().to_owned(),
            canonical_party_id: binding.canonical_party_id.as_str().to_owned(),
            identity_resolution_generation: binding.identity_resolution_generation.to_string(),
            verification_method: binding.verification_method.into(),
            verified_by: binding.verified_by.as_str().to_owned(),
            verified_at_unix_nanos: binding.verified_at_unix_nanos.to_string(),
        }
    }
}

impl SubjectBindingStateV1 {
    fn into_domain(self) -> Result<SubjectBinding, SdkError> {
        Ok(SubjectBinding {
            submitted_party_id: record_id(self.submitted_party_id, "submitted Party")?,
            canonical_party_id: record_id(self.canonical_party_id, "canonical Party")?,
            identity_resolution_generation: decimal_u64(
                self.identity_resolution_generation,
                "identity_resolution_generation",
            )?,
            verification_method: self.verification_method.into(),
            verified_by: actor_id(self.verified_by)?,
            verified_at_unix_nanos: decimal_i64(
                self.verified_at_unix_nanos,
                "verified_at_unix_nanos",
            )?,
        })
    }
}

impl From<&RescopeRequirement> for RescopeRequirementStateV1 {
    fn from(requirement: &RescopeRequirement) -> Self {
        Self {
            previous_canonical_party_id: requirement.previous_canonical_party_id.as_str().to_owned(),
            proposed_canonical_party_id: requirement.proposed_canonical_party_id.as_str().to_owned(),
            previous_identity_resolution_generation: requirement
                .previous_identity_resolution_generation
                .to_string(),
            proposed_identity_resolution_generation: requirement
                .proposed_identity_resolution_generation
                .to_string(),
            detected_at_unix_nanos: requirement.detected_at_unix_nanos.to_string(),
        }
    }
}

impl RescopeRequirementStateV1 {
    fn into_domain(self) -> Result<RescopeRequirement, SdkError> {
        Ok(RescopeRequirement {
            previous_canonical_party_id: record_id(
                self.previous_canonical_party_id,
                "previous canonical Party",
            )?,
            proposed_canonical_party_id: record_id(
                self.proposed_canonical_party_id,
                "proposed canonical Party",
            )?,
            previous_identity_resolution_generation: decimal_u64(
                self.previous_identity_resolution_generation,
                "previous_identity_resolution_generation",
            )?,
            proposed_identity_resolution_generation: decimal_u64(
                self.proposed_identity_resolution_generation,
                "proposed_identity_resolution_generation",
            )?,
            detected_at_unix_nanos: decimal_i64(
                self.detected_at_unix_nanos,
                "detected_at_unix_nanos",
            )?,
        })
    }
}

impl From<&ApprovalEvidence> for ApprovalEvidenceStateV1 {
    fn from(approval: &ApprovalEvidence) -> Self {
        Self {
            approved_by: approval.approved_by.as_str().to_owned(),
            approved_at_unix_nanos: approval.approved_at_unix_nanos.to_string(),
        }
    }
}

impl ApprovalEvidenceStateV1 {
    fn into_domain(self) -> Result<ApprovalEvidence, SdkError> {
        Ok(ApprovalEvidence {
            approved_by: actor_id(self.approved_by)?,
            approved_at_unix_nanos: decimal_i64(
                self.approved_at_unix_nanos,
                "approved_at_unix_nanos",
            )?,
        })
    }
}

impl From<&ProcessingRestriction> for ProcessingRestrictionStateV1 {
    fn from(restriction: &ProcessingRestriction) -> Self {
        Self {
            restriction_id: restriction.restriction_id.as_str().to_owned(),
            tenant_id: restriction.tenant_id.as_str().to_owned(),
            canonical_party_id: restriction.canonical_party_id.as_str().to_owned(),
            scope: restriction.scope.into(),
            status: restriction.status.into(),
            version: restriction.version.to_string(),
            policy_version: restriction.policy_version.as_str().to_owned(),
            placed_by: restriction.placed_by.as_str().to_owned(),
            placed_at_unix_nanos: restriction.placed_at_unix_nanos.to_string(),
            effective_from_unix_nanos: restriction.effective_from_unix_nanos.to_string(),
            expires_at_unix_nanos: restriction.expires_at_unix_nanos.map(|value| value.to_string()),
            released_by: restriction
                .released_by
                .as_ref()
                .map(|value| value.as_str().to_owned()),
            released_at_unix_nanos: restriction.released_at_unix_nanos.map(|value| value.to_string()),
        }
    }
}

impl ProcessingRestrictionStateV1 {
    fn into_domain(self) -> Result<ProcessingRestriction, SdkError> {
        let restriction = ProcessingRestriction {
            restriction_id: record_id(self.restriction_id, "processing restriction")?,
            tenant_id: tenant_id(self.tenant_id)?,
            canonical_party_id: record_id(self.canonical_party_id, "canonical Party")?,
            scope: self.scope.into(),
            status: self.status.into(),
            version: decimal_u64(self.version, "version")?,
            policy_version: schema_version(self.policy_version)?,
            placed_by: actor_id(self.placed_by)?,
            placed_at_unix_nanos: decimal_i64(
                self.placed_at_unix_nanos,
                "placed_at_unix_nanos",
            )?,
            effective_from_unix_nanos: decimal_i64(
                self.effective_from_unix_nanos,
                "effective_from_unix_nanos",
            )?,
            expires_at_unix_nanos: optional_decimal_i64(
                self.expires_at_unix_nanos,
                "expires_at_unix_nanos",
            )?,
            released_by: optional_actor_id(self.released_by)?,
            released_at_unix_nanos: optional_decimal_i64(
                self.released_at_unix_nanos,
                "released_at_unix_nanos",
            )?,
        };
        validate_processing_restriction(&restriction)?;
        Ok(restriction)
    }
}

impl From<&CustomerDataLegalHold> for LegalHoldStateV1 {
    fn from(hold: &CustomerDataLegalHold) -> Self {
        Self {
            hold_id: hold.hold_id.as_str().to_owned(),
            tenant_id: hold.tenant_id.as_str().to_owned(),
            canonical_party_id: hold.canonical_party_id.as_str().to_owned(),
            scope: (&hold.scope).into(),
            authority_reference: hold.authority_reference.as_str().to_owned(),
            reason_code: hold.reason_code.clone(),
            policy_version: hold.policy_version.as_str().to_owned(),
            status: hold.status.into(),
            version: hold.version.to_string(),
            placed_by: hold.placed_by.as_str().to_owned(),
            effective_from_unix_nanos: hold.effective_from_unix_nanos.to_string(),
            effective_until_unix_nanos: hold
                .effective_until_unix_nanos
                .map(|value| value.to_string()),
            released_by: hold
                .released_by
                .as_ref()
                .map(|value| value.as_str().to_owned()),
            released_at_unix_nanos: hold.released_at_unix_nanos.map(|value| value.to_string()),
        }
    }
}

impl LegalHoldStateV1 {
    fn into_domain(self) -> Result<CustomerDataLegalHold, SdkError> {
        let hold = CustomerDataLegalHold {
            hold_id: record_id(self.hold_id, "customer-data legal hold")?,
            tenant_id: tenant_id(self.tenant_id)?,
            canonical_party_id: record_id(self.canonical_party_id, "canonical Party")?,
            scope: self.scope.into_domain()?,
            authority_reference: record_id(self.authority_reference, "hold authority")?,
            reason_code: self.reason_code,
            policy_version: schema_version(self.policy_version)?,
            status: self.status.into(),
            version: decimal_u64(self.version, "version")?,
            placed_by: actor_id(self.placed_by)?,
            effective_from_unix_nanos: decimal_i64(
                self.effective_from_unix_nanos,
                "effective_from_unix_nanos",
            )?,
            effective_until_unix_nanos: optional_decimal_i64(
                self.effective_until_unix_nanos,
                "effective_until_unix_nanos",
            )?,
            released_by: optional_actor_id(self.released_by)?,
            released_at_unix_nanos: optional_decimal_i64(
                self.released_at_unix_nanos,
                "released_at_unix_nanos",
            )?,
        };
        validate_legal_hold(&hold)?;
        Ok(hold)
    }
}

impl From<PrivacyCaseKind> for PrivacyCaseKindState {
    fn from(value: PrivacyCaseKind) -> Self {
        match value {
            PrivacyCaseKind::Access => Self::Access,
            PrivacyCaseKind::PortabilityExport => Self::PortabilityExport,
            PrivacyCaseKind::RestrictProcessing => Self::RestrictProcessing,
            PrivacyCaseKind::Erasure => Self::Erasure,
        }
    }
}

impl From<PrivacyCaseKindState> for PrivacyCaseKind {
    fn from(value: PrivacyCaseKindState) -> Self {
        match value {
            PrivacyCaseKindState::Access => Self::Access,
            PrivacyCaseKindState::PortabilityExport => Self::PortabilityExport,
            PrivacyCaseKindState::RestrictProcessing => Self::RestrictProcessing,
            PrivacyCaseKindState::Erasure => Self::Erasure,
        }
    }
}

impl From<SubjectVerificationMethod> for SubjectVerificationMethodState {
    fn from(value: SubjectVerificationMethod) -> Self {
        match value {
            SubjectVerificationMethod::AuthenticatedPortal => Self::AuthenticatedPortal,
            SubjectVerificationMethod::StaffAssisted => Self::StaffAssisted,
            SubjectVerificationMethod::VerifiedDocument => Self::VerifiedDocument,
            SubjectVerificationMethod::ExistingHighAssuranceIdentity => {
                Self::ExistingHighAssuranceIdentity
            }
        }
    }
}

impl From<SubjectVerificationMethodState> for SubjectVerificationMethod {
    fn from(value: SubjectVerificationMethodState) -> Self {
        match value {
            SubjectVerificationMethodState::AuthenticatedPortal => Self::AuthenticatedPortal,
            SubjectVerificationMethodState::StaffAssisted => Self::StaffAssisted,
            SubjectVerificationMethodState::VerifiedDocument => Self::VerifiedDocument,
            SubjectVerificationMethodState::ExistingHighAssuranceIdentity => {
                Self::ExistingHighAssuranceIdentity
            }
        }
    }
}

impl From<ResumeStage> for ResumeStageState {
    fn from(value: ResumeStage) -> Self {
        match value {
            ResumeStage::Scoping => Self::Scoping,
            ResumeStage::Planning => Self::Planning,
            ResumeStage::Executing => Self::Executing,
            ResumeStage::Converging => Self::Converging,
        }
    }
}

impl From<ResumeStageState> for ResumeStage {
    fn from(value: ResumeStageState) -> Self {
        match value {
            ResumeStageState::Scoping => Self::Scoping,
            ResumeStageState::Planning => Self::Planning,
            ResumeStageState::Executing => Self::Executing,
            ResumeStageState::Converging => Self::Converging,
        }
    }
}

impl From<PrivacyCaseStatus> for PrivacyCaseStatusState {
    fn from(value: PrivacyCaseStatus) -> Self {
        match value {
            PrivacyCaseStatus::Draft => Self::Draft,
            PrivacyCaseStatus::Submitted => Self::Submitted,
            PrivacyCaseStatus::SubjectVerified => Self::SubjectVerified,
            PrivacyCaseStatus::Scoping => Self::Scoping,
            PrivacyCaseStatus::Scoped => Self::Scoped,
            PrivacyCaseStatus::Planned => Self::Planned,
            PrivacyCaseStatus::AwaitingApproval => Self::AwaitingApproval,
            PrivacyCaseStatus::Executing => Self::Executing,
            PrivacyCaseStatus::Converging => Self::Converging,
            PrivacyCaseStatus::RescopeRequired => Self::RescopeRequired,
            PrivacyCaseStatus::FailedRetryable(stage) => Self::FailedRetryable(stage.into()),
            PrivacyCaseStatus::Completed => Self::Completed,
            PrivacyCaseStatus::PartiallyCompleted => Self::PartiallyCompleted,
            PrivacyCaseStatus::Denied => Self::Denied,
            PrivacyCaseStatus::Cancelled => Self::Cancelled,
            PrivacyCaseStatus::FailedTerminal => Self::FailedTerminal,
        }
    }
}

impl From<PrivacyCaseStatusState> for PrivacyCaseStatus {
    fn from(value: PrivacyCaseStatusState) -> Self {
        match value {
            PrivacyCaseStatusState::Draft => Self::Draft,
            PrivacyCaseStatusState::Submitted => Self::Submitted,
            PrivacyCaseStatusState::SubjectVerified => Self::SubjectVerified,
            PrivacyCaseStatusState::Scoping => Self::Scoping,
            PrivacyCaseStatusState::Scoped => Self::Scoped,
            PrivacyCaseStatusState::Planned => Self::Planned,
            PrivacyCaseStatusState::AwaitingApproval => Self::AwaitingApproval,
            PrivacyCaseStatusState::Executing => Self::Executing,
            PrivacyCaseStatusState::Converging => Self::Converging,
            PrivacyCaseStatusState::RescopeRequired => Self::RescopeRequired,
            PrivacyCaseStatusState::FailedRetryable(stage) => Self::FailedRetryable(stage.into()),
            PrivacyCaseStatusState::Completed => Self::Completed,
            PrivacyCaseStatusState::PartiallyCompleted => Self::PartiallyCompleted,
            PrivacyCaseStatusState::Denied => Self::Denied,
            PrivacyCaseStatusState::Cancelled => Self::Cancelled,
            PrivacyCaseStatusState::FailedTerminal => Self::FailedTerminal,
        }
    }
}

impl From<RestrictionScope> for RestrictionScopeState {
    fn from(value: RestrictionScope) -> Self {
        match value {
            RestrictionScope::Processing => Self::Processing,
            RestrictionScope::Communication => Self::Communication,
            RestrictionScope::ProcessingAndCommunication => Self::ProcessingAndCommunication,
        }
    }
}

impl From<RestrictionScopeState> for RestrictionScope {
    fn from(value: RestrictionScopeState) -> Self {
        match value {
            RestrictionScopeState::Processing => Self::Processing,
            RestrictionScopeState::Communication => Self::Communication,
            RestrictionScopeState::ProcessingAndCommunication => Self::ProcessingAndCommunication,
        }
    }
}

impl From<RestrictionStatus> for RestrictionStatusState {
    fn from(value: RestrictionStatus) -> Self {
        match value {
            RestrictionStatus::Active => Self::Active,
            RestrictionStatus::Released => Self::Released,
            RestrictionStatus::Expired => Self::Expired,
        }
    }
}

impl From<RestrictionStatusState> for RestrictionStatus {
    fn from(value: RestrictionStatusState) -> Self {
        match value {
            RestrictionStatusState::Active => Self::Active,
            RestrictionStatusState::Released => Self::Released,
            RestrictionStatusState::Expired => Self::Expired,
        }
    }
}

impl From<&LegalHoldScope> for LegalHoldScopeState {
    fn from(value: &LegalHoldScope) -> Self {
        match value {
            LegalHoldScope::AllCustomerData => Self::AllCustomerData,
            LegalHoldScope::DataClass(data_class) => Self::DataClass(*data_class),
            LegalHoldScope::Owner(owner) => Self::Owner(owner.as_str().to_owned()),
        }
    }
}

impl LegalHoldScopeState {
    fn into_domain(self) -> Result<LegalHoldScope, SdkError> {
        match self {
            Self::AllCustomerData => Ok(LegalHoldScope::AllCustomerData),
            Self::DataClass(data_class) => Ok(LegalHoldScope::DataClass(data_class)),
            Self::Owner(owner) => Ok(LegalHoldScope::Owner(module_id(owner)?)),
        }
    }
}

impl From<LegalHoldStatus> for LegalHoldStatusState {
    fn from(value: LegalHoldStatus) -> Self {
        match value {
            LegalHoldStatus::Active => Self::Active,
            LegalHoldStatus::Released => Self::Released,
        }
    }
}

impl From<LegalHoldStatusState> for LegalHoldStatus {
    fn from(value: LegalHoldStatusState) -> Self {
        match value {
            LegalHoldStatusState::Active => Self::Active,
            LegalHoldStatusState::Released => Self::Released,
        }
    }
}

fn validate_privacy_case(case: &PrivacyCase) -> Result<(), SdkError> {
    if case.version == 0
        || case.created_at_unix_nanos < 0
        || case.last_transition_at_unix_nanos < case.created_at_unix_nanos
    {
        return Err(persisted_error("privacy-case version or transition time is invalid"));
    }

    if case.scope_snapshot_id.is_some() && case.subject_binding.is_none() {
        return Err(persisted_error(
            "privacy-case scope evidence requires verified subject binding",
        ));
    }
    if case.action_plan_id.is_some() && case.scope_snapshot_id.is_none() {
        return Err(persisted_error(
            "privacy-case action plan requires immutable scope evidence",
        ));
    }
    if case.approval.is_some() && case.action_plan_id.is_none() {
        return Err(persisted_error(
            "privacy-case approval requires an immutable action plan",
        ));
    }

    if let Some(binding) = &case.subject_binding
        && (binding.verified_at_unix_nanos < case.created_at_unix_nanos
            || binding.verified_at_unix_nanos > case.last_transition_at_unix_nanos)
        {
            return Err(persisted_error(
                "privacy-case subject-verification time is inconsistent",
            ));
        }

    if let Some(requirement) = &case.pending_rescope {
        let binding = case.subject_binding.as_ref().ok_or_else(|| {
            persisted_error("privacy-case rescope evidence requires subject binding")
        })?;
        if requirement.previous_canonical_party_id != binding.canonical_party_id
            || requirement.previous_identity_resolution_generation
                != binding.identity_resolution_generation
            || requirement.proposed_identity_resolution_generation
                <= requirement.previous_identity_resolution_generation
            || requirement.detected_at_unix_nanos < case.created_at_unix_nanos
            || requirement.detected_at_unix_nanos > case.last_transition_at_unix_nanos
            || case.scope_snapshot_id.is_some()
            || case.action_plan_id.is_some()
            || case.approval.is_some()
        {
            return Err(persisted_error(
                "privacy-case pending rescope evidence is inconsistent",
            ));
        }
    }

    if let Some(approval) = &case.approval
        && (approval.approved_at_unix_nanos < case.created_at_unix_nanos
            || approval.approved_at_unix_nanos > case.last_transition_at_unix_nanos)
        {
            return Err(persisted_error("privacy-case approval time is inconsistent"));
        }

    let subject_required = matches!(
        case.status,
        PrivacyCaseStatus::SubjectVerified
            | PrivacyCaseStatus::Scoping
            | PrivacyCaseStatus::Scoped
            | PrivacyCaseStatus::Planned
            | PrivacyCaseStatus::AwaitingApproval
            | PrivacyCaseStatus::Executing
            | PrivacyCaseStatus::Converging
            | PrivacyCaseStatus::RescopeRequired
            | PrivacyCaseStatus::FailedRetryable(_)
            | PrivacyCaseStatus::Completed
            | PrivacyCaseStatus::PartiallyCompleted
    );
    if subject_required && case.subject_binding.is_none() {
        return Err(persisted_error(
            "privacy-case lifecycle requires verified subject binding",
        ));
    }

    if case.status == PrivacyCaseStatus::RescopeRequired && case.pending_rescope.is_none() {
        return Err(persisted_error(
            "rescope-required privacy case is missing rescope evidence",
        ));
    }
    if case.pending_rescope.is_some()
        && !matches!(
            case.status,
            PrivacyCaseStatus::RescopeRequired
                | PrivacyCaseStatus::Cancelled
                | PrivacyCaseStatus::FailedTerminal
        )
    {
        return Err(persisted_error(
            "privacy-case rescope evidence is invalid for its lifecycle state",
        ));
    }

    match case.status {
        PrivacyCaseStatus::Draft => {
            if case.version != 1
                || case.subject_binding.is_some()
                || case.scope_snapshot_id.is_some()
                || case.action_plan_id.is_some()
                || case.approval.is_some()
                || case.pending_rescope.is_some()
            {
                return Err(persisted_error("draft privacy-case invariants are invalid"));
            }
        }
        PrivacyCaseStatus::Submitted => {
            if case.subject_binding.is_some()
                || case.scope_snapshot_id.is_some()
                || case.action_plan_id.is_some()
                || case.approval.is_some()
                || case.pending_rescope.is_some()
            {
                return Err(persisted_error(
                    "submitted privacy-case invariants are invalid",
                ));
            }
        }
        PrivacyCaseStatus::SubjectVerified | PrivacyCaseStatus::Scoping => {
            if case.scope_snapshot_id.is_some()
                || case.action_plan_id.is_some()
                || case.approval.is_some()
                || case.pending_rescope.is_some()
            {
                return Err(persisted_error(
                    "pre-scope privacy-case evidence is inconsistent",
                ));
            }
        }
        PrivacyCaseStatus::Scoped => {
            if case.scope_snapshot_id.is_none()
                || case.action_plan_id.is_some()
                || case.approval.is_some()
                || case.pending_rescope.is_some()
            {
                return Err(persisted_error("scoped privacy-case invariants are invalid"));
            }
        }
        PrivacyCaseStatus::Planned => {
            if case.scope_snapshot_id.is_none()
                || case.action_plan_id.is_none()
                || case.pending_rescope.is_some()
            {
                return Err(persisted_error("planned privacy-case invariants are invalid"));
            }
        }
        PrivacyCaseStatus::AwaitingApproval => {
            if case.scope_snapshot_id.is_none()
                || case.action_plan_id.is_none()
                || case.approval.is_some()
                || case.pending_rescope.is_some()
            {
                return Err(persisted_error(
                    "awaiting-approval privacy-case invariants are invalid",
                ));
            }
        }
        PrivacyCaseStatus::Executing
        | PrivacyCaseStatus::Converging
        | PrivacyCaseStatus::Completed
        | PrivacyCaseStatus::PartiallyCompleted => {
            if case.scope_snapshot_id.is_none()
                || case.action_plan_id.is_none()
                || case.pending_rescope.is_some()
            {
                return Err(persisted_error(
                    "execution privacy-case evidence is incomplete",
                ));
            }
        }
        PrivacyCaseStatus::FailedRetryable(stage) => match stage {
            ResumeStage::Scoping => {
                if case.scope_snapshot_id.is_some()
                    || case.action_plan_id.is_some()
                    || case.pending_rescope.is_some()
                {
                    return Err(persisted_error(
                        "retryable scoping evidence is inconsistent",
                    ));
                }
            }
            ResumeStage::Planning => {
                if case.scope_snapshot_id.is_none() || case.pending_rescope.is_some() {
                    return Err(persisted_error(
                        "retryable planning evidence is incomplete",
                    ));
                }
            }
            ResumeStage::Executing | ResumeStage::Converging => {
                if case.scope_snapshot_id.is_none()
                    || case.action_plan_id.is_none()
                    || case.pending_rescope.is_some()
                {
                    return Err(persisted_error(
                        "retryable execution evidence is incomplete",
                    ));
                }
            }
        },
        PrivacyCaseStatus::RescopeRequired
        | PrivacyCaseStatus::Denied
        | PrivacyCaseStatus::Cancelled
        | PrivacyCaseStatus::FailedTerminal => {}
    }

    Ok(())
}

fn validate_processing_restriction(restriction: &ProcessingRestriction) -> Result<(), SdkError> {
    if restriction.version == 0
        || restriction.placed_at_unix_nanos < 0
        || restriction.effective_from_unix_nanos < restriction.placed_at_unix_nanos
    {
        return Err(persisted_error(
            "processing-restriction version or effective time is invalid",
        ));
    }
    if restriction
        .expires_at_unix_nanos
        .is_some_and(|value| value <= restriction.effective_from_unix_nanos)
    {
        return Err(persisted_error(
            "processing-restriction expiry is invalid",
        ));
    }

    match restriction.status {
        RestrictionStatus::Active => {
            if restriction.released_by.is_some() || restriction.released_at_unix_nanos.is_some() {
                return Err(persisted_error(
                    "active processing restriction contains release evidence",
                ));
            }
        }
        RestrictionStatus::Released => {
            let released_at = restriction.released_at_unix_nanos.ok_or_else(|| {
                persisted_error("released processing restriction is missing release time")
            })?;
            if restriction.version < 2
                || restriction.released_by.is_none()
                || released_at < restriction.placed_at_unix_nanos
            {
                return Err(persisted_error(
                    "released processing-restriction evidence is invalid",
                ));
            }
        }
        RestrictionStatus::Expired => {
            if restriction.version < 2
                || restriction.expires_at_unix_nanos.is_none()
                || restriction.released_by.is_some()
                || restriction.released_at_unix_nanos.is_some()
            {
                return Err(persisted_error(
                    "expired processing-restriction evidence is invalid",
                ));
            }
        }
    }
    Ok(())
}

fn validate_legal_hold(hold: &CustomerDataLegalHold) -> Result<(), SdkError> {
    if hold.version == 0 || hold.effective_from_unix_nanos < 0 {
        return Err(persisted_error(
            "customer-data legal-hold version or effective time is invalid",
        ));
    }
    if hold
        .effective_until_unix_nanos
        .is_some_and(|value| value <= hold.effective_from_unix_nanos)
    {
        return Err(persisted_error(
            "customer-data legal-hold end time is invalid",
        ));
    }
    super::validate_reason_code(&hold.reason_code)
        .map_err(|error| persisted_domain_error("customer-data legal hold", error))?;

    match hold.status {
        LegalHoldStatus::Active => {
            if hold.released_by.is_some() || hold.released_at_unix_nanos.is_some() {
                return Err(persisted_error(
                    "active customer-data legal hold contains release evidence",
                ));
            }
        }
        LegalHoldStatus::Released => {
            let released_at = hold.released_at_unix_nanos.ok_or_else(|| {
                persisted_error("released customer-data legal hold is missing release time")
            })?;
            if hold.version < 2
                || hold.released_by.is_none()
                || released_at < hold.effective_from_unix_nanos
            {
                return Err(persisted_error(
                    "released customer-data legal-hold evidence is invalid",
                ));
            }
        }
    }
    Ok(())
}

fn record_id(value: String, label: &str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value)
        .map_err(|error| persisted_error(format!("persisted {label} identity is invalid: {error}")))
}

fn optional_record_id(value: Option<String>, label: &str) -> Result<Option<RecordId>, SdkError> {
    value.map(|value| record_id(value, label)).transpose()
}

fn tenant_id(value: String) -> Result<TenantId, SdkError> {
    TenantId::try_new(value)
        .map_err(|error| persisted_error(format!("persisted tenant identity is invalid: {error}")))
}

fn actor_id(value: String) -> Result<ActorId, SdkError> {
    ActorId::try_new(value)
        .map_err(|error| persisted_error(format!("persisted actor identity is invalid: {error}")))
}

fn optional_actor_id(value: Option<String>) -> Result<Option<ActorId>, SdkError> {
    value.map(actor_id).transpose()
}

fn module_id(value: String) -> Result<ModuleId, SdkError> {
    ModuleId::try_new(value)
        .map_err(|error| persisted_error(format!("persisted owner identity is invalid: {error}")))
}

fn schema_version(value: String) -> Result<SchemaVersion, SdkError> {
    SchemaVersion::try_new(value).map_err(|error| {
        persisted_error(format!("persisted policy version is invalid: {error}"))
    })
}

fn decimal_i64(value: String, field: &str) -> Result<i64, SdkError> {
    let parsed = value
        .parse::<i64>()
        .map_err(|error| persisted_error(format!("persisted {field} decimal is invalid: {error}")))?;
    if parsed.to_string() != value {
        return Err(persisted_error(format!(
            "persisted {field} decimal is not canonical"
        )));
    }
    Ok(parsed)
}

fn optional_decimal_i64(value: Option<String>, field: &str) -> Result<Option<i64>, SdkError> {
    value.map(|value| decimal_i64(value, field)).transpose()
}

fn decimal_u64(value: String, field: &str) -> Result<u64, SdkError> {
    let parsed = value
        .parse::<u64>()
        .map_err(|error| persisted_error(format!("persisted {field} decimal is invalid: {error}")))?;
    if parsed.to_string() != value {
        return Err(persisted_error(format!(
            "persisted {field} decimal is not canonical"
        )));
    }
    Ok(parsed)
}

fn validate_size(bytes: &[u8], maximum_bytes: u64, label: &str) -> Result<(), SdkError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum_bytes {
        return Err(persisted_error(format!(
            "{label} state exceeds the maximum of {maximum_bytes} bytes"
        )));
    }
    Ok(())
}

fn persisted_domain_error(label: &str, error: PrivacyDomainError) -> SdkError {
    persisted_error(format!(
        "{label} failed strict persisted-state validation: {}: {error}",
        error.code()
    ))
}

fn persisted_error(message: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted Customer Privacy state is invalid.",
    )
    .with_internal_reference(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_id(value: &str) -> RecordId {
        RecordId::try_new(value).unwrap()
    }

    fn tenant_id() -> TenantId {
        TenantId::try_new("tenant-a").unwrap()
    }

    fn actor_id(value: &str) -> ActorId {
        ActorId::try_new(value).unwrap()
    }

    fn policy_version() -> SchemaVersion {
        SchemaVersion::try_new("privacy-policy/1").unwrap()
    }

    fn completed_case() -> PrivacyCase {
        let mut case = PrivacyCase::new(
            record_id("case-1"),
            tenant_id(),
            PrivacyCaseKind::Erasure,
            policy_version(),
            10,
            None,
        )
        .unwrap();
        case.submit(1, 11).unwrap();
        case.verify_subject(
            2,
            record_id("party-submitted"),
            record_id("party-canonical"),
            7,
            SubjectVerificationMethod::AuthenticatedPortal,
            actor_id("subject-actor"),
            12,
        )
        .unwrap();
        case.begin_scoping(3, 13).unwrap();
        case.record_scope(4, record_id("scope-1"), 14).unwrap();
        case.record_plan(5, record_id("plan-1"), true, 15)
            .unwrap();
        case.approve(6, actor_id("privacy-approver"), 16)
            .unwrap();
        case.begin_execution(7, 17).unwrap();
        case.begin_convergence(8, 18).unwrap();
        case.complete(9, super::super::CompletionOutcome::Completed, 19)
            .unwrap();
        case
    }

    #[test]
    fn privacy_case_round_trip_is_strict_profiled_canonical_json() {
        let case = completed_case();
        let bytes = encode_privacy_case_state(&case).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(text.contains("\"canonicalization_profile\":\"crm.cjson/v1\""));
        assert!(text.contains("\"version\":\"10\""));
        assert!(text.contains("\"created_at_unix_nanos\":\"10\""));
        assert_eq!(decode_privacy_case_state(&bytes).unwrap(), case);

        let mut whitespace = b" ".to_vec();
        whitespace.extend_from_slice(&bytes);
        assert!(decode_privacy_case_state(&whitespace).is_err());

        let mut unknown = text;
        unknown.pop();
        unknown.push_str(",\"unknown_field\":true}");
        assert!(decode_privacy_case_state(unknown.as_bytes()).is_err());
    }

    #[test]
    fn privacy_case_rejects_noncanonical_decimal_and_wrong_profile() {
        let case = completed_case();
        let mut state = PrivacyCaseStateV1::from(&case);
        state.version = "010".to_owned();
        assert!(state.into_domain().is_err());

        let bytes = encode_privacy_case_state(&case).unwrap();
        let wrong_profile = String::from_utf8(bytes)
            .unwrap()
            .replace("crm.cjson/v1", "crm.cjson/v2");
        assert!(decode_privacy_case_state(wrong_profile.as_bytes()).is_err());
    }

    #[test]
    fn processing_restriction_round_trip_preserves_release_evidence() {
        let mut restriction = ProcessingRestriction::place(
            record_id("restriction-1"),
            tenant_id(),
            record_id("party-1"),
            RestrictionScope::ProcessingAndCommunication,
            policy_version(),
            actor_id("privacy-officer"),
            10,
            10,
            Some(20),
        )
        .unwrap();
        restriction
            .release(1, actor_id("privacy-officer"), 15)
            .unwrap();
        let bytes = encode_processing_restriction_state(&restriction).unwrap();
        assert_eq!(
            decode_processing_restriction_state(&bytes).unwrap(),
            restriction
        );

        let mut contradictory = ProcessingRestrictionStateV1::from(&restriction);
        contradictory.status = RestrictionStatusState::Active;
        assert!(contradictory.into_domain().is_err());
    }

    #[test]
    fn legal_hold_round_trip_preserves_scope_and_append_only_release() {
        let mut hold = CustomerDataLegalHold::place(
            record_id("hold-1"),
            tenant_id(),
            record_id("party-1"),
            LegalHoldScope::DataClass(DataClass::Personal),
            record_id("authority-1"),
            "LITIGATION_HOLD",
            policy_version(),
            actor_id("legal-officer"),
            10,
            None,
        )
        .unwrap();
        hold.release(1, actor_id("legal-officer"), 20).unwrap();
        let bytes = encode_legal_hold_state(&hold).unwrap();
        assert_eq!(decode_legal_hold_state(&bytes).unwrap(), hold);
        assert_ne!(legal_hold_state_descriptor_hash(), [0; 32]);
        assert_ne!(
            privacy_case_state_descriptor_hash(),
            processing_restriction_state_descriptor_hash()
        );
    }
}
