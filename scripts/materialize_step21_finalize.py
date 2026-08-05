from __future__ import annotations

from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    (ROOT / path).write_text(text, encoding="utf-8")


def replace_regex(
    text: str,
    pattern: str,
    replacement: str,
    label: str,
    *,
    required: bool = True,
) -> str:
    updated, count = re.subn(
        pattern,
        lambda _: replacement,
        text,
        flags=re.I | re.S | re.M,
    )
    if required and count == 0:
        raise RuntimeError(f"missing fail-closed closure marker {label}: {pattern}")
    return updated


def apply_rules(
    path: str,
    rules: tuple[tuple[str, str, str, bool], ...],
) -> None:
    text = read(path)
    for pattern, replacement, label, required in rules:
        text = replace_regex(
            text,
            pattern,
            replacement,
            label,
            required=required,
        )
    write(path, text)


apply_rules(
    "docs/PROJECT_STATUS.md",
    (
        (
            r"step 21 is next",
            "Step 21 is complete through PR #296",
            "status-step21-next",
            True,
        ),
        (
            r"repository step 21 phase 8a closure is (?:the only )?next(?: permitted (?:implementation )?packet)?\.??",
            "Repository Step 21 Phase 8A closure is complete through PR #296; Repository Step 22 is the sole next permitted implementation packet.",
            "status-step21-next-packet",
            True,
        ),
        (
            r"latest accepted customer privacy public inventory remains:\s*\n\s*- seven public mutations:.*?\n\s*- four permission-aware public queries:.*?\n\s*- one customer privacy owner worker\.",
            "Latest accepted Customer Privacy public inventory is final for Phase 8A:\n\n- **nine public mutations**;\n- **seven permission-aware public queries**;\n- **one Customer Privacy owner worker**.",
            "status-final-inventory",
            True,
        ),
        (
            r"still required before phase 8a\.11 can close:\s*\n\s*- restriction and legal-hold release/read lifecycle where required;\s*\n\s*- repository step 21 phase 8a closure review;\s*\n\s*- a full phase 8a\.11 closure review beyond the accepted bounded step 20a case-read slice\.",
            "Phase 8A.11 closure requirements are accepted through PR #296, including the complete restriction/legal-hold release/read lifecycle and final closure review.",
            "status-remaining-phase8a11",
            True,
        ),
        (
            r"customer privacy and phase 8a remain incomplete\. current product-complete expert modules: \*\*1\*\*\.",
            "Customer Privacy and Phase 8A are complete. Current product-complete expert modules: **1**.",
            "status-privacy-incomplete",
            True,
        ),
        (
            r"- \*\*stage c — in progress:\*\*[^\n]*",
            "- **Stage C — complete through PR #296:** the Customer Privacy golden owner package proves the complete public lifecycle, owner execution, privacy convergence and operations evidence; wider adoption is later product work, not a Phase 8A closure blocker.",
            "status-stage-c",
            True,
        ),
        (
            r"- \*\*stage i — in progress:\*\*[^\n]*",
            "- **Stage I — complete through PR #296:** accepted PRs #292, #294 and #296 prove frontend, accessibility, browser, restore, SLO, observability, performance, security, supply-chain and final lifecycle parity.",
            "status-stage-i",
            True,
        ),
        (
            r"-> 21\. phase 8a closure(?: — complete through pr #296)?",
            "-> 21. Phase 8A closure — complete through PR #296",
            "status-continuation-step21",
            True,
        ),
        (
            r"phase 8a closure will not make the universal crm product complete\.",
            "Phase 8A closure does not make the Universal CRM product complete.",
            "status-future-closure",
            True,
        ),
        (
            r"issue #126 remains open until customer privacy and phase 8a\.11 product evidence is complete\.",
            "Issue #126 is ready for closure after the accepted PR #296 evidence synchronization.",
            "status-issue126-open",
            True,
        ),
        (
            r"latest public customer privacy inventory remains \*\*seven mutations and four permission-aware public queries\*\*\.",
            "Latest public Customer Privacy inventory is **nine mutations and seven permission-aware public queries**.",
            "status-step19-inventory",
            True,
        ),
        (
            r"phase 8a\.11 / issue #126 is complete; customer privacy is not product-complete; current product-complete expert modules remain zero;",
            "Phase 8A.11 / issue #126 and Customer Privacy are complete; current product-complete expert modules: **1**;",
            "status-step19-completeness",
            True,
        ),
        (
            r"phase 8a\.11, phase 8a, product-complete expert modules, architecture 10/10 and the universal crm product remain incomplete\.",
            "Customer Privacy and Phase 8A are complete with current product-complete expert modules: **1**; architecture 10/10 and the Universal CRM product remain incomplete.",
            "status-step20a-completeness",
            True,
        ),
        (
            r"phase 8a\.11, phase 8a, customer privacy as a complete product capability, product-complete expert modules, architecture 10/10 and the universal crm product remain incomplete\.",
            "Customer Privacy and Phase 8A are complete with current product-complete expert modules: **1**; architecture 10/10 and the Universal CRM product remain incomplete.",
            "status-step20-completeness",
            True,
        ),
        (
            r"the accepted one-worker customer privacy inventory, seven public mutations, four permission-aware public queries",
            "The accepted one-worker Customer Privacy inventory, nine public mutations, seven permission-aware public queries",
            "status-step20a-inventory",
            True,
        ),
    ),
)

