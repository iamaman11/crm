#!/usr/bin/env python3
"""Temporarily materialize accepted PR #253 evidence into live normative docs."""

from __future__ import annotations

from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]

ACCEPTED = (
    "PR #253 / accepted source `475533b185b871418273c1c1e3f63a1d62542677` / "
    "squash merge `7dcda204be07209d9e4996fdc9c5fd364cea179e` / "
    "7 of 7 applicable permanent workflows on one unchanged exact head"
)

MEASUREMENT = (
    "The accepted exact current-main baseline contains 113 workspace packages, "
    "841 internal dependency edges, maximum dependency depth 18, maximum direct "
    "dependents 105, maximum transitive reverse impact 106, a conservative public "
    "Rust surface of 5,377 items, 40 permanent workflows, 41 jobs, 1,712 path-filter "
    "entries, 31 PostgreSQL workflows and 94 equivalent suppression entries "
    "(3 direct lint tables, 87 source-level `allow` attributes, 0 `expect` attributes "
    "and 4 ignored foundation tests)."
)

NEXT = (
    "Repository step 13 remains in progress. The next permitted implementation packet "
    "registers the accepted suppression baseline, mechanically blocks every new "
    "unregistered equivalent bypass while allowing reductions, removes the three "
    "direct lint-table exceptions without hidden replacements and calibrates role-aware "
    "dependency, public-surface, central-LOC, reverse-impact and change-cost budgets. "
    "Repository step 14 remains blocked."
)


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    (ROOT / path).write_text(text, encoding="utf-8")


def replace_regex(text: str, pattern: str, replacement: str, *, count: int = 1) -> str:
    updated, matches = re.subn(pattern, replacement, text, count=count, flags=re.DOTALL)
    if matches != count:
        raise RuntimeError(f"expected {count} match(es) for {pattern!r}, found {matches}")
    return updated


def insert_before(text: str, marker: str, addition: str) -> str:
    if addition.strip() in text:
        return text
    if marker not in text:
        raise RuntimeError(f"missing insertion marker {marker!r}")
    return text.replace(marker, addition.rstrip() + "\n\n" + marker, 1)


def sync_status() -> None:
    path = "docs/PROJECT_STATUS.md"
    text = read(path)
    text = replace_regex(
        text,
        r"Latest accepted repository implementation packet is PR #249 .*?Repository step 12 and Stage D are complete; repository step 13 is the next permitted implementation step and is not started\.",
        (
            f"Latest accepted repository implementation packet is {ACCEPTED}. "
            "It completes the mandatory ADR-031 current-main measurement and governance-calibration packet. "
            "Repository step 12 and Stage D remain complete; repository step 13 is in progress."
        ),
    )
    evidence = (
        "## Accepted repository step 13 current-main measurement\n\n"
        f"{ACCEPTED}. {MEASUREMENT}\n\n"
        "The measurement classifies stable contracts and SDK/ports separately from mutable "
        "implementation, aggregation and process-composition risk. Representative accepted "
        "change-cost evidence is 35 files / 10 packages / 6 central files / 2 workflow files "
        "for an ordinary capability, 206 / 22 / 52 / 4 for a new-owner wave and "
        "21 / 5 / 5 / 0 for the final contribution-aggregation batch. Thin-wrapper results "
        "remain candidate-only and authorize no consolidation.\n\n"
        f"{NEXT} No arbitrary duration or packet-count limit is added; step 13 completes only "
        "when every ADR-031 exit criterion is mechanically proven."
    )
    text = insert_before(text, "## Accepted repository step 12 batch 1", evidence)
    text = text.replace(
        "Repository step 12 is complete; repository step 13 is the next permitted implementation step and is not started.",
        "Repository step 12 is complete; repository step 13 is in progress after accepted PR #253 measurement.",
    )
    text = replace_regex(
        text,
        r"## Next permitted repository packet\n\nRepository step 13 is the current next permitted implementation step and is not started\. Its bounded packet must complete calibrated dependency, Rust public-surface, reverse-fan-out and exception governance, including removal of the three direct lint exceptions\.",
        "## Next permitted repository packet\n\n" + NEXT,
    )
    write(path, text)


