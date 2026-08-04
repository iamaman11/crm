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
        cls.complexity = read("docs/WORKSPACE_COMPLEXITY_BASELINE.md")
        cls.catalog = read("docs/MODULE_CATALOG.md")
        cls.product_plan = read("docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md")
        cls.product_plane = read("docs/PHASE8A_CUSTOMER_PRIVACY_PRODUCT_PLANE.md")
        cls.delivery = read("docs/DELIVERY_GOVERNANCE.md")
        cls.adr32 = read(
            "docs/adr/ADR-032-step-22-runtime-fanin-and-permanent-gate-value.md"
        )
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
            "2b0b558077c444d4469137c8a2bcca2c14ae426",
            "36 of 36",
        )
        for document in self.normative_documents:
            for marker in required:
                self.assertIn(marker, document)

    def test_repository_step_15_exact_evidence_is_synchronized(self) -> None:
        required = (
            "PR #263",
            "6c2a54f6780988a12fec3cd77ca2cd39ad349140",
            "bd205e0af77b676654dff8ddf26d3b5b195880b2",
            "PR #264",
            "e6c9d2901109c8d5b9e0f3cf783214407e26451a",
            "e9fe1f352386d80a29d122db5d1ed6c47266bfaf",
            "PR #265",
            "ef572bdf31c584c397c215cd1b62ee47cad54e64",
            "2a0ee9c33fbe23cdf9c6dccb1acc7e3bd8bcba3a",
            "PR #266",
            "ded5d80ae11bbf044b5bfe5b572e8dab521f884a",
            "1f889a810c82da3d0fee12427eacccbe43613bac",
            "PR #267",
            "f1b72dbee09f152005cb3584b9bcc1573bf2c4fe",
            "4a14a5a0bda6d25d27e7c2da7c5f4809fa1efbdf",
            "19 of 19",
        )
        for document in self.normative_documents:
            with self.subTest(document=document[:60]):
                for marker in required:
                    self.assertIn(marker, document)

    def test_repository_step_16_exact_evidence_is_synchronized(self) -> None:
        required = (
            "PR #269",
            "74b1d7b0f8764fcd90839b7aab25f8f82fe5e552",
            "6f82de0a7b2dcd1ab5dd0ae6473d46d7d9d34bdd",
            "20 of 20",
            "PR #270",
            "8e2baac0822eefbb6d3c474ffce0cee69e3e4e98",
            "ce0ca881461d1ee8964a11b28c1fcff46cf145cb",
            "17 of 17",
        )
        for document in self.normative_documents:
            with self.subTest(document=document[:60]):
                for marker in required:
                    self.assertIn(marker, document)

    def test_current_metrics_are_exact_and_historical_baseline_is_labeled(self) -> None:
        self.assertEqual(len(self.workspace["workspace"]["members"]), 112)
        for document in self.normative_documents:
            for marker in ("112", "835", "5,377", "18", "270", "91"):
                self.assertIn(marker, document)
            for match in re.finditer(r"\b113\b", document):
                context = document[
                    max(0, match.start() - 120) : match.end() + 120
                ].lower()
                self.assertTrue(
                    "historical" in context
                    or "→ 112" in context
                    or "step 13" in context,
                    f"unqualified current 113-package claim: {context}",
                )

    def test_step_19_is_complete_step_20a_is_accepted_and_step_20b_is_next(
        self,
    ) -> None:
        required = (
            "PR #287",
            "23b2f4ea660bcd46884fe054cd0c37e89b1495c4",
            "c0fec3ae08c836ab483737442ed4377c99c85e9a",
            "11 of 11",
            "PR #288",
            "b99a4fc4ff7ef5cfd47813b487900b4c2a9f3b77",
            "bc653de5f1a853791d3ab4a03f59f3daad54bf54",
            "24 of 24",
            "PR #289",
            "3e21e79e1600727ebcda222af389d568d857cff8",
            "d1c4dd278853a1e6a426fab284c70b3529d42833",
            "PR #290",
            "9bbb339f39133955a7f42ea67f3334e597066e2e",
            "49c5e35814adceb2be9d4cc2302bf10032b807a0",
            "19 of 19",
        )
        for document in self.normative_documents:
            lowered = document.lower()
            for marker in required:
                self.assertIn(marker, document)
            self.assertIn("step 19", lowered)
            self.assertIn("step 20", lowered)
            self.assertNotRegex(
                lowered,
                r"step 19[^\n.;]{0,100}(?:not started|in progress|next)",
            )
            self.assertNotRegex(document, r"(?m)^\s*-\s*;\s*$")
        self.assertIn("Repository Steps 1–19 are complete", self.status)
        self.assertIn("Repository Step 20A is accepted through PR #292", self.status)
        self.assertIn("Repository Step 20 remains in progress", self.status)
        self.assertIn("Repository Step 20B", self.status)
        self.assertIn(
            "19. real Customer Privacy worker lifecycle and complete process/end-to-end acceptance — **complete through PR #290**",
            self.plan,
        )
        self.assertIn("20. Phase 8A frontend", self.plan)
        for marker in required:
            self.assertIn(marker, self.product_plan)
        self.assertIn("After Steps 20–21 complete Phase 8A", self.product_plan)
        self.assertNotIn("After Steps 19–21 complete Phase 8A", self.product_plan)
        self.assertIn(
            "Repository Step 20A is accepted through PR #292", self.product_plan
        )
        self.assertIn("Repository Step 20B", self.product_plan)
        self.assertNotIn(
            "> Repository Step 19 — real Customer Privacy worker lifecycle",
            self.product_plan,
        )

    def test_product_readiness_is_not_overstated(self) -> None:
        for document in self.normative_documents:
            lowered = document.lower()
            self.assertIn("phase 8a", lowered)
            self.assertIn("customer privacy", lowered)
            self.assertFalse(
                re.search(
                    r"architecture 10/10 (?:is )?(?:complete|accepted|achieved)",
                    lowered,
                )
            )
        self.assertIn("Phase 8A.11 / issue #126 remains in progress", self.status)
        self.assertIn("Current product-complete expert modules: **0**", self.status)
        self.assertIn("Architecture 10/10 is **not declared**", self.status)
        self.assertIn("Stages C and I remain incomplete or in progress", self.status)
        self.assertNotIn("Stages C, F and I", self.status)

    def test_architecture_stage_and_step_order_are_complete(self) -> None:
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

    def test_step22_decisions_remain_binding(self) -> None:
        documents = (self.plan, self.roadmap, self.phase8, self.status, self.adr32)
        for document in documents:
            lowered = document.lower()
            for marker in (
                "crm-application-runtime",
                "owner-specific",
                "process-composition",
                "non-growth",
            ):
                self.assertIn(marker.lower(), lowered)
        for classification in (
            "removed",
            "platform-generic",
            "owner-specific-unavoidable",
            "test-only",
        ):
            self.assertIn(classification, self.adr32)
            self.assertIn(classification, self.plan)
        for disposition in ("retain", "simplify", "merge", "remove"):
            self.assertIn(disposition, self.adr32)
            self.assertIn(disposition, self.delivery)
        self.assertIn(
            "zero unresolved runtime-fan-in or gate-value decisions", self.plan
        )

    def test_active_step_20a_evidence_sync_packet_is_exact(self) -> None:
        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(self.packet["packet_id"], "repository-step-20a-evidence-sync")
        self.assertEqual(self.packet["status"], "active")
        self.assertEqual(
            self.packet["baseline"],
            {"ref": "main", "sha": "fffd6baf35544eea736d183af0a5ba38518cce9a"},
        )
        self.assertEqual(self.packet["tracking_issues"], [194, 126])
        self.assertEqual(
            set(self.packet["allowed_paths"]),
            {
                "docs/ACTIVE_PACKET.md",
                "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
                "docs/IMPLEMENTATION_ROADMAP.md",
                "docs/MODULE_CATALOG.md",
                "docs/PHASE8A_CUSTOMER_PRIVACY_PRODUCT_PLANE.md",
                "docs/PHASE8_DELIVERY_PLAN.md",
                "docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md",
                "docs/PROJECT_STATUS.md",
                "docs/WORKSPACE_COMPLEXITY_BASELINE.md",
                "repository-packet.json",
                "tests/test_architecture_documentation_consistency.py",
                "tests/test_repository_navigation.py",
            },
        )
        self.assertEqual(
            self.packet["required_checks"],
            [
                "Affected Scope CI",
                "Complexity Baseline CI",
                "Governance CI",
                "Product Plane CI",
                "Rust Generated Sync",
                "Rust CI",
            ],
        )
        self.assertIn(self.packet["packet_id"], self.active_packet)
        self.assertIn(self.packet["baseline"]["sha"], self.active_packet)
        self.assertIn(
            "record PR #292 source 938cebed1e78bf7debf40dc544431bfe819970f4 squash merge fffd6baf35544eea736d183af0a5ba38518cce9a and 17 of 17 applicable permanent workflows in every live normative source",
            self.packet["deliverables"],
        )
        self.assertIn(
            "start or implement Repository Step 20B operations work",
            self.packet["non_goals"],
        )
        evidence_documents = (
            self.status,
            self.roadmap,
            self.phase8,
            self.plan,
            self.catalog,
            self.product_plan,
            self.product_plane,
            self.complexity,
        )
        for document in evidence_documents:
            for marker in (
                "PR #292",
                "938cebed1e78bf7debf40dc544431bfe819970f4",
                "fffd6baf35544eea736d183af0a5ba38518cce9a",
                "17 of 17",
                "Step 20B",
            ):
                self.assertIn(marker, document)
            lowered = document.lower()
            self.assertNotRegex(
                lowered, r"repository step 20 is (?:the )?(?:only )?next"
            )
            self.assertNotRegex(lowered, r"step 20 is next")
            self.assertNotIn(
                "next permitted bounded implementation packet is repository step 20",
                lowered,
            )

            for line in document.splitlines():
                lowered_line = line.lower()
                self.assertFalse(
                    "step 20" in lowered_line
                    and "next permitted" in lowered_line
                    and "step 20b" not in lowered_line,
                    line,
                )
        self.assertIn("real PostgreSQL", self.product_plane)
        self.assertIn("assembled `crm-api`", self.product_plane)
        self.assertIn("Chromium acceptance", self.product_plane)
        self.assertIn("Repository Step 20 remains in progress", self.product_plane)
        self.assertNotIn("No Step 20 or later implementation may begin", self.status)
        self.assertIn("Repository Step 20B may begin only after", self.status)
        self.assertIn("Stage I — in progress", self.status)
        self.assertIn(
            "Stage F generic conformance and contract lifecycle — **Complete through PR #290**",
            self.roadmap,
        )
        self.assertNotIn(
            "frontend and operations parity remain future work", self.status
        )
        self.assertNotIn(
            "- frontend, accessibility and browser acceptance;", self.roadmap
        )
        self.assertNotIn("after accepted Repository Step 19", self.roadmap)

    def test_repository_map_and_product_inventory_remain_exact(self) -> None:
        self.assertIn(
            "Generated by scripts/generate_repository_navigation.py",
            self.repository_map,
        )
        self.assertIn("**Workspace packages:** 112", self.repository_map)
        self.assertRegex(self.repository_map, r"source-digest: sha256:[0-9a-f]{64}")
        inventory_patterns = (
            r"(?:7|seven) (?:public )?mutations",
            r"(?:4|four) permission-aware public queries",
            r"(?:1|one) Customer Privacy owner worker",
        )
        for document in (self.phase8, self.catalog):
            for pattern in inventory_patterns:
                self.assertRegex(document, pattern)
        self.assertIn("`crm.customer-privacy` / `owner-execution`", self.catalog)
        self.assertIn("phase `260`", self.catalog)
        self.assertIn(
            "Current merged authoritative/coordination module count: **12**",
            self.catalog,
        )
        self.assertIn("Current merged business-module total: **13**", self.catalog)


if __name__ == "__main__":
    unittest.main()