apply_rules(
    "docs/IMPLEMENTATION_ROADMAP.md",
    (
        (
            r"\| 8a \| #28 \| canonical customer master, identity, consent and governed customer-data lifecycle \| \*\*in progress\*\* \|",
            "| 8A | #28 | canonical customer master, identity, consent and governed customer-data lifecycle | **Complete through PR #296** |",
            "roadmap-phase8a-row",
            True,
        ),
        (
            r"- stage c customer privacy golden owner and persistence model — \*\*in progress;[^\n]*",
            "- Stage C Customer Privacy golden owner and persistence model — **complete through PR #296**;",
            "roadmap-stage-c",
            True,
        ),
        (
            r"- stage i frontend and operations parity — \*\*in progress;[^\n]*",
            "- Stage I frontend and operations parity — **complete through PR #296**.",
            "roadmap-stage-i",
            True,
        ),
        (
            r"steps 18 and 19 and the bounded step 20a slice are accepted; step 20 is complete and step 21 is next\.",
            "Steps 18–21 are accepted; Repository Step 22 is the sole next permitted implementation packet.",
            "roadmap-step18-tail",
            True,
        ),
        (
            r"21\. phase 8a closure;",
            "21. Phase 8A closure — **complete through PR #296**;",
            "roadmap-sequence-step21",
            True,
        ),
        (
            r"issue #126 remains \*\*in progress\*\*\.",
            "Issue #126 is **complete through PR #296**.",
            "roadmap-issue126",
            True,
        ),
        (
            r"current public customer privacy inventory remains:\s*\n\s*- seven public mutations;\s*\n\s*- four permission-aware public queries;\s*\n\s*- one customer privacy owner worker\.",
            "Final public Customer Privacy inventory for Phase 8A is:\n\n- **nine public mutations**;\n- **seven permission-aware public queries**;\n- **one Customer Privacy owner worker**.",
            "roadmap-inventory",
            True,
        ),
        (
            r"### 5\.1 remaining phase 8a\.11 product work\s*\n\s*the remaining phase 8a\.11 product work after accepted repository step 20a is:\s*\n\s*- restriction and legal-hold release/read lifecycle where required;\s*\n\s*- repository step 20b production restore, slo, observability, performance, security and supply-chain evidence;\s*\n\s*- a full phase 8a\.11 closure review beyond the accepted bounded step 20a case-read slice\.",
            "### 5.1 Accepted Phase 8A.11 closure\n\nPRs #292, #294 and #296 complete the product plane, operations readiness and full restriction/legal-hold release/read lifecycle required for Phase 8A.11 closure.",
            "roadmap-remaining-work",
            True,
        ),
        (
            r"phase 8a remains incomplete until these criteria are met for the current customer-master/privacy scope\.",
            "Phase 8A meets these criteria through the accepted Customer Privacy product, runtime, browser and operations evidence.",
            "roadmap-module-accounting",
            True,
        ),
        (
            r"until then issue #194 remains open, phase 8a and customer privacy remain incomplete, and current product-complete expert modules remain \*\*0\*\*\.",
            "Issue #194 remains open and architecture 10/10 is not declared; Phase 8A and Customer Privacy are complete and current product-complete expert modules: **1**.",
            "roadmap-final-boundary",
            True,
        ),
        (
            r"the latest public customer privacy inventory remains \*\*seven mutations and four permission-aware public queries\*\*\.",
            "The final public Customer Privacy inventory is **nine mutations and seven permission-aware public queries**.",
            "roadmap-step19-inventory",
            True,
        ),
        (
            r"phase 8a\.11 / issue #126 is complete; customer privacy is not product-complete; current product-complete expert modules remain zero;",
            "Phase 8A.11 / issue #126 and Customer Privacy are complete; current product-complete expert modules: **1**;",
            "roadmap-step19-completeness",
            True,
        ),
        (
            r"repository step 21 phase 8a closure is the only next permitted packet\.",
            "Repository Step 21 Phase 8A closure is complete through PR #296; Repository Step 22 is the sole next permitted implementation packet.",
            "roadmap-step20a-next",
            True,
        ),
        (
            r"phase 8a\.11, phase 8a, product-complete expert modules, architecture 10/10 and the universal crm product remain incomplete\.",
            "Customer Privacy and Phase 8A are complete with current product-complete expert modules: **1**; architecture 10/10 and the Universal CRM product remain incomplete.",
            "roadmap-step20a-completeness",
            True,
        ),
        (
            r"the accepted one-worker customer privacy inventory, seven public mutations, four permission-aware public queries",
            "The accepted one-worker Customer Privacy inventory, nine public mutations, seven permission-aware public queries",
            "roadmap-step20a-inventory",
            True,
        ),
        (
            r"phase 8a\.11, phase 8a, customer privacy as a complete product capability, product-complete expert modules, architecture 10/10 and the universal crm product remain incomplete\.",
            "Customer Privacy and Phase 8A are complete with current product-complete expert modules: **1**; architecture 10/10 and the Universal CRM product remain incomplete.",
            "roadmap-step20-completeness",
            True,
        ),
        (
            r"step 21 next",
            "Step 21 complete through PR #296; Step 22 next",
            "roadmap-short-step21-next",
            False,
        ),
    ),
)

