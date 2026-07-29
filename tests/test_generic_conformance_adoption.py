from __future__ import annotations

import json
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
SUITE = ROOT / "services/crm-api/tests/support/generic_conformance.rs"
ADOPTION = ROOT / "services/crm-api/tests/generic_conformance_process_e2e.rs"
WORKFLOW = ROOT / ".github/workflows/generic-conformance.yml"
PACKET = ROOT / "repository-packet.json"


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

    def test_packet_preserves_package_and_dependency_budgets(self) -> None:
        packet = json.loads(PACKET.read_text(encoding="utf-8"))
        self.assertEqual(packet["packet_id"], "repository-step-7")
        self.assertEqual(
            packet["baseline"]["sha"],
            "b053d9bc1c74a7a238aec4698f7f513a1834fefc",
        )
        self.assertNotIn("Cargo.toml", packet["allowed_paths"])
        self.assertNotIn("Cargo.lock", packet["allowed_paths"])
        self.assertIn("Cargo.toml", packet["forbidden_paths"])
        self.assertIn("Cargo.lock", packet["forbidden_paths"])
        self.assertIn(
            "tests/test_architecture_documentation_consistency.py",
            packet["allowed_paths"],
        )
        self.assertIn("tests/test_repository_navigation.py", packet["allowed_paths"])


if __name__ == "__main__":
    unittest.main()
