use crate::canonicalization::persisted_state_json;
use crm_module_sdk::{
    ErrorCategory as RetentionErrorCategory, RetentionPolicyId,
    SdkError as RetentionSdkError,
};
use sha2::{Digest as RetentionDigest, Sha256 as RetentionSha256};

pub const RETENTION_EVALUATE_COORDINATE: &str = "customer_privacy.retention.evaluate@1.0.0";
pub const RETENTION_DECISION_STATE_SCHEMA_ID: &str =
    "crm.customer-privacy.retention_decision.state";
pub const RETENTION_DECISION_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const RETENTION_DECISION_STATE_MAXIMUM_BYTES: u64 = 2 * 1024 * 1024;
pub const RETENTION_DECISION_STATE_RETENTION_POLICY_ID: &str =
    "crm.customer_privacy.retention_decision";
pub const RETENTION_DECISION_MAXIMUM_HOLDS: usize = 1_000;

const RETENTION_DECISION_STATE_DESCRIPTOR: &[u8] = b"crm.customer-privacy.retention_decision.state/v1:decision_id,tenant_id,canonical_party_id,privacy_case_id,action_plan_id,action_plan_digest,evaluated_at_unix_nanos,items,decision_digest";
const RETENTION_DECISION_ID_PREFIX: &str = "privacy-retention-decision-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivacyRetentionError {
    InvalidArgument {
        field: &'static str,
        safe_message: &'static str,
    },
    LineageMismatch {
        safe_message: &'static str,
    },
    OverBound {
        safe_message: &'static str,
    },
    DecisionConflict {
        safe_message: &'static str,
    },
}

impl PrivacyRetentionError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidArgument { .. } => "CUSTOMER_PRIVACY_RETENTION_INVALID_ARGUMENT",
            Self::LineageMismatch { .. } => "CUSTOMER_PRIVACY_RETENTION_LINEAGE_MISMATCH",
            Self::OverBound { .. } => "CUSTOMER_PRIVACY_RETENTION_OVER_BOUND",
            Self::DecisionConflict { .. } => "CUSTOMER_PRIVACY_RETENTION_DECISION_CONFLICT",
        }
    }

    pub const fn retryable(&self) -> bool {
        false
    }
}

impl std::fmt::Display for PrivacyRetentionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidArgument {
                field,
                safe_message,
            } => write!(formatter, "{field}: {safe_message}"),
            Self::LineageMismatch { safe_message }
            | Self::OverBound { safe_message }
            | Self::DecisionConflict { safe_message } => formatter.write_str(safe_message),
        }
    }
}

impl std::error::Error for PrivacyRetentionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetentionDecisionReason {
    ActiveLegalHold,
    MandatoryRetention,
    ApprovedPrivacyAction,
}