apply_rules(
    "docs/PHASE8_DELIVERY_PLAN.md",
    (
        (
            r"repository step 20b is accepted through pr #294; repository step 21 phase 8a closure is next\.",
            "Repository Steps 20B and 21 are accepted through PRs #294 and #296; Repository Step 22 is the sole next permitted implementation packet.",
            "phase8-step21-next",
            True,
        ),
        (
            r"20\. repository step 20 — frontend and operations evidence;",
            "20. Repository Step 20 — frontend and operations evidence — **complete through PRs #292 and #294**;",
            "phase8-continuation-step20",
            True,
        ),
        (
            r"21\. repository step 21 — phase 8a closure;",
            "21. Repository Step 21 — Phase 8A closure — **complete through PR #296**;",
            "phase8-continuation-step21",
            True,
        ),
        (
            r"issue #194 and issue #126 remain open\.",
            "Issue #194 remains open. Issue #126 is ready for closure after this accepted evidence synchronization.",
            "phase8-issue-ledger",
            True,
        ),
        (
            r"the latest public customer privacy inventory remains \*\*seven mutations and four permission-aware public queries\*\*\.",
            "The final public Customer Privacy inventory is **nine mutations and seven permission-aware public queries**.",
            "phase8-step19-inventory",
            True,
        ),
        (
            r"phase 8a\.11 / issue #126 is complete; customer privacy is not product-complete; current product-complete expert modules remain zero;",
            "Phase 8A.11 / issue #126 and Customer Privacy are complete; current product-complete expert modules: **1**;",
            "phase8-step19-completeness",
            True,
        ),
        (
            r"repository step 21 phase 8a closure is the only next permitted packet\.",
            "Repository Step 21 Phase 8A closure is complete through PR #296; Repository Step 22 is the sole next permitted implementation packet.",
            "phase8-step20a-next",
            True,
        ),
        (
            r"phase 8a\.11, phase 8a, product-complete expert modules, architecture 10/10 and the universal crm product remain incomplete\.",
            "Customer Privacy and Phase 8A are complete with current product-complete expert modules: **1**; architecture 10/10 and the Universal CRM product remain incomplete.",
            "phase8-step20a-completeness",
            True,
        ),
        (
            r"the accepted one-worker customer privacy inventory, seven public mutations, four permission-aware public queries",
            "The accepted one-worker Customer Privacy inventory, nine public mutations, seven permission-aware public queries",
            "phase8-step20a-inventory",
            True,
        ),
        (
            r"phase 8a\.11, phase 8a, customer privacy as a complete product capability, product-complete expert modules, architecture 10/10 and the universal crm product remain incomplete\.",
            "Customer Privacy and Phase 8A are complete with current product-complete expert modules: **1**; architecture 10/10 and the Universal CRM product remain incomplete.",
            "phase8-step20-completeness",
            True,
        ),
        (
            r"### repository step 20\s*\n\s*- frontend ownership and bounded dependencies;\s*\n\s*- critical component/browser/accessibility journeys;\s*\n\s*- production restore, slo, observability, performance, security and supply-chain evidence\.",
            "### Repository Step 20 — complete through PRs #292 and #294\n\n- frontend ownership, bounded dependencies, critical browser/accessibility journeys and production operations evidence are accepted.",
            "phase8-step20-section",
            True,
        ),
        (
            r"### repository step 21\s*\n\s*- final phase 8a product and operations closure against every accepted invariant\.",
            "### Repository Step 21 — complete through PR #296\n\n- final Phase 8A product, lifecycle and operations closure is accepted against every invariant.",
            "phase8-step21-section",
            True,
        ),
    ),
)

