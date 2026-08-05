from __future__ import annotations

from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
SOURCE = "fd84cd25dfa25a75eac0fdc4a719cc76c84cfc95"
MERGE = "c21894f47f24e81da1cc150f9ea457fcfdc2bd63"
EVIDENCE = (
    f"PR #296 / accepted source `{SOURCE}` / squash merge `{MERGE}` / "
    "35 of 35 applicable permanent workflows on one unchanged exact head"
)
NEXT = (
    "Repository Step 22 Phase 8A architecture remeasurement, "
    "`crm-application-runtime` runtime-fan-in decision and permanent-gate "
    "value/cost review is the sole next permitted implementation packet."
)
CLOSURE = f"""

## Accepted Repository Step 21 and Phase 8A closure

{EVIDENCE} completes Repository Step 21, Phase 8A.11 / issue #126 and Phase 8A.

The accepted final Customer Privacy production inventory is exactly **nine public mutations**, **seven permission-aware public queries** and **one first-party owner worker** (`crm.customer-privacy` / `owner-execution`, phase `260`). The accepted lifecycle includes processing-restriction and legal-hold release/read coordinates, optimistic versioning, exact idempotent replay, immutable event/audit/outbox/business-transaction evidence, FORCE-RLS visibility and uniform concealment, clean PostgreSQL rollback/reapply, real `crm-api` process proof and bounded operations search-projection convergence before backup.

Customer Privacy is the first **Product complete** expert module. Current product-complete expert modules: **1**. The broader Universal CRM product remains incomplete, issue #194 remains open and architecture 10/10 is **not declared**. {NEXT}
"""

DOCS = (
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "docs/CUSTOMER_PRIVACY_OPERATIONS_READINESS.md",
    "docs/IMPLEMENTATION_ROADMAP.md",
    "docs/MODULE_CATALOG.md",
    "docs/PHASE8A_CUSTOMER_PRIVACY_PRODUCT_PLANE.md",
    "docs/PHASE8_DELIVERY_PLAN.md",
    "docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md",
    "docs/PROJECT_STATUS.md",
    "docs/WORKSPACE_COMPLEXITY_BASELINE.md",
)


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    (ROOT / path).write_text(text, encoding="utf-8")


def replace_all(text: str, old: str, new: str) -> str:
    return text.replace(old, new)


