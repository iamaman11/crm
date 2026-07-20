from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    path.write_text(text.replace(old, new, 1))


process_test = Path(
    "crates/crm-customer-enrichment-provider-process-composition/tests/postgres_conflict_process_hold.rs"
)
replace_once(
    process_test,
    'const SEED_CAPABILITY: &str = "customer_enrichment.provider_process.seed";',
    'const SEED_CAPABILITY: &str = "customer_enrichment.response.record";',
    "provider conflict process seed capability",
)

persistence_test = Path(
    "crates/crm-customer-enrichment-provider-process-composition/tests/postgres_conflict_persistence.rs"
)
replace_once(
    persistence_test,
    'scope: "customer_enrichment.provider_conflict.seed@1.0.0".to_owned(),',
    'scope: "customer_enrichment.response.record@1.0.0".to_owned(),',
    "provider conflict persistence seed scope",
)
replace_once(
    persistence_test,
    'CapabilityId::try_new("customer_enrichment.provider_conflict.seed")',
    'CapabilityId::try_new("customer_enrichment.response.record")',
    "provider conflict persistence seed capability",
)
replace_once(
    persistence_test,
    """    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = 'tenant-a' AND capability_id = 'customer_enrichment.response.record'",
        )
        .await,
        1
    );
""",
    """    assert_eq!(
        scalar(
            &admin,
            "SELECT count(*)::bigint FROM crm.audit_records WHERE tenant_id = 'tenant-a' AND capability_id = 'customer_enrichment.response.record'",
        )
        .await,
        2
    );
""",
    "provider conflict persistence seed and conflict audit count",
)

persistence = Path(
    "crates/crm-customer-enrichment-provider-process-composition/src/conflict_persistence.rs"
)
replace_once(
    persistence,
    """            if conflict.resolution().is_none() {
                if unresolved.replace(conflict).is_some() {
                    return Err(conflict_state_invalid(
                        "request has more than one unresolved provider-response conflict",
                    ));
                }
            }
""",
    """            if conflict.resolution().is_none() && unresolved.replace(conflict).is_some() {
                return Err(conflict_state_invalid(
                    "request has more than one unresolved provider-response conflict",
                ));
            }
""",
    "canonical unresolved conflict lookup shape",
)

print("aligned provider conflict fixtures and canonical lookup shape")
