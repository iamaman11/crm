use crm_module_sdk::{ActorId, DataClass, ModuleId, RecordId, SchemaVersion, TenantId};
use std::error::Error;
use std::fmt;

const MAX_REASON_CODE_BYTES: usize = 96;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivacyDomainError {
    InvalidArgument {
        field: &'static str,
        safe_message: &'static str,
    },
    VersionConflict {
        expected: u64,
        actual: u64,
    },
    InvalidTransition {
        aggregate: &'static str,
        from: &'static str,
        operation: &'static str,
    },
}

impl PrivacyDomainError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidArgument { .. } => "CUSTOMER_PRIVACY_INVALID_ARGUMENT",
            Self::VersionConflict { .. } => "CUSTOMER_PRIVACY_VERSION_CONFLICT",
            Self::InvalidTransition { .. } => "CUSTOMER_PRIVACY_INVALID_TRANSITION",
        }
    }

    pub const fn retryable(&self) -> bool {
        matches!(self, Self::VersionConflict { .. })
    }
}

impl fmt::Display for PrivacyDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument {
                field,
                safe_message,
            } => write!(formatter, "{field}: {safe_message}"),
            Self::VersionConflict { expected, actual } => write!(
                formatter,
                "expected aggregate version {expected}, but current version is {actual}"
            ),
            Self::InvalidTransition {
                aggregate,
                from,
                operation,
            } => write!(
                formatter,
                "{operation} is not allowed for {aggregate} in state {from}"
            ),
        }
    }
}

impl Error for PrivacyDomainError {}

fn validate_timestamp(field: &'static str, value: i64) -> Result<(), PrivacyDomainError> {
    if value < 0 {
        return Err(PrivacyDomainError::InvalidArgument {
            field,
            safe_message: "timestamp must not be negative",
        });
    }
    Ok(())
}

fn validate_monotonic_timestamp(
    field: &'static str,
    value: i64,
    previous: i64,
) -> Result<(), PrivacyDomainError> {
    validate_timestamp(field, value)?;
    if value < previous {
        return Err(PrivacyDomainError::InvalidArgument {
            field,
            safe_message: "timestamp must not precede the prior aggregate transition",
        });
    }
    Ok(())
}

fn validate_expected_version(expected: u64, actual: u64) -> Result<(), PrivacyDomainError> {
    if expected != actual {
        return Err(PrivacyDomainError::VersionConflict { expected, actual });
    }
    Ok(())
}

