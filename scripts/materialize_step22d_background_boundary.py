#!/usr/bin/env python3
"""Materialize the second Customer 360 runtime consumer behind the owner boundary."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BACKGROUND = "crates/crm-application-runtime/src/background.rs"
REGISTRY = "crates/crm-application-runtime/src/bootstrap_visibility/registry.rs"
STABLE_ID = "crm-application-runtime::dependencies::crm-customer-360-query-adapter"


def canonical(payload: object) -> str:
    return json.dumps(payload, indent=2, sort_keys=True) + "\n"


def replace_once(path: str, old: str, new: str) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one match, found {count}")
    target.write_text(text.replace(old, new, 1), encoding="utf-8")


def update_background() -> None:
    replace_once(
        BACKGROUND,
        "use crm_customer_360_query_adapter::MODULE_ID as CUSTOMER_360_MODULE_ID;",
        "use crm_first_party_modules::CUSTOMER_360_MODULE_ID;",
    )


def update_packet() -> None:
    path = ROOT / "repository-packet.json"
    payload = json.loads(path.read_text(encoding="utf-8"))
    allowed = set(payload["allowed_paths"])
    allowed.add(BACKGROUND)
    payload["allowed_paths"] = sorted(allowed)
    path.write_text(canonical(payload), encoding="utf-8")


def update_ledger() -> None:
    path = ROOT / "step22-runtime-fanin-decisions.json"
    payload = json.loads(path.read_text(encoding="utf-8"))
    removals = payload["remediation_evidence"]["removals"]
    matching = [entry for entry in removals if entry["stable_id"] == STABLE_ID]
    if len(matching) != 1:
        raise RuntimeError("Customer 360 remediation entry must be unique")
    matching[0]["runtime_sources"] = sorted({REGISTRY, BACKGROUND})
    path.write_text(canonical(payload), encoding="utf-8")


def update_validator() -> None:
    replace_once(
        "scripts/check_step22_runtime_fanin_decisions.py",
        '            "runtime_sources": [\n                "crates/crm-application-runtime/src/bootstrap_visibility/registry.rs"\n            ],',
        '            "runtime_sources": [\n                "crates/crm-application-runtime/src/background.rs",\n                "crates/crm-application-runtime/src/bootstrap_visibility/registry.rs",\n            ],',
    )


def update_docs() -> None:
    replace_once(
        "docs/STEP22_CUSTOMER_360_QUERY_FANIN_REDUCTION.md",
        "2. `bootstrap_visibility/registry.rs` imports that constant through `crm_first_party_modules`.",
        "2. `background.rs` and `bootstrap_visibility/registry.rs` import that constant through `crm_first_party_modules`.",
    )


def update_guards() -> None:
    registry_literal = f'"{REGISTRY}",'
    background_literal = f'"{BACKGROUND}",'
    for path in (
        "tests/test_architecture_documentation_consistency.py",
        "tests/test_repository_navigation.py",
    ):
        target = ROOT / path
        lines = target.read_text(encoding="utf-8").splitlines()
        if any(background_literal in line for line in lines):
            continue
        matching = [index for index, line in enumerate(lines) if registry_literal in line]
        if not matching:
            raise RuntimeError(f"{path}: allowed-path marker missing")
        index = matching[0]
        indent = lines[index][: len(lines[index]) - len(lines[index].lstrip())]
        lines.insert(index, f"{indent}{background_literal}")
        target.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> None:
    update_background()
    update_packet()
    update_ledger()
    update_validator()
    update_docs()
    update_guards()


if __name__ == "__main__":
    main()
