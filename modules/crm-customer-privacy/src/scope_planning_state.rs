pub fn encode_action_plan_state(plan: &PrivacyActionPlan) -> Result<Vec<u8>, SdkError> {
    let bytes = persisted_state_json::to_vec(&PrivacyActionPlanStateV1::from(plan)).map_err(
        |error| planning_persisted_error(format!("action plan serialization failed: {error}")),
    )?;
    validate_action_plan_state_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_action_plan_state(bytes: &[u8]) -> Result<PrivacyActionPlan, SdkError> {
    validate_action_plan_state_size(bytes)?;
    let state: PrivacyActionPlanStateV1 = persisted_state_json::from_slice(bytes)
        .map_err(|error| planning_persisted_error(format!("action plan JSON is invalid: {error}")))?;
    let plan = state.into_domain()?;
    if encode_action_plan_state(&plan)? != bytes {
        return Err(planning_persisted_error(
            "persisted action plan is not the strict canonical v1 encoding",
        ));
    }
    Ok(plan)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrivacyActionPlanStateV1 {
    plan_id: String,
    lineage: ActionPlanLineageStateV1,
    planned_at_unix_nanos: String,
    items: Vec<PrivacyActionPlanItemStateV1>,
    plan_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionPlanLineageStateV1 {
    privacy_case_id: String,
    tenant_id: String,
    canonical_party_id: String,
    identity_resolution_generation: String,
    source_case_version: String,
    scope_snapshot_id: String,
    scope_snapshot_binding_digest: String,
    scope_completeness_digest: String,
    registry_digest: String,
    purpose_code: String,
    effective_request_at_unix_ms: String,
    snapshot_captured_at_unix_nanos: String,
    case_kind: String,
    policy_version: String,
    jurisdiction_code: String,
    approval_required: bool,
    crypto_shred_supported: bool,
    lineage_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrivacyActionPlanItemStateV1 {
    sequence: u32,
    owner_module_id: String,
    resource_type: String,
    resource_id: String,
    resource_version: String,
    data_class: DataClass,
    evidence_class: EvidenceClass,
    retention_policy_id: String,
    action: PlannedPrivacyAction,
    reason: PrivacyPlanReason,
    item_digest: String,
}

impl From<&PrivacyActionPlan> for PrivacyActionPlanStateV1 {
    fn from(plan: &PrivacyActionPlan) -> Self {
        Self {
            plan_id: plan.plan_id.as_str().to_owned(),
            lineage: ActionPlanLineageStateV1::from(&plan.lineage),
            planned_at_unix_nanos: plan.planned_at_unix_nanos.to_string(),
            items: plan
                .items
                .iter()
                .map(PrivacyActionPlanItemStateV1::from)
                .collect(),
            plan_digest: hex_encode(&plan.digest),
        }
    }
}

impl PrivacyActionPlanStateV1 {
    fn into_domain(self) -> Result<PrivacyActionPlan, SdkError> {
        let expected_plan_id = self.plan_id;
        let expected_plan_digest = hex_decode(&self.plan_digest, "plan_digest")?;
        let lineage = self.lineage.into_domain()?;
        let items = self
            .items
            .into_iter()
            .map(|item| item.into_domain(lineage.digest()))
            .collect::<Result<Vec<_>, _>>()?;
        let plan = PrivacyActionPlan::rehydrate(
            lineage,
            decimal_i64(self.planned_at_unix_nanos, "planned_at_unix_nanos")?,
            items,
        )
        .map_err(planning_error)?;
        if plan.plan_id.as_str() != expected_plan_id {
            return Err(planning_persisted_error(
                "persisted action plan id does not match deterministic content",
            ));
        }
        if plan.digest != expected_plan_digest {
            return Err(planning_persisted_error(
                "persisted action plan digest does not match deterministic content",
            ));
        }
        Ok(plan)
    }
}

impl From<&ActionPlanLineage> for ActionPlanLineageStateV1 {
    fn from(lineage: &ActionPlanLineage) -> Self {
        Self {
            privacy_case_id: lineage.privacy_case_id.as_str().to_owned(),
            tenant_id: lineage.tenant_id.as_str().to_owned(),
            canonical_party_id: lineage.canonical_party_id.as_str().to_owned(),
            identity_resolution_generation: lineage
                .identity_resolution_generation
                .to_string(),
            source_case_version: lineage.source_case_version.to_string(),
            scope_snapshot_id: lineage.scope_snapshot_id.as_str().to_owned(),
            scope_snapshot_binding_digest: hex_encode(&lineage.scope_snapshot_binding_digest),
            scope_completeness_digest: hex_encode(&lineage.scope_completeness_digest),
            registry_digest: hex_encode(&lineage.registry_digest),
            purpose_code: lineage.purpose_code.clone(),
            effective_request_at_unix_ms: lineage.effective_request_at_unix_ms.to_string(),
            snapshot_captured_at_unix_nanos: lineage
                .snapshot_captured_at_unix_nanos
                .to_string(),
            case_kind: case_kind_label(lineage.case_kind).to_owned(),
            policy_version: lineage.policy_version.as_str().to_owned(),
            jurisdiction_code: lineage.jurisdiction_code.clone(),
            approval_required: lineage.approval_required,
            crypto_shred_supported: lineage.crypto_shred_supported,
            lineage_digest: hex_encode(&lineage.digest),
        }
    }
}

impl ActionPlanLineageStateV1 {
    fn into_domain(self) -> Result<ActionPlanLineage, SdkError> {
        let expected_digest = hex_decode(&self.lineage_digest, "lineage_digest")?;
        let lineage = ActionPlanLineage::new(
            RecordId::try_new(self.privacy_case_id)
                .map_err(|error| planning_persisted_error(format!("privacy case id is invalid: {error}")))?,
            TenantId::try_new(self.tenant_id)
                .map_err(|error| planning_persisted_error(format!("tenant id is invalid: {error}")))?,
            RecordId::try_new(self.canonical_party_id).map_err(|error| {
                planning_persisted_error(format!("canonical Party id is invalid: {error}"))
            })?,
            decimal_u64(
                self.identity_resolution_generation,
                "identity_resolution_generation",
            )?,
            decimal_u64(self.source_case_version, "source_case_version")?,
            RecordId::try_new(self.scope_snapshot_id).map_err(|error| {
                planning_persisted_error(format!("scope snapshot id is invalid: {error}"))
            })?,
            hex_decode(
                &self.scope_snapshot_binding_digest,
                "scope_snapshot_binding_digest",
            )?,
            hex_decode(
                &self.scope_completeness_digest,
                "scope_completeness_digest",
            )?,
            hex_decode(&self.registry_digest, "registry_digest")?,
            self.purpose_code,
            decimal_i64(
                self.effective_request_at_unix_ms,
                "effective_request_at_unix_ms",
            )?,
            decimal_i64(
                self.snapshot_captured_at_unix_nanos,
                "snapshot_captured_at_unix_nanos",
            )?,
            parse_case_kind(&self.case_kind)?,
            SchemaVersion::try_new(self.policy_version).map_err(|error| {
                planning_persisted_error(format!("policy version is invalid: {error}"))
            })?,
            self.jurisdiction_code,
            self.approval_required,
            self.crypto_shred_supported,
        )
        .map_err(planning_error)?;
        if lineage.digest != expected_digest {
            return Err(planning_persisted_error(
                "persisted action-plan lineage digest does not match deterministic content",
            ));
        }
        Ok(lineage)
    }
}

impl From<&PrivacyActionPlanItem> for PrivacyActionPlanItemStateV1 {
    fn from(item: &PrivacyActionPlanItem) -> Self {
        Self {
            sequence: item.sequence,
            owner_module_id: item.owner_module_id.as_str().to_owned(),
            resource_type: item.resource_type.clone(),
            resource_id: item.resource_id.as_str().to_owned(),
            resource_version: item.resource_version.to_string(),
            data_class: item.data_class,
            evidence_class: item.evidence_class,
            retention_policy_id: item.retention_policy_id.as_str().to_owned(),
            action: item.action,
            reason: item.reason,
            item_digest: hex_encode(&item.digest),
        }
    }
}

impl PrivacyActionPlanItemStateV1 {
    fn into_domain(self, lineage_digest: &[u8; 32]) -> Result<PrivacyActionPlanItem, SdkError> {
        let expected_digest = hex_decode(&self.item_digest, "item_digest")?;
        let item = PrivacyActionPlanItem::new(
            lineage_digest,
            self.sequence,
            ModuleId::try_new(self.owner_module_id).map_err(|error| {
                planning_persisted_error(format!("owner module id is invalid: {error}"))
            })?,
            self.resource_type,
            RecordId::try_new(self.resource_id).map_err(|error| {
                planning_persisted_error(format!("resource id is invalid: {error}"))
            })?,
            decimal_u64(self.resource_version, "resource_version")?,
            self.data_class,
            self.evidence_class,
            RetentionPolicyId::try_new(self.retention_policy_id).map_err(|error| {
                planning_persisted_error(format!("retention policy id is invalid: {error}"))
            })?,
            self.action,
            self.reason,
        )
        .map_err(planning_error)?;
        if item.digest != expected_digest {
            return Err(planning_persisted_error(
                "persisted action-plan item digest does not match deterministic content",
            ));
        }
        Ok(item)
    }
}

fn validate_action_plan_state_size(bytes: &[u8]) -> Result<(), SdkError> {
    if bytes.len() as u64 > ACTION_PLAN_STATE_MAXIMUM_BYTES {
        return Err(planning_persisted_error(
            "action plan exceeds its governed maximum size",
        ));
    }
    Ok(())
}
