from __future__ import annotations

from pathlib import Path
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


class ArchitectureDocumentationConsistencyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.readme = read("README.md")
        cls.agents = read("AGENTS.md")
        cls.docs_index = read("docs/README.md")
        cls.status = read("docs/PROJECT_STATUS.md")
        cls.roadmap = read("docs/IMPLEMENTATION_ROADMAP.md")
        cls.phase8 = read("docs/PHASE8_DELIVERY_PLAN.md")
        cls.plan = read("docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md")
        cls.catalog = read("docs/MODULE_CATALOG.md")
        cls.workflow = read("docs/DEVELOPMENT_WORKFLOW.md")
        cls.module_development = read("docs/MODULE_DEVELOPMENT.md")
        cls.repo_runner = read("scripts/repo.py")
        cls.workspace = tomllib.loads(read("Cargo.toml"))

    @property
    def authoritative_status_documents(self) -> tuple[str, ...]:
        return self.plan, self.status, self.roadmap, self.phase8

    @property
    def privacy_status_documents(self) -> tuple[str, ...]:
        return self.plan, self.status, self.roadmap, self.phase8, self.catalog

    def assert_evidence_in_documents(
        self,
        documents: tuple[str, ...],
        *,
        pr: str,
        source: str,
        merge: str,
        workflows: str,
    ) -> None:
        for document in documents:
            with self.subTest(pr=pr, document=document[:40]):
                self.assertIn(pr, document)
                self.assertIn(source, document)
                self.assertIn(merge, document)
                self.assertIn(workflows, document)

    def test_current_phase_and_next_packet_are_authoritative_and_consistent(self) -> None:
        for document in (self.status, self.roadmap, self.phase8):
            lowered = document.lower()
            self.assertIn("phase 8a", lowered)
            self.assertIn("approval", lowered)
            self.assertIn("permission-aware", lowered)
            self.assertIn("bounded contribution", lowered)
            self.assertIn("final customer-subject policy", lowered)
            self.assertIn("immediate deny-only", lowered)
            self.assertIn("repository step 5", lowered)
            self.assertIn("explain", lowered)
            self.assertIn("packet-check", lowered)
            self.assertIn("generated", lowered)
            for pr in ("PR #218", "PR #220", "PR #222", "PR #224", "PR #226"):
                self.assertIn(pr, document)

        self.assertIn("Phases 0.1–7 are complete", self.status)
        for document in (self.status, self.roadmap, self.phase8):
            self.assertIn("Current product-complete expert modules: **0**", document)

    def test_root_readme_is_orientation_not_a_second_live_roadmap(self) -> None:
        self.assertIn("not duplicated in this orientation file", self.readme)
        self.assertIn("docs/PROJECT_STATUS.md", self.readme)
        self.assertNotIn("The current bounded product packet is", self.readme)
        self.assertNotIn("Phases 0.1–7 are complete", self.readme)
        self.assertNotIn("Current product-complete expert modules", self.readme)

    def test_architecture_program_is_single_linked_execution_lane(self) -> None:
        for document in (self.readme, self.status, self.roadmap, self.phase8):
            self.assertIn("ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md", document)
            self.assertIn("#194", document)

        self.assertIn(
            "single current execution plan for architecture complexity and developer experience",
            self.plan,
        )
        self.assertIn("Tracking issue: #194", self.plan)
        self.assertIn("Current program position", self.plan)
        for stage in (
            "A — documentation and policy baseline",
            "B — dependency, crate and exception governance",
            "C — golden owner package and persistence model",
            "D — contribution aggregation",
            "E — affected-scope CI",
            "F — generic conformance and contract lifecycle",
            "G — transitional consolidation",
            "H — reproducible environment and navigation",
            "I — frontend and operations parity",
        ):
            self.assertIn(stage, self.plan)

    def test_repository_execution_order_is_strict_and_mechanically_visible(self) -> None:
        self.assertIn("### 2.4 Single repository execution order", self.plan)
        self.assertIn("At most one implementation packet may be active", self.plan)
        self.assertIn("The next permitted packet is the first unfinished item", self.plan)
        self.assertIn(
            "No item may be described as “next” when an earlier unfinished item exists",
            self.plan,
        )

        steps = (
            "1. supported Rust toolchain, workspace `rust-version` and measured lint baseline — **Complete through PR #218**;",
            "2. Customer Privacy approval runtime only — **Complete through PR #220**;",
            "3. first bounded contribution-aggregation packet: expand owner-owned first-party registration and reduce selected concrete generic-runtime imports without behavior changes — **Complete through PR #222**;",
            "4. immediate deny-only Customer Privacy processing restrictions using final subject locks — **Complete through PR #226**;",
            "5. `repo.py explain`, `repo.py packet-check`, generated `docs/ACTIVE_PACKET.md` and generated repository map — **Next**;",
        )
        positions = []
        for step in steps:
            self.assertIn(step, self.plan)
            positions.append(self.plan.index(step))
        self.assertEqual(positions, sorted(positions))

        self.assertIn("## Next permitted repository packet", self.status)
        self.assertIn("## Following permitted repository packet", self.status)
        self.assertIn("## 9. Binding repository continuation", self.phase8)
        for step in range(1, 6):
            self.assertIn(f"Repository step {step}", self.roadmap)

        for statement in (
            "run in parallel",
            "separate parallel lane",
            "runs alongside Phase 8A",
        ):
            for document in self.authoritative_status_documents:
                self.assertNotIn(statement, document)

    def test_rust_governance_acceptance_is_synchronized(self) -> None:
        self.assert_evidence_in_documents(
            self.authoritative_status_documents,
            pr="PR #218",
            source="71c88f3e894f1fd943f373d8509e7569cf9aa291",
            merge="e8fea1645fe108aa8334c40a445299dde8b444f0",
            workflows="30 of 30",
        )
        for document in self.authoritative_status_documents:
            self.assertIn("1.97.1", document)
            self.assertIn("three exact expiring", document.lower())

    def test_customer_privacy_approval_acceptance_is_synchronized(self) -> None:
        self.assert_evidence_in_documents(
            self.authoritative_status_documents,
            pr="PR #220",
            source="98000b0c1c2c15e14c7ee0cd2a366020040567e6",
            merge="01118df3b6349b6d854c4182c17f7eb9a6316b9c",
            workflows="21 of 21",
        )
        for document in self.authoritative_status_documents:
            self.assertIn("approval", document.lower())

    def test_repository_step_3_acceptance_is_synchronized(self) -> None:
        self.assert_evidence_in_documents(
            self.privacy_status_documents,
            pr="PR #222",
            source="b5651e784a156758b39eaa04abc1124c7c0832f9",
            merge="fd86ab1408e435ccc9f47b7a86ab3dd66df64ec1",
            workflows="16 of 16",
        )
        for document in self.privacy_status_documents:
            self.assertIn("Customer Accounts", document)
            self.assertIn("first-party", document.lower())

        self.assertIn("6 mutations / 4 queries / 0 workers", self.plan)
        self.assertNotIn("4 mutations / 4 queries / 0 workers", self.plan)
        self.assertIn("workspace remains at 113 packages", self.roadmap.lower())
        self.assertIn("workspace packages remain 113", self.catalog.lower())

    def test_final_customer_subject_policy_prerequisite_is_synchronized(self) -> None:
        self.assert_evidence_in_documents(
            self.privacy_status_documents,
            pr="PR #224",
            source="e57307fcb1b5192d5e6340247cb6633f32b7ba34",
            merge="67804d9478b2bbaf342a398b649e23bd5ead6c08",
            workflows="28 of 28",
        )
        for document in self.privacy_status_documents:
            lowered = document.lower()
            self.assertIn("transaction-scoped", lowered)
            self.assertIn("guard", lowered)

        for document in self.authoritative_status_documents:
            self.assertIn("repository step 4", document.lower())
            self.assertIn("immediate deny-only", document.lower())

    def test_repository_step_4_restriction_acceptance_is_synchronized(self) -> None:
        self.assert_evidence_in_documents(
            self.privacy_status_documents,
            pr="PR #226",
            source="ad08a691ec759b8b3b523fa66a034cecf4138ff0",
            merge="a46460623e90c5649d36bedba055fb55023d9349",
            workflows="34 of 34",
        )
        for document in self.privacy_status_documents:
            lowered = document.lower()
            self.assertIn("restriction", lowered)
            self.assertTrue(
                "protected-owner" in lowered
                or "protected owner" in lowered
                or "final owner guard" in lowered
            )

        for document in self.authoritative_status_documents:
            lowered = document.lower()
            self.assertIn("repository step 5", lowered)
            self.assertIn("packet-check", lowered)
            self.assertIn("generated", lowered)

        self.assertIn("customer_privacy.restriction.place@1.0.0", self.status)
        self.assertIn("customer_privacy.restriction.place@1.0.0", self.roadmap)
        self.assertIn("customer_privacy.restriction.place@1.0.0", self.phase8)
        self.assertIn("customer_privacy.restriction.place@1.0.0", self.catalog)
        self.assertIn("six public mutations", self.status.lower())
        self.assertIn("six public mutations", self.roadmap.lower())
        self.assertIn("six mutations", self.phase8.lower())
        self.assertIn("six mutations", self.catalog.lower())
        self.assertIn("6 mutations / 4 queries / 0 workers", self.plan)

    def test_navigation_has_one_stable_human_index(self) -> None:
        self.assertIn("docs/README.md", self.readme)
        self.assertIn("docs/README.md", self.agents)
        self.assertIn("Stable navigation index", self.docs_index)
        self.assertIn("Source-of-truth hierarchy", self.docs_index)
        self.assertIn("Choose by task", self.docs_index)
        self.assertIn("Generated navigation target", self.docs_index)
        self.assertIn("not a source of runtime or delivery truth", self.docs_index)
        self.assertIn("README is stable orientation", self.readme)
        self.assertIn("orientation only", self.plan)
        self.assertIn("navigation outputs, not sources of truth", self.workflow)

    def test_plan_contains_all_expert_gap_closures(self) -> None:
        required = (
            "normal capability added to an existing owner creates zero new crates",
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
            "Feature behavior and crate consolidation must be separate PRs",
            "113 effective packages",
            "Reproducible local development",
            "Rust public API surface",
            "Contract lifecycle",
            "Persistence and migration ownership",
            "Temporary exceptions",
            "change locality",
            "python scripts/repo.py doctor",
            "python scripts/repo.py bootstrap",
            "python scripts/repo.py dev-up",
            "python scripts/repo.py smoke",
            "approval runtime only",
            "exact Rust `1.97.1`",
            "Single repository execution order",
        )
        plan_lower = self.plan.lower()
        for statement in required:
            self.assertIn(statement.lower(), plan_lower)

    def test_plan_workspace_count_matches_current_manifest(self) -> None:
        members = self.workspace["workspace"]["members"]
        self.assertEqual(len(members), 113)
        self.assertIn(
            f"current accepted workspace contains **{len(members)} effective packages**",
            self.plan,
        )

    def test_module_and_workflow_guides_match_target_cost_model(self) -> None:
        for document in (self.agents, self.workflow, self.module_development):
            self.assertIn("zero new crates", document.lower())
            self.assertIn("module-owned production contribution", document.lower())

        self.assertIn("Current scaffold versus 10/10 target", self.module_development)
        self.assertIn("crates/crm-<domain>-application/", self.module_development)
        self.assertIn("crates/crm-<domain>-postgres/", self.module_development)
        self.assertIn("crates/crm-<domain>-production/", self.module_development)
        self.assertIn(
            "python scripts/repo.py test --package crm-sales",
            self.module_development,
        )
        self.assertNotIn(
            "python scripts/repo.py test crm-sales",
            self.module_development,
        )

    def test_documented_repository_commands_match_implemented_surface(self) -> None:
        implemented = (
            "architecture",
            "manifests",
            "contracts",
            "conformance",
            "format",
            "lock",
            "test",
            "test-all",
            "affected",
            "check-affected",
            "quality",
        )
        for command in implemented:
            self.assertIn(f'add_parser(\n        "{command}"', self.repo_runner)

        for command_line in (
            "python scripts/repo.py lock",
            "python scripts/repo.py test --package <package>",
            "python scripts/repo.py test-all",
        ):
            self.assertIn(command_line, self.readme)
            self.assertIn(command_line, self.docs_index)

    def test_planned_navigation_and_local_commands_are_not_claimed_as_implemented(self) -> None:
        self.assertIn("Planned and required for the 10/10 target", self.docs_index)
        self.assertIn("must not be represented as implemented", self.docs_index)
        self.assertIn("are planned under issue #194", self.agents)
        self.assertIn("are not available until implemented", self.module_development)

        for missing_path in (
            ROOT / "docs/ACTIVE_PACKET.md",
            ROOT / "docs/generated/REPOSITORY_MAP.md",
        ):
            self.assertFalse(missing_path.exists())

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
            "The current bounded product packet is **Customer Privacy scope discovery",
            "The next Phase 8A packet remains scope discovery and immutable snapshot",
            "production discovery remains unimplemented",
            "Customer Privacy public runtime remains four mutations, two queries",
            "The next bounded architecture packet must establish a supported Rust toolchain",
            "Implement the supported Rust toolchain, workspace `rust-version` decision",
            "The current next packet is repository step 1",
            "Repository step 1 — supported Rust toolchain, workspace `rust-version` and measured lint baseline — Next",
            "remains blocked until repository step 1 is accepted",
            "Implement Customer Privacy approval runtime only as repository step 2",
            "Repository step 2 — Customer Privacy approval runtime — Next",
            "Approval runtime is the next packet",
            "The next permitted repository and product packet is **Customer Privacy approval runtime only**",
            "Repository step 3 — bounded contribution aggregation without behavior change — Next",
            "bounded contribution aggregation is the next repository packet",
            "The next permitted implementation packet is repository step 3",
            "final customer-subject policy prerequisite is next",
            "Repository step 4 remains blocked until this prerequisite is accepted",
            "Repository step 4 has not started",
            "Restrictions remain unimplemented",
            "no Customer Privacy restriction decision",
            "no restriction behavior yet",
            "immediate deny-only Customer Privacy processing restrictions using final subject locks remain repository step 4 and the current next packet",
            "4. immediate deny-only Customer Privacy processing restrictions using final subject locks — **Next**",
        )
        for statement in stale:
            for document in (
                self.readme,
                self.agents,
                self.docs_index,
                self.status,
                self.roadmap,
                self.phase8,
                self.plan,
                self.catalog,
            ):
                self.assertNotIn(statement, document)

    def test_permanent_conformance_runs_this_guard(self) -> None:
        self.assertIn(
            "tests/test_architecture_documentation_consistency.py",
            self.repo_runner,
        )


if __name__ == "__main__":
    unittest.main()