fn validate_reason_code(reason_code: &str) -> Result<(), PrivacyDomainError> {
    if reason_code.is_empty() {
        return Err(PrivacyDomainError::InvalidArgument {
            field: "reason_code",
            safe_message: "reason code must not be empty",
        });
    }
    if reason_code.len() > MAX_REASON_CODE_BYTES {
        return Err(PrivacyDomainError::InvalidArgument {
            field: "reason_code",
            safe_message: "reason code exceeds the bounded maximum",
        });
    }
    if !reason_code
        .bytes()
        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(PrivacyDomainError::InvalidArgument {
            field: "reason_code",
            safe_message: "reason code must use uppercase ASCII, digits or underscores",
        });
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyCaseKind {
    Access,
    PortabilityExport,
    RestrictProcessing,
    Erasure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubjectVerificationMethod {
    AuthenticatedPortal,
    StaffAssisted,
    VerifiedDocument,
    ExistingHighAssuranceIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeStage {
    Scoping,
    Planning,
    Executing,
    Converging,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionOutcome {
    Completed,
    PartiallyCompleted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyCaseStatus {
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
    FailedRetryable(ResumeStage),
    Completed,
    PartiallyCompleted,
    Denied,
    Cancelled,
    FailedTerminal,
}

impl PrivacyCaseStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Submitted => "submitted",
            Self::SubjectVerified => "subject_verified",
            Self::Scoping => "scoping",
            Self::Scoped => "scoped",
            Self::Planned => "planned",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Executing => "executing",
            Self::Converging => "converging",
            Self::RescopeRequired => "rescope_required",
            Self::FailedRetryable(_) => "failed_retryable",
            Self::Completed => "completed",
            Self::PartiallyCompleted => "partially_completed",
            Self::Denied => "denied",
            Self::Cancelled => "cancelled",
            Self::FailedTerminal => "failed_terminal",
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed
                | Self::PartiallyCompleted
                | Self::Denied
                | Self::Cancelled
                | Self::FailedTerminal
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectBinding {
    pub submitted_party_id: RecordId,
    pub canonical_party_id: RecordId,
    pub identity_resolution_generation: u64,
    pub verification_method: SubjectVerificationMethod,
    pub verified_by: ActorId,
    pub verified_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RescopeRequirement {
    pub previous_canonical_party_id: RecordId,
    pub proposed_canonical_party_id: RecordId,
    pub previous_identity_resolution_generation: u64,
    pub proposed_identity_resolution_generation: u64,
    pub detected_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalEvidence {
    pub approved_by: ActorId,
    pub approved_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyCase {
    case_id: RecordId,
    tenant_id: TenantId,
    kind: PrivacyCaseKind,
    status: PrivacyCaseStatus,
    version: u64,
    policy_version: SchemaVersion,
    created_at_unix_nanos: i64,
    last_transition_at_unix_nanos: i64,
    previous_case_id: Option<RecordId>,
    subject_binding: Option<SubjectBinding>,
    pending_rescope: Option<RescopeRequirement>,
    scope_snapshot_id: Option<RecordId>,
    action_plan_id: Option<RecordId>,
    approval: Option<ApprovalEvidence>,
}

impl PrivacyCase {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        case_id: RecordId,
        tenant_id: TenantId,
        kind: PrivacyCaseKind,
        policy_version: SchemaVersion,
        created_at_unix_nanos: i64,
        previous_case_id: Option<RecordId>,
    ) -> Result<Self, PrivacyDomainError> {
        validate_timestamp("created_at_unix_nanos", created_at_unix_nanos)?;
        Ok(Self {
            case_id,
            tenant_id,
            kind,
            status: PrivacyCaseStatus::Draft,
            version: 1,
            policy_version,
            created_at_unix_nanos,
            last_transition_at_unix_nanos: created_at_unix_nanos,
            previous_case_id,
            subject_binding: None,
            pending_rescope: None,
            scope_snapshot_id: None,
            action_plan_id: None,
            approval: None,
        })
    }

    pub fn case_id(&self) -> &RecordId {
        &self.case_id
    }

    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    pub const fn kind(&self) -> PrivacyCaseKind {
        self.kind
    }

    pub const fn status(&self) -> PrivacyCaseStatus {
        self.status
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn policy_version(&self) -> &SchemaVersion {
        &self.policy_version
    }

    pub const fn created_at_unix_nanos(&self) -> i64 {
        self.created_at_unix_nanos
    }

    pub fn previous_case_id(&self) -> Option<&RecordId> {
        self.previous_case_id.as_ref()
    }

    pub fn subject_binding(&self) -> Option<&SubjectBinding> {
        self.subject_binding.as_ref()
    }

    pub fn pending_rescope(&self) -> Option<&RescopeRequirement> {
        self.pending_rescope.as_ref()
    }

    pub fn scope_snapshot_id(&self) -> Option<&RecordId> {
        self.scope_snapshot_id.as_ref()
    }

    pub fn action_plan_id(&self) -> Option<&RecordId> {
        self.action_plan_id.as_ref()
    }

    pub fn approval(&self) -> Option<&ApprovalEvidence> {
        self.approval.as_ref()
    }

    pub fn submit(
        &mut self,
        expected_version: u64,
        at_unix_nanos: i64,
    ) -> Result<(), PrivacyDomainError> {
        self.require_status("privacy_case", "submit", PrivacyCaseStatus::Draft)?;
        self.transition(
            expected_version,
            at_unix_nanos,
            PrivacyCaseStatus::Submitted,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn verify_subject(
        &mut self,
        expected_version: u64,
        submitted_party_id: RecordId,
        canonical_party_id: RecordId,
        identity_resolution_generation: u64,
        verification_method: SubjectVerificationMethod,
        verified_by: ActorId,
        verified_at_unix_nanos: i64,
    ) -> Result<(), PrivacyDomainError> {
        self.require_status(
            "privacy_case",
            "verify_subject",
            PrivacyCaseStatus::Submitted,
        )?;
        validate_expected_version(expected_version, self.version)?;
        validate_monotonic_timestamp(
            "verified_at_unix_nanos",
            verified_at_unix_nanos,
            self.last_transition_at_unix_nanos,
        )?;
        self.subject_binding = Some(SubjectBinding {
            submitted_party_id,
            canonical_party_id,
            identity_resolution_generation,
            verification_method,
            verified_by,
            verified_at_unix_nanos,
        });
        self.finish_transition(verified_at_unix_nanos, PrivacyCaseStatus::SubjectVerified);
        Ok(())
    }

    pub fn begin_scoping(
        &mut self,
        expected_version: u64,
        at_unix_nanos: i64,
    ) -> Result<(), PrivacyDomainError> {
        self.require_status(
            "privacy_case",
            "begin_scoping",
            PrivacyCaseStatus::SubjectVerified,
        )?;
        self.transition(expected_version, at_unix_nanos, PrivacyCaseStatus::Scoping)
    }

    pub fn record_scope(
        &mut self,
        expected_version: u64,
        scope_snapshot_id: RecordId,
        at_unix_nanos: i64,
    ) -> Result<(), PrivacyDomainError> {
        self.require_status("privacy_case", "record_scope", PrivacyCaseStatus::Scoping)?;
        validate_expected_version(expected_version, self.version)?;
        validate_monotonic_timestamp(
            "scoped_at_unix_nanos",
            at_unix_nanos,
            self.last_transition_at_unix_nanos,
        )?;
        self.scope_snapshot_id = Some(scope_snapshot_id);
        self.finish_transition(at_unix_nanos, PrivacyCaseStatus::Scoped);
        Ok(())
    }

    pub fn record_plan(
        &mut self,
        expected_version: u64,
        action_plan_id: RecordId,
        approval_required: bool,
        at_unix_nanos: i64,
    ) -> Result<(), PrivacyDomainError> {
        self.require_status("privacy_case", "record_plan", PrivacyCaseStatus::Scoped)?;
        validate_expected_version(expected_version, self.version)?;
        validate_monotonic_timestamp(
            "planned_at_unix_nanos",
            at_unix_nanos,
            self.last_transition_at_unix_nanos,
        )?;
        self.action_plan_id = Some(action_plan_id);
        let next = if approval_required {
            PrivacyCaseStatus::AwaitingApproval
        } else {
            PrivacyCaseStatus::Planned
        };
        self.finish_transition(at_unix_nanos, next);
        Ok(())
    }

    pub fn approve(
        &mut self,
        expected_version: u64,
        approved_by: ActorId,
        approved_at_unix_nanos: i64,
    ) -> Result<(), PrivacyDomainError> {
        self.require_status(
            "privacy_case",
            "approve",
            PrivacyCaseStatus::AwaitingApproval,
        )?;
        validate_expected_version(expected_version, self.version)?;
        validate_monotonic_timestamp(
            "approved_at_unix_nanos",
            approved_at_unix_nanos,
            self.last_transition_at_unix_nanos,
        )?;
        self.approval = Some(ApprovalEvidence {
            approved_by,
            approved_at_unix_nanos,
        });
        self.finish_transition(approved_at_unix_nanos, PrivacyCaseStatus::Planned);
        Ok(())
    }

    pub fn begin_execution(
        &mut self,
        expected_version: u64,
        at_unix_nanos: i64,
    ) -> Result<(), PrivacyDomainError> {
        self.require_status(
            "privacy_case",
            "begin_execution",
            PrivacyCaseStatus::Planned,
        )?;
        if self.action_plan_id.is_none() {
            return Err(PrivacyDomainError::InvalidArgument {
                field: "action_plan_id",
                safe_message: "an immutable action plan is required before execution",
            });
        }
        self.transition(
            expected_version,
            at_unix_nanos,
            PrivacyCaseStatus::Executing,
        )
    }

    pub fn begin_convergence(
        &mut self,
        expected_version: u64,
        at_unix_nanos: i64,
    ) -> Result<(), PrivacyDomainError> {
        self.require_status(
            "privacy_case",
            "begin_convergence",
            PrivacyCaseStatus::Executing,
        )?;
        self.transition(
            expected_version,
            at_unix_nanos,
            PrivacyCaseStatus::Converging,
        )
    }

    pub fn complete(
        &mut self,
        expected_version: u64,
        outcome: CompletionOutcome,
        at_unix_nanos: i64,
    ) -> Result<(), PrivacyDomainError> {
        self.require_status("privacy_case", "complete", PrivacyCaseStatus::Converging)?;
        let next = match outcome {
            CompletionOutcome::Completed => PrivacyCaseStatus::Completed,
            CompletionOutcome::PartiallyCompleted => PrivacyCaseStatus::PartiallyCompleted,
        };
        self.transition(expected_version, at_unix_nanos, next)
    }

    pub fn deny(
        &mut self,
        expected_version: u64,
        at_unix_nanos: i64,
    ) -> Result<(), PrivacyDomainError> {
        if !matches!(
            self.status,
            PrivacyCaseStatus::Submitted
                | PrivacyCaseStatus::SubjectVerified
                | PrivacyCaseStatus::Scoping
                | PrivacyCaseStatus::Scoped
                | PrivacyCaseStatus::Planned
                | PrivacyCaseStatus::AwaitingApproval
                | PrivacyCaseStatus::Executing
        ) {
            return Err(self.invalid_transition("privacy_case", "deny"));
        }
        self.transition(expected_version, at_unix_nanos, PrivacyCaseStatus::Denied)
    }

    pub fn cancel(
        &mut self,
        expected_version: u64,
        at_unix_nanos: i64,
    ) -> Result<(), PrivacyDomainError> {
        if !matches!(
            self.status,
            PrivacyCaseStatus::Draft
                | PrivacyCaseStatus::Submitted
                | PrivacyCaseStatus::SubjectVerified
                | PrivacyCaseStatus::Scoping
                | PrivacyCaseStatus::Scoped
                | PrivacyCaseStatus::Planned
                | PrivacyCaseStatus::AwaitingApproval
                | PrivacyCaseStatus::RescopeRequired
                | PrivacyCaseStatus::FailedRetryable(_)
        ) {
            return Err(self.invalid_transition("privacy_case", "cancel"));
        }
        self.transition(
            expected_version,
            at_unix_nanos,
            PrivacyCaseStatus::Cancelled,
        )
    }

    pub fn fail_retryable(
        &mut self,
        expected_version: u64,
        at_unix_nanos: i64,
    ) -> Result<ResumeStage, PrivacyDomainError> {
        let resume_stage = match self.status {
            PrivacyCaseStatus::SubjectVerified | PrivacyCaseStatus::Scoping => ResumeStage::Scoping,
            PrivacyCaseStatus::Scoped
            | PrivacyCaseStatus::Planned
            | PrivacyCaseStatus::AwaitingApproval => ResumeStage::Planning,
            PrivacyCaseStatus::Executing => ResumeStage::Executing,
            PrivacyCaseStatus::Converging => ResumeStage::Converging,
            _ => {
                return Err(self.invalid_transition("privacy_case", "fail_retryable"));
            }
        };
        self.transition(
            expected_version,
            at_unix_nanos,
            PrivacyCaseStatus::FailedRetryable(resume_stage),
        )?;
        Ok(resume_stage)
    }

    pub fn resume(
        &mut self,
        expected_version: u64,
        at_unix_nanos: i64,
    ) -> Result<ResumeStage, PrivacyDomainError> {
        let PrivacyCaseStatus::FailedRetryable(resume_stage) = self.status else {
            return Err(self.invalid_transition("privacy_case", "resume"));
        };
        let next = match resume_stage {
            ResumeStage::Scoping => PrivacyCaseStatus::Scoping,
            ResumeStage::Planning => PrivacyCaseStatus::Scoped,
            ResumeStage::Executing => PrivacyCaseStatus::Executing,
            ResumeStage::Converging => PrivacyCaseStatus::Converging,
        };
        self.transition(expected_version, at_unix_nanos, next)?;
        Ok(resume_stage)
    }

    pub fn fail_terminal(
        &mut self,
        expected_version: u64,
        at_unix_nanos: i64,
    ) -> Result<(), PrivacyDomainError> {
        if self.status.is_terminal() {
            return Err(self.invalid_transition("privacy_case", "fail_terminal"));
        }
        self.transition(
            expected_version,
            at_unix_nanos,
            PrivacyCaseStatus::FailedTerminal,
        )
    }

    pub fn require_rescope(
        &mut self,
        expected_version: u64,
        proposed_canonical_party_id: RecordId,
        proposed_identity_resolution_generation: u64,
        detected_at_unix_nanos: i64,
    ) -> Result<(), PrivacyDomainError> {
        if !matches!(
            self.status,
            PrivacyCaseStatus::SubjectVerified
                | PrivacyCaseStatus::Scoping
                | PrivacyCaseStatus::Scoped
                | PrivacyCaseStatus::Planned
                | PrivacyCaseStatus::AwaitingApproval
                | PrivacyCaseStatus::FailedRetryable(_)
        ) {
            return Err(self.invalid_transition("privacy_case", "require_rescope"));
        }
        validate_expected_version(expected_version, self.version)?;
        validate_monotonic_timestamp(
            "detected_at_unix_nanos",
            detected_at_unix_nanos,
            self.last_transition_at_unix_nanos,
        )?;
        let binding = self
            .subject_binding
            .as_ref()
            .ok_or(PrivacyDomainError::InvalidArgument {
                field: "subject_binding",
                safe_message: "verified subject binding is required",
            })?;
        if proposed_identity_resolution_generation <= binding.identity_resolution_generation {
            return Err(PrivacyDomainError::InvalidArgument {
                field: "proposed_identity_resolution_generation",
                safe_message: "rescope generation must advance authoritative lineage",
            });
        }
        self.pending_rescope = Some(RescopeRequirement {
            previous_canonical_party_id: binding.canonical_party_id.clone(),
            proposed_canonical_party_id,
            previous_identity_resolution_generation: binding.identity_resolution_generation,
            proposed_identity_resolution_generation,
            detected_at_unix_nanos,
        });
        self.scope_snapshot_id = None;
        self.action_plan_id = None;
        self.approval = None;
        self.finish_transition(detected_at_unix_nanos, PrivacyCaseStatus::RescopeRequired);
        Ok(())
    }

    pub fn accept_rescope(
        &mut self,
        expected_version: u64,
        verified_by: ActorId,
        verification_method: SubjectVerificationMethod,
        verified_at_unix_nanos: i64,
    ) -> Result<(), PrivacyDomainError> {
        self.require_status(
            "privacy_case",
            "accept_rescope",
            PrivacyCaseStatus::RescopeRequired,
        )?;
        validate_expected_version(expected_version, self.version)?;
        validate_monotonic_timestamp(
            "verified_at_unix_nanos",
            verified_at_unix_nanos,
            self.last_transition_at_unix_nanos,
        )?;
        let requirement =
            self.pending_rescope
                .take()
                .ok_or(PrivacyDomainError::InvalidArgument {
                    field: "pending_rescope",
                    safe_message: "rescope evidence is missing",
                })?;
        let binding = self
            .subject_binding
            .as_mut()
            .ok_or(PrivacyDomainError::InvalidArgument {
                field: "subject_binding",
                safe_message: "verified subject binding is required",
            })?;
        binding.canonical_party_id = requirement.proposed_canonical_party_id;
        binding.identity_resolution_generation =
            requirement.proposed_identity_resolution_generation;
        binding.verification_method = verification_method;
        binding.verified_by = verified_by;
        binding.verified_at_unix_nanos = verified_at_unix_nanos;
        self.finish_transition(verified_at_unix_nanos, PrivacyCaseStatus::SubjectVerified);
        Ok(())
    }

    fn require_status(
        &self,
        aggregate: &'static str,
        operation: &'static str,
        expected: PrivacyCaseStatus,
    ) -> Result<(), PrivacyDomainError> {
        if self.status != expected {
            return Err(self.invalid_transition(aggregate, operation));
        }
        Ok(())
    }

    fn transition(
        &mut self,
        expected_version: u64,
        at_unix_nanos: i64,
        next: PrivacyCaseStatus,
    ) -> Result<(), PrivacyDomainError> {
        validate_expected_version(expected_version, self.version)?;
        validate_monotonic_timestamp(
            "transition_at_unix_nanos",
            at_unix_nanos,
            self.last_transition_at_unix_nanos,
        )?;
        if self.status.is_terminal() {
            return Err(self.invalid_transition("privacy_case", "transition"));
        }
        self.finish_transition(at_unix_nanos, next);
        Ok(())
    }

    fn finish_transition(&mut self, at_unix_nanos: i64, next: PrivacyCaseStatus) {
        self.status = next;
        self.last_transition_at_unix_nanos = at_unix_nanos;
        self.version += 1;
    }

    fn invalid_transition(
        &self,
        aggregate: &'static str,
        operation: &'static str,
    ) -> PrivacyDomainError {
        PrivacyDomainError::InvalidTransition {
            aggregate,
            from: self.status.label(),
            operation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestrictionScope {
    Processing,
    Communication,
    ProcessingAndCommunication,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestrictionStatus {
    Active,
    Released,
    Expired,
}

impl RestrictionStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Released => "released",
            Self::Expired => "expired",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessingRestriction {
    restriction_id: RecordId,
    tenant_id: TenantId,
    canonical_party_id: RecordId,
    scope: RestrictionScope,
    status: RestrictionStatus,
    version: u64,
    policy_version: SchemaVersion,
    placed_by: ActorId,
    placed_at_unix_nanos: i64,
    effective_from_unix_nanos: i64,
    expires_at_unix_nanos: Option<i64>,
    released_by: Option<ActorId>,
    released_at_unix_nanos: Option<i64>,
}

impl ProcessingRestriction {
    #[allow(clippy::too_many_arguments)]
    pub fn place(
        restriction_id: RecordId,
        tenant_id: TenantId,
        canonical_party_id: RecordId,
        scope: RestrictionScope,
        policy_version: SchemaVersion,
        placed_by: ActorId,
        placed_at_unix_nanos: i64,
        effective_from_unix_nanos: i64,
        expires_at_unix_nanos: Option<i64>,
    ) -> Result<Self, PrivacyDomainError> {
        validate_timestamp("placed_at_unix_nanos", placed_at_unix_nanos)?;
        validate_timestamp("effective_from_unix_nanos", effective_from_unix_nanos)?;
        if effective_from_unix_nanos < placed_at_unix_nanos {
            return Err(PrivacyDomainError::InvalidArgument {
                field: "effective_from_unix_nanos",
                safe_message: "restriction cannot become effective before placement",
            });
        }
        if let Some(expires_at) = expires_at_unix_nanos {
            validate_timestamp("expires_at_unix_nanos", expires_at)?;
            if expires_at <= effective_from_unix_nanos {
                return Err(PrivacyDomainError::InvalidArgument {
                    field: "expires_at_unix_nanos",
                    safe_message: "restriction expiry must follow its effective time",
                });
            }
        }
        Ok(Self {
            restriction_id,
            tenant_id,
            canonical_party_id,
            scope,
            status: RestrictionStatus::Active,
            version: 1,
            policy_version,
            placed_by,
            placed_at_unix_nanos,
            effective_from_unix_nanos,
            expires_at_unix_nanos,
            released_by: None,
            released_at_unix_nanos: None,
        })
    }

    pub fn restriction_id(&self) -> &RecordId {
        &self.restriction_id
    }

    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    pub fn canonical_party_id(&self) -> &RecordId {
        &self.canonical_party_id
    }

    pub const fn scope(&self) -> RestrictionScope {
        self.scope
    }

    pub const fn status(&self) -> RestrictionStatus {
        self.status
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn policy_version(&self) -> &SchemaVersion {
        &self.policy_version
    }

    pub fn placed_by(&self) -> &ActorId {
        &self.placed_by
    }

    pub const fn placed_at_unix_nanos(&self) -> i64 {
        self.placed_at_unix_nanos
    }

    pub fn released_by(&self) -> Option<&ActorId> {
        self.released_by.as_ref()
    }

    pub const fn released_at_unix_nanos(&self) -> Option<i64> {
        self.released_at_unix_nanos
    }

    pub fn is_active_at(&self, at_unix_nanos: i64) -> bool {
        self.status == RestrictionStatus::Active
            && at_unix_nanos >= self.effective_from_unix_nanos
            && self
                .expires_at_unix_nanos
                .is_none_or(|expires_at| at_unix_nanos < expires_at)
    }

    pub fn release(
        &mut self,
        expected_version: u64,
        released_by: ActorId,
        released_at_unix_nanos: i64,
    ) -> Result<(), PrivacyDomainError> {
        if self.status != RestrictionStatus::Active {
            return Err(PrivacyDomainError::InvalidTransition {
                aggregate: "processing_restriction",
                from: self.status.label(),
                operation: "release",
            });
        }
        validate_expected_version(expected_version, self.version)?;
        validate_monotonic_timestamp(
            "released_at_unix_nanos",
            released_at_unix_nanos,
            self.placed_at_unix_nanos,
        )?;
        self.status = RestrictionStatus::Released;
        self.released_by = Some(released_by);
        self.released_at_unix_nanos = Some(released_at_unix_nanos);
        self.version += 1;
        Ok(())
    }

    pub fn expire(
        &mut self,
        expected_version: u64,
        at_unix_nanos: i64,
    ) -> Result<(), PrivacyDomainError> {
        if self.status != RestrictionStatus::Active {
            return Err(PrivacyDomainError::InvalidTransition {
                aggregate: "processing_restriction",
                from: self.status.label(),
                operation: "expire",
            });
        }
        validate_expected_version(expected_version, self.version)?;
        let expires_at = self
            .expires_at_unix_nanos
            .ok_or(PrivacyDomainError::InvalidArgument {
                field: "expires_at_unix_nanos",
                safe_message: "restriction has no configured expiry",
            })?;
        if at_unix_nanos < expires_at {
            return Err(PrivacyDomainError::InvalidArgument {
                field: "at_unix_nanos",
                safe_message: "restriction cannot expire before its configured expiry",
            });
        }
        self.status = RestrictionStatus::Expired;
        self.version += 1;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegalHoldScope {
    AllCustomerData,
    DataClass(DataClass),
    Owner(ModuleId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegalHoldStatus {
    Active,
    Released,
}

impl LegalHoldStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Released => "released",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerDataLegalHold {
    hold_id: RecordId,
    tenant_id: TenantId,
    canonical_party_id: RecordId,
    scope: LegalHoldScope,
    authority_reference: RecordId,
    reason_code: String,
    policy_version: SchemaVersion,
    status: LegalHoldStatus,
    version: u64,
    placed_by: ActorId,
    effective_from_unix_nanos: i64,
    effective_until_unix_nanos: Option<i64>,
    released_by: Option<ActorId>,
    released_at_unix_nanos: Option<i64>,
}

impl CustomerDataLegalHold {
    #[allow(clippy::too_many_arguments)]
    pub fn place(
        hold_id: RecordId,
        tenant_id: TenantId,
        canonical_party_id: RecordId,
        scope: LegalHoldScope,
        authority_reference: RecordId,
        reason_code: impl Into<String>,
        policy_version: SchemaVersion,
        placed_by: ActorId,
        effective_from_unix_nanos: i64,
        effective_until_unix_nanos: Option<i64>,
    ) -> Result<Self, PrivacyDomainError> {
        validate_timestamp("effective_from_unix_nanos", effective_from_unix_nanos)?;
        let reason_code = reason_code.into();
        validate_reason_code(&reason_code)?;
        if let Some(effective_until) = effective_until_unix_nanos {
            validate_timestamp("effective_until_unix_nanos", effective_until)?;
            if effective_until <= effective_from_unix_nanos {
                return Err(PrivacyDomainError::InvalidArgument {
                    field: "effective_until_unix_nanos",
                    safe_message: "legal-hold end must follow its effective time",
                });
            }
        }
        Ok(Self {
            hold_id,
            tenant_id,
            canonical_party_id,
            scope,
            authority_reference,
            reason_code,
            policy_version,
            status: LegalHoldStatus::Active,
            version: 1,
            placed_by,
            effective_from_unix_nanos,
            effective_until_unix_nanos,
            released_by: None,
            released_at_unix_nanos: None,
        })
    }

    pub fn hold_id(&self) -> &RecordId {
        &self.hold_id
    }

    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    pub fn canonical_party_id(&self) -> &RecordId {
        &self.canonical_party_id
    }

    pub fn scope(&self) -> &LegalHoldScope {
        &self.scope
    }

    pub fn authority_reference(&self) -> &RecordId {
        &self.authority_reference
    }

    pub fn reason_code(&self) -> &str {
        &self.reason_code
    }

    pub fn policy_version(&self) -> &SchemaVersion {
        &self.policy_version
    }

    pub const fn status(&self) -> LegalHoldStatus {
        self.status
    }

    pub const fn version(&self) -> u64 {
        self.version
    }

    pub fn placed_by(&self) -> &ActorId {
        &self.placed_by
    }

    pub fn released_by(&self) -> Option<&ActorId> {
        self.released_by.as_ref()
    }

    pub const fn released_at_unix_nanos(&self) -> Option<i64> {
        self.released_at_unix_nanos
    }

    pub fn is_active_at(&self, at_unix_nanos: i64) -> bool {
        self.status == LegalHoldStatus::Active
            && at_unix_nanos >= self.effective_from_unix_nanos
            && self
                .effective_until_unix_nanos
                .is_none_or(|effective_until| at_unix_nanos < effective_until)
    }

    pub fn release(
        &mut self,
        expected_version: u64,
        released_by: ActorId,
        released_at_unix_nanos: i64,
    ) -> Result<(), PrivacyDomainError> {
        if self.status != LegalHoldStatus::Active {
            return Err(PrivacyDomainError::InvalidTransition {
                aggregate: "customer_data_legal_hold",
                from: self.status.label(),
                operation: "release",
            });
        }
        validate_expected_version(expected_version, self.version)?;
        validate_monotonic_timestamp(
            "released_at_unix_nanos",
            released_at_unix_nanos,
            self.effective_from_unix_nanos,
        )?;
        self.status = LegalHoldStatus::Released;
        self.released_by = Some(released_by);
        self.released_at_unix_nanos = Some(released_at_unix_nanos);
        self.version += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_id(value: &str) -> RecordId {
        RecordId::try_new(value).expect("test record id must be valid")
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

    fn verified_case(kind: PrivacyCaseKind) -> PrivacyCase {
        let mut case = PrivacyCase::new(
            record_id("privacy-case-1"),
            tenant_id(),
            kind,
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
        case
    }

    #[test]
    fn privacy_case_happy_path_is_versioned_and_terminal() {
        let mut case = verified_case(PrivacyCaseKind::Erasure);
        case.begin_scoping(3, 13).unwrap();
        case.record_scope(4, record_id("scope-1"), 14).unwrap();
        case.record_plan(5, record_id("plan-1"), true, 15).unwrap();
        case.approve(6, actor_id("privacy-approver"), 16).unwrap();
        case.begin_execution(7, 17).unwrap();
        case.begin_convergence(8, 18).unwrap();
        case.complete(9, CompletionOutcome::Completed, 19).unwrap();

        assert_eq!(case.status(), PrivacyCaseStatus::Completed);
        assert_eq!(case.version(), 10);
        assert!(case.cancel(10, 20).is_err());
    }

    #[test]
    fn stale_case_version_is_rejected_without_transition() {
        let mut case = verified_case(PrivacyCaseKind::Access);
        let error = case.begin_scoping(2, 13).unwrap_err();
        assert_eq!(error.code(), "CUSTOMER_PRIVACY_VERSION_CONFLICT");
        assert!(error.retryable());
        assert_eq!(case.status(), PrivacyCaseStatus::SubjectVerified);
        assert_eq!(case.version(), 3);
    }

    #[test]
    fn retryable_failure_resumes_the_exact_stage() {
        let mut case = verified_case(PrivacyCaseKind::PortabilityExport);
        case.begin_scoping(3, 13).unwrap();
        case.record_scope(4, record_id("scope-1"), 14).unwrap();
        case.record_plan(5, record_id("plan-1"), false, 15).unwrap();
        case.begin_execution(6, 16).unwrap();

        assert_eq!(case.fail_retryable(7, 17).unwrap(), ResumeStage::Executing);
        assert_eq!(
            case.status(),
            PrivacyCaseStatus::FailedRetryable(ResumeStage::Executing)
        );
        assert_eq!(case.resume(8, 18).unwrap(), ResumeStage::Executing);
        assert_eq!(case.status(), PrivacyCaseStatus::Executing);
    }

    #[test]
    fn canonical_change_requires_explicit_rescope_and_new_generation() {
        let mut case = verified_case(PrivacyCaseKind::Erasure);
        let error = case
            .require_rescope(3, record_id("party-new"), 7, 13)
            .unwrap_err();
        assert_eq!(error.code(), "CUSTOMER_PRIVACY_INVALID_ARGUMENT");
        assert_eq!(case.status(), PrivacyCaseStatus::SubjectVerified);

        case.require_rescope(3, record_id("party-new"), 8, 13)
            .unwrap();
        assert_eq!(case.status(), PrivacyCaseStatus::RescopeRequired);
        case.accept_rescope(
            4,
            actor_id("privacy-reviewer"),
            SubjectVerificationMethod::StaffAssisted,
            14,
        )
        .unwrap();
        let binding = case.subject_binding().unwrap();
        assert_eq!(binding.canonical_party_id.as_str(), "party-new");
        assert_eq!(binding.identity_resolution_generation, 8);
    }

    #[test]
    fn restriction_is_live_until_explicit_release_or_exact_expiry() {
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

        assert!(restriction.is_active_at(10));
        assert!(restriction.is_active_at(19));
        assert!(!restriction.is_active_at(20));
        assert!(
            restriction
                .release(0, actor_id("privacy-officer"), 15)
                .is_err()
        );
        restriction
            .release(1, actor_id("privacy-officer"), 15)
            .unwrap();
        assert_eq!(restriction.status(), RestrictionStatus::Released);
        assert!(!restriction.is_active_at(16));
        assert!(restriction.expire(2, 20).is_err());
    }

    #[test]
    fn legal_hold_uses_bounded_reason_and_append_only_release() {
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

        assert!(hold.is_active_at(1_000));
        hold.release(1, actor_id("legal-officer"), 20).unwrap();
        assert_eq!(hold.status(), LegalHoldStatus::Released);
        assert_eq!(hold.version(), 2);
        assert!(!hold.is_active_at(21));
        assert!(hold.release(2, actor_id("legal-officer"), 22).is_err());

        let invalid = CustomerDataLegalHold::place(
            record_id("hold-2"),
            tenant_id(),
            record_id("party-1"),
            LegalHoldScope::AllCustomerData,
            record_id("authority-2"),
            "free form reason",
            policy_version(),
            actor_id("legal-officer"),
            10,
            None,
        )
        .unwrap_err();
        assert_eq!(invalid.code(), "CUSTOMER_PRIVACY_INVALID_ARGUMENT");
    }
}
