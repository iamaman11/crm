from __future__ import annotations

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SUITE = ROOT / "services/crm-api/tests/support/generic_conformance.rs"
ADOPTION = ROOT / "services/crm-api/tests/generic_conformance_process_e2e.rs"
WORKFLOW = ROOT / ".github/workflows/generic-conformance.yml"
WORKER_SUITE = ROOT / "services/crm-api/tests/support/generic_worker_conformance.rs"
ENRICHMENT_ACTIVATION = (
    ROOT
    / "crates/crm-customer-enrichment-application-composition/tests/postgres_application_worker_process.rs"
)
ENRICHMENT_PRODUCTION = (
    ROOT
    / "crates/crm-application-runtime/tests/postgres_customer_enrichment_application_worker.rs"
)
IMPORT_RECOVERY = ROOT / "services/crm-api/tests/import_process_retryable_e2e.rs"
ENRICHMENT_WORKFLOW = ROOT / ".github/workflows/customer-enrichment-review-process-runtime.yml"
ENRICHMENT_SCRIPT = ROOT / "scripts/run_customer_enrichment_review_process.sh"
IMPORT_WORKFLOW = ROOT / ".github/workflows/import-retryable-process-runtime.yml"


class GenericConformanceAdoptionTests(unittest.TestCase):
    def test_one_business_neutral_suite_is_reused_by_contrasting_owners(self) -> None:
        suite = SUITE.read_text(encoding="utf-8")
        adoption = ADOPTION.read_text(encoding="utf-8")

        self.assertIn("pub struct MutationConformanceSuite", suite)
        self.assertIn("pub struct QueryConformanceSuite", suite)
        self.assertNotIn("customer_privacy", suite)
        self.assertNotIn("customer_enrichment", suite)
        self.assertIn("MutationConformanceSuite::new", adoption)
        self.assertIn("QueryConformanceSuite::new", adoption)
        self.assertIn("customer_enrichment.request.create", adoption)
        self.assertIn("customer_privacy.case.list", adoption)

    def test_permanent_gate_runs_real_process_adoption(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("cargo test -p crm-api --test generic_conformance_process_e2e", workflow)
        self.assertIn("python -m unittest tests.test_generic_conformance_adoption", workflow)
        self.assertIn("postgres:17-alpine", workflow)

    def test_one_business_neutral_worker_suite_is_reused_by_real_workers(self) -> None:
        suite = WORKER_SUITE.read_text(encoding="utf-8")
        activation = ENRICHMENT_ACTIVATION.read_text(encoding="utf-8")
        production = ENRICHMENT_PRODUCTION.read_text(encoding="utf-8")
        import_recovery = IMPORT_RECOVERY.read_text(encoding="utf-8")

        self.assertIn("pub struct WorkerConformanceSuite", suite)
        self.assertIn("assert_retryable_failure_preserves_progress", suite)
        self.assertIn("assert_exact_recovery", suite)
        for owner_term in (
            "customer_enrichment",
            "customer-data-operations",
            "privacy",
            "party",
            "import",
        ):
            self.assertNotIn(owner_term, suite.lower())
        for adoption in (activation, production, import_recovery):
            self.assertIn("generic_worker_conformance.rs", adoption)
            self.assertIn("WorkerConformanceSuite::new", adoption)

        self.assertIn("activation-gated suspended cycle", activation)
        self.assertIn("activation-gated uninstalling cycle", activation)
        self.assertIn("live authorization denial target isolation", production)
        self.assertIn("cross-tenant scan isolation", production)
        self.assertIn("completed replay", production)
        self.assertIn("retryable failure checkpoint preservation", suite)
        self.assertIn("stop crm-api after durable retryable import failure", import_recovery)
        self.assertIn("wait_for_party_contention_waiters(&admin, 2)", import_recovery)
        self.assertIn("wait_event = 'advisory'", import_recovery)
        self.assertIn("WITH RECURSIVE direct_party_waiters", import_recovery)
        self.assertIn("blocking_chain(waiter_pid, blocker_pid)", import_recovery)
        self.assertIn("pg_blocking_pids", import_recovery)
        self.assertIn("count(DISTINCT chain.waiter_pid)", import_recovery)
        self.assertIn("transitive_executor_waiters", import_recovery)
        self.assertIn("competing recovered executor A", import_recovery)
        self.assertIn("competing recovered executor B", import_recovery)
        self.assertIn("post-contention completed replay", import_recovery)

    def test_existing_permanent_gates_execute_worker_representatives(self) -> None:
        enrichment_workflow = ENRICHMENT_WORKFLOW.read_text(encoding="utf-8")
        enrichment_script = ENRICHMENT_SCRIPT.read_text(encoding="utf-8")
        import_workflow = IMPORT_WORKFLOW.read_text(encoding="utf-8")

        self.assertIn("run_customer_enrichment_review_process.sh", enrichment_workflow)
        self.assertIn("postgres_application_worker_process", enrichment_script)
        self.assertIn("postgres_customer_enrichment_application_worker", enrichment_script)
        self.assertIn("import_process_retryable_e2e", import_workflow)
        self.assertIn(
            "crm_api_process_persists_retryable_target_failure_without_advancing_checkpoint_and_recovers",
            import_workflow,
        )
        self.assertNotIn("generic-worker-conformance.yml", enrichment_workflow)


if __name__ == "__main__":
    unittest.main()
