#!/usr/bin/env python3
from pathlib import Path

PLAN = Path("docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md")
STATUS = Path("docs/PROJECT_STATUS.md")
ARCH_TEST = Path("tests/test_architecture_documentation_consistency.py")
NAV_TEST = Path("tests/test_repository_navigation.py")

OLD_ALLOWED = "ALLOWED_PACKET_PATHS = ['docs/ACTIVE_PACKET.md', 'docs/adr/ADR-031-step-13-complexity-remeasurement-and-anti-circumvention.md', 'repository-packet.json', 'scripts/generate_repository_navigation.py', 'tests/test_architecture_documentation_consistency.py', 'tests/test_repository_navigation.py']"
NEW_ALLOWED = "ALLOWED_PACKET_PATHS = ['docs/ACTIVE_PACKET.md', 'docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md', 'docs/PROJECT_STATUS.md', 'repository-packet.json', 'scripts/generate_repository_navigation.py', 'tests/test_architecture_documentation_consistency.py', 'tests/test_repository_navigation.py']"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


def replace_block(text: str, start: str, end: str, replacement: str, label: str) -> str:
    start_index = text.find(start)
    if start_index < 0:
        raise SystemExit(f"{label}: start marker missing")
    end_index = text.find(end, start_index)
    if end_index < 0:
        raise SystemExit(f"{label}: end marker missing")
    return text[:start_index] + replacement + text[end_index:]


plan = PLAN.read_text(encoding="utf-8")
plan = replace_once(
    plan,
    "Current execution checkpoint: **2026-07-31**",
    "Current execution checkpoint: **2026-08-01**",
    "plan checkpoint date",
)
plan = replace_once(
    plan,
    "The next permitted repository packet is **repository step 12: complete first-party contribution aggregation for all currently active owners without behavior changes**. No repository step 13 or later work may begin before step 12 has unchanged exact-head acceptance and evidence synchronization.",
    "Repository steps 1–12 are complete. ADR-031 governs the next permitted repository step: repository step 13 remains not started, and its first bounded packet is current-main 113-package complexity remeasurement plus anti-circumvention governance calibration. Dependency centralization, crate consolidation, public-surface remediation and exception removal may begin only in later bounded step-13 packets after that measurement is accepted and synchronized. Repository step 14 remains blocked.",
    "stale step 12 next-packet claim",
)
plan = replace_once(
    plan,
    "Maximum dependency depth and reverse fan-out\n",
    "Maximum dependency depth and reverse fan-out\nDirect fan-in of composition, infrastructure and process-host packages\nRole classification for high fan-out packages\n",
    "plan metrics fan-in",
)
plan = replace_once(
    plan,
    "Navigation freshness and unresolved explain gaps\n",
    "Navigation freshness and unresolved explain gaps\nManifest-level and source-level policy suppression inventory\nOrdinary-capability and new-owner files/packages/central-touch cost\n",
    "plan metrics suppressions",
)
plan = replace_once(
    plan,
    "- expired architecture exceptions: zero.\n",
    "- expired architecture exceptions: zero;\n- new unregistered manifest-level or source-level suppressions: zero after the accepted inventory baseline;\n- implementation/composition reverse impact and process-host direct owner fan-in do not grow without measured rationale;\n- the first step-13 packet performs measurement and calibration only, not structural remediation.\n",
    "plan budgets",
)
plan_evidence = '''## Repository step 13 plan-hardening evidence

ADR-031 is accepted through PR #251 / accepted source `22e515453e3ed66d0f059bd3c0fe926cee524620` / squash merge `be1411136fd36397b22e26737b441351894fdb66` / 5 of 5 applicable permanent workflows on one unchanged exact head.

The decision closes a governance gap in the earlier step-13 wording. Passing existing repository rules remains necessary but is not sufficient evidence that the rules themselves minimize complexity. The first bounded repository-step-13 packet is therefore limited to measurement and governance calibration:

1. regenerate the exact current-main 113-package dependency, fan-in, fan-out, public-surface, central-LOC, affected-closure and CI-cost baseline;
2. inventory manifest-level and source-level suppressions and equivalent bypass forms, including `#[allow(...)]`, `#![allow(...)]` and `#[expect(...)]`;
3. classify shared boundaries by role so stable SDK/contract fan-out is not confused with mutable implementation/composition fan-out;
4. establish representative ordinary-capability and new-owner change-cost evidence;
5. calibrate warning and blocking budgets before structural remediation.

That first packet MUST NOT centralize dependency features, consolidate crates, remove exceptions, change runtime composition or modify product behavior. Those changes remain later bounded step-13 work and require before/after evidence. Repository step 13 is still **not started** by PR #251 or this documentation synchronization; repository step 14 remains blocked.

'''
plan = replace_once(
    plan,
    "## Repository step 12 completion evidence\n",
    plan_evidence + "## Repository step 12 completion evidence\n",
    "plan ADR evidence section",
)
plan = replace_once(
    plan,
    "Repository step 13 is the **next permitted implementation step** and is **not started**. No later repository step may start before step 13 is accepted and synchronized. This architecture completion does not change Customer Privacy or Phase 8A product readiness; current product-complete expert modules remain **0**.",
    "Repository step 13 is the **next permitted repository step** and is **not started**. Its first packet is the ADR-031 current-main remeasurement and governance-calibration packet described above; structural remediation follows only after that evidence is accepted and synchronized. No later repository step may start before step 13 is complete. Customer Privacy and Phase 8A product readiness remain unchanged; current product-complete expert modules remain **0**.",
    "plan final continuation",
)
PLAN.write_text(plan, encoding="utf-8")

