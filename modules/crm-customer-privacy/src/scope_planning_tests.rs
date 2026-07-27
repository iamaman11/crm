#[cfg(test)]
mod planning_tests {
    use super::*;

    fn tenant_id() -> TenantId {
        TenantId::try_new("tenant-a").unwrap()
    }

    fn record_id(value: &str) -> RecordId {
        RecordId::try_new(value).unwrap()
    }

    fn retention_policy() -> RetentionPolicyId {
        RetentionPolicyId::try_new("privacy-owner/default").unwrap()
    }

    fn policy(
        version: &str,
        approval_required: bool,
        crypto_shred_supported: bool,
    ) -> ActionPlanningPolicy {
        ActionPlanningPolicy::new(
            SchemaVersion::try_new(version).unwrap(),
            "EU",
            approval_required,
            crypto_shred_supported,
        )
        .unwrap()
    }

    fn discovery_snapshot(evidence_classes: &[EvidenceClass]) -> DiscoveryScopeSnapshot {
        let registry = OwnerScopeRegistry::canonical_v1().unwrap();
        let lineage = ScopeDiscoveryLineage::new(
            record_id("privacy-case-1"),
            tenant_id(),
            record_id("party-canonical"),
            9,
            registry.registry_version().clone(),
            *registry.digest(),
            "ERASURE",
            100,
        )
        .unwrap();
        let contributions = registry
            .contracts()
            .iter()
            .enumerate()
            .map(|(index, contract)| {
                let resources = evidence_classes
                    .get(index)
                    .copied()
                    .map(|evidence_class| {
                        ScopeResource::new(
                            format!("owner-{index}.resource"),
                            record_id(&format!("resource-{index}")),
                            7,
                            match index % 4 {
                                0 => DataClass::Personal,
                                1 => DataClass::SensitivePersonal,
                                2 => DataClass::Financial,
                                _ => DataClass::Biometric,
                            },
                            evidence_class,
                            retention_policy(),
                        )
                        .unwrap()
                    })
                    .into_iter()
                    .collect::<Vec<_>>();
                let count = resources.len() as u64;
                let contribution = OwnerScopeContribution::new(
                    contract.clone(),
                    tenant_id(),
                    record_id("party-canonical"),
                    9,
                    resources,
                    ContributionCompletenessProof::new(
                        true,
                        1,
                        count,
                        count,
                        [(index + 1) as u8; 32],
                    )
                    .unwrap(),
                )
                .unwrap();
                DiscoveryOwnerScopeContribution::new(lineage.clone(), contribution).unwrap()
            })
            .collect::<Vec<_>>();
        DiscoveryScopeSnapshot::finalize(lineage, registry, 100_000_000, contributions).unwrap()
    }

    fn all_evidence_classes() -> Vec<EvidenceClass> {
        vec![
            EvidenceClass::DestroyableSubjectData,
            EvidenceClass::RetainMinimizedEvidence,
            EvidenceClass::ImmutableRequiredEvidence,
            EvidenceClass::DerivedRebuildableState,
            EvidenceClass::CryptoShreddableData,
        ]
    }

    #[test]
    fn planning_coordinates_bounds_and_state_contract_are_exact() {
        assert_eq!(ACTION_PLAN_BUILD_COORDINATE, "customer_privacy.plan.build@1.0.0");
        assert_eq!(ACTION_PLAN_GET_COORDINATE, "customer_privacy.case.plan.get@1.0.0");
        assert_eq!(
            OWNER_OUTCOMES_LIST_COORDINATE,
            "customer_privacy.case.owner_outcomes.list@1.0.0"
        );
        assert_eq!(ACTION_PLAN_STATE_SCHEMA_ID, "crm.customer-privacy.action_plan.state");
        assert_eq!(ACTION_PLAN_STATE_SCHEMA_VERSION, "1.0.0");
        assert_eq!(ACTION_PLAN_STATE_MAXIMUM_BYTES, 512 * 1024);
        assert_eq!(OWNER_OUTCOME_DEFAULT_PAGE_SIZE, 64);
        assert_eq!(OWNER_OUTCOME_MAXIMUM_PAGE_SIZE, 128);
        assert_eq!(OWNER_OUTCOME_MAXIMUM_CURSOR_BYTES, 2_048);
        assert!(action_plan_state_descriptor_hash().iter().any(|byte| *byte != 0));
    }