impl RetentionDecisionReason {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ActiveLegalHold => "active_legal_hold",
            Self::MandatoryRetention => "mandatory_retention",
            Self::ApprovedPrivacyAction => "approved_privacy_action",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecidingLegalHoldEvidence {
    hold_id: RecordId,
    authority_reference: RecordId,
    reason_code: String,
    policy_version: SchemaVersion,
    scope_label: String,
    review_at_unix_nanos: Option<i64>,
    matching_hold_count: u32,
    matching_holds_digest: [u8; 32],
}

impl DecidingLegalHoldEvidence {
    pub fn hold_id(&self) -> &RecordId {
        &self.hold_id
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

    pub fn scope_label(&self) -> &str {
        &self.scope_label
    }

    pub const fn review_at_unix_nanos(&self) -> Option<i64> {
        self.review_at_unix_nanos
    }

    pub const fn matching_hold_count(&self) -> u32 {
        self.matching_hold_count
    }

    pub const fn matching_holds_digest(&self) -> &[u8; 32] {
        &self.matching_holds_digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyRetentionDecisionItem {
    sequence: u32,
    owner_module_id: ModuleId,
    resource_type: String,
    resource_id: RecordId,
    resource_version: u64,
    data_class: DataClass,
    evidence_class: EvidenceClass,
    retention_policy_id: RetentionPolicyId,
    approved_action: PlannedPrivacyAction,
    final_action: PlannedPrivacyAction,
    reason: RetentionDecisionReason,
    legal_hold: Option<DecidingLegalHoldEvidence>,
    digest: [u8; 32],
}

impl PrivacyRetentionDecisionItem {
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

    pub const fn approved_action(&self) -> PlannedPrivacyAction {
        self.approved_action
    }

    pub const fn final_action(&self) -> PlannedPrivacyAction {
        self.final_action
    }

    pub const fn reason(&self) -> RetentionDecisionReason {
        self.reason
    }

    pub fn legal_hold(&self) -> Option<&DecidingLegalHoldEvidence> {
        self.legal_hold.as_ref()
    }

    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyRetentionDecisionSet {
    decision_id: RecordId,
    tenant_id: TenantId,
    canonical_party_id: RecordId,
    privacy_case_id: RecordId,
    action_plan_id: RecordId,
    action_plan_digest: [u8; 32],
    evaluated_at_unix_nanos: i64,
    items: Vec<PrivacyRetentionDecisionItem>,
    digest: [u8; 32],
}

impl PrivacyRetentionDecisionSet {
    pub fn build(
        plan: &PrivacyActionPlan,
        holds: &[CustomerDataLegalHold],
        evaluated_at_unix_nanos: i64,
    ) -> Result<Self, PrivacyRetentionError> {
        if evaluated_at_unix_nanos <= 0
            || evaluated_at_unix_nanos < plan.planned_at_unix_nanos()
        {
            return Err(PrivacyRetentionError::InvalidArgument {
                field: "evaluated_at_unix_nanos",
                safe_message: "retention evaluation must not precede the immutable action plan",
            });
        }
        if holds.len() > RETENTION_DECISION_MAXIMUM_HOLDS {
            return Err(PrivacyRetentionError::OverBound {
                safe_message: "legal-hold evidence exceeds the governed bound",
            });
        }

        let lineage = plan.lineage();
        let mut ordered_holds = holds.iter().collect::<Vec<_>>();
        ordered_holds.sort_by(|left, right| left.hold_id.cmp(&right.hold_id));
        for (index, hold) in ordered_holds.iter().enumerate() {
            if hold.tenant_id != *lineage.tenant_id()
                || hold.canonical_party_id != *lineage.canonical_party_id()
            {
                return Err(PrivacyRetentionError::LineageMismatch {
                    safe_message: "legal hold differs from the action-plan tenant or subject",
                });
            }
            if index > 0 && ordered_holds[index - 1].hold_id == hold.hold_id {
                return Err(PrivacyRetentionError::DecisionConflict {
                    safe_message: "one legal hold appears more than once in the evidence set",
                });
            }
        }

        let mut items = Vec::with_capacity(plan.items().len());
        for plan_item in plan.items() {
            let matching = ordered_holds
                .iter()
                .copied()
                .filter(|hold| {
                    hold.is_active_at(evaluated_at_unix_nanos)
                        && legal_hold_matches_item(hold, plan_item)
                })
                .collect::<Vec<_>>();
            items.push(decide_item(plan_item, &matching)?);
        }

        Self::rehydrate(
            lineage.tenant_id().clone(),
            lineage.canonical_party_id().clone(),
            lineage.privacy_case_id().clone(),
            plan.plan_id().clone(),
            *plan.digest(),
            evaluated_at_unix_nanos,
            items,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn rehydrate(
        tenant_id: TenantId,
        canonical_party_id: RecordId,
        privacy_case_id: RecordId,
        action_plan_id: RecordId,
        action_plan_digest: [u8; 32],
        evaluated_at_unix_nanos: i64,
        items: Vec<PrivacyRetentionDecisionItem>,
    ) -> Result<Self, PrivacyRetentionError> {
        if evaluated_at_unix_nanos <= 0 || action_plan_digest.iter().all(|byte| *byte == 0) {
            return Err(PrivacyRetentionError::InvalidArgument {
                field: "lineage",
                safe_message: "retention-decision lineage is invalid",
            });
        }
        for (index, item) in items.iter().enumerate() {
            let expected_sequence = u32::try_from(index + 1).map_err(|_| {
                PrivacyRetentionError::OverBound {
                    safe_message: "retention-decision sequence exceeds the supported range",
                }
            })?;
            if item.sequence != expected_sequence {
                return Err(PrivacyRetentionError::DecisionConflict {
                    safe_message: "retention-decision items are not contiguous",
                });
            }
            validate_decision_semantics(item)?;
            if item.digest != retention_item_digest(item) {
                return Err(PrivacyRetentionError::DecisionConflict {
                    safe_message: "retention-decision item digest is invalid",
                });
            }
            if let Some(previous) = index.checked_sub(1).map(|value| &items[value])
                && previous.sequence >= item.sequence {
                    return Err(PrivacyRetentionError::DecisionConflict {
                        safe_message: "retention-decision items are not in canonical order",
                    });
                }
        }
        let digest = retention_decision_digest(
            &tenant_id,
            &canonical_party_id,
            &privacy_case_id,
            &action_plan_id,
            &action_plan_digest,
            evaluated_at_unix_nanos,
            &items,
        );
        let decision_id = RecordId::try_new(format!(
            "{RETENTION_DECISION_ID_PREFIX}{}",
            retention_hex(&digest)
        ))
        .map_err(|_| PrivacyRetentionError::InvalidArgument {
            field: "decision_id",
            safe_message: "derived retention-decision id is invalid",
        })?;
        Ok(Self {
            decision_id,
            tenant_id,
            canonical_party_id,
            privacy_case_id,
            action_plan_id,
            action_plan_digest,
            evaluated_at_unix_nanos,
            items,
            digest,
        })
    }

    pub fn decision_id(&self) -> &RecordId {
        &self.decision_id
    }

    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    pub fn canonical_party_id(&self) -> &RecordId {
        &self.canonical_party_id
    }

    pub fn privacy_case_id(&self) -> &RecordId {
        &self.privacy_case_id
    }

    pub fn action_plan_id(&self) -> &RecordId {
        &self.action_plan_id
    }

    pub const fn action_plan_digest(&self) -> &[u8; 32] {
        &self.action_plan_digest
    }

    pub const fn evaluated_at_unix_nanos(&self) -> i64 {
        self.evaluated_at_unix_nanos
    }

    pub fn items(&self) -> &[PrivacyRetentionDecisionItem] {
        &self.items
    }

    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

pub fn retention_decision_state_descriptor_hash() -> [u8; 32] {
    RetentionSha256::digest(RETENTION_DECISION_STATE_DESCRIPTOR).into()
}

pub fn encode_retention_decision_state(
    decision: &PrivacyRetentionDecisionSet,
) -> Result<Vec<u8>, RetentionSdkError> {
    let bytes = persisted_state_json::to_vec(&RetentionDecisionStateV1::from(decision)).map_err(
        |error| retention_state_error(format!("retention decision serialization failed: {error}")),
    )?;
    validate_state_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_retention_decision_state(
    bytes: &[u8],
) -> Result<PrivacyRetentionDecisionSet, RetentionSdkError> {
    validate_state_size(bytes)?;
    let state: RetentionDecisionStateV1 = persisted_state_json::from_slice(bytes).map_err(|error| {
        retention_state_error(format!("retention decision JSON is invalid: {error}"))
    })?;
    let decision = state.into_domain()?;
    if encode_retention_decision_state(&decision)? != bytes {
        return Err(retention_state_error(
            "persisted retention decision is not the strict canonical v1 encoding",
        ));
    }
    Ok(decision)
}

fn decide_item(
    item: &PrivacyActionPlanItem,
    matching_holds: &[&CustomerDataLegalHold],
) -> Result<PrivacyRetentionDecisionItem, PrivacyRetentionError> {
    let approved_action = item.action();
    let (final_action, reason, legal_hold) = if is_destructive(approved_action)
        && !matching_holds.is_empty()
    {
        (
            PlannedPrivacyAction::Retain,
            RetentionDecisionReason::ActiveLegalHold,
            Some(legal_hold_evidence(matching_holds)?),
        )
    } else if let Some(final_action) =
        mandatory_retention_action(approved_action, item.evidence_class())
    {
        (
            final_action,
            RetentionDecisionReason::MandatoryRetention,
            None,
        )
    } else {
        (
            approved_action,
            RetentionDecisionReason::ApprovedPrivacyAction,
            None,
        )
    };

    let mut decision = PrivacyRetentionDecisionItem {
        sequence: item.sequence(),
        owner_module_id: item.owner_module_id().clone(),
        resource_type: item.resource_type().to_owned(),
        resource_id: item.resource_id().clone(),
        resource_version: item.resource_version(),
        data_class: item.data_class(),
        evidence_class: item.evidence_class(),
        retention_policy_id: item.retention_policy_id().clone(),
        approved_action,
        final_action,
        reason,
        legal_hold,
        digest: [0; 32],
    };
    decision.digest = retention_item_digest(&decision);
    validate_decision_semantics(&decision)?;
    Ok(decision)
}

fn legal_hold_evidence(
    matching_holds: &[&CustomerDataLegalHold],
) -> Result<DecidingLegalHoldEvidence, PrivacyRetentionError> {
    let deciding = matching_holds
        .first()
        .copied()
        .ok_or(PrivacyRetentionError::DecisionConflict {
            safe_message: "legal-hold decision has no matching hold evidence",
        })?;
    let count = u32::try_from(matching_holds.len()).map_err(|_| PrivacyRetentionError::OverBound {
        safe_message: "matching legal-hold count exceeds the supported range",
    })?;
    let mut hasher = framed_retention_hasher(b"crm.customer-privacy.matching-legal-holds/v1");
    for hold in matching_holds {
        hash_retention_field(&mut hasher, hold.hold_id.as_str().as_bytes());
        hash_retention_field(&mut hasher, &hold.version.to_be_bytes());
        hash_retention_field(&mut hasher, hold.authority_reference.as_str().as_bytes());
        hash_retention_field(&mut hasher, hold.reason_code.as_bytes());
        hash_retention_field(&mut hasher, hold.policy_version.as_str().as_bytes());
    }
    Ok(DecidingLegalHoldEvidence {
        hold_id: deciding.hold_id.clone(),
        authority_reference: deciding.authority_reference.clone(),
        reason_code: deciding.reason_code.clone(),
        policy_version: deciding.policy_version.clone(),
        scope_label: legal_hold_scope_label(&deciding.scope),
        review_at_unix_nanos: deciding.effective_until_unix_nanos,
        matching_hold_count: count,
        matching_holds_digest: hasher.finalize().into(),
    })
}

fn legal_hold_matches_item(
    hold: &CustomerDataLegalHold,
    item: &PrivacyActionPlanItem,
) -> bool {
    legal_hold_scope_matches(&hold.scope, item.owner_module_id(), item.data_class())
}

fn legal_hold_scope_matches(
    scope: &LegalHoldScope,
    owner_module_id: &ModuleId,
    data_class: DataClass,
) -> bool {
    match scope {
        LegalHoldScope::AllCustomerData => true,
        LegalHoldScope::DataClass(expected) => *expected == data_class,
        LegalHoldScope::Owner(expected) => expected == owner_module_id,
    }
}

fn legal_hold_scope_label(scope: &LegalHoldScope) -> String {
    match scope {
        LegalHoldScope::AllCustomerData => "all_customer_data".to_owned(),
        LegalHoldScope::DataClass(value) => {
            format!("data_class:{}", retention_data_class_label(*value))
        }
        LegalHoldScope::Owner(value) => format!("owner:{}", value.as_str()),
    }
}

fn mandatory_retention_action(
    approved_action: PlannedPrivacyAction,
    evidence_class: EvidenceClass,
) -> Option<PlannedPrivacyAction> {
    match evidence_class {
        EvidenceClass::ImmutableRequiredEvidence
            if approved_action == PlannedPrivacyAction::Retain
                || is_destructive(approved_action) =>
        {
            Some(PlannedPrivacyAction::Retain)
        }
        EvidenceClass::RetainMinimizedEvidence if is_destructive(approved_action) => {
            Some(PlannedPrivacyAction::Anonymize)
        }
        _ => None,
    }
}

fn is_destructive(action: PlannedPrivacyAction) -> bool {
    matches!(
        action,
        PlannedPrivacyAction::Anonymize
            | PlannedPrivacyAction::Delete
            | PlannedPrivacyAction::CryptoShred
    )
}

fn validate_decision_semantics(
    item: &PrivacyRetentionDecisionItem,
) -> Result<(), PrivacyRetentionError> {
    match item.reason {
        RetentionDecisionReason::ActiveLegalHold => {
            if !is_destructive(item.approved_action)
                || item.final_action != PlannedPrivacyAction::Retain
                || item.legal_hold.is_none()
            {
                return Err(PrivacyRetentionError::DecisionConflict {
                    safe_message: "active legal-hold decision is inconsistent",
                });
            }
        }
        RetentionDecisionReason::MandatoryRetention => {
            let expected = mandatory_retention_action(item.approved_action, item.evidence_class);
            if expected != Some(item.final_action) || item.legal_hold.is_some() {
                return Err(PrivacyRetentionError::DecisionConflict {
                    safe_message: "mandatory-retention decision is inconsistent",
                });
            }
        }
        RetentionDecisionReason::ApprovedPrivacyAction => {
            if item.final_action != item.approved_action || item.legal_hold.is_some() {
                return Err(PrivacyRetentionError::DecisionConflict {
                    safe_message: "approved-action decision is inconsistent",
                });
            }
        }
    }
    Ok(())
}

fn retention_item_digest(item: &PrivacyRetentionDecisionItem) -> [u8; 32] {
    let mut hasher = framed_retention_hasher(b"crm.customer-privacy.retention-decision-item/v1");
    hash_retention_field(&mut hasher, &item.sequence.to_be_bytes());
    hash_retention_field(&mut hasher, item.owner_module_id.as_str().as_bytes());
    hash_retention_field(&mut hasher, item.resource_type.as_bytes());
    hash_retention_field(&mut hasher, item.resource_id.as_str().as_bytes());
    hash_retention_field(&mut hasher, &item.resource_version.to_be_bytes());
    hash_retention_field(
        &mut hasher,
        retention_data_class_label(item.data_class).as_bytes(),
    );
    hash_retention_field(&mut hasher, item.evidence_class.label().as_bytes());
    hash_retention_field(&mut hasher, item.retention_policy_id.as_str().as_bytes());
    hash_retention_field(&mut hasher, item.approved_action.label().as_bytes());
    hash_retention_field(&mut hasher, item.final_action.label().as_bytes());
    hash_retention_field(&mut hasher, item.reason.label().as_bytes());
    if let Some(hold) = &item.legal_hold {
        hash_retention_field(&mut hasher, hold.hold_id.as_str().as_bytes());
        hash_retention_field(&mut hasher, hold.authority_reference.as_str().as_bytes());
        hash_retention_field(&mut hasher, hold.reason_code.as_bytes());
        hash_retention_field(&mut hasher, hold.policy_version.as_str().as_bytes());
        hash_retention_field(&mut hasher, hold.scope_label.as_bytes());
        hash_retention_field(
            &mut hasher,
            &hold.review_at_unix_nanos.unwrap_or_default().to_be_bytes(),
        );
        hash_retention_field(&mut hasher, &hold.matching_hold_count.to_be_bytes());
        hash_retention_field(&mut hasher, &hold.matching_holds_digest);
    }
    hasher.finalize().into()
}

#[allow(clippy::too_many_arguments)]
fn retention_decision_digest(
    tenant_id: &TenantId,
    canonical_party_id: &RecordId,
    privacy_case_id: &RecordId,
    action_plan_id: &RecordId,
    action_plan_digest: &[u8; 32],
    evaluated_at_unix_nanos: i64,
    items: &[PrivacyRetentionDecisionItem],
) -> [u8; 32] {
    let mut hasher = framed_retention_hasher(b"crm.customer-privacy.retention-decision/v1");
    hash_retention_field(&mut hasher, tenant_id.as_str().as_bytes());
    hash_retention_field(&mut hasher, canonical_party_id.as_str().as_bytes());
    hash_retention_field(&mut hasher, privacy_case_id.as_str().as_bytes());
    hash_retention_field(&mut hasher, action_plan_id.as_str().as_bytes());
    hash_retention_field(&mut hasher, action_plan_digest);
    hash_retention_field(&mut hasher, &evaluated_at_unix_nanos.to_be_bytes());
    hash_retention_field(&mut hasher, &(items.len() as u64).to_be_bytes());
    for item in items {
        hash_retention_field(&mut hasher, item.digest());
    }
    hasher.finalize().into()
}

fn retention_data_class_label(value: DataClass) -> &'static str {
    match value {
        DataClass::Public => "public",
        DataClass::Internal => "internal",
        DataClass::Confidential => "confidential",
        DataClass::Restricted => "restricted",
        DataClass::Personal => "personal",
        DataClass::SensitivePersonal => "sensitive_personal",
        DataClass::Biometric => "biometric",
        DataClass::Financial => "financial",
        DataClass::Credential => "credential",
    }
}

fn framed_retention_hasher(domain: &[u8]) -> RetentionSha256 {
    let mut hasher = RetentionSha256::new();
    hash_retention_field(&mut hasher, domain);
    hasher
}

fn hash_retention_field(hasher: &mut RetentionSha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn retention_hex(value: &[u8; 32]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in value {
        output.push(TABLE[(byte >> 4) as usize] as char);
        output.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    output
}

fn retention_hex_decode(value: &str, field: &'static str) -> Result<[u8; 32], RetentionSdkError> {
    if value.len() != 64 {
        return Err(retention_state_error(format!("{field} must contain 64 hexadecimal characters")));
    }
    let mut output = [0; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = retention_hex_nibble(chunk[0])
            .ok_or_else(|| retention_state_error(format!("{field} is not hexadecimal")))?;
        let low = retention_hex_nibble(chunk[1])
            .ok_or_else(|| retention_state_error(format!("{field} is not hexadecimal")))?;
        output[index] = (high << 4) | low;
    }
    Ok(output)
}

fn retention_hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn retention_decimal_u64(value: String, field: &'static str) -> Result<u64, RetentionSdkError> {
    if value.is_empty() || (value.len() > 1 && value.starts_with('0')) {
        return Err(retention_state_error(format!("{field} is not canonical decimal")));
    }
    value
        .parse::<u64>()
        .map_err(|_| retention_state_error(format!("{field} is outside the supported range")))
}

fn retention_decimal_i64(value: String, field: &'static str) -> Result<i64, RetentionSdkError> {
    if value.is_empty()
        || value == "-0"
        || (value.len() > 1 && value.starts_with('0'))
        || (value.starts_with('-') && value.len() > 2 && value.as_bytes()[1] == b'0')
    {
        return Err(retention_state_error(format!("{field} is not canonical decimal")));
    }
    value
        .parse::<i64>()
        .map_err(|_| retention_state_error(format!("{field} is outside the supported range")))
}

fn validate_state_size(bytes: &[u8]) -> Result<(), RetentionSdkError> {
    if bytes.len() as u64 > RETENTION_DECISION_STATE_MAXIMUM_BYTES {
        return Err(retention_state_error(
            "retention decision exceeds its governed maximum size",
        ));
    }
    Ok(())
}

fn retention_domain_error(error: PrivacyRetentionError) -> RetentionSdkError {
    retention_state_error(format!("{}: {error}", error.code()))
}

fn retention_state_error(reference: impl Into<String>) -> RetentionSdkError {
    RetentionSdkError::new(
        "CUSTOMER_PRIVACY_RETENTION_DECISION_INVALID",
        RetentionErrorCategory::Internal,
        false,
        "Persisted Customer Privacy retention-decision evidence is invalid.",
    )
    .with_internal_reference(reference)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RetentionDecisionStateV1 {
    decision_id: String,
    tenant_id: String,
    canonical_party_id: String,
    privacy_case_id: String,
    action_plan_id: String,
    action_plan_digest: String,
    evaluated_at_unix_nanos: String,
    items: Vec<RetentionDecisionItemStateV1>,
    decision_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RetentionDecisionItemStateV1 {
    sequence: u32,
    owner_module_id: String,
    resource_type: String,
    resource_id: String,
    resource_version: String,
    data_class: DataClass,
    evidence_class: EvidenceClass,
    retention_policy_id: String,
    approved_action: PlannedPrivacyAction,
    final_action: PlannedPrivacyAction,
    reason: RetentionDecisionReason,
    legal_hold: Option<LegalHoldEvidenceStateV1>,
    item_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LegalHoldEvidenceStateV1 {
    hold_id: String,
    authority_reference: String,
    reason_code: String,
    policy_version: String,
    scope_label: String,
    review_at_unix_nanos: Option<String>,
    matching_hold_count: u32,
    matching_holds_digest: String,
}

impl From<&PrivacyRetentionDecisionSet> for RetentionDecisionStateV1 {
    fn from(value: &PrivacyRetentionDecisionSet) -> Self {
        Self {
            decision_id: value.decision_id.as_str().to_owned(),
            tenant_id: value.tenant_id.as_str().to_owned(),
            canonical_party_id: value.canonical_party_id.as_str().to_owned(),
            privacy_case_id: value.privacy_case_id.as_str().to_owned(),
            action_plan_id: value.action_plan_id.as_str().to_owned(),
            action_plan_digest: retention_hex(&value.action_plan_digest),
            evaluated_at_unix_nanos: value.evaluated_at_unix_nanos.to_string(),
            items: value
                .items
                .iter()
                .map(RetentionDecisionItemStateV1::from)
                .collect(),
            decision_digest: retention_hex(&value.digest),
        }
    }
}

impl RetentionDecisionStateV1 {
    fn into_domain(self) -> Result<PrivacyRetentionDecisionSet, RetentionSdkError> {
        let expected_id = self.decision_id;
        let expected_digest = retention_hex_decode(&self.decision_digest, "decision_digest")?;
        let decision = PrivacyRetentionDecisionSet::rehydrate(
            TenantId::try_new(self.tenant_id)
                .map_err(|error| retention_state_error(format!("tenant id is invalid: {error}")))?,
            RecordId::try_new(self.canonical_party_id).map_err(|error| {
                retention_state_error(format!("canonical Party id is invalid: {error}"))
            })?,
            RecordId::try_new(self.privacy_case_id).map_err(|error| {
                retention_state_error(format!("privacy case id is invalid: {error}"))
            })?,
            RecordId::try_new(self.action_plan_id).map_err(|error| {
                retention_state_error(format!("action plan id is invalid: {error}"))
            })?,
            retention_hex_decode(&self.action_plan_digest, "action_plan_digest")?,
            retention_decimal_i64(self.evaluated_at_unix_nanos, "evaluated_at_unix_nanos")?,
            self.items
                .into_iter()
                .map(RetentionDecisionItemStateV1::into_domain)
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(retention_domain_error)?;
        if decision.decision_id.as_str() != expected_id || decision.digest != expected_digest {
            return Err(retention_state_error(
                "retention-decision identity or digest differs from deterministic content",
            ));
        }
        Ok(decision)
    }
}

impl From<&PrivacyRetentionDecisionItem> for RetentionDecisionItemStateV1 {
    fn from(value: &PrivacyRetentionDecisionItem) -> Self {
        Self {
            sequence: value.sequence,
            owner_module_id: value.owner_module_id.as_str().to_owned(),
            resource_type: value.resource_type.clone(),
            resource_id: value.resource_id.as_str().to_owned(),
            resource_version: value.resource_version.to_string(),
            data_class: value.data_class,
            evidence_class: value.evidence_class,
            retention_policy_id: value.retention_policy_id.as_str().to_owned(),
            approved_action: value.approved_action,
            final_action: value.final_action,
            reason: value.reason,
            legal_hold: value.legal_hold.as_ref().map(LegalHoldEvidenceStateV1::from),
            item_digest: retention_hex(&value.digest),
        }
    }
}

impl RetentionDecisionItemStateV1 {
    fn into_domain(self) -> Result<PrivacyRetentionDecisionItem, RetentionSdkError> {
        let expected_digest = retention_hex_decode(&self.item_digest, "item_digest")?;
        let item = PrivacyRetentionDecisionItem {
            sequence: self.sequence,
            owner_module_id: ModuleId::try_new(self.owner_module_id).map_err(|error| {
                retention_state_error(format!("owner module id is invalid: {error}"))
            })?,
            resource_type: self.resource_type,
            resource_id: RecordId::try_new(self.resource_id).map_err(|error| {
                retention_state_error(format!("resource id is invalid: {error}"))
            })?,
            resource_version: retention_decimal_u64(self.resource_version, "resource_version")?,
            data_class: self.data_class,
            evidence_class: self.evidence_class,
            retention_policy_id: RetentionPolicyId::try_new(self.retention_policy_id).map_err(
                |error| retention_state_error(format!("retention policy id is invalid: {error}")),
            )?,
            approved_action: self.approved_action,
            final_action: self.final_action,
            reason: self.reason,
            legal_hold: self
                .legal_hold
                .map(LegalHoldEvidenceStateV1::into_domain)
                .transpose()?,
            digest: expected_digest,
        };
        Ok(item)
    }
}

impl From<&DecidingLegalHoldEvidence> for LegalHoldEvidenceStateV1 {
    fn from(value: &DecidingLegalHoldEvidence) -> Self {
        Self {
            hold_id: value.hold_id.as_str().to_owned(),
            authority_reference: value.authority_reference.as_str().to_owned(),
            reason_code: value.reason_code.clone(),
            policy_version: value.policy_version.as_str().to_owned(),
            scope_label: value.scope_label.clone(),
            review_at_unix_nanos: value.review_at_unix_nanos.map(|value| value.to_string()),
            matching_hold_count: value.matching_hold_count,
            matching_holds_digest: retention_hex(&value.matching_holds_digest),
        }
    }
}

impl LegalHoldEvidenceStateV1 {
    fn into_domain(self) -> Result<DecidingLegalHoldEvidence, RetentionSdkError> {
        if self.matching_hold_count == 0 {
            return Err(retention_state_error(
                "matching legal-hold count must be positive",
            ));
        }
        Ok(DecidingLegalHoldEvidence {
            hold_id: RecordId::try_new(self.hold_id).map_err(|error| {
                retention_state_error(format!("legal hold id is invalid: {error}"))
            })?,
            authority_reference: RecordId::try_new(self.authority_reference).map_err(|error| {
                retention_state_error(format!("authority reference is invalid: {error}"))
            })?,
            reason_code: self.reason_code,
            policy_version: SchemaVersion::try_new(self.policy_version).map_err(|error| {
                retention_state_error(format!("policy version is invalid: {error}"))
            })?,
            scope_label: self.scope_label,
            review_at_unix_nanos: self
                .review_at_unix_nanos
                .map(|value| retention_decimal_i64(value, "review_at_unix_nanos"))
                .transpose()?,
            matching_hold_count: self.matching_hold_count,
            matching_holds_digest: retention_hex_decode(
                &self.matching_holds_digest,
                "matching_holds_digest",
            )?,
        })
    }
}

#[cfg(test)]
mod retention_tests {
    use super::*;
    use crm_module_sdk::{ActorId, RecordId};

    fn hold(scope: LegalHoldScope, id: &str) -> CustomerDataLegalHold {
        CustomerDataLegalHold::place(
            RecordId::try_new(id).unwrap(),
            TenantId::try_new("tenant-a").unwrap(),
            RecordId::try_new("party-a").unwrap(),
            scope,
            RecordId::try_new("authority-a").unwrap(),
            "LITIGATION_HOLD",
            SchemaVersion::try_new("privacy-policy/1").unwrap(),
            ActorId::try_new("legal-officer").unwrap(),
            10,
            Some(100),
        )
        .unwrap()
    }

    fn decision_item(
        evidence_class: EvidenceClass,
        approved_action: PlannedPrivacyAction,
        reason: RetentionDecisionReason,
        final_action: PlannedPrivacyAction,
        legal_hold: Option<DecidingLegalHoldEvidence>,
    ) -> PrivacyRetentionDecisionItem {
        let mut item = PrivacyRetentionDecisionItem {
            sequence: 1,
            owner_module_id: ModuleId::try_new("crm.parties").unwrap(),
            resource_type: "party".to_owned(),
            resource_id: RecordId::try_new("party-a").unwrap(),
            resource_version: 1,
            data_class: DataClass::Personal,
            evidence_class,
            retention_policy_id: RetentionPolicyId::try_new("privacy-policy").unwrap(),
            approved_action,
            final_action,
            reason,
            legal_hold,
            digest: [0; 32],
        };
        item.digest = retention_item_digest(&item);
        item
    }

    #[test]
    fn scope_matching_is_exact() {
        let owner = ModuleId::try_new("crm.parties").unwrap();
        assert!(legal_hold_scope_matches(
            &LegalHoldScope::AllCustomerData,
            &owner,
            DataClass::Personal,
        ));
        assert!(legal_hold_scope_matches(
            &LegalHoldScope::Owner(owner.clone()),
            &owner,
            DataClass::Personal,
        ));
        assert!(legal_hold_scope_matches(
            &LegalHoldScope::DataClass(DataClass::Personal),
            &owner,
            DataClass::Personal,
        ));
        assert!(!legal_hold_scope_matches(
            &LegalHoldScope::DataClass(DataClass::Financial),
            &owner,
            DataClass::Personal,
        ));
    }

    #[test]
    fn legal_hold_wins_before_mandatory_retention() {
        let deciding = legal_hold_evidence(&[&hold(
            LegalHoldScope::AllCustomerData,
            "hold-a",
        )])
        .unwrap();
        let item = decision_item(
            EvidenceClass::ImmutableRequiredEvidence,
            PlannedPrivacyAction::Delete,
            RetentionDecisionReason::ActiveLegalHold,
            PlannedPrivacyAction::Retain,
            Some(deciding),
        );
        assert!(validate_decision_semantics(&item).is_ok());
    }

    #[test]
    fn mandatory_retention_overrides_destructive_action() {
        let immutable = decision_item(
            EvidenceClass::ImmutableRequiredEvidence,
            PlannedPrivacyAction::Delete,
            RetentionDecisionReason::MandatoryRetention,
            PlannedPrivacyAction::Retain,
            None,
        );
        let minimized = decision_item(
            EvidenceClass::RetainMinimizedEvidence,
            PlannedPrivacyAction::Delete,
            RetentionDecisionReason::MandatoryRetention,
            PlannedPrivacyAction::Anonymize,
            None,
        );
        assert!(validate_decision_semantics(&immutable).is_ok());
        assert!(validate_decision_semantics(&minimized).is_ok());
    }

    #[test]
    fn approved_non_mandatory_action_is_preserved() {
        let item = decision_item(
            EvidenceClass::DestroyableSubjectData,
            PlannedPrivacyAction::Delete,
            RetentionDecisionReason::ApprovedPrivacyAction,
            PlannedPrivacyAction::Delete,
            None,
        );
        assert!(validate_decision_semantics(&item).is_ok());
    }
}


#[cfg(test)]
mod mandatory_retention_tests {
    use super::*;

    #[test]
    fn immutable_evidence_has_explicit_mandatory_authority_without_breaking_restrictions() {
        assert_eq!(
            mandatory_retention_action(
                PlannedPrivacyAction::Retain,
                EvidenceClass::ImmutableRequiredEvidence,
            ),
            Some(PlannedPrivacyAction::Retain)
        );
        assert_eq!(
            mandatory_retention_action(
                PlannedPrivacyAction::Delete,
                EvidenceClass::ImmutableRequiredEvidence,
            ),
            Some(PlannedPrivacyAction::Retain)
        );
        assert_eq!(
            mandatory_retention_action(
                PlannedPrivacyAction::RestrictOnly,
                EvidenceClass::ImmutableRequiredEvidence,
            ),
            None
        );
    }

    #[test]
    fn minimized_evidence_only_constrains_destructive_actions() {
        assert_eq!(
            mandatory_retention_action(
                PlannedPrivacyAction::Anonymize,
                EvidenceClass::RetainMinimizedEvidence,
            ),
            Some(PlannedPrivacyAction::Anonymize)
        );
        assert_eq!(
            mandatory_retention_action(
                PlannedPrivacyAction::Retain,
                EvidenceClass::RetainMinimizedEvidence,
            ),
            None
        );
        assert_eq!(
            mandatory_retention_action(
                PlannedPrivacyAction::Delete,
                EvidenceClass::DestroyableSubjectData,
            ),
            None
        );
    }
}