status = STATUS.read_text(encoding="utf-8")
status = replace_once(status, "Status date: 2026-07-31", "Status date: 2026-08-01", "status date")
status = replace_once(
    status,
    "- `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4 for the only permitted repository packet order;\n",
    "- `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4 for the only permitted repository packet order;\n- `adr/ADR-031-step-13-complexity-remeasurement-and-anti-circumvention.md` for the binding step-13 entry and exit evidence;\n",
    "status ADR reference",
)
status = replace_once(
    status,
    "Latest accepted repository implementation packet is PR #249 / accepted source `7876945586e5a6cc94f8d3b0f6ba2b57316484d2` / squash merge `f36592211bed3e0df7cf3771164b4bc24026eff3` / 37 of 37 applicable permanent workflows on one unchanged exact head. Repository step 12 and Stage D are complete; repository step 13 is the next permitted implementation step and is not started.\n",
    "Latest accepted repository implementation packet is PR #249 / accepted source `7876945586e5a6cc94f8d3b0f6ba2b57316484d2` / squash merge `f36592211bed3e0df7cf3771164b4bc24026eff3` / 37 of 37 applicable permanent workflows on one unchanged exact head. Repository step 12 and Stage D are complete; repository step 13 is the next permitted implementation step and is not started.\n\nLatest accepted architecture plan correction is ADR-031 through PR #251 / accepted source `22e515453e3ed66d0f059bd3c0fe926cee524620` / squash merge `be1411136fd36397b22e26737b441351894fdb66` / 5 of 5 applicable permanent workflows on one unchanged exact head. The first step-13 packet performs current-main 113-package measurement and governance calibration only; it does not perform dependency, crate, exception or runtime remediation.\n",
    "status latest architecture correction",
)
status = replace_once(
    status,
    "-> 13. complete dependency/public-surface/reverse-fan-out/exception governance\n",
    "-> 13. current-main 113-package remeasurement and anti-circumvention calibration, then bounded dependency/public-surface/reverse-fan-out/exception remediation\n",
    "status continuation step 13",
)
status_evidence = '''## Repository step 13 plan-hardening evidence

ADR-031 is accepted through PR #251 / accepted source `22e515453e3ed66d0f059bd3c0fe926cee524620` / squash merge `be1411136fd36397b22e26737b441351894fdb66` / 5 of 5 applicable permanent workflows on one unchanged exact head.

The next permitted packet is repository-step-13 measurement and governance calibration only. It must regenerate the exact 113-package complexity baseline, inventory equivalent manifest/source bypass forms, classify central systems by role, measure ordinary-capability and new-owner change cost, and calibrate budgets before any structural remediation. It must not centralize dependency features, consolidate crates, remove exceptions or change runtime/product behavior.

Repository step 13 remains **not started**. Repository step 14 remains blocked. Customer Privacy and Phase 8A remain incomplete.

'''
status = replace_once(
    status,
    "## Repository step 12 completion evidence\n",
    status_evidence + "## Repository step 12 completion evidence\n",
    "status ADR evidence section",
)
status = replace_once(
    status,
    "Repository step 13 is the **next permitted implementation step** and is **not started**. No later repository step may start before step 13 is accepted and synchronized. This architecture completion does not change Customer Privacy or Phase 8A product readiness; current product-complete expert modules remain **0**.",
    "Repository step 13 is the **next permitted repository step** and is **not started**. Its first bounded packet is measurement and governance calibration only; dependency, crate, exception and runtime remediation remain later step-13 work after accepted evidence. Repository step 14 remains blocked. Customer Privacy and Phase 8A product readiness remain unchanged; current product-complete expert modules remain **0**.",
    "status final continuation",
)
STATUS.write_text(status, encoding="utf-8")