def sync_plan() -> None:
    path = "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md"
    text = read(path)
    evidence = (
        "## Repository step 13 current-main measurement evidence\n\n"
        f"{ACCEPTED}. {MEASUREMENT}\n\n"
        "Role-aware central measurements include `crm-application-runtime` at 63 direct "
        "dependencies, dependency depth 17 and 7,243 non-comment LOC; `crm-api` at 19 "
        "direct dependencies, depth 18 and 9 non-comment LOC; `crm-first-party-modules` "
        "at 16 direct dependencies and 204 non-comment LOC; and `crm-core-data` at "
        "71 direct consumers, reverse impact 76 and 9,922 non-comment LOC. High fan-out "
        "stable contract/SDK boundaries remain distinct from mutable implementation and "
        "composition risks.\n\n"
        "The accepted representative change-cost measurements are 35 files / 10 packages / "
        "6 central files / 2 workflow files for an ordinary capability, 206 / 22 / 52 / 4 "
        "for a new owner wave, and 21 / 5 / 5 / 0 for final contribution aggregation.\n\n"
        f"{NEXT} Later evidence-driven remediation remains permitted without an arbitrary "
        "duration or packet-count cap; completion is governed only by the ADR-031 exit evidence."
    )
    text = insert_before(text, "## Repository step 12 completion evidence", evidence)
    text = text.replace(
        "Repository step 13 is the current next permitted implementation step and is not started. Its bounded packet must complete calibrated dependency, Rust public-surface, reverse-fan-out and exception governance, including removal of the three direct lint exceptions.",
        NEXT,
    )
    text = text.replace(
        "Repository step 13 is the **next permitted repository step** and is **not started**. Its first packet is the ADR-031 current-main remeasurement and governance-calibration packet described above; structural remediation follows only after that evidence is accepted and synchronized.",
        "Repository step 13 is **in progress**. Its ADR-031 current-main measurement and governance-calibration packet is accepted through PR #253; suppression enforcement, direct-lint removal and calibrated remediation remain next.",
    )
    write(path, text)


def sync_roadmap() -> None:
    path = "docs/IMPLEMENTATION_ROADMAP.md"
    text = read(path)
    text = text.replace(
        "Repository step 13 is the current next permitted implementation step and is not started. Its bounded packet must complete calibrated dependency, Rust public-surface, reverse-fan-out and exception governance, including removal of the three direct lint exceptions.",
        NEXT,
    )
    evidence = (
        "### 3.2 Accepted repository step 13 measurement\n\n"
        f"{ACCEPTED}. {MEASUREMENT}\n\n{NEXT} The measurement changes no product behavior, "
        "dependency, package, route, contract, schema, migration, persistence or worker."
    )
    text = insert_before(text, "### 3.1 Remaining stage-to-step ownership", evidence)
    write(path, text)


def sync_phase8() -> None:
    path = "docs/PHASE8_DELIVERY_PLAN.md"
    text = read(path)
    text = text.replace(
        "Repository step 13 is the current next permitted implementation step and is not started. Its bounded packet must complete calibrated dependency, Rust public-surface, reverse-fan-out and exception governance, including removal of the three direct lint exceptions.",
        NEXT,
    )
    evidence = (
        "## 5.1 Accepted repository step 13 measurement\n\n"
        f"{ACCEPTED}. {MEASUREMENT}\n\n{NEXT} This architecture-only packet does not change "
        "Customer Privacy behavior, public inventory, workers or Phase 8A readiness."
    )
    text = insert_before(text, "## 6. Accepted scope discovery and immutable snapshot", evidence)
    write(path, text)


def sync_catalog() -> None:
    path = "docs/MODULE_CATALOG.md"
    text = read(path)
    old = (
        "Repository steps 1–12 and Stage D are complete. Repository step 13 is the only next "
        "permitted repository step and is executing its current-main measurement and "
        "governance-calibration packet. Repository step 14 remains blocked. This architecture "
        "work does not advance Customer Privacy product readiness."
    )
    new = (
        f"Repository steps 1–12 and Stage D are complete. {ACCEPTED}. {MEASUREMENT} "
        f"{NEXT} This architecture work does not advance Customer Privacy product readiness."
    )
    if old not in text:
        raise RuntimeError("module catalog current-step paragraph was not found")
    text = text.replace(old, new, 1)
    write(path, text)


def main() -> int:
    sync_status()
    sync_plan()
    sync_roadmap()
    sync_phase8()
    sync_catalog()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
