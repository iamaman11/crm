from __future__ import annotations

import json
from pathlib import Path
import re
import tomllib
import unittest

from scripts.check_step22_runtime_fanin_decisions import validate_decisions

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
        cls.operations = read("docs/CUSTOMER_PRIVACY_OPERATIONS_READINESS.md")
        cls.operations_policy = json.loads(
            read("customer-privacy-operations-policy.json")
        )
        cls.affected_scope_policy = json.loads(read("affected-scope-policy.json"))
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

    def test_step_21_is_complete_and_step_22_is_next(self) -> None:
        required = (
            "PR #296",
            "fd84cd25dfa25a75eac0fdc4a719cc76c84cfc95",
            "c21894f47f24e81da1cc150f9ea457fcfdc2bd63",
            "35 of 35",
        )
        for document in self.normative_documents:
            lowered = document.lower()
            for marker in required:
                self.assertIn(marker, document)
            self.assertIn("step 21", lowered)
            self.assertIn("step 22", lowered)
            self.assertNotRegex(
                lowered,
                r"(?:repository )?step 21(?: phase 8a closure)? is (?:not started|in progress|next)\b",
            )
            self.assertNotRegex(document, r"(?m)^\s*-\s*;\s*$")
        self.assertIn("Repository Steps 1–21 are complete", self.status)
        self.assertIn("Phase 8A.11 / issue #126 is complete", self.status)
        self.assertIn("Repository Step 22", self.status)
        self.assertIn(
            "21. Phase 8A closure — **complete through PR #296**;",
            self.plan,
        )
        self.assertIn("9 public mutations", self.phase8)
        self.assertIn("7 permission-aware public queries", self.phase8)
        self.assertIn("Product complete", self.catalog)

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
        self.assertIn("Phase 8A.11 / issue #126 is complete", self.status)
        self.assertIn("Current product-complete expert modules: **1**", self.status)
        self.assertIn("Architecture 10/10 is **not declared**", self.status)
        self.assertIn("Architecture Stages A–I are complete", self.status)
        self.assertIn("issue #194 remains open", self.status)

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

    def test_active_step_22d_customer_360_query_fanin_packet_is_exact(self) -> None:
        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(
            self.packet["packet_id"],
            "repository-step-22d-customer-360-query-fanin-reduction",
        )
        self.assertEqual(
            self.packet["baseline"],
            {"ref": "main", "sha": "9b2495c9a594f5539aa586f6d775a8ea12442a48"},
        )
        self.assertEqual(self.packet["tracking_issues"], [194])
        allowed_paths = set(self.packet["allowed_paths"])
        for path in (
            "Cargo.lock",
            "crates/crm-application-runtime/Cargo.toml",
            "crates/crm-application-runtime/src/bootstrap_visibility/registry.rs",
            "crates/crm-first-party-modules/src/lib.rs",
            "docs/STEP22_CUSTOMER_360_QUERY_FANIN_REDUCTION.md",
            "scripts/check_step22_runtime_fanin_decisions.py",
            "step22-runtime-fanin-decisions.json",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_repository_navigation.py",
            "tests/test_workspace_analysis.py",
        ):
            self.assertIn(path, allowed_paths)
        forbidden_paths = set(self.packet["forbidden_paths"])
        self.assertIn(".github/workflows/**", forbidden_paths)
        self.assertNotIn("Cargo.toml", allowed_paths)
        self.assertNotIn("Cargo.toml", forbidden_paths)
        self.assertIn("step22-architecture-inventory.json", forbidden_paths)
        self.assertIn(self.packet["packet_id"], self.active_packet)
        self.assertIn(self.packet["baseline"]["sha"], self.active_packet)

        counts = validate_decisions(ROOT)
        self.assertEqual(
            counts,
            {
                "all": 63,
                "final": 19,
                "platform_generic": 16,
                "test_only": 1,
                "removed": 2,
                "owner_specific_unavoidable": 0,
                "unresolved": 44,
            },
        )
        non_goals = " ".join(self.packet["non_goals"])
        self.assertIn("classify crm-first-party-modules", non_goals)
        self.assertIn("remediate another crm-application-runtime dependency", non_goals)
        self.assertIn("declare all runtime classifications complete", non_goals)

        operations_scope = next(
            scope
            for scope in self.affected_scope_policy["scopes"]
            if scope["id"] == "operations"
        )
        self.assertEqual(operations_scope["owner"], "platform-operations")
        for path in (
            "scripts/check_step22_runtime_fanin_decisions.py",
            "step22-runtime-fanin-decisions.json",
        ):
            self.assertIn(path, operations_scope["path_patterns"])
        self.assertEqual(operations_scope["required_workflows"], ["Governance CI"])

        step22c = read("docs/STEP22_CUSTOMER_PRIVACY_QUERY_FANIN_REDUCTION.md")
        for marker in ("63", "62", "18", "45", "Customer Privacy"):
            self.assertIn(marker, step22c)

        step22d = read("docs/STEP22_CUSTOMER_360_QUERY_FANIN_REDUCTION.md")
        for marker in (
            "62",
            "61",
            "19",
            "44",
            "crm-application-runtime::dependencies::crm-customer-360-query-adapter",
            "9b2495c9a594f5539aa586f6d775a8ea12442a48",
            "Repository Step 22",
        ):
            self.assertIn(marker, step22d)

    def test_repository_map_and_product_inventory_remain_exact(self) -> None:
        self.assertIn(
            "Generated by scripts/generate_repository_navigation.py",
            self.repository_map,
        )
        self.assertIn("**Workspace packages:** 112", self.repository_map)
        self.assertRegex(self.repository_map, r"source-digest: sha256:[0-9a-f]{64}")
        inventory_patterns = (
            r"(?:9|nine) (?:public )?mutations",
            r"(?:7|seven) permission-aware public queries",
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
