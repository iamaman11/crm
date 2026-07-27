#[cfg(test)]
mod discovery_tests {
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

    fn lineage(
        registry: &OwnerScopeRegistry,
        purpose_code: &str,
        effective_request_at_unix_ms: i64,
    ) -> ScopeDiscoveryLineage {
        ScopeDiscoveryLineage::new(
            record_id("case-1"),
            tenant_id(),
            record_id("party-canonical"),
            9,
            registry.registry_version().clone(),
            *registry.digest(),
            purpose_code,
            effective_request_at_unix_ms,
        )
        .unwrap()
    }

    fn inner_contributions(registry: &OwnerScopeRegistry) -> Vec<OwnerScopeContribution> {
        registry
            .contracts()
            .iter()
            .enumerate()
            .map(|(index, contract)| {
                let resource = ScopeResource::new(
                    format!("owner-{index}.resource"),
                    record_id(&format!("resource-{index}")),
                    7,
                    DataClass::Personal,
                    EvidenceClass::DestroyableSubjectData,
                    retention_policy(),
                )
                .unwrap();
                OwnerScopeContribution::new(
                    contract.clone(),
                    tenant_id(),
                    record_id("party-canonical"),
                    9,
                    vec![resource],
                    ContributionCompletenessProof::new(
                        true,
                        1,
                        1,
                        1,
                        [(index + 1) as u8; 32],
                    )
                    .unwrap(),
                )
                .unwrap()
            })
            .collect()
    }

    fn bound_contributions(
        registry: &OwnerScopeRegistry,
        lineage: &ScopeDiscoveryLineage,
    ) -> Vec<DiscoveryOwnerScopeContribution> {
        inner_contributions(registry)
            .into_iter()
            .map(|contribution| {
                DiscoveryOwnerScopeContribution::new(lineage.clone(), contribution).unwrap()
            })
            .collect()
    }

    #[test]
    fn discovery_snapshot_binds_purpose_and_effective_request_time() {
        let registry = OwnerScopeRegistry::canonical_v1().unwrap();
        let first_lineage = lineage(&registry, "ERASURE_DISCOVERY", 100);
        let first = DiscoveryScopeSnapshot::finalize(
            first_lineage.clone(),
            registry.clone(),
            200_000_000,
            bound_contributions(&registry, &first_lineage),
        )
        .unwrap();

        let purpose_lineage = lineage(&registry, "ACCESS_DISCOVERY", 100);
        let purpose_changed = DiscoveryScopeSnapshot::finalize(
            purpose_lineage.clone(),
            registry.clone(),
            200_000_000,
            bound_contributions(&registry, &purpose_lineage),
        )
        .unwrap();

        let time_lineage = lineage(&registry, "ERASURE_DISCOVERY", 101);
        let time_changed = DiscoveryScopeSnapshot::finalize(
            time_lineage.clone(),
            registry.clone(),
            200_000_000,
            bound_contributions(&registry, &time_lineage),
        )
        .unwrap();

        assert_eq!(first.aggregation(), purpose_changed.aggregation());
        assert_eq!(first.aggregation(), time_changed.aggregation());
        assert_ne!(first.snapshot_id(), purpose_changed.snapshot_id());
        assert_ne!(first.snapshot_id(), time_changed.snapshot_id());
        assert_ne!(first.binding_digest(), purpose_changed.binding_digest());
        assert_ne!(first.binding_digest(), time_changed.binding_digest());
    }

    #[test]
    fn discovery_snapshot_rejects_registry_and_time_drift() {
        let registry = OwnerScopeRegistry::canonical_v1().unwrap();
        let valid_lineage = lineage(&registry, "ERASURE_DISCOVERY", 100);
        let error = DiscoveryScopeSnapshot::finalize(
            valid_lineage.clone(),
            registry.clone(),
            99_999_999,
            bound_contributions(&registry, &valid_lineage),
        )
        .unwrap_err();
        assert_eq!(error.code(), "CUSTOMER_PRIVACY_SCOPE_INVALID_ARGUMENT");

        let drifted_lineage = ScopeDiscoveryLineage::new(
            record_id("case-1"),
            tenant_id(),
            record_id("party-canonical"),
            9,
            registry.registry_version().clone(),
            [9; 32],
            "ERASURE_DISCOVERY",
            100,
        )
        .unwrap();
        let error = DiscoveryScopeSnapshot::finalize(
            drifted_lineage.clone(),
            registry.clone(),
            200_000_000,
            bound_contributions(&registry, &drifted_lineage),
        )
        .unwrap_err();
        assert_eq!(error.code(), "CUSTOMER_PRIVACY_SCOPE_REGISTRY_CONFLICT");
    }

    #[test]
    fn bound_owner_contribution_rejects_full_lineage_mismatch() {
        let registry = OwnerScopeRegistry::canonical_v1().unwrap();
        let valid_lineage = lineage(&registry, "ERASURE_DISCOVERY", 100);
        let mut contribution = inner_contributions(&registry).remove(0);
        contribution.tenant_id = TenantId::try_new("tenant-b").unwrap();
        let error =
            DiscoveryOwnerScopeContribution::new(valid_lineage, contribution).unwrap_err();
        assert_eq!(error.code(), "CUSTOMER_PRIVACY_SCOPE_LINEAGE_MISMATCH");
    }

    #[test]
    fn canonical_discovery_snapshot_round_trip_rejects_noncanonical_bytes() {
        let registry = OwnerScopeRegistry::canonical_v1().unwrap();
        let lineage = lineage(&registry, "ERASURE_DISCOVERY", 100);
        let snapshot = DiscoveryScopeSnapshot::finalize(
            lineage.clone(),
            registry.clone(),
            200_000_000,
            bound_contributions(&registry, &lineage),
        )
        .unwrap();

        let bytes = encode_discovery_scope_snapshot_state(&snapshot).unwrap();
        assert_eq!(
            decode_discovery_scope_snapshot_state(&bytes).unwrap(),
            snapshot
        );
        assert!(
            discovery_scope_snapshot_state_descriptor_hash()
                .iter()
                .any(|byte| *byte != 0)
        );

        let mut spaced = bytes.clone();
        spaced.insert(1, b' ');
        assert!(decode_discovery_scope_snapshot_state(&spaced).is_err());

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_owned(), serde_json::Value::Bool(true));
        let unknown = serde_json::to_vec(&value).unwrap();
        assert!(decode_discovery_scope_snapshot_state(&unknown).is_err());
    }
}