    #[test]
    fn access_and_portability_plans_are_disclosure_only_retain_plans() {
        let snapshot = discovery_snapshot(&all_evidence_classes());
        let access = PrivacyActionPlan::build(
            &snapshot,
            4,
            PrivacyCaseKind::Access,
            policy("1.0.0", false, false),
            101_000_000,
        )
        .unwrap();
        assert!(access
            .items()
            .iter()
            .all(|item| item.action() == PlannedPrivacyAction::Retain
                && item.reason() == PrivacyPlanReason::AccessDisclosureOnly));

        let portability = PrivacyActionPlan::build(
            &snapshot,
            4,
            PrivacyCaseKind::PortabilityExport,
            policy("1.0.0", false, false),
            101_000_000,
        )
        .unwrap();
        assert!(portability
            .items()
            .iter()
            .all(|item| item.action() == PlannedPrivacyAction::Retain
                && item.reason() == PrivacyPlanReason::PortabilityDisclosureOnly));
    }

    #[test]
    fn restriction_plan_contains_only_restrict_only_actions() {
        let snapshot = discovery_snapshot(&all_evidence_classes());
        let plan = PrivacyActionPlan::build(
            &snapshot,
            4,
            PrivacyCaseKind::RestrictProcessing,
            policy("1.0.0", true, false),
            101_000_000,
        )
        .unwrap();
        assert!(plan.items().iter().all(|item| {
            item.action() == PlannedPrivacyAction::RestrictOnly
                && item.reason() == PrivacyPlanReason::RestrictionRequested
        }));
        assert!(plan.lineage().approval_required());
    }

