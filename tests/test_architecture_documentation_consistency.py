from __future__ import annotations

import json
from pathlib import Path
import re
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


class ArchitectureDocumentationConsistencyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.status = read("docs/PROJECT_STATUS.md")
        cls.roadmap = read("docs/IMPLEMENTATION_ROADMAP.md")
        cls.phase8 = read("docs/PHASE8_DELIVERY_PLAN.md")
        cls.plan = read("docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md")
        cls.catalog = read("docs/MODULE_CATALOG.md")
        cls.active_packet = read("docs/ACTIVE_PACKET.md")
        cls.repository_map = read("docs/generated/REPOSITORY_MAP.md")
        cls.packet = json.loads(read("repository-packet.json"))
        cls.workspace = tomllib.loads(read("Cargo.toml"))

    @property
    def normative_documents(self) -> tuple[str, ...]:
        return self.status, self.roadmap, self.phase8, self.plan, self.catalog

    def test_repository_step_14_exact_evidence_is_synchronized(self) -> None:
        required = (
            "PR #259",
            "8aa0b33c6609e74f98363071c6e7c44ec59fc098",
            "2b0b558077c444d44691371c8a2bcca2c14ae426",
            "36 of 36",
        )
        for document in self.normative_documents:
            with self.subTest(document=document[:60]):
                for marker in required:
                    self.assertIn(marker, document)

    def test_current_metrics_are_exact_and_historical_baseline_is_labeled(self) -> None:
        self.assertEqual(len(self.workspace["workspace"]["members"]), 112)

        for document in self.normative_documents:
            with self.subTest(document=document[:60]):
                self.assertIn("112", document)
                self.assertIn("835", document)
                self.assertIn("5,377", document)
                self.assertIn("18", document)
                self.assertIn("270", document)
                self.assertIn("91", document)

        for document in self.normative_documents:
            for match in re.finditer(r"113", document):
                context = document[max(0, match.start() - 120) : match.end() + 120].lower()
                self.assertTrue(
                    "historical" in context or "→ 112" in context or "step 13" in context,
                    f"unqualified current 113-package claim: {context}",
                )

    def test_stage_and_next_packet_status_are_consistent(self) -> None:
        for document in self.normative_documents:
            lowered = document.lower()
            with self.subTest(document=document[:60]):
                self.assertIn("step 14", lowered)
                self.assertIn("stage g", lowered)
                self.assertIn("step 15", lowered)
                self.assertIn("not started", lowered)
                self.assertNotIn("step 14 is the next", lowered)
                self.assertNotIn("step 14 is next", lowered)
                self.assertNotIn("stage g — not started", lowered)
                self.assertNotIn("stage g remains not started", lowered)

        self.assertIn("15. Party tombstone", self.plan)
        self.assertIn("**next, not started**", self.plan)
        self.assertIn("Repository Step 15 is the next permitted implementation packet", self.status)

    def test_product_readiness_is_not_overstated(self) -> None:
        for document in self.normative_documents:
            lowered = document.lower()
            with self.subTest(document=document[:60]):
                self.assertIn("phase 8a", lowered)
                self.assertIn("customer privacy", lowered)
                self.assertIn("0", document)
                self.assertFalse(
                    re.search(r"architecture 10/10 (?:is )?(?:complete|accepted|achieved)", lowered)
                )

        self.assertIn("Phase 8A.11 / issue #126 remains in progress", self.status)
        self.assertIn("Current product-complete expert modules: **0**", self.status)
        self.assertIn("Architecture 10/10 is **not declared**", self.status)

    def test_architecture_stage_ledger_is_complete_and_ordered(self) -> None:
        stages = (
            "A — documentation and policy baseline",
            "B — dependency, crate and exception governance",
            "C — golden owner package and persistence model",
            "D — contribution aggregation",
            "E — affected-scope CI",
            "F — generic conformance and contract lifecycle",
            "G — transitional consolidation",
            "H — reproducible environment and navigation",
            "I — frontend and operations parity",
        )
        for stage in stages:
            self.assertIn(stage, self.plan)

        step_markers = [
            "1. supported Rust toolchain",
            "14. first measured behavior-neutral transitional domain-cluster consolidation",
            "15. Party tombstone, no-orphan proof and projection/search/cache convergence",
            "16. reusable generic worker conformance",
            "17. contract compatibility",
            "18. deterministic local lifecycle commands",
            "19. real Customer Privacy worker lifecycle",
            "20. Phase 8A frontend",
            "21. Phase 8A closure",
            "22. Phase 8A architecture remeasurement",
            "23. first Phase 8B expert-domain wave",
            "24. second contrasting expert-domain wave",
            "25. final architecture 10/10 closure review",
        ]
        positions = [self.plan.index(marker) for marker in step_markers]
        self.assertEqual(positions, sorted(positions))

    def test_step_14_behavior_neutral_guarantees_are_documented(self) -> None:
        for document in self.normative_documents:
            lowered = document.lower()
            with self.subTest(document=document[:60]):
                self.assertIn("crm-customer-accounts-capability-composition", document)
                self.assertIn("crm-customer-accounts-query-adapter", document)
                self.assertIn("behavior-neutral", lowered)

        for marker in (
            "public mutations",
            "queries",
            "workers",
            "contracts",
            "schemas",
            "migrations",
            "tenant isolation",
            "FORCE RLS",
            "authorization",
            "idempotency",
            "audit",
        ):
            self.assertIn(marker.lower(), self.status.lower())

    def test_active_evidence_packet_is_exact_and_bounded(self) -> None:
        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(self.packet["packet_id"], "repository-step-14-exit-evidence-sync")
        self.assertEqual(self.packet["status"], "active")
        self.assertEqual(
            self.packet["baseline"],
            {
                "ref": "main",
                "sha": "2b0b558077c444d44691371c8a2bcca2c14ae426",
            },
        )
        self.assertEqual(
            set(self.packet["allowed_paths"]),
            {
                "docs/ACTIVE_PACKET.md",
                "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
                "docs/IMPLEMENTATION_ROADMAP.md",
                "docs/MODULE_CATALOG.md",
                "docs/PHASE8_DELIVERY_PLAN.md",
                "docs/PROJECT_STATUS.md",
                "repository-packet.json",
                "tests/test_architecture_documentation_consistency.py",
                "tests/test_repository_navigation.py",
            },
        )
        self.assertEqual(
            self.packet["required_checks"],
            ["Affected Scope CI", "Governance CI", "Rust Generated Sync"],
        )
        self.assertIn("start Repository Step 15", self.packet["non_goals"])
        self.assertIn(self.packet["packet_id"], self.active_packet)
        self.assertIn(self.packet["baseline"]["sha"], self.active_packet)

    def test_repository_map_reflects_current_workspace(self) -> None:
        self.assertIn("Generated by scripts/generate_repository_navigation.py", self.repository_map)
        self.assertIn("**Workspace packages:** 112", self.repository_map)
        self.assertRegex(self.repository_map, r"source-digest: sha256:[0-9a-f]{64}")
        for member in self.workspace["workspace"]["members"]:
            self.assertIn(f"`{member}`", self.repository_map)

    def test_phase8_and_catalog_keep_product_boundaries(self) -> None:
        inventory_patterns = (
            r"(?:7|seven) (?:public )?mutations",
            r"(?:4|four) permission-aware public queries",
            r"(?:0|zero) Customer Privacy workers",
        )
        for document in (self.phase8, self.catalog):
            for pattern in inventory_patterns:
                self.assertRegex(document, pattern)
        self.assertIn("Current merged authoritative/coordination module count: **12**", self.catalog)
        self.assertIn("Current merged business-module total: **13**", self.catalog)
        self.assertIn("Phase 8B", self.roadmap)
        self.assertIn("Step 23", self.roadmap)
        self.assertIn("Step 24", self.roadmap)

    def test_final_10_10_boundary_is_mechanical(self) -> None:
        self.assertIn("## 12. Final architecture 10/10 closure criteria", self.plan)
        self.assertIn("Step 22", self.plan)
        self.assertIn("Steps 23 and 24", self.plan)
        self.assertIn("Step 25", self.plan)
        self.assertIn("mechanically proven", self.plan)
        self.assertIn("Issue #194", self.status)
        self.assertIn("Issue #126", self.status)


if __name__ == "__main__":
    unittest.main()
