use crm_customer_privacy::{
    ACCESS_EXPORT_MANIFEST_MEDIA_TYPE, ActionPlanningPolicy, ContributionCompletenessProof,
    DiscoveryOwnerScopeContribution, DiscoveryScopeSnapshot, EvidenceClass, OwnerScopeContribution,
    OwnerScopeRegistry, PrivacyAccessExportManifest, PrivacyAccessExportReference,
    PrivacyActionPlan, PrivacyCaseKind, ScopeDiscoveryLineage, ScopeResource,
    decode_access_export_manifest, decode_access_export_reference, encode_access_export_manifest,
    encode_access_export_reference,
};
use crm_module_sdk::{DataClass, FileId, RecordId, RetentionPolicyId, SchemaVersion, TenantId};

const EFFECTIVE_REQUEST_AT_UNIX_MS: i64 = 100;
const CAPTURED_AT_UNIX_NANOS: i64 = 100_000_000;
const PLANNED_AT_UNIX_NANOS: i64 = 200_000_000;
const PREPARED_AT_UNIX_NANOS: i64 = 300_000_000;

fn tenant_id() -> TenantId {
    TenantId::try_new("tenant-access-export").unwrap()
}

fn record_id(value: &str) -> RecordId {
    RecordId::try_new(value).unwrap()
}

fn retention_policy() -> RetentionPolicyId {
    RetentionPolicyId::try_new("privacy-owner/default").unwrap()
}

fn policy() -> ActionPlanningPolicy {
    ActionPlanningPolicy::new(SchemaVersion::try_new("1.0.0").unwrap(), "EU", false, false).unwrap()
}

fn discovery_snapshot() -> DiscoveryScopeSnapshot {
    let registry = OwnerScopeRegistry::canonical_v1().unwrap();
    let lineage = ScopeDiscoveryLineage::new(
        record_id("privacy-case-access-export"),
        tenant_id(),
        record_id("party-access-export"),
        17,
        registry.registry_version().clone(),
        *registry.digest(),
        "ACCESS_EXPORT",
        EFFECTIVE_REQUEST_AT_UNIX_MS,
    )
    .unwrap();
    let contributions = registry
        .contracts()
        .iter()
        .enumerate()
        .map(|(index, contract)| {
            let resources = if index < 3 {
                vec![
                    ScopeResource::new(
                        format!("owner-{index}.resource"),
                        record_id(&format!("resource-{index}")),
                        7,
                        DataClass::Personal,
                        EvidenceClass::DestroyableSubjectData,
                        retention_policy(),
                    )
                    .unwrap(),
                ]
            } else {
                Vec::new()
            };
            let count = resources.len() as u64;
            let contribution = OwnerScopeContribution::new(
                contract.clone(),
                tenant_id(),
                record_id("party-access-export"),
                17,
                resources,
                ContributionCompletenessProof::new(true, 1, count, count, [(index + 1) as u8; 32])
                    .unwrap(),
            )
            .unwrap();
            DiscoveryOwnerScopeContribution::new(lineage.clone(), contribution).unwrap()
        })
        .collect::<Vec<_>>();
    DiscoveryScopeSnapshot::finalize(
        lineage,
        registry,
        CAPTURED_AT_UNIX_NANOS,
        contributions,
    )
    .unwrap()
}

fn access_plan(kind: PrivacyCaseKind) -> PrivacyActionPlan {
    PrivacyActionPlan::build(
        &discovery_snapshot(),
        4,
        kind,
        policy(),
        PLANNED_AT_UNIX_NANOS,
    )
    .unwrap()
}

#[test]
fn access_manifest_identity_and_strict_state_are_deterministic() {
    let plan = access_plan(PrivacyCaseKind::Access);
    let first = PrivacyAccessExportManifest::build(&plan).unwrap();
    let second = PrivacyAccessExportManifest::build(&plan).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.items().len(), 3);

    let bytes = encode_access_export_manifest(&first).unwrap();
    assert_eq!(decode_access_export_manifest(&bytes).unwrap(), first);

    let mut whitespace = bytes.clone();
    whitespace.push(b'\n');
    assert!(decode_access_export_manifest(&whitespace).is_err());

    let text = String::from_utf8(bytes).unwrap();
    let unknown = text.replacen('{', "{\"future\":true,", 1);
    assert!(decode_access_export_manifest(unknown.as_bytes()).is_err());
}

#[test]
fn prepared_completed_and_replayed_reference_is_immutable() {
    let manifest =
        PrivacyAccessExportManifest::build(&access_plan(PrivacyCaseKind::PortabilityExport))
            .unwrap();
    let mut reference =
        PrivacyAccessExportReference::prepare(manifest, PREPARED_AT_UNIX_NANOS).unwrap();
    let prepared = reference.clone();
    assert_eq!(
        decode_access_export_reference(&encode_access_export_reference(&prepared).unwrap())
            .unwrap(),
        prepared
    );

    let file_id = FileId::try_new("privacy-export-artifact-test").unwrap();
    let retention = RetentionPolicyId::try_new("customer_privacy_access_export").unwrap();
    reference
        .complete(
            prepared.export_job_id(),
            file_id.clone(),
            ACCESS_EXPORT_MANIFEST_MEDIA_TYPE.to_owned(),
            [9; 32],
            123,
            retention.clone(),
            PREPARED_AT_UNIX_NANOS,
        )
        .unwrap();
    let completed = reference.clone();
    reference
        .complete(
            prepared.export_job_id(),
            file_id,
            ACCESS_EXPORT_MANIFEST_MEDIA_TYPE.to_owned(),
            [9; 32],
            123,
            retention,
            PREPARED_AT_UNIX_NANOS,
        )
        .unwrap();
    assert_eq!(reference, completed);
    assert_eq!(
        decode_access_export_reference(&encode_access_export_reference(&completed).unwrap())
            .unwrap(),
        completed
    );

    let error = reference
        .complete(
            prepared.export_job_id(),
            FileId::try_new("privacy-export-artifact-conflict").unwrap(),
            ACCESS_EXPORT_MANIFEST_MEDIA_TYPE.to_owned(),
            [8; 32],
            124,
            RetentionPolicyId::try_new("customer_privacy_access_export").unwrap(),
            PREPARED_AT_UNIX_NANOS,
        )
        .unwrap_err();
    assert_eq!(
        error.code.as_str(),
        "CUSTOMER_PRIVACY_ACCESS_EXPORT_CONFLICT"
    );
}

#[test]
fn erasure_plan_cannot_be_represented_as_access_export() {
    let plan = PrivacyActionPlan::build(
        &discovery_snapshot(),
        4,
        PrivacyCaseKind::Erasure,
        ActionPlanningPolicy::new(SchemaVersion::try_new("1.0.0").unwrap(), "EU", true, true)
            .unwrap(),
        PLANNED_AT_UNIX_NANOS,
    )
    .unwrap();
    let error = PrivacyAccessExportManifest::build(&plan).unwrap_err();
    assert_eq!(
        error.code.as_str(),
        "CUSTOMER_PRIVACY_ACCESS_EXPORT_CONFLICT"
    );
}
