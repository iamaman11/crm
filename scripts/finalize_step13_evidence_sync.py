#!/usr/bin/env python3
"""Finalize PR #253 evidence synchronization without rewriting historical evidence."""

from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
STATUS = ROOT / "docs/PROJECT_STATUS.md"
GUARD = ROOT / "tests/test_architecture_documentation_consistency.py"

OLD_PLAN_BOUNDARY = """The next permitted packet is repository-step-13 measurement and governance calibration only. It must regenerate the exact 113-package complexity baseline, inventory equivalent manifest/source bypass forms, classify central systems by role, measure ordinary-capability and new-owner change cost, and calibrate budgets before any structural remediation. It must not centralize dependency features, consolidate crates, remove exceptions or change runtime/product behavior.

Repository step 13 remains **not started**. Repository step 14 remains blocked. Customer Privacy and Phase 8A remain incomplete."""

NEW_PLAN_BOUNDARY = """At the historical PR #251 boundary, the next permitted packet was repository-step-13 measurement and governance calibration only. That packet is now accepted through PR #253 / accepted source `475533b185b871418273c1c1e3f63a1d62542677` / squash merge `7dcda204be07209d9e4996fdc9c5fd364cea179e` / 7 of 7 applicable permanent workflows on one unchanged exact head.

Repository step 13 remains **in progress**. The next permitted implementation packet registers and enforces the accepted suppression baseline, removes the three direct lint-table exceptions without hidden replacements and calibrates role-aware dependency, public-surface, central-LOC, reverse-impact and change-cost budgets. Repository step 14 remains blocked. Customer Privacy and Phase 8A remain incomplete."""

OLD_COMPLETION = """Repository step 13 is the **next permitted repository step** and is **not started**. Its first bounded packet is measurement and governance calibration only; dependency, crate, exception and runtime remediation remain later step-13 work after accepted evidence. Repository step 14 remains blocked. Customer Privacy and Phase 8A product readiness remain unchanged; current product-complete expert modules remain **0**."""

NEW_COMPLETION = """Repository step 13 is **in progress** after accepted PR #253 measurement and governance calibration. Its next bounded implementation packet registers and enforces the accepted suppression baseline, removes the three direct lint-table exceptions without hidden replacements and calibrates role-aware budgets; evidence-driven later step-13 remediation remains permitted until every ADR-031 exit criterion is proven. Repository step 14 remains blocked. Customer Privacy and Phase 8A product readiness remain unchanged; current product-complete expert modules remain **0**."""

OLD_ASSERTION = '            "Latest accepted repository implementation packet is PR #249",'
NEW_ASSERTION = '            "Latest accepted repository implementation packet is PR #253",'

PR249_EVIDENCE = '''            (
                self.authoritative_status_documents,
                "PR #249",
                "7876945586e5a6cc94f8d3b0f6ba2b57316484d2",
                "f36592211bed3e0df7cf3771164b4bc24026eff3",
                "37 of 37",
            ),'''

PR253_EVIDENCE = '''            (
                self.authoritative_status_documents,
                "PR #253",
                "475533b185b871418273c1c1e3f63a1d62542677",
                "7dcda204be07209d9e4996fdc9c5fd364cea179e",
                "7 of 7",
            ),'''

STALE_MARKER = '            "Repository step 11 is the only next implementation packet",\n'
STALE_ADDITION = (
    '            "Repository step 13 remains **not started**",\n'
    '            "Repository step 13 is the **next permitted repository step** and is **not started**",\n'
)


def replace_exact(text: str, old: str, new: str, label: str) -> str:
    if old in text:
        return text.replace(old, new, 1)
    if new in text:
        return text
    raise RuntimeError(f"missing {label}")


def main() -> int:
    status = STATUS.read_text(encoding="utf-8")
    status = replace_exact(status, OLD_PLAN_BOUNDARY, NEW_PLAN_BOUNDARY, "plan-hardening boundary")
    status = replace_exact(status, OLD_COMPLETION, NEW_COMPLETION, "step-12 completion boundary")
    STATUS.write_text(status, encoding="utf-8")

    guard = GUARD.read_text(encoding="utf-8")
    guard = replace_exact(guard, OLD_ASSERTION, NEW_ASSERTION, "latest repository assertion")
    if PR253_EVIDENCE not in guard:
        if PR249_EVIDENCE not in guard:
            raise RuntimeError("missing PR #249 evidence insertion point")
        guard = guard.replace(PR249_EVIDENCE, PR249_EVIDENCE + "\n" + PR253_EVIDENCE, 1)
    if "Repository step 13 remains **not started**" not in guard:
        if STALE_MARKER not in guard:
            raise RuntimeError("missing stale-claim insertion point")
        guard = guard.replace(STALE_MARKER, STALE_MARKER + STALE_ADDITION, 1)
    GUARD.write_text(guard, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