arch = ARCH_TEST.read_text(encoding="utf-8")
arch = replace_once(arch, OLD_ALLOWED, NEW_ALLOWED, "architecture allowed paths")
arch_method = '''    def test_active_packet_is_machine_declared_and_generated(self) -> None:
        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(self.packet["packet_id"], "repository-step-13-plan-evidence-sync")
        self.assertEqual(self.packet["status"], "active")
        self.assertEqual(self.packet["baseline"]["ref"], "main")
        self.assertEqual(
            self.packet["baseline"]["sha"],
            "be1411136fd36397b22e26737b441351894fdb66",
        )
        self.assertEqual(self.packet["tracking_issues"], [194, 126])
        self.assertEqual(self.packet["allowed_paths"], ALLOWED_PACKET_PATHS)
        self.assertEqual(
            self.packet["required_checks"],
            [
                "Affected Scope CI",
                "Customer Privacy Access Export CI",
                "Customer Privacy Owner Execution CI",
                "Governance CI",
                "Rust CI",
                "Rust Generated Sync",
            ],
        )
        self.assertIn(
            "the phrase declaring repository step 12 as the next permitted repository packet is absent from live authoritative documents",
            self.packet["acceptance"],
        )
        self.assertIn("repository-step-13-plan-evidence-sync", self.active_packet)
        self.assertIn("be1411136fd36397b22e26737b441351894fdb66", self.active_packet)
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

'''
arch = replace_block(
    arch,
    "    def test_active_packet_is_machine_declared_and_generated(self) -> None:\n",
    "    def test_stage_accountability_and_live_catalog_are_current(self) -> None:\n",
    arch_method,
    "architecture packet method",
)
ARCH_TEST.write_text(arch, encoding="utf-8")

nav = NAV_TEST.read_text(encoding="utf-8")
nav = replace_once(nav, OLD_ALLOWED, NEW_ALLOWED, "navigation allowed paths")
nav_method = '''    def test_active_packet_declaration_is_valid_and_exact(self) -> None:
        packet = load_packet(ROOT)
        self.assertEqual(packet["packet_id"], "repository-step-13-plan-evidence-sync")
        self.assertEqual(packet["status"], "active")
        self.assertEqual(packet["baseline"]["ref"], "main")
        self.assertEqual(
            packet["baseline"]["sha"],
            "be1411136fd36397b22e26737b441351894fdb66",
        )
        self.assertEqual(packet["tracking_issues"], [194, 126])
        self.assertEqual(packet["allowed_paths"], ALLOWED_PACKET_PATHS)
        self.assertEqual(
            packet["required_checks"],
            [
                "Affected Scope CI",
                "Customer Privacy Access Export CI",
                "Customer Privacy Owner Execution CI",
                "Governance CI",
                "Rust CI",
                "Rust Generated Sync",
            ],
        )
        self.assertIn(
            "the live normative plan and project status agree with ADR-031 and accepted PR #251 evidence",
            packet["acceptance"],
        )

'''
nav = replace_block(
    nav,
    "    def test_active_packet_declaration_is_valid_and_exact(self) -> None:\n",
    "    def test_affected_scope_workflow_executes_real_packet_check(self) -> None:\n",
    nav_method,
    "navigation packet method",
)
nav = replace_once(
    nav,
    '                    ("Customer Privacy Owner Execution CI", ".github/workflows/customer-privacy-owner-execution.yml"),\n                    ("Rust CI", ".github/workflows/rust.yml"),\n',
    '                    ("Customer Privacy Owner Execution CI", ".github/workflows/customer-privacy-owner-execution.yml"),\n                    ("Governance CI", ".github/workflows/governance.yml"),\n                    ("Rust CI", ".github/workflows/rust.yml"),\n',
    "navigation workflow fixture",
)
nav = replace_once(
    nav,
    '                    "dfd1478dcfc084cf855fcc409c9b8faec8eaa5cf"\n',
    '                    "be1411136fd36397b22e26737b441351894fdb66"\n',
    "navigation baseline fixture",
)
NAV_TEST.write_text(nav, encoding="utf-8")
