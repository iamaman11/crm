use super::PrivacyCaseKind;

/// Trusted-internal exact coordinate for deterministic Customer Privacy planning.
pub const ACTION_PLAN_BUILD_COORDINATE: &str = "customer_privacy.plan.build@1.0.0";
/// Published permission-aware query coordinate for one immutable action plan.
pub const ACTION_PLAN_GET_COORDINATE: &str = "customer_privacy.case.plan.get@1.0.0";
/// Published permission-aware query coordinate for append-once owner outcomes.
pub const OWNER_OUTCOMES_LIST_COORDINATE: &str =
    "customer_privacy.case.owner_outcomes.list@1.0.0";
/// Canonical persisted-state schema for an immutable privacy action plan.
pub const ACTION_PLAN_STATE_SCHEMA_ID: &str = "crm.customer-privacy.action_plan.state";
/// Immutable persisted-state schema version for action plans.
pub const ACTION_PLAN_STATE_SCHEMA_VERSION: &str = "1.0.0";
/// Governed maximum encoded action-plan size.
pub const ACTION_PLAN_STATE_MAXIMUM_BYTES: u64 = 512 * 1024;
/// Retention policy for immutable privacy action-plan evidence.
pub const ACTION_PLAN_STATE_RETENTION_POLICY_ID: &str = "crm.customer_privacy.action_plan";
/// Default page size for permission-aware owner-outcome reads.
pub const OWNER_OUTCOME_DEFAULT_PAGE_SIZE: u32 = 64;
/// Maximum page size for permission-aware owner-outcome reads.
pub const OWNER_OUTCOME_MAXIMUM_PAGE_SIZE: u32 = 128;
/// Maximum opaque cursor size for owner-outcome reads.
pub const OWNER_OUTCOME_MAXIMUM_CURSOR_BYTES: usize = 2_048;
/// Maximum number of resource items in one action plan.
pub const ACTION_PLAN_MAXIMUM_ITEMS: usize = 16_384;

const ACTION_PLAN_STATE_DESCRIPTOR: &[u8] = b"crm.customer-privacy.action_plan.state/v1:plan_id,lineage,planned_at_unix_nanos_decimal,items,plan_digest";
const ACTION_PLAN_ID_PREFIX: &str = "privacy-action-plan-";
const MAXIMUM_JURISDICTION_CODE_BYTES: usize = 64;
const MAXIMUM_PURPOSE_CODE_BYTES: usize = 96;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivacyPlanningError {
    InvalidArgument {
        field: &'static str,
        safe_message: &'static str,
    },
    SnapshotMismatch {
        safe_message: &'static str,
    },
    UnsupportedCryptoShred {
        owner_module_id: ModuleId,
        resource_id: RecordId,
    },
    ClassificationConflict {
        owner_module_id: ModuleId,
        resource_id: RecordId,
        safe_message: &'static str,
    },
}

impl PrivacyPlanningError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidArgument { .. } => "CUSTOMER_PRIVACY_PLANNING_INVALID_ARGUMENT",
            Self::SnapshotMismatch { .. } => "CUSTOMER_PRIVACY_PLANNING_SNAPSHOT_MISMATCH",
            Self::UnsupportedCryptoShred { .. } => {
                "CUSTOMER_PRIVACY_PLANNING_CRYPTO_SHRED_UNSUPPORTED"
            }
            Self::ClassificationConflict { .. } => {
                "CUSTOMER_PRIVACY_PLANNING_CLASSIFICATION_CONFLICT"
            }
        }
    }

    pub const fn retryable(&self) -> bool {
        false
    }
}

impl fmt::Display for PrivacyPlanningError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument {
                field,
                safe_message,
            } => write!(formatter, "{field}: {safe_message}"),
            Self::SnapshotMismatch { safe_message } => formatter.write_str(safe_message),
            Self::UnsupportedCryptoShred {
                owner_module_id,
                resource_id,
            } => write!(
                formatter,
                "crypto-shred is not supported for {owner_module_id}/{resource_id}"
            ),
            Self::ClassificationConflict {
                owner_module_id,
                resource_id,
                safe_message,
            } => write!(
                formatter,
                "{safe_message}: {owner_module_id}/{resource_id}"
            ),
        }
    }
}

impl Error for PrivacyPlanningError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlannedPrivacyAction {
    Retain,
    RestrictOnly,
    Anonymize,
    Delete,
    CryptoShred,
    NoOpAlreadyCompliant,
}

