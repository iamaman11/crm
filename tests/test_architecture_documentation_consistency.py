from __future__ import annotations

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


class ArchitectureDocumentationConsistencyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.readme = read("README.md")
        cls.status = read("docs/PROJECT_STATUS.md")
        cls.roadmap = read("docs/IMPLEMENTATION_ROADMAP.md")
        cls.phase8 = read("docs/PHASE8_DELIVERY_PLAN.md")
        cls.plan = read("docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md")
        cls.repo_runner = read("scripts/repo.py")

    def test_current_phase_and_next_packet_are_consistent(self) -> None:
        for document in (self.readme, self.status, self.roadmap, self.phase8):
            self.assertIn("Phase 8A", document)
            self.assertIn("scope discovery and immutable snapshot", document.lower())

        self.assertIn("Phases 0.1–7 are complete", self.readme)
        self.assertIn("Phases 0.1–7 are complete", self.status)
        self.assertIn("Current product-complete expert modules: **0**", self.readme)
        self.assertIn("Current product-complete expert modules: **0**", self.status)
        self.assertIn("Current product-complete expert modules: **0**", self.roadmap)
        self.assertIn("Current product-complete expert modules: **0**", self.phase8)

    def test_architecture_program_is_single_linked_execution_lane(self) -> None:
        for document in (self.readme, self.status, self.roadmap, self.phase8):
            self.assertIn("ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md", document)
            self.assertIn("#194", document)

        self.assertIn(
            "single current execution plan for architecture complexity and developer experience",
            self.plan,
        )
        self.assertIn("Tracking issue: #194", self.plan)

    def test_plan_contains_all_expert_gap_closures(self) -> None:
        required = (
            "normal capability added to an existing owner creates **zero new crates**",
            "three to five technical packages",
            "module-owned production contribution",
            "root `[workspace.dependencies]`",
            "affected-scope CI",
            "Generic conformance",
            "python scripts/repo.py explain",
            "python scripts/repo.py packet-check",
            "`docs/ACTIVE_PACKET.md`",
            "`docs/generated/REPOSITORY_MAP.md`",
            "Transitional crate consolidation",
            "Frontend and operations",
            "restore, SLO, performance, security and supply-chain",
            "feature behavior and crate consolidation must be separate PRs",
            "109 members",
        )
        plan_lower = self.plan.lower()
        for statement in required:
            with self.subTest(statement=statement):
                self.assertIn(statement.lower(), plan_lower)

    def test_phase8_prevents_new_privacy_crate_proliferation(self) -> None:
        self.assertIn(
            "Do not implement discovery/snapshot as one new crate per command",
            self.phase8,
        )
        self.assertIn(
            "perform consolidation only in a separate behavior-neutral PR",
            self.phase8,
        )
        self.assertIn(
            "do not modify generic router or worker algorithms",
            self.roadmap,
        )

    def test_stale_status_claims_are_absent(self) -> None:
        stale = (
            "Phase 6 is complete and Phase 7 is in progress",
            "five privacy owners accepted",
            "next new privacy owner",
            "Customer Enrichment implementation is not started",
        )
        for statement in stale:
            for document in (
                self.readme,
                self.status,
                self.roadmap,
                self.phase8,
                self.plan,
            ):
                with self.subTest(statement=statement):
                    self.assertNotIn(statement, document)

    def test_permanent_conformance_runs_this_guard(self) -> None:
        self.assertIn(
            "tests/test_architecture_documentation_consistency.py",
            self.repo_runner,
        )


if __name__ == "__main__":
    unittest.main()