def replace_required(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        raise RuntimeError(f"missing expected {label}: {old!r}")
    return text.replace(old, new)


def replace_function(text: str, name: str, next_name: str, body: str) -> str:
    pattern = rf"    def {re.escape(name)}\(self\).*?(?=    def {re.escape(next_name)}\(self\))"
    updated, count = re.subn(pattern, body.rstrip() + "\n\n", text, flags=re.S)
    if count != 1:
        raise RuntimeError(f"expected one function {name}, replaced {count}")
    return updated


for path in DOCS:
    text = read(path)
    text = replace_all(text, "Repository Steps 1–20 are complete", "Repository Steps 1–21 are complete")
    text = replace_all(
        text,
        "Repository Step 20 is complete; Repository Step 21 Phase 8A closure is the only next permitted implementation packet.",
        f"Repository Step 21 is complete through {EVIDENCE}. Phase 8A.11 and Phase 8A are complete. {NEXT}",
    )
    text = replace_all(
        text,
        "Repository Step 21 Phase 8A closure is the only next permitted implementation packet.",
        NEXT,
    )
    text = replace_all(
        text,
        "Repository Step 21 Phase 8A closure is the only next permitted implementation packet",
        NEXT.rstrip("."),
    )
    text = replace_all(text, "Phase 8A.11 / issue #126 remains in progress", "Phase 8A.11 / issue #126 is complete")
    text = replace_all(text, "Phase 8A.11 / issue #126 is in progress.", "Phase 8A.11 / issue #126 is complete.")
    text = replace_all(text, "Phase 8A.11 / issue #126 remains **In progress**.", "Phase 8A.11 / issue #126 is **Complete**.")
    text = replace_all(text, "Current product-complete expert modules: **0**", "Current product-complete expert modules: **1**")
    text = replace_all(text, "After Step 21 complete Phase 8A", "Phase 8A is complete; after Step 22 begin Phase 8B")
    if "## Accepted Repository Step 21 and Phase 8A closure" not in text:
        text = text.rstrip() + CLOSURE + "\n"
    write(path, text)

# Architecture plan: close Stages C/I and Step 21 while preserving Step 22 as a checkpoint.
path = "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md"
text = read(path)
text = replace_required(
    text,
    "| C — golden owner package and persistence model | **In progress** | Customer Privacy golden package, final subject policy, restriction/legal-hold placement, retention precedence, durable owner execution/outcomes, governed access/export, owner actions, Party tombstone/no-orphan convergence and accepted owner-worker lifecycle through PR #290 | frontend/operations evidence at Step 20 and Phase 8A closure at Step 21 |",
    f"| C — golden owner package and persistence model | **Complete through PR #296** | Customer Privacy golden package, final subject policy, complete restriction/legal-hold lifecycle, retention precedence, durable owner execution/outcomes, governed access/export, owner actions, Party tombstone/no-orphan convergence, accepted owner worker and {EVIDENCE} | preserve the accepted owner, persistence, privacy and process boundaries |",
    "Stage C row",
)
text = replace_required(
    text,
    "| I — frontend and operations parity | **Incomplete** | existing product/process checks remain mandatory | frontend/accessibility/browser and restore/SLO/performance/security/supply-chain evidence at Steps 20–21 |",
    f"| I — frontend and operations parity | **Complete through PR #296** | accepted product-plane/browser evidence through PR #292, operations evidence through PR #294 and final lifecycle closure through {EVIDENCE} | preserve frontend, accessibility, browser, restore, SLO, observability, security and supply-chain evidence |",
    "Stage I row",
)
text = replace_required(
    text,
    "21. Phase 8A closure;",
    "21. Phase 8A closure — **complete through PR #296**;",
    "Step 21 ledger",
)
write(path, text)

# Phase 8 delivery plan: close the wave and publish final inventory.
path = "docs/PHASE8_DELIVERY_PLAN.md"
text = read(path)
text = replace_required(
    text,
    "The complete functional and completion contract for these waves is normative in `PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md`. Phase 8A remains **In progress**. It must close before Phase 8B implementation begins. Repository Step 22 then remeasures architecture, resolves `crm-application-runtime` fan-in and reviews permanent-gate value/cost; it does not automatically declare 10/10.",
    f"The complete functional and completion contract for these waves is normative in `PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md`. Phase 8A is **Complete through PR #296** with {EVIDENCE}. Repository Step 22 now remeasures architecture, resolves `crm-application-runtime` fan-in and reviews permanent-gate value/cost before Phase 8B implementation; it does not automatically declare 10/10.",
    "Phase 8 current state",
)
text = replace_required(
    text,
    "| 8A.11 / #126 | Customer Privacy and Phase 8A closure | **In progress** |",
    "| 8A.11 / #126 | Customer Privacy and Phase 8A closure | **Complete through PR #296** |",
    "Phase 8A.11 table row",
)
text = replace_required(
    text,
    "Latest accepted public Customer Privacy inventory remains:\n\n- **7 public mutations**;\n- **4 permission-aware public queries**;\n- **1 Customer Privacy owner worker** (`crm.customer-privacy` / `owner-execution`, phase `260`).",
    "Latest accepted public Customer Privacy inventory is final for Phase 8A:\n\n- **9 public mutations**;\n- **7 permission-aware public queries**;\n- **1 Customer Privacy owner worker** (`crm.customer-privacy` / `owner-execution`, phase `260`).",
    "Phase 8 inventory",
)
text = replace_required(
    text,
    "These results do not complete Phase 8A.11. Customer Privacy remains incomplete and Current product-complete expert modules: **1**.",
    f"These results, together with {EVIDENCE}, complete Phase 8A.11 and Phase 8A. Customer Privacy is Product complete and Current product-complete expert modules: **1**.",
    "Phase 8 completeness statement",
)
text = replace_all(text, "## 6. Remaining Phase 8A.11 product work", "## 6. Accepted Phase 8A.11 closure")
write(path, text)

# Product portfolio plan: close Phase 8A and hand off only to Step 22.
path = "docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md"
text = read(path)
text = replace_required(
    text,
    "| Phase 8A | Customer master, identity, data quality, enrichment, consent and privacy lifecycle | Current active program | **In progress** |",
    "| Phase 8A | Customer master, identity, data quality, enrichment, consent and privacy lifecycle | Accepted through Repository Step 21 | **Complete through PR #296** |",
    "product Phase 8A row",
)
text = replace_required(
    text,
    "| Repository Steps 19–21 | Complete remaining Phase 8A product/runtime/UX/operations evidence | Sequential architecture order | **Next through planned** |",
    "| Repository Steps 19–21 | Complete remaining Phase 8A product/runtime/UX/operations evidence | Sequential architecture order | **Complete through PR #296** |",
    "product Steps 19-21 row",
)
text = replace_required(
    text,
    "Phase 8A must still finish:\n\n- a real Customer Privacy worker lifecycle and complete process/end-to-end acceptance;\n- customer privacy frontend/browser/accessibility acceptance;\n- restore, SLO, observability, performance, security and supply-chain evidence.",
    f"Phase 8A is complete through {EVIDENCE}. Accepted evidence includes the real Customer Privacy owner-worker lifecycle, complete process/end-to-end acceptance, frontend/browser/accessibility acceptance, restore, SLO, observability, performance, security, supply-chain proof and the final restriction/legal-hold release/read lifecycle.",
    "product remaining Phase 8A block",
)
write(path, text)

# Module catalog: Customer Privacy becomes the first product-complete expert module.
path = "docs/MODULE_CATALOG.md"
text = read(path)
lines = text.splitlines()
for index, line in enumerate(lines):
    if line.startswith("| `crm.customer-privacy` |"):
        parts = line.split("|")
        if len(parts) < 7:
            raise RuntimeError("unexpected Customer Privacy catalog row")
        parts[3] = " **Product complete** "
        parts[4] = " Complete Phase 8A privacy case, restriction, legal-hold, owner-execution, access/export, product-plane and operations lifecycle with nine mutations, seven queries and one owner worker "
        parts[5] = " Future cross-wave enhancements only; no remaining Phase 8A exit work "
        lines[index] = "|".join(parts)
        break
else:
    raise RuntimeError("Customer Privacy catalog row not found")
text = "\n".join(lines) + ("\n" if text.endswith("\n") else "")
text = replace_required(
    text,
    "Latest accepted public inventory remains **seven mutations and four permission-aware public queries**.",
    "Latest accepted public inventory is **nine mutations and seven permission-aware public queries**.",
    "catalog inventory",
)
write(path, text)

# Project status: close Phase 8A and architecture stages C/I without declaring 10/10.
path = "docs/PROJECT_STATUS.md"
text = read(path)
text = replace_required(
    text,
    "**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is complete. Phase 8A.11 / issue #126 remains in progress.**",
    "**Phases 0.1–7 and Phase 8A are complete. Phase 8A.11 / issue #126 is complete through PR #296. Repository Step 22 is next.**",
    "status headline",
)
text = replace_required(
    text,
    "Architecture Stages A, B, D, E, F, G and H are complete. Stages C and I remain incomplete or in progress according to the architecture plan.",
    "Architecture Stages A–I are complete. Repository Step 22 remains the mandatory architecture remeasurement and decision checkpoint; architecture 10/10 is not declared.",
    "status architecture stages",
)
write(path, text)

# Update permanent documentation guards to the accepted Step 21/Step 22 boundary.
path = "tests/test_architecture_documentation_consistency.py"
text = read(path)
text = replace_function(
    text,
    "test_step_20_is_complete_and_step_21_is_next",
    "test_product_readiness_is_not_overstated",
    '''    def test_step_21_is_complete_and_step_22_is_next(self) -> None:
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
                r"step 21[^\\n.;]{0,100}(?:not started|in progress|\\bnext\\b)",
            )
            self.assertNotRegex(document, r"(?m)^\\s*-\\s*;\\s*$")
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
''',
)
text = replace_function(
    text,
    "test_product_readiness_is_not_overstated",
    "test_architecture_stage_and_step_order_are_complete",
    '''    def test_product_readiness_is_not_overstated(self) -> None:
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
''',
)
text = replace_function(
    text,
    "test_active_step_21_control_lifecycle_packet_is_exact",
    "test_repository_map_and_product_inventory_remain_exact",
    '''    def test_active_step_21_evidence_sync_packet_is_exact(self) -> None:
        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(self.packet["packet_id"], "repository-step-21-evidence-sync")
        self.assertEqual(
            self.packet["baseline"],
            {"ref": "main", "sha": "c21894f47f24e81da1cc150f9ea457fcfdc2bd63"},
        )
        self.assertEqual(self.packet["tracking_issues"], [194, 126, 28])
        self.assertEqual(
            self.packet["required_checks"],
            [
                "Affected Scope CI",
                "Complexity Baseline CI",
                "Customer Privacy Access Export CI",
                "Customer Privacy Hold Retention CI",
                "Customer Privacy Operations CI",
                "Customer Privacy Owner Execution CI",
                "Customer Privacy Restriction Policy CI",
                "Governance CI",
                "Rust Generated Sync",
                "Rust CI",
            ],
        )
        self.assertIn("docs/PROJECT_STATUS.md", self.packet["allowed_paths"])
        self.assertIn("docs/MODULE_CATALOG.md", self.packet["allowed_paths"])
        self.assertIn(self.packet["packet_id"], self.active_packet)
        self.assertIn(self.packet["baseline"]["sha"], self.active_packet)
        for marker in (
            "PR #296",
            "fd84cd25dfa25a75eac0fdc4a719cc76c84cfc95",
            "c21894f47f24e81da1cc150f9ea457fcfdc2bd63",
            "35 of 35",
        ):
            self.assertIn(marker, self.status)
''',
)
text = text.replace(r'r"(?:7|seven) (?:public )?mutations"', r'r"(?:9|nine) (?:public )?mutations"')
text = text.replace(r'r"(?:4|four) permission-aware public queries"', r'r"(?:7|seven) permission-aware public queries"')
write(path, text)

path = "tests/test_repository_navigation.py"
text = read(path)
text = replace_function(
    text,
    "test_active_step_21_control_lifecycle_packet_declaration_is_exact",
    "test_generated_navigation_is_deterministic_and_current",
    '''    def test_active_step_21_evidence_sync_packet_declaration_is_exact(self) -> None:
        packet = load_packet(ROOT)
        self.assertEqual(packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(packet["packet_id"], "repository-step-21-evidence-sync")
        self.assertEqual(packet["status"], "active")
        self.assertEqual(
            packet["baseline"],
            {"ref": "main", "sha": "c21894f47f24e81da1cc150f9ea457fcfdc2bd63"},
        )
        self.assertEqual(packet["tracking_issues"], [194, 126, 28])
        self.assertTrue(
            {
                "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
                "docs/IMPLEMENTATION_ROADMAP.md",
                "docs/MODULE_CATALOG.md",
                "docs/PHASE8_DELIVERY_PLAN.md",
                "docs/PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md",
                "docs/PROJECT_STATUS.md",
                "tests/test_architecture_documentation_consistency.py",
                "tests/test_repository_navigation.py",
            }.issubset(set(packet["allowed_paths"]))
        )
        self.assertIn("PR #296", " ".join(packet["deliverables"]))
        self.assertIn("Repository Step 22", " ".join(packet["deliverables"]))
''',
)
text = text.replace(
    '"repository-step-21-customer-privacy-control-lifecycle",\n            first[ACTIVE_PACKET_PATH],',
    '"repository-step-21-evidence-sync", first[ACTIVE_PACKET_PATH],',
)
text = text.replace(
    '"767b12b20f311088a8487446bd9ee6413fb9ac7c", first[ACTIVE_PACKET_PATH]',
    '"c21894f47f24e81da1cc150f9ea457fcfdc2bd63", first[ACTIVE_PACKET_PATH]',
)
text = text.replace(
    'return_value="767b12b20f311088a8487446bd9ee6413fb9ac7c",',
    'return_value="c21894f47f24e81da1cc150f9ea457fcfdc2bd63",',
)
text = text.replace('"Step 21 Customer Privacy control lifecycle"', '"Step 21 accepted evidence synchronization"')
write(path, text)

print("Materialized accepted Repository Step 21 and Phase 8A evidence.")
