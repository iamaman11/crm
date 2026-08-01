from __future__ import annotations

import json
from pathlib import Path
import re
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[1]

ALLOWED_PACKET_PATHS = [
    ".github/workflows/complexity-baseline.yml",
    "affected-scope-policy.json",
    "docs/ACTIVE_PACKET.md",
    "docs/MODULE_CATALOG.md",
    "repository-packet.json",
    "scripts/analyze_step13_complexity.py",
    "step13-complexity-policy.json",
    "tests/test_architecture_documentation_consistency.py",
    "tests/test_repository_navigation.py",
    "tests/test_step13_complexity_analysis.py",
]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


class ArchitectureDocumentationConsistencyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.readme = read("README.md")
        cls.agents = read("AGENTS.md")
        cls.docs_index = read("docs/README.md")
        cls.active_packet = read("docs/ACTIVE_PACKET.md")
        cls.repository_map = read("docs/generated/REPOSITORY_MAP.md")
        cls.status = read("docs/PROJECT_STATUS.md")
        cls.roadmap = read("docs/IMPLEMENTATION_ROADMAP.md")
        cls.phase8 = read("docs/PHASE8_DELIVERY_PLAN.md")
        cls.plan = read("docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md")
        cls.catalog = read("docs/MODULE_CATALOG.md")
        cls.workflow = read("docs/DEVELOPMENT_WORKFLOW.md")
        cls.module_development = read("docs/MODULE_DEVELOPMENT.md")
        cls.adr31 = read("docs/adr/ADR-031-step-13-complexity-remeasurement-and-anti-circumvention.md")
        cls.repo_runner = read("scripts/repo.py")
        cls.generator = read("scripts/generate_repository_navigation.py")
        cls.packet = json.loads(read("repository-packet.json"))
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
            for pr in ("PR #218", "PR #220", "PR #222", "PR #224", "PR #226", "PR #230", "PR #235"):
                self.assertIn(pr, document)

        self.assertIn("Phases 0.1–7 are complete", self.status)
        for document in (self.status, self.roadmap, self.phase8):
            self.assertIn("Current product-complete expert modules: **0**", document)

    def test_root_readme_is_orientation_not_a_second_live_roadmap(self) -> None:
        self.assertIn("not duplicated in this orientation file", self.readme)
        self.assertIn("docs/PROJECT_STATUS.md", self.readme)
        self.assertIn("docs/ACTIVE_PACKET.md", self.readme)
        self.assertIn("docs/generated/REPOSITORY_MAP.md", self.readme)
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
            "5. `repo.py explain`, `repo.py packet-check`, generated `docs/ACTIVE_PACKET.md` and generated repository map — **Complete through PR #228**;",
            "6. Customer Privacy legal-hold and mandatory-retention precedence — **Complete through PR #230**;",
            "7. reusable generic mutation and query conformance suites adopted by representative owners — **Complete through PR #235**;",
            "8. replay-safe resumable Customer Privacy owner execution and crash-window recovery — **Complete through PR #237**;",
            "9. affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks — **Complete through PR #239**;",
            "10. governed Customer Privacy access/export assembly — **Complete through PR #241**;",
            "11. owner-specific deletion, anonymization and supported crypto-shred execution — **Complete through PR #244**;",
            "12. complete first-party contribution aggregation for all currently active owners without behavior changes — **Complete through PR #249**;",
            "13. complete calibrated dependency, Rust public-surface, reverse-fan-out and exception governance, including removal of the three direct lint exceptions;",
            "14. first measured behavior-neutral transitional domain-cluster consolidation;",
            "17. contract compatibility, deprecation, consumer-migration and retirement lifecycle enforcement;",
            "18. deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke`;",
            "22. Phase 8A architecture remeasurement, remaining-gate review and publication of the measured Phase 8B extension baseline — **not a final 10/10 declaration**;",
            "23. first Phase 8B expert-domain wave proving bounded extension cost;",
            "24. second contrasting expert-domain wave proving bounded extension cost as module count grows;",
            "25. final architecture 10/10 closure review only when every section 12 criterion is mechanically proven.",
        )
        positions = []
        for step in steps:
            self.assertIn(step, self.plan)
            positions.append(self.plan.index(step))
        self.assertEqual(positions, sorted(positions))

        self.assertIn("## Next permitted repository packet", self.status)
        self.assertIn("## Following permitted repository packet", self.status)
        self.assertIn("## 9. Binding repository continuation", self.phase8)
        self.assertIn(
            "9. repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product checks — **complete through PR #239**;",
            self.phase8,
        )
        self.assertIn(
            "10. repository step 10 — governed access/export assembly — **complete through PR #241**;",
            self.phase8,
        )
        self.assertIn(
            "11. repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — **complete through PR #244**;",
            self.phase8,
        )
        self.assertIn(
            "12. repository step 12 — complete first-party contribution aggregation for all currently active owners without behavior changes — **complete through PR #249**;",
            self.phase8,
        )
        self.assertIn(
            "22. repository step 22 — Phase 8A architecture remeasurement and remaining-gate review, not a final 10/10 declaration;",
            self.phase8,
        )
        self.assertNotIn(
            "10. repository step 10 — governed access/export assembly — **next**;",
            self.phase8,
        )
        self.assertIn(
            "Repository step 13 is the next permitted implementation step and is not started.",
            self.phase8,
        )
        for step in range(1, 6):
            self.assertIn(f"Repository step {step}", self.roadmap)

        for document in self.authoritative_status_documents:
            self.assertNotIn(
                "Repository step 12 remains the current permitted implementation step.",
                document,
            )
        self.assertIn(
            "Latest accepted repository implementation packet is PR #249",
            self.status,
        )
        self.assertIn(
            "Repository step 13 is the current next permitted implementation step and is not started.",
            self.status,
        )
        self.assertIn(
            "Repository step 14 follows only after repository step 13 is accepted and synchronized.",
            self.status,
        )
        self.assertIn("- Stage D is complete:", self.status)
        self.assertIn(
            "-> 12. complete first-party contribution aggregation for all currently active owners — complete through PR #249",
            self.status,
        )
        self.assertNotIn("Stage D is in progress", self.status)
        self.assertNotIn("step 12 batch 1 complete through PR #246", self.status.lower())

        for statement in ("run in parallel", "separate parallel lane", "runs alongside Phase 8A"):
            for document in self.authoritative_status_documents:
                self.assertNotIn(statement, document)

    def test_accepted_repository_evidence_is_synchronized(self) -> None:
        evidence = (
            (
                self.authoritative_status_documents,
                "PR #218",
                "71c88f3e894f1fd943f373d8509e7569cf9aa291",
                "e8fea1645fe108aa8334c40a445299dde8b444f0",
                "30 of 30",
            ),
            (
                self.authoritative_status_documents,
                "PR #220",
                "98000b0c1c2c15e14c7ee0cd2a366020040567e6",
                "01118df3b6349b6d854c4182c17f7eb9a6316b9c",
                "21 of 21",
            ),
            (
                self.privacy_status_documents,
                "PR #222",
                "b5651e784a156758b39eaa04abc1124c7c0832f9",
                "fd86ab1408e435ccc9f47b7a86ab3dd66df64ec1",
                "16 of 16",
            ),
            (
                self.privacy_status_documents,
                "PR #224",
                "e57307fcb1b5192d5e6340247cb6633f32b7ba34",
                "67804d9478b2bbaf342a398b649e23bd5ead6c08",
                "28 of 28",
            ),
            (
                self.privacy_status_documents,
                "PR #226",
                "ad08a691ec759b8b3b523fa66a034cecf4138ff0",
                "a46460623e90c5649d36bedba055fb55023d9349",
                "34 of 34",
            ),
            (
                self.authoritative_status_documents,
                "PR #228",
                "a9aa0bef028d906b61e83803436167bf6f91e634",
                "727a244fcf174dc517dec6fdbb6b8997eb205f14",
                "5 of 5",
            ),
            (
                self.authoritative_status_documents,
                "PR #230",
                "131285e07ad7c36c00e399b65d55591db13f0948",
                "18e6218a7e7495219ac9e8c71cafcda1be64a31b",
                "32 of 32",
            ),
            (
                self.authoritative_status_documents,
                "PR #235",
                "7a0cd34dc17085ecd1a8ee233171c0463d91ceba",
                "43d194231fbce1cee28c44e89726929e450f3d18",
                "17 of 17",
            ),
            (
                self.authoritative_status_documents,
                "PR #237",
                "f926ece93dc2b24683f982828e72bf9170dc123a",
                "9f21a2b40f6af5ce57045fc4c1fbfc1bd6cb5b90",
                "33 of 33",
            ),
            (
                self.authoritative_status_documents,
                "PR #244",
                "405d2dbb97bb371b51cfb1d4ffb5549a57262878",
                "4b08202fe9dd0c0df83567e24e6b9d86fb79c9db",
                "34 of 34",
            ),
            (
                self.authoritative_status_documents,
                "PR #246",
                "3b4fe7cdf458daac9c12f816d0d6a87039e613f3",
                "f090fa8785ac2bbb7e5cf186b4c5011cb9aeb978",
                "37 of 37",
            ),
            (
                self.authoritative_status_documents,
                "PR #248",
                "b15482361ab2b322591d488843ab9b46ff676dba",
                "b4222364c21cb74127834f5ff4f0739343d26379",
                "37 of 37",
            ),
            (
                self.authoritative_status_documents,
                "PR #249",
                "7876945586e5a6cc94f8d3b0f6ba2b57316484d2",
                "f36592211bed3e0df7cf3771164b4bc24026eff3",
                "37 of 37",
            ),
        )
        for documents, pr, source, merge, workflows in evidence:
            self.assert_evidence_in_documents(
                documents,
                pr=pr,
                source=source,
                merge=merge,
                workflows=workflows,
            )

        for document in self.authoritative_status_documents:
            self.assertIn("1.97.1", document)
            self.assertIn("three exact expiring", document.lower())
        for document in self.privacy_status_documents:
            self.assertIn("Customer Accounts", document)
            self.assertIn("restriction", document.lower())

        self.assertIn("customer_privacy.restriction.place@1.0.0", self.status)
        self.assertIn("customer_privacy.restriction.place@1.0.0", self.roadmap)
        self.assertIn("customer_privacy.restriction.place@1.0.0", self.phase8)
        self.assertIn("customer_privacy.restriction.place@1.0.0", self.catalog)
        self.assertIn("7 mutations / 4 queries / 0 workers", self.plan)
        for document in self.authoritative_status_documents:
            self.assertIn("customer_privacy.legal_hold.place@1.0.0", document)
        self.assertIn("workspace remains at 113 packages", self.roadmap.lower())
        self.assertIn("workspace packages remain 113", self.catalog.lower())

    def test_navigation_has_one_stable_human_index(self) -> None:
        self.assertIn("docs/README.md", self.readme)
        self.assertIn("docs/README.md", self.agents)
        self.assertIn("Stable navigation index", self.docs_index)
        self.assertIn("Source-of-truth hierarchy", self.docs_index)
        self.assertIn("Choose by task", self.docs_index)
        self.assertIn("## 6. Generated navigation", self.docs_index)
        self.assertIn("not a source of runtime or delivery truth", self.docs_index)
        self.assertIn("README is stable orientation", self.readme)
        self.assertIn("orientation only", self.plan)
        self.assertIn("navigation outputs, not sources of truth", self.workflow)

    def test_active_packet_is_machine_declared_and_generated(self) -> None:
        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(self.packet["packet_id"], "repository-step-13-current-main-measurement")
        self.assertEqual(self.packet["status"], "active")
        self.assertEqual(self.packet["baseline"]["ref"], "main")
        self.assertEqual(
            self.packet["baseline"]["sha"],
            "222187d988c321aee4d2e7bf81ba01b3205fd14c",
        )
        self.assertEqual(self.packet["tracking_issues"], [194, 126])
        self.assertEqual(self.packet["allowed_paths"], ALLOWED_PACKET_PATHS)
        self.assertEqual(
            self.packet["required_checks"],
            [
                "Affected Scope CI",
                "Complexity Baseline CI",
                "Governance CI",
                "Rust CI",
                "Rust Generated Sync",
            ],
        )
        self.assertIn(
            "the analyzer runs deterministically on the exact pull-request head with full git history",
            self.packet["acceptance"],
        )
        self.assertIn("repository-step-13-current-main-measurement", self.active_packet)
        self.assertIn("222187d988c321aee4d2e7bf81ba01b3205fd14c", self.active_packet)
        for document in (self.plan, self.status):
            self.assertIn("PR #251", document)
            self.assertIn("22e515453e3ed66d0f059bd3c0fe926cee524620", document)
            self.assertIn("be1411136fd36397b22e26737b441351894fdb66", document)
            self.assertIn("5 of 5 applicable permanent workflows", document)
        self.assertNotIn(
            "the next permitted repository packet is **repository step 12",
            self.plan.lower(),
        )
        self.assertIn("## Repository step 13 plan-hardening evidence", self.plan)
        self.assertIn("first bounded repository-step-13 packet is therefore limited to measurement and governance calibration", self.plan)
        self.assertIn("The next permitted packet is repository-step-13 measurement and governance calibration only", self.status)
        self.assertIn("Repository step 14 remains blocked", self.status)
        self.assertIn("Repository step 13 remains the next permitted implementation step", self.adr31)

    def test_stage_accountability_and_live_catalog_are_current(self) -> None:
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

        self.assertIn("repository step 12", self.plan.lower())
        self.assertIn("repository step 22 is a phase 8a architecture measurement checkpoint", self.plan.lower())
        self.assertIn("repository step 25", self.plan.lower())
        self.assertIn("repository step 13 owns the remaining dependency/public-surface/reverse-fan-out calibration", self.status.lower())
        self.assertIn("## Next permitted repository packet\n\nRepository step 13 is the current next permitted implementation step and is not started", self.status)
        self.assertIn("## Following permitted repository packet\n\nRepository step 14 follows only after repository step 13 is accepted and synchronized", self.status)
        self.assertIn("seven mutations, four permission-aware public queries and zero Customer Privacy workers through PR #244", self.catalog)
        self.assertIn("customer_privacy.access_export.request@1.0.0", self.catalog)
        self.assertIn(
            "Repository step 10 is accepted through PR #241 / accepted source `2bb3a671deb18a6ae3bcea228ed01ed287b9de6a` / squash merge `19232f6f3e2ae87aabeb080257c1aac5477a6616` / 34 of 34 applicable permanent workflows on one unchanged exact head.",
            self.status,
        )
        self.assertNotIn("Repository step 10 is accepted through PR #244", self.status)
        self.assertIn("Repository steps 1–12", self.catalog)
        self.assertNotIn("Repository step 11 is the only next implementation packet", self.catalog)

    def test_repository_map_matches_authoritative_inventory(self) -> None:
        members = self.workspace["workspace"]["members"]
        self.assertEqual(len(members), 113)
        self.assertIn("Generated by scripts/generate_repository_navigation.py", self.repository_map)
        self.assertIn("Workspace packages:** 113", self.repository_map)
        self.assertIn("Business manifests:** 14", self.repository_map)
        self.assertIn("Published capability coordinates:** 119", self.repository_map)
        self.assertIn("Published event coordinates:** 70", self.repository_map)
        self.assertIn("Platform runtime routes: 7", self.repository_map)
        self.assertIn("Worker runtime routes: 5", self.repository_map)
        self.assertIn("Non-runtime contract routes: 16", self.repository_map)
        self.assertRegex(self.repository_map, r"sha256:[0-9a-f]{64}")
        self.assertIn("orientation only", self.repository_map)
        for member in members:
            self.assertIn(f"`{member}`", self.repository_map)

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
            "Stage-to-step accountability",
            "Score-recovery accountability",
            "repository step 12 is the bounded completion step",
            "not an absolute ban",
            "two contrasting later expert-domain waves",
            "contract compatibility, deprecation, consumer-migration and retirement lifecycle enforcement",
            "removal of the three direct lint exceptions",
            "step 25",
        )
        plan_lower = self.plan.lower()
        for statement in required:
            self.assertIn(statement.lower(), plan_lower)

    def test_module_and_workflow_guides_match_target_cost_model(self) -> None:
        for document in (self.agents, self.workflow, self.module_development):
            self.assertIn("zero new crates", document.lower())
            self.assertIn("module-owned production contribution", document.lower())
            self.assertIn("python scripts/repo.py explain", document)
            self.assertIn("python scripts/repo.py packet-check", document)
            self.assertIn("generate_repository_navigation.py --check", document)

        self.assertIn("Current scaffold versus 10/10 target", self.module_development)
        self.assertIn("crates/crm-<domain>-application/", self.module_development)
        self.assertIn("crates/crm-<domain>-postgres/", self.module_development)
        self.assertIn("crates/crm-<domain>-production/", self.module_development)
        self.assertIn("python scripts/repo.py test --package crm-sales", self.module_development)
        self.assertNotIn("python scripts/repo.py test crm-sales", self.module_development)

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
            "explain",
            "packet-check",
            "check-affected",
            "quality",
        )
        for command in implemented:
            self.assertIn(f'add_parser(\n        "{command}"', self.repo_runner)

        for command_line in (
            "python scripts/repo.py lock",
            "python scripts/repo.py test --package <package>",
            "python scripts/repo.py test-all",
            "python scripts/repo.py explain <module-or-coordinate>",
            "python scripts/repo.py packet-check --base origin/main",
            "python scripts/generate_repository_navigation.py --check",
        ):
            self.assertIn(command_line, self.readme)

        self.assertIn("generate_repository_navigation.py", self.repo_runner)
        self.assertIn("--check", self.generator)
        self.assertIn("--write", self.generator)
        self.assertIn("tests/test_repository_navigation.py", self.repo_runner)

    def test_future_local_lifecycle_commands_are_not_claimed_as_implemented(self) -> None:
        for command in ("doctor", "bootstrap", "dev-up", "dev-reset", "seed-demo", "smoke"):
            self.assertNotIn(f'add_parser(\n        "{command}"', self.repo_runner)
            self.assertIn(f"python scripts/repo.py {command}", self.docs_index)
        self.assertIn("Planned and required for the 10/10 target", self.docs_index)
        self.assertIn("future repository step 15", self.agents)
        self.assertIn("future repository step 15", self.workflow)
        self.assertIn("future repository step 15", self.module_development)

    def test_generated_outputs_exist_and_are_not_hand_maintained(self) -> None:
        for path in (
            ROOT / "docs/ACTIVE_PACKET.md",
            ROOT / "docs/generated/REPOSITORY_MAP.md",
        ):
            self.assertTrue(path.exists())
            text = path.read_text(encoding="utf-8")
            self.assertIn("do not edit", text.lower())
            self.assertRegex(text, r"source-digest: sha256:[0-9a-f]{64}")

        self.assertIn("--write", self.generator)
        self.assertIn("--check", self.generator)
        self.assertIn("Synchronize generated repository navigation", read(".github/workflows/rust-generated-sync.yml"))

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
            "The current next packet is repository step 1",
            "Repository step 2 — Customer Privacy approval runtime — Next",
            "Repository step 3 — bounded contribution aggregation without behavior change — Next",
            "final customer-subject policy prerequisite is next",
            "Repository step 4 has not started",
            "Restrictions remain unimplemented",
            "no Customer Privacy restriction decision",
            "no restriction behavior yet",
            "4. immediate deny-only Customer Privacy processing restrictions using final subject locks — **Next**",
            "5. `repo.py explain`, `repo.py packet-check`, generated `docs/ACTIVE_PACKET.md` and generated repository map — **Next**",
            "Until `repo.py explain` is implemented",
            "explain` and `packet-check` are planned",
            "explain` and `packet-check` are documented in `docs/README.md` but are not available",
            "future `repo.py explain`",
            "7. reusable generic mutation and query conformance suites adopted by representative owners — **Next**;",
            "Repository step 7 — reusable generic mutation and query conformance — Next",
            "repository step 7 — reusable generic mutation/query conformance — **next**",
            "Repository step 7 is reusable generic mutation and query conformance.",
            "Repository step 11 is the only next implementation packet",
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
                self.workflow,
                self.module_development,
            ):
                self.assertNotIn(statement, document)

    def test_permanent_conformance_runs_this_guard(self) -> None:
        self.assertIn("tests/test_architecture_documentation_consistency.py", self.repo_runner)
        self.assertIn("tests/test_repository_navigation.py", self.repo_runner)
        self.assertIn("scripts/generate_repository_navigation.py", self.repo_runner)


if __name__ == "__main__":
    unittest.main()
