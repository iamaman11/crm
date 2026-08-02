from __future__ import annotations

from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


class ProductDevelopmentPlanTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = read("docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md")
        cls.roadmap = read("docs/IMPLEMENTATION_ROADMAP.md")
        cls.phase8 = read("docs/PHASE8_DELIVERY_PLAN.md")
        cls.coverage = read("docs/CRM_CAPABILITY_COVERAGE.md")
        cls.navigation = read("docs/README.md")

    def test_plan_is_normative_and_navigation_is_explicit(self) -> None:
        self.assertIn(
            "Status: **Normative product-portfolio and functional-completeness plan**",
            self.plan,
        )
        for document in (self.roadmap, self.phase8, self.navigation):
            self.assertIn("PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md", document)
        self.assertIn("CRM_CAPABILITY_COVERAGE.md", self.plan)
        self.assertIn("MODULE_CATALOG.md", self.plan)
        self.assertIn("PROJECT_STATUS.md", self.plan)

    def test_complete_product_wave_order_is_bound(self) -> None:
        markers = [
            "## 5. Phase 8A",
            "## 6. Phase 8B",
            "## 7. Phase 8C",
            "## 10. Phase 8D",
            "## 11. Phase 8E",
            "## 12. Phase 8F",
            "## 13. Phase 8G",
            "## 14. Phase 8H",
            "## 15. Phase 9",
            "## 16. Phase 10",
            "## 17. Phase 11",
        ]
        positions = [self.plan.index(marker) for marker in markers]
        self.assertEqual(positions, sorted(positions))

        for phase in (
            "Phase 8A",
            "Phase 8B",
            "Phase 8C",
            "Phase 8D",
            "Phase 8E",
            "Phase 8F",
            "Phase 8G",
            "Phase 8H",
            "Phase 9",
            "Phase 10",
            "Phase 11",
        ):
            self.assertIn(phase, self.roadmap)
            self.assertIn(phase, self.phase8)

    def test_architecture_validation_waves_are_pre_registered(self) -> None:
        for document in (self.plan, self.roadmap, self.phase8):
            self.assertIn("Step 23", document)
            self.assertIn("Catalog", document)
            self.assertIn("Pricing", document)
            self.assertIn("Step 24", document)
            self.assertIn("Quote", document)
            self.assertIn("CPQ", document)

        self.assertIn("effective-dated pricing foundation", self.plan)
        self.assertIn("process-heavy orchestration foundation", self.plan)
        self.assertIn(
            "must not prematurely absorb the full Phase 8C product surface",
            self.plan,
        )

    def test_automation_trigger_condition_and_robot_model_is_complete(self) -> None:
        required = (
            "### 7.1 Trigger model",
            "record created, updated, deleted or restored",
            "lifecycle or pipeline stage entry/exit",
            "inbound verified webhook events",
            "scheduled date/time and recurring schedules",
            "relative timers, inactivity and SLA thresholds",
            "manual user start",
            "API/integration start",
            "ordering, deduplication, re-entry, loop prevention, tenant quotas and replay behavior",
            "### 7.2 Conditions and decision logic",
            "AND/OR/NOT groups",
            "bounded decision tables",
            "explainable rule outcomes",
            "### 7.3 Robot/action catalog",
            "create or update a domain resource through its owner capability",
            "request serial/parallel approval",
            "call a reusable subflow",
            "invoke a governed AI tool",
        )
        for marker in required:
            self.assertIn(marker, self.plan)

    def test_automation_execution_and_visual_authoring_are_durable(self) -> None:
        required = (
            "immutable definition versions",
            "durable execution instances and step history",
            "retries with bounded backoff",
            "idempotency and duplicate-trigger suppression",
            "leases, crash recovery and replay",
            "waits and timers that survive deployment/restart",
            "parallel branches and deterministic joins",
            "dead-letter and operator recovery queues",
            "version-aware migration policy for running instances",
            "### 7.5 Visual workflow studio",
            "drag-and-drop graph editor",
            "simulation using representative/synthetic data",
            "dry-run mode with no side effects",
            "version diff, clone and rollback",
            "run explorer, failure diagnostics and replay controls",
        )
        for marker in required:
            self.assertIn(marker, self.plan)

    def test_programmability_cannot_bypass_governance(self) -> None:
        required = (
            "Automation-first, not automation-bypass",
            "same exact versioned domain capability",
            "no raw database connectivity",
            "no unrestricted network or secret access",
            "declared input/output schemas",
            "deterministic timeout, memory and CPU quotas",
            "signed package/version identity",
            "complete execution audit",
            "kill switch and revocation",
        )
        for marker in required:
            self.assertIn(marker, self.plan)

        self.assertIn(
            "may not create an alternate mutation, authorization, privacy or audit path",
            self.roadmap,
        )
        self.assertIn(
            "may not bypass owner, authorization, privacy, audit or idempotency boundaries",
            self.phase8,
        )

    def test_pipeline_funnel_and_kanban_are_first_class(self) -> None:
        required = (
            "## 8. Pipelines, funnels, Kanban and configurable processes",
            "multiple pipelines per supported owner/resource type",
            "versioned stage definitions and ordering",
            "stage entry/exit criteria",
            "required fields and checklists",
            "allowed transitions and role restrictions",
            "migration of active records when a pipeline version changes",
            "funnel conversion and leakage analytics",
            "drag-and-drop with server-authoritative transition validation",
            "customizable cards and fields",
            "grouping and swimlanes",
            "filters, saved views and sharing",
            "WIP/staleness/SLA indicators",
            "bulk move with preview and partial-failure results",
            "responsive and keyboard-accessible operation",
            "large-pipeline virtualization and stable pagination",
        )
        for marker in required:
            self.assertIn(marker, self.plan)

    def test_cadences_playbooks_and_guided_work_are_explicit(self) -> None:
        required = (
            "## 9. Sales cadences, sequences, playbooks and guided work",
            "manual, rule-based and workflow-based enrollment",
            "email, call, task, meeting, social/manual and wait steps",
            "automatic stop on reply, opt-out, conversion, disqualification or policy condition",
            "quiet hours, locale/time zone and frequency caps",
            "consent and suppression enforcement",
            "owner/team work queues and next-best task ordering",
            "A/B variants and performance analytics",
            "reusable sales/service/success playbooks",
            "AI-assisted drafting/recommendation only through governed policy",
        )
        for marker in required:
            self.assertIn(marker, self.plan)

    def test_broad_crm_product_families_remain_covered(self) -> None:
        plan_markers = (
            "expanded sales and revenue operations",
            "omnichannel service, knowledge and field service",
            "marketing automation and lifecycle journeys",
            "customer success, partners, projects and documents",
            "analytics, data platform, administration and product maturity",
            "AI-native CRM",
            "marketplace and programmable ecosystem",
            "enterprise and vertical proof",
        )
        for marker in plan_markers:
            self.assertIn(marker.lower(), self.plan.lower())

        coverage_sections = (
            "## 4. Sales force automation",
            "## 5. Product, pricing, CPQ and quote-to-revenue",
            "## 6. Customer service and support",
            "## 7. Communications and omnichannel engagement",
            "## 8. Marketing automation and growth",
            "## 14. Workflow, automation and orchestration",
            "## 18. Administration, customization and low-code governance",
            "## 20. AI-native CRM",
            "## 21. Marketplace and ecosystem",
            "## 22. Product experience and delivery surfaces",
        )
        for marker in coverage_sections:
            self.assertIn(marker, self.coverage)

    def test_product_completion_requires_cross_plane_evidence(self) -> None:
        self.assertIn("## 18. Cross-wave product acceptance contract", self.plan)
        for marker in (
            "authoritative owner and mutable aggregates",
            "public/internal capabilities, queries, events and workers",
            "persistence, migrations, rollback and reapply",
            "privacy, consent, retention and deletion interaction",
            "keyboard/accessibility and browser acceptance",
            "import/migration and demo/seed path",
            "observability, SLO, recovery and failure-mode evidence",
            "bounded extension cost without unrelated runtime edits or CI fan-out",
        ):
            self.assertIn(marker, self.plan)

        self.assertIn("## 19. Automation-specific quality targets", self.plan)
        self.assertIn("## 20. Kanban/pipeline quality targets", self.plan)
        self.assertIn(
            "absence of measurable thresholds blocks completion",
            self.plan,
        )

    def test_product_accounting_and_immediate_next_step_are_honest(self) -> None:
        self.assertIn("## 21. Product portfolio accounting", self.plan)
        for state in (
            "Production-complete",
            "Platform-ready",
            "In progress",
            "Planned",
            "Optional/vertical",
            "External integration",
        ):
            self.assertIn(state, self.plan)

        self.assertIn(
            "The current count of product-complete expert modules remains **0**",
            self.plan,
        )
        self.assertIn("## 22. Immediate continuation", self.plan)
        self.assertIn(
            "Repository Step 15 — Party tombstone, no-orphan proof and projection/search/cache convergence",
            self.plan,
        )
        self.assertIn("Repository Step 15 remains the next implementation packet", self.plan)


if __name__ == "__main__":
    unittest.main()