impl PlannedPrivacyAction {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Retain => "retain",
            Self::RestrictOnly => "restrict_only",
            Self::Anonymize => "anonymize",
            Self::Delete => "delete",
            Self::CryptoShred => "crypto_shred",
            Self::NoOpAlreadyCompliant => "no_op_already_compliant",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyPlanReason {
    AccessDisclosureOnly,
    PortabilityDisclosureOnly,
    RestrictionRequested,
    ErasureDestroyableSubjectData,
    ErasureRetainMinimizedEvidence,
    ErasureImmutableRequiredEvidence,
    ErasureDerivedRebuildableState,
    ErasureCryptoShreddableData,
    OwnerAlreadyCompliant,
}

impl PrivacyPlanReason {
    pub const fn label(self) -> &'static str {
        match self {
            Self::AccessDisclosureOnly => "access_disclosure_only",
            Self::PortabilityDisclosureOnly => "portability_disclosure_only",
            Self::RestrictionRequested => "restriction_requested",
            Self::ErasureDestroyableSubjectData => "erasure_destroyable_subject_data",
            Self::ErasureRetainMinimizedEvidence => "erasure_retain_minimized_evidence",
            Self::ErasureImmutableRequiredEvidence => "erasure_immutable_required_evidence",
            Self::ErasureDerivedRebuildableState => "erasure_derived_rebuildable_state",
            Self::ErasureCryptoShreddableData => "erasure_crypto_shreddable_data",
            Self::OwnerAlreadyCompliant => "owner_already_compliant",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionPlanningPolicy {
    policy_version: SchemaVersion,
    jurisdiction_code: String,
    approval_required: bool,
    crypto_shred_supported: bool,
}

impl ActionPlanningPolicy {
    pub fn new(
        policy_version: SchemaVersion,
        jurisdiction_code: impl Into<String>,
        approval_required: bool,
        crypto_shred_supported: bool,
    ) -> Result<Self, PrivacyPlanningError> {
        let jurisdiction_code = jurisdiction_code.into();
        if !valid_jurisdiction_code(&jurisdiction_code) {
            return Err(PrivacyPlanningError::InvalidArgument {
                field: "jurisdiction_code",
                safe_message: "jurisdiction code is invalid",
            });
        }
        Ok(Self {
            policy_version,
            jurisdiction_code,
            approval_required,
            crypto_shred_supported,
        })
    }

    pub fn policy_version(&self) -> &SchemaVersion {
        &self.policy_version
    }

    pub fn jurisdiction_code(&self) -> &str {
        &self.jurisdiction_code
    }

    pub const fn approval_required(&self) -> bool {
        self.approval_required
    }

    pub const fn crypto_shred_supported(&self) -> bool {
        self.crypto_shred_supported
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionPlanLineage {
    privacy_case_id: RecordId,
    tenant_id: TenantId,
    canonical_party_id: RecordId,
    identity_resolution_generation: u64,
    source_case_version: u64,
    scope_snapshot_id: RecordId,
    scope_snapshot_binding_digest: [u8; 32],
    scope_completeness_digest: [u8; 32],
    registry_digest: [u8; 32],
    purpose_code: String,
    effective_request_at_unix_ms: i64,
    snapshot_captured_at_unix_nanos: i64,
    case_kind: PrivacyCaseKind,
    policy_version: SchemaVersion,
    jurisdiction_code: String,
    approval_required: bool,
    crypto_shred_supported: bool,
    digest: [u8; 32],
}

impl ActionPlanLineage {
    #[allow(clippy::too_many_arguments)]
    fn new(
        privacy_case_id: RecordId,
        tenant_id: TenantId,
        canonical_party_id: RecordId,
        identity_resolution_generation: u64,
        source_case_version: u64,
        scope_snapshot_id: RecordId,
        scope_snapshot_binding_digest: [u8; 32],
        scope_completeness_digest: [u8; 32],
        registry_digest: [u8; 32],
        purpose_code: String,
        effective_request_at_unix_ms: i64,
        snapshot_captured_at_unix_nanos: i64,
        case_kind: PrivacyCaseKind,
        policy_version: SchemaVersion,
        jurisdiction_code: String,
        approval_required: bool,
        crypto_shred_supported: bool,
    ) -> Result<Self, PrivacyPlanningError> {
        if identity_resolution_generation == 0 {
            return Err(PrivacyPlanningError::InvalidArgument {
                field: "identity_resolution_generation",
                safe_message: "identity-resolution generation must be positive",
            });
        }
        if source_case_version == 0 {
            return Err(PrivacyPlanningError::InvalidArgument {
                field: "source_case_version",
                safe_message: "source case version must be positive",
            });
        }
        if scope_snapshot_binding_digest.iter().all(|byte| *byte == 0)
            || scope_completeness_digest.iter().all(|byte| *byte == 0)
            || registry_digest.iter().all(|byte| *byte == 0)
        {
            return Err(PrivacyPlanningError::InvalidArgument {
                field: "digest",
                safe_message: "planning lineage digests must not be all zeroes",
            });
        }
        if !valid_purpose_code(&purpose_code) {
            return Err(PrivacyPlanningError::InvalidArgument {
                field: "purpose_code",
                safe_message: "purpose code is invalid",
            });
        }
        if effective_request_at_unix_ms <= 0 || snapshot_captured_at_unix_nanos <= 0 {
            return Err(PrivacyPlanningError::InvalidArgument {
                field: "timestamp",
                safe_message: "planning lineage timestamps must be positive",
            });
        }
        if !valid_jurisdiction_code(&jurisdiction_code) {
            return Err(PrivacyPlanningError::InvalidArgument {
                field: "jurisdiction_code",
                safe_message: "jurisdiction code is invalid",
            });
        }
        let mut lineage = Self {
            privacy_case_id,
            tenant_id,
            canonical_party_id,
            identity_resolution_generation,
            source_case_version,
            scope_snapshot_id,
            scope_snapshot_binding_digest,
            scope_completeness_digest,
            registry_digest,
            purpose_code,
            effective_request_at_unix_ms,
            snapshot_captured_at_unix_nanos,
            case_kind,
            policy_version,
            jurisdiction_code,
            approval_required,
            crypto_shred_supported,
            digest: [0; 32],
        };
        lineage.digest = action_plan_lineage_digest(&lineage);
        Ok(lineage)
    }

    fn from_snapshot(
        snapshot: &DiscoveryScopeSnapshot,
        source_case_version: u64,
        case_kind: PrivacyCaseKind,
        policy: &ActionPlanningPolicy,
    ) -> Result<Self, PrivacyPlanningError> {
        let discovery = snapshot.lineage();
        Self::new(
            discovery.privacy_case_id().clone(),
            discovery.tenant_id().clone(),
            discovery.canonical_party_id().clone(),
            discovery.identity_resolution_generation(),
            source_case_version,
            snapshot.snapshot_id().clone(),
            *snapshot.binding_digest(),
            *snapshot.aggregation().completeness_digest(),
            *discovery.registry_digest(),
            discovery.purpose_code().to_owned(),
            discovery.effective_request_at_unix_ms(),
            snapshot.captured_at_unix_nanos(),
            case_kind,
            policy.policy_version().clone(),
            policy.jurisdiction_code().to_owned(),
            policy.approval_required(),
            policy.crypto_shred_supported(),
        )
    }

    pub fn privacy_case_id(&self) -> &RecordId {
        &self.privacy_case_id
    }

    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    pub fn canonical_party_id(&self) -> &RecordId {
        &self.canonical_party_id
    }

    pub const fn identity_resolution_generation(&self) -> u64 {
        self.identity_resolution_generation
    }

    pub const fn source_case_version(&self) -> u64 {
        self.source_case_version
    }

    pub fn scope_snapshot_id(&self) -> &RecordId {
        &self.scope_snapshot_id
    }

    pub const fn scope_snapshot_binding_digest(&self) -> &[u8; 32] {
        &self.scope_snapshot_binding_digest
    }

    pub const fn scope_completeness_digest(&self) -> &[u8; 32] {
        &self.scope_completeness_digest
    }

    pub const fn registry_digest(&self) -> &[u8; 32] {
        &self.registry_digest
    }

    pub fn purpose_code(&self) -> &str {
        &self.purpose_code
    }

    pub const fn effective_request_at_unix_ms(&self) -> i64 {
        self.effective_request_at_unix_ms
    }

    pub const fn snapshot_captured_at_unix_nanos(&self) -> i64 {
        self.snapshot_captured_at_unix_nanos
    }

    pub const fn case_kind(&self) -> PrivacyCaseKind {
        self.case_kind
    }

    pub fn policy_version(&self) -> &SchemaVersion {
        &self.policy_version
    }

    pub fn jurisdiction_code(&self) -> &str {
        &self.jurisdiction_code
    }

    pub const fn approval_required(&self) -> bool {
        self.approval_required
    }

    pub const fn crypto_shred_supported(&self) -> bool {
        self.crypto_shred_supported
    }

    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyActionPlanItem {
    sequence: u32,
    owner_module_id: ModuleId,
    resource_type: String,
    resource_id: RecordId,
    resource_version: u64,
    data_class: DataClass,
    evidence_class: EvidenceClass,
    retention_policy_id: RetentionPolicyId,
    action: PlannedPrivacyAction,
    reason: PrivacyPlanReason,
    digest: [u8; 32],
}

impl PrivacyActionPlanItem {
    #[allow(clippy::too_many_arguments)]
    fn new(
        lineage_digest: &[u8; 32],
        sequence: u32,
        owner_module_id: ModuleId,
        resource_type: String,
        resource_id: RecordId,
        resource_version: u64,
        data_class: DataClass,
        evidence_class: EvidenceClass,
        retention_policy_id: RetentionPolicyId,
        action: PlannedPrivacyAction,
        reason: PrivacyPlanReason,
    ) -> Result<Self, PrivacyPlanningError> {
        if sequence == 0 {
            return Err(PrivacyPlanningError::InvalidArgument {
                field: "sequence",
                safe_message: "plan item sequence must be positive",
            });
        }
        if resource_type.is_empty() || resource_version == 0 {
            return Err(PrivacyPlanningError::InvalidArgument {
                field: "resource",
                safe_message: "plan item resource identity is invalid",
            });
        }
        let mut item = Self {
            sequence,
            owner_module_id,
            resource_type,
            resource_id,
            resource_version,
            data_class,
            evidence_class,
            retention_policy_id,
            action,
            reason,
            digest: [0; 32],
        };
        item.digest = action_plan_item_digest(lineage_digest, &item);
        Ok(item)
    }

    pub const fn sequence(&self) -> u32 {
        self.sequence
    }

    pub fn owner_module_id(&self) -> &ModuleId {
        &self.owner_module_id
    }

    pub fn resource_type(&self) -> &str {
        &self.resource_type
    }

    pub fn resource_id(&self) -> &RecordId {
        &self.resource_id
    }

    pub const fn resource_version(&self) -> u64 {
        self.resource_version
    }

    pub const fn data_class(&self) -> DataClass {
        self.data_class
    }

    pub const fn evidence_class(&self) -> EvidenceClass {
        self.evidence_class
    }

    pub fn retention_policy_id(&self) -> &RetentionPolicyId {
        &self.retention_policy_id
    }

    pub const fn action(&self) -> PlannedPrivacyAction {
        self.action
    }

    pub const fn reason(&self) -> PrivacyPlanReason {
        self.reason
    }

    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    fn identity_key(&self) -> (&str, &str, &str) {
        (
            self.owner_module_id.as_str(),
            self.resource_type.as_str(),
            self.resource_id.as_str(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyActionPlan {
    plan_id: RecordId,
    lineage: ActionPlanLineage,
    planned_at_unix_nanos: i64,
    items: Vec<PrivacyActionPlanItem>,
    digest: [u8; 32],
}

impl PrivacyActionPlan {
    pub fn build(
        snapshot: &DiscoveryScopeSnapshot,
        source_case_version: u64,
        case_kind: PrivacyCaseKind,
        policy: ActionPlanningPolicy,
        planned_at_unix_nanos: i64,
    ) -> Result<Self, PrivacyPlanningError> {
        if planned_at_unix_nanos < snapshot.captured_at_unix_nanos() {
            return Err(PrivacyPlanningError::SnapshotMismatch {
                safe_message: "plan time precedes the immutable scope snapshot",
            });
        }
        let resources = snapshot.aggregation().resources();
        if resources.len() > ACTION_PLAN_MAXIMUM_ITEMS {
            return Err(PrivacyPlanningError::InvalidArgument {
                field: "items",
                safe_message: "scope snapshot exceeds the maximum planning item count",
            });
        }
        let lineage = ActionPlanLineage::from_snapshot(
            snapshot,
            source_case_version,
            case_kind,
            &policy,
        )?;
        let mut drafts = resources.iter().collect::<Vec<_>>();
        drafts.sort_by(|left, right| scoped_resource_cmp(left, right));
        let mut items = Vec::with_capacity(drafts.len());
        for (index, scoped) in drafts.into_iter().enumerate() {
            let resource = scoped.resource();
            let (action, reason) = classify_action(
                case_kind,
                resource.evidence_class(),
                policy.crypto_shred_supported(),
                scoped.owner_module_id(),
                resource.resource_id(),
            )?;
            items.push(PrivacyActionPlanItem::new(
                lineage.digest(),
                u32::try_from(index + 1).map_err(|_| PrivacyPlanningError::InvalidArgument {
                    field: "sequence",
                    safe_message: "plan item sequence exceeds the supported range",
                })?,
                scoped.owner_module_id().clone(),
                resource.resource_type().to_owned(),
                resource.resource_id().clone(),
                resource.resource_version(),
                resource.data_class(),
                resource.evidence_class(),
                resource.retention_policy_id().clone(),
                action,
                reason,
            )?);
        }
        Self::rehydrate(lineage, planned_at_unix_nanos, items)
    }

    fn rehydrate(
        lineage: ActionPlanLineage,
        planned_at_unix_nanos: i64,
        items: Vec<PrivacyActionPlanItem>,
    ) -> Result<Self, PrivacyPlanningError> {
        if planned_at_unix_nanos < lineage.snapshot_captured_at_unix_nanos {
            return Err(PrivacyPlanningError::SnapshotMismatch {
                safe_message: "plan time precedes the immutable scope snapshot",
            });
        }
        if items.len() > ACTION_PLAN_MAXIMUM_ITEMS {
            return Err(PrivacyPlanningError::InvalidArgument {
                field: "items",
                safe_message: "action plan exceeds the maximum item count",
            });
        }
        for (index, item) in items.iter().enumerate() {
            let expected_sequence = u32::try_from(index + 1).map_err(|_| {
                PrivacyPlanningError::InvalidArgument {
                    field: "sequence",
                    safe_message: "plan item sequence exceeds the supported range",
                }
            })?;
            if item.sequence != expected_sequence {
                return Err(PrivacyPlanningError::ClassificationConflict {
                    owner_module_id: item.owner_module_id.clone(),
                    resource_id: item.resource_id.clone(),
                    safe_message: "plan item sequence is not contiguous",
                });
            }
            let (expected_action, expected_reason) = classify_action(
                lineage.case_kind,
                item.evidence_class,
                lineage.crypto_shred_supported,
                &item.owner_module_id,
                &item.resource_id,
            )?;
            if item.action != expected_action || item.reason != expected_reason {
                return Err(PrivacyPlanningError::ClassificationConflict {
                    owner_module_id: item.owner_module_id.clone(),
                    resource_id: item.resource_id.clone(),
                    safe_message: "plan item action does not match the frozen classification",
                });
            }
            if item.action == PlannedPrivacyAction::NoOpAlreadyCompliant {
                return Err(PrivacyPlanningError::ClassificationConflict {
                    owner_module_id: item.owner_module_id.clone(),
                    resource_id: item.resource_id.clone(),
                    safe_message: "initial planning cannot infer owner compliance",
                });
            }
            let expected_digest = action_plan_item_digest(lineage.digest(), item);
            if item.digest != expected_digest {
                return Err(PrivacyPlanningError::ClassificationConflict {
                    owner_module_id: item.owner_module_id.clone(),
                    resource_id: item.resource_id.clone(),
                    safe_message: "plan item digest does not match deterministic content",
                });
            }
            if let Some(previous) = index.checked_sub(1).map(|value| &items[value]) {
                if plan_item_cmp(previous, item) == Ordering::Greater {
                    return Err(PrivacyPlanningError::ClassificationConflict {
                        owner_module_id: item.owner_module_id.clone(),
                        resource_id: item.resource_id.clone(),
                        safe_message: "plan items are not in canonical order",
                    });
                }
                if previous.identity_key() == item.identity_key() {
                    return Err(PrivacyPlanningError::ClassificationConflict {
                        owner_module_id: item.owner_module_id.clone(),
                        resource_id: item.resource_id.clone(),
                        safe_message: "one resource appears more than once in the action plan",
                    });
                }
            }
        }
        let digest = action_plan_digest(&lineage, planned_at_unix_nanos, &items);
        let plan_id = RecordId::try_new(format!(
            "{ACTION_PLAN_ID_PREFIX}{}",
            hex_encode(&digest)
        ))
        .map_err(|_| PrivacyPlanningError::InvalidArgument {
            field: "plan_id",
            safe_message: "derived action plan id is invalid",
        })?;
        Ok(Self {
            plan_id,
            lineage,
            planned_at_unix_nanos,
            items,
            digest,
        })
    }

    pub fn plan_id(&self) -> &RecordId {
        &self.plan_id
    }

    pub fn lineage(&self) -> &ActionPlanLineage {
        &self.lineage
    }

    pub const fn planned_at_unix_nanos(&self) -> i64 {
        self.planned_at_unix_nanos
    }

    pub fn items(&self) -> &[PrivacyActionPlanItem] {
        &self.items
    }

    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

pub fn action_plan_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(ACTION_PLAN_STATE_DESCRIPTOR).into()
}

fn classify_action(
    case_kind: PrivacyCaseKind,
    evidence_class: EvidenceClass,
    crypto_shred_supported: bool,
    owner_module_id: &ModuleId,
    resource_id: &RecordId,
) -> Result<(PlannedPrivacyAction, PrivacyPlanReason), PrivacyPlanningError> {
    let result = match case_kind {
        PrivacyCaseKind::Access => (
            PlannedPrivacyAction::Retain,
            PrivacyPlanReason::AccessDisclosureOnly,
        ),
        PrivacyCaseKind::PortabilityExport => (
            PlannedPrivacyAction::Retain,
            PrivacyPlanReason::PortabilityDisclosureOnly,
        ),
        PrivacyCaseKind::RestrictProcessing => (
            PlannedPrivacyAction::RestrictOnly,
            PrivacyPlanReason::RestrictionRequested,
        ),
        PrivacyCaseKind::Erasure => match evidence_class {
            EvidenceClass::DestroyableSubjectData => (
                PlannedPrivacyAction::Delete,
                PrivacyPlanReason::ErasureDestroyableSubjectData,
            ),
            EvidenceClass::RetainMinimizedEvidence => (
                PlannedPrivacyAction::Anonymize,
                PrivacyPlanReason::ErasureRetainMinimizedEvidence,
            ),
            EvidenceClass::ImmutableRequiredEvidence => (
                PlannedPrivacyAction::Retain,
                PrivacyPlanReason::ErasureImmutableRequiredEvidence,
            ),
            EvidenceClass::DerivedRebuildableState => (
                PlannedPrivacyAction::Delete,
                PrivacyPlanReason::ErasureDerivedRebuildableState,
            ),
            EvidenceClass::CryptoShreddableData if crypto_shred_supported => (
                PlannedPrivacyAction::CryptoShred,
                PrivacyPlanReason::ErasureCryptoShreddableData,
            ),
            EvidenceClass::CryptoShreddableData => {
                return Err(PrivacyPlanningError::UnsupportedCryptoShred {
                    owner_module_id: owner_module_id.clone(),
                    resource_id: resource_id.clone(),
                });
            }
        },
    };
    Ok(result)
}

fn plan_item_cmp(left: &PrivacyActionPlanItem, right: &PrivacyActionPlanItem) -> Ordering {
    left.owner_module_id
        .cmp(&right.owner_module_id)
        .then_with(|| left.resource_type.cmp(&right.resource_type))
        .then_with(|| left.resource_id.cmp(&right.resource_id))
        .then_with(|| left.resource_version.cmp(&right.resource_version))
        .then_with(|| data_class_label(left.data_class).cmp(data_class_label(right.data_class)))
        .then_with(|| left.evidence_class.cmp(&right.evidence_class))
        .then_with(|| left.retention_policy_id.cmp(&right.retention_policy_id))
        .then_with(|| left.action.cmp(&right.action))
        .then_with(|| left.reason.cmp(&right.reason))
}

fn action_plan_lineage_digest(lineage: &ActionPlanLineage) -> [u8; 32] {
    let mut hasher = framed_hasher(b"crm.customer-privacy.action-plan-lineage/v1");
    hash_field(&mut hasher, lineage.privacy_case_id.as_str().as_bytes());
    hash_field(&mut hasher, lineage.tenant_id.as_str().as_bytes());
    hash_field(&mut hasher, lineage.canonical_party_id.as_str().as_bytes());
    hash_field(
        &mut hasher,
        &lineage.identity_resolution_generation.to_be_bytes(),
    );
    hash_field(&mut hasher, &lineage.source_case_version.to_be_bytes());
    hash_field(&mut hasher, lineage.scope_snapshot_id.as_str().as_bytes());
    hash_field(&mut hasher, &lineage.scope_snapshot_binding_digest);
    hash_field(&mut hasher, &lineage.scope_completeness_digest);
    hash_field(&mut hasher, &lineage.registry_digest);
    hash_field(&mut hasher, lineage.purpose_code.as_bytes());
    hash_field(
        &mut hasher,
        &lineage.effective_request_at_unix_ms.to_be_bytes(),
    );
    hash_field(
        &mut hasher,
        &lineage.snapshot_captured_at_unix_nanos.to_be_bytes(),
    );
    hash_field(&mut hasher, case_kind_label(lineage.case_kind).as_bytes());
    hash_field(&mut hasher, lineage.policy_version.as_str().as_bytes());
    hash_field(&mut hasher, lineage.jurisdiction_code.as_bytes());
    hash_field(&mut hasher, &[u8::from(lineage.approval_required)]);
    hash_field(&mut hasher, &[u8::from(lineage.crypto_shred_supported)]);
    hasher.finalize().into()
}

fn action_plan_item_digest(
    lineage_digest: &[u8; 32],
    item: &PrivacyActionPlanItem,
) -> [u8; 32] {
    let mut hasher = framed_hasher(b"crm.customer-privacy.action-plan-item/v1");
    hash_field(&mut hasher, lineage_digest);
    hash_field(&mut hasher, &item.sequence.to_be_bytes());
    hash_field(&mut hasher, item.owner_module_id.as_str().as_bytes());
    hash_field(&mut hasher, item.resource_type.as_bytes());
    hash_field(&mut hasher, item.resource_id.as_str().as_bytes());
    hash_field(&mut hasher, &item.resource_version.to_be_bytes());
    hash_field(&mut hasher, data_class_label(item.data_class).as_bytes());
    hash_field(&mut hasher, item.evidence_class.label().as_bytes());
    hash_field(&mut hasher, item.retention_policy_id.as_str().as_bytes());
    hash_field(&mut hasher, item.action.label().as_bytes());
    hash_field(&mut hasher, item.reason.label().as_bytes());
    hasher.finalize().into()
}

fn action_plan_digest(
    lineage: &ActionPlanLineage,
    planned_at_unix_nanos: i64,
    items: &[PrivacyActionPlanItem],
) -> [u8; 32] {
    let mut hasher = framed_hasher(b"crm.customer-privacy.action-plan/v1");
    hash_field(&mut hasher, lineage.digest());
    hash_field(&mut hasher, &planned_at_unix_nanos.to_be_bytes());
    hash_field(&mut hasher, &(items.len() as u64).to_be_bytes());
    for item in items {
        hash_field(&mut hasher, item.digest());
    }
    hasher.finalize().into()
}

fn case_kind_label(value: PrivacyCaseKind) -> &'static str {
    match value {
        PrivacyCaseKind::Access => "access",
        PrivacyCaseKind::PortabilityExport => "portability_export",
        PrivacyCaseKind::RestrictProcessing => "restrict_processing",
        PrivacyCaseKind::Erasure => "erasure",
    }
}

fn parse_case_kind(value: &str) -> Result<PrivacyCaseKind, SdkError> {
    match value {
        "access" => Ok(PrivacyCaseKind::Access),
        "portability_export" => Ok(PrivacyCaseKind::PortabilityExport),
        "restrict_processing" => Ok(PrivacyCaseKind::RestrictProcessing),
        "erasure" => Ok(PrivacyCaseKind::Erasure),
        _ => Err(planning_persisted_error(
            "persisted action-plan case kind is unknown",
        )),
    }
}

fn valid_jurisdiction_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_JURISDICTION_CODE_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_uppercase()
                || byte.is_ascii_digit()
                || byte == b'_'
                || byte == b'-'
        })
}

fn valid_purpose_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_PURPOSE_CODE_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_uppercase()
                || byte.is_ascii_digit()
                || byte == b'_'
                || byte == b'-'
        })
}

fn planning_error(error: PrivacyPlanningError) -> SdkError {
    planning_persisted_error(format!("{}: {error}", error.code()))
}

fn planning_persisted_error(internal_reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_ACTION_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "Persisted customer privacy action-plan evidence is invalid.",
    )
    .with_internal_reference(internal_reference)
}

include!("scope_planning_state.rs");
include!("scope_planning_tests.rs");
