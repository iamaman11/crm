#![forbid(unsafe_code)]

const GENERATION_UP: &str = include_str!(
    "../../../database/migrations/20260722_0001_identity_resolution_topology_generation.up.sql"
);
const GENERATION_DOWN: &str = include_str!(
    "../../../database/migrations/20260722_0001_identity_resolution_topology_generation.down.sql"
);
const TRANSACTION_UP: &str = include_str!(
    "../../../database/migrations/20260722_0002_identity_resolution_topology_generation_transaction.up.sql"
);
const TOPOLOGY_LOCK_UP: &str = include_str!(
    "../../../database/migrations/20260722_0003_identity_resolution_topology_lock_fail_fast.up.sql"
);
const SUBJECT_LOCK_UP: &str = include_str!(
    "../../../database/migrations/20260722_0004_customer_subject_lock_fail_fast.up.sql"
);

#[test]
fn generation_is_monotonic_tenant_bound_and_transaction_linked() {
    for fragment in [
        "CREATE TABLE crm.identity_resolution_topology_generations",
        "tenant_id text PRIMARY KEY REFERENCES crm.tenants",
        "generation bigint NOT NULL CHECK (generation > 0)",
        "ALTER TABLE crm.identity_resolution_topology_generations ENABLE ROW LEVEL SECURITY",
        "ALTER TABLE crm.identity_resolution_topology_generations FORCE ROW LEVEL SECURITY",
        "RETURN COALESCE(current_generation, 1)",
        "SET generation = current_generation + 1",
        "generation,\n      last_business_transaction_id\n    ) VALUES (\n      bound_tenant,\n      2,",
        "AFTER INSERT OR DELETE ON crm.relationships",
    ] {
        assert!(
            GENERATION_UP.contains(fragment),
            "generation migration must retain authoritative fragment: {fragment}"
        );
    }

    for fragment in [
        "FOREIGN KEY (tenant_id, last_business_transaction_id)",
        "REFERENCES crm.business_transactions (tenant_id, business_transaction_id)",
        "DEFERRABLE INITIALLY DEFERRED",
    ] {
        assert!(
            TRANSACTION_UP.contains(fragment),
            "generation transaction lineage must retain fragment: {fragment}"
        );
    }
}

#[test]
fn shared_topology_and_subject_locks_are_fail_fast() {
    assert!(TOPOLOGY_LOCK_UP.contains("pg_try_advisory_xact_lock"));
    assert!(TOPOLOGY_LOCK_UP.contains("ERRCODE = '55P03'"));
    assert!(TOPOLOGY_LOCK_UP.contains("crm.identity-resolution.canonical-redirect|"));

    assert!(SUBJECT_LOCK_UP.contains("pg_try_advisory_xact_lock"));
    assert!(SUBJECT_LOCK_UP.contains("ERRCODE = '55P03'"));
    assert!(SUBJECT_LOCK_UP.contains("crm.customer.subject-lock/v1|%s:%s|%s:%s"));
}

#[test]
fn rollback_restores_the_pre_generation_redirect_guard() {
    for fragment in [
        "CREATE OR REPLACE FUNCTION crm.enforce_identity_resolution_canonical_redirect()",
        "PERFORM pg_advisory_xact_lock",
        "DROP FUNCTION IF EXISTS crm.current_identity_resolution_generation(text)",
        "DROP FUNCTION IF EXISTS crm.lock_customer_subject(text, text)",
        "DROP FUNCTION IF EXISTS crm.lock_identity_resolution_topology(text)",
    ] {
        assert!(
            GENERATION_DOWN.contains(fragment),
            "generation rollback must retain fragment: {fragment}"
        );
    }
}