    #[test]
    fn erasure_plan_uses_the_exact_frozen_evidence_mapping() {
        let snapshot = discovery_snapshot(&all_evidence_classes());
        let plan = PrivacyActionPlan::build(
            &snapshot,
            4,
            PrivacyCaseKind::Erasure,
            policy("1.0.0", true, true),
            101_000_000,
        )
        .unwrap();
        let actions = plan
            .items()
            .iter()
            .map(|item| (item.evidence_class(), (item.action(), item.reason())))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            actions[&EvidenceClass::DestroyableSubjectData],
            (
                PlannedPrivacyAction::Delete,
                PrivacyPlanReason::ErasureDestroyableSubjectData
            )
        );
        assert_eq!(
            actions[&EvidenceClass::RetainMinimizedEvidence],
            (
                PlannedPrivacyAction::Anonymize,
                PrivacyPlanReason::ErasureRetainMinimizedEvidence
            )
        );
        assert_eq!(
            actions[&EvidenceClass::ImmutableRequiredEvidence],
            (
                PlannedPrivacyAction::Retain,
                PrivacyPlanReason::ErasureImmutableRequiredEvidence
            )
        );
        assert_eq!(
            actions[&EvidenceClass::DerivedRebuildableState],
            (
                PlannedPrivacyAction::Delete,
                PrivacyPlanReason::ErasureDerivedRebuildableState
            )
        );
        assert_eq!(
            actions[&EvidenceClass::CryptoShreddableData],
            (
                PlannedPrivacyAction::CryptoShred,
                PrivacyPlanReason::ErasureCryptoShreddableData
            )
        );
    }

    #[test]
    fn unsupported_crypto_shred_fails_closed_without_destructive_fallback() {
        let snapshot = discovery_snapshot(&[EvidenceClass::CryptoShreddableData]);
        let error = PrivacyActionPlan::build(
            &snapshot,
            4,
            PrivacyCaseKind::Erasure,
            policy("1.0.0", true, false),
            101_000_000,
        )
        .unwrap_err();
        assert_eq!(
            error.code(),
            "CUSTOMER_PRIVACY_PLANNING_CRYPTO_SHRED_UNSUPPORTED"
        );
    }

    #[test]
    fn plan_identity_is_deterministic_and_bound_to_case_policy_and_kind() {
        let snapshot = discovery_snapshot(&all_evidence_classes());
        let first = PrivacyActionPlan::build(
            &snapshot,
            4,
            PrivacyCaseKind::Erasure,
            policy("1.0.0", true, true),
            101_000_000,
        )
        .unwrap();
        let second = PrivacyActionPlan::build(
            &snapshot,
            4,
            PrivacyCaseKind::Erasure,
            policy("1.0.0", true, true),
            101_000_000,
        )
        .unwrap();
        assert_eq!(first, second);

        let different_version = PrivacyActionPlan::build(
            &snapshot,
            5,
            PrivacyCaseKind::Erasure,
            policy("1.0.0", true, true),
            101_000_000,
        )
        .unwrap();
        assert_ne!(first.plan_id(), different_version.plan_id());

        let different_policy = PrivacyActionPlan::build(
            &snapshot,
            4,
            PrivacyCaseKind::Erasure,
            policy("2.0.0", true, true),
            101_000_000,
        )
        .unwrap();
        assert_ne!(first.plan_id(), different_policy.plan_id());

        let different_kind = PrivacyActionPlan::build(
            &snapshot,
            4,
            PrivacyCaseKind::Access,
            policy("1.0.0", true, true),
            101_000_000,
        )
        .unwrap();
        assert_ne!(first.plan_id(), different_kind.plan_id());
    }

    #[test]
    fn empty_complete_scope_produces_one_empty_immutable_plan() {
        let snapshot = discovery_snapshot(&[]);
        let plan = PrivacyActionPlan::build(
            &snapshot,
            4,
            PrivacyCaseKind::Access,
            policy("1.0.0", false, false),
            101_000_000,
        )
        .unwrap();
        assert!(plan.items().is_empty());
        assert!(plan.plan_id().as_str().starts_with("privacy-action-plan-"));
    }

    #[test]
    fn strict_state_round_trip_rejects_unknown_fields_tampering_and_whitespace() {
        let snapshot = discovery_snapshot(&all_evidence_classes());
        let plan = PrivacyActionPlan::build(
            &snapshot,
            4,
            PrivacyCaseKind::Erasure,
            policy("1.0.0", true, true),
            101_000_000,
        )
        .unwrap();
        let bytes = encode_action_plan_state(&plan).unwrap();
        assert_eq!(decode_action_plan_state(&bytes).unwrap(), plan);

        let mut whitespace = bytes.clone();
        whitespace.push(b'\n');
        assert!(decode_action_plan_state(&whitespace).is_err());

        let text = String::from_utf8(bytes.clone()).unwrap();
        let unknown = text.replacen('{', "{\"future\":true,", 1);
        assert!(decode_action_plan_state(unknown.as_bytes()).is_err());

        let tampered = text.replacen("\"action\":\"delete\"", "\"action\":\"retain\"", 1);
        assert_ne!(tampered, text);
        assert!(decode_action_plan_state(tampered.as_bytes()).is_err());
    }

    #[test]
    fn initial_planner_never_infers_no_op_already_compliant() {
        let snapshot = discovery_snapshot(&all_evidence_classes());
        for case_kind in [
            PrivacyCaseKind::Access,
            PrivacyCaseKind::PortabilityExport,
            PrivacyCaseKind::RestrictProcessing,
            PrivacyCaseKind::Erasure,
        ] {
            let plan = PrivacyActionPlan::build(
                &snapshot,
                4,
                case_kind,
                policy("1.0.0", true, true),
                101_000_000,
            )
            .unwrap();
            assert!(plan
                .items()
                .iter()
                .all(|item| item.action() != PlannedPrivacyAction::NoOpAlreadyCompliant));
        }
    }
}