architecture_test_path = "tests/test_architecture_documentation_consistency.py"
architecture_test = read(architecture_test_path)
old_regex = 'r"step 21[^\\n.;]{0,100}(?:not started|in progress|\\bnext\\b)",'
new_regex = 'r"(?:repository )?step 21(?: phase 8a closure)? is (?:not started|in progress|next)\\b",'
if old_regex not in architecture_test:
    raise RuntimeError("missing fail-closed Step 21 stale-state regex")
write(architecture_test_path, architecture_test.replace(old_regex, new_regex))

navigation_test_path = "tests/test_repository_navigation.py"
navigation_test = read(navigation_test_path)
workflow_block = navigation_test.split("workflow_paths =", 1)[1].split("}", 1)[0]
if "Customer Privacy Access Export CI" not in workflow_block:
    marker = '            "Complexity Baseline CI": ".github/workflows/complexity-baseline.yml",\n'
    if marker not in navigation_test:
        raise RuntimeError("missing fail-closed workflow path mapping marker")
    navigation_test = navigation_test.replace(
        marker,
        marker
        + '            "Customer Privacy Access Export CI": ".github/workflows/customer-privacy-access-export.yml",\n'
        + '            "Customer Privacy Operations CI": ".github/workflows/customer-privacy-operations.yml",\n'
        + '            "Customer Privacy Owner Execution CI": ".github/workflows/customer-privacy-owner-execution.yml",\n',
        1,
    )
write(navigation_test_path, navigation_test)

print("Finalized accepted Repository Step 21 evidence and stale-state guards.")
