#!/usr/bin/env python3
"""One-shot exact Cargo.lock synchronization for the Step 22C edge removal."""

from __future__ import annotations

import copy
import json
from pathlib import Path
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
LOCK_PATH = ROOT / "Cargo.lock"
VALIDATOR_PATH = ROOT / "scripts/check_step22_runtime_fanin_decisions.py"
BASELINE_COMMIT = "6fe0e8e7702b01a78f5db3f174c09b686de27402"
REMOVED_DEPENDENCY = "crm-customer-privacy-query-adapter"
CANONICAL_PROST_REFERENCE = "prost"
REMOVED_LOCK_LINE = ' "crm-customer-privacy-query-adapter",\n'
CANONICAL_PROST_LINE = ' "prost",\n'
VERSIONED_PROST_LINE = ' "prost 0.14.3",\n'
RUNTIME_BLOCK = re.compile(
    r'(?ms)^\[\[package\]\]\nname = "crm-application-runtime"\n.*?'
    r'(?=^\[\[package\]\]|\Z)'
)

VALIDATOR_MARKER = '''    if "crm-customer-privacy-query-adapter" not in owner_dependencies:
        raise DecisionLedgerError(
            "Customer Privacy production must retain the query adapter internally"
        )

'''
VALIDATOR_INSERTION = '''    if "crm-customer-privacy-query-adapter" not in owner_dependencies:
        raise DecisionLedgerError(
            "Customer Privacy production must retain the query adapter internally"
        )

    lockfile = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    packages = lockfile.get("package", [])
    if not isinstance(packages, list):
        raise DecisionLedgerError("Cargo.lock package inventory is missing")

    def lock_package(name: str) -> dict[str, Any]:
        matching = [
            package
            for package in packages
            if isinstance(package, dict) and package.get("name") == name
        ]
        if len(matching) != 1:
            raise DecisionLedgerError(
                f"Cargo.lock must contain exactly one {name} package record"
            )
        return matching[0]

    runtime_lock_dependencies = lock_package("crm-application-runtime").get(
        "dependencies", []
    )
    if "crm-customer-privacy-query-adapter" in runtime_lock_dependencies:
        raise DecisionLedgerError(
            "Cargo.lock still records the removed direct runtime query-adapter edge"
        )
    if "prost" not in runtime_lock_dependencies:
        raise DecisionLedgerError(
            "Cargo.lock does not retain the canonical runtime prost reference"
        )
    if "prost 0.14.3" in runtime_lock_dependencies:
        raise DecisionLedgerError(
            "Cargo.lock contains a non-baseline version-qualified runtime prost reference"
        )
    owner_lock_dependencies = lock_package("crm-customer-privacy-production").get(
        "dependencies", []
    )
    if "crm-customer-privacy-query-adapter" not in owner_lock_dependencies:
        raise DecisionLedgerError(
            "Cargo.lock no longer records the owner-internal query adapter"
        )
    lock_package("crm-customer-privacy-query-adapter")

'''


def git_show(spec: str) -> str:
    return subprocess.run(
        ["git", "show", spec],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout


def package_records(payload: dict) -> list[dict]:
    packages = payload.get("package")
    if not isinstance(packages, list) or not all(
        isinstance(package, dict) for package in packages
    ):
        raise RuntimeError("Cargo.lock package inventory is invalid")
    return packages


def unique_package(packages: list[dict], name: str) -> dict:
    matching = [package for package in packages if package.get("name") == name]
    if len(matching) != 1:
        raise RuntimeError(f"expected exactly one {name} package, got {len(matching)}")
    return matching[0]


def expected_lock_text(baseline_text: str) -> str:
    matching = RUNTIME_BLOCK.findall(baseline_text)
    if len(matching) != 1:
        raise RuntimeError(
            f"expected exactly one crm-application-runtime lock block, got {len(matching)}"
        )
    runtime_block = matching[0]
    if runtime_block.count(REMOVED_LOCK_LINE) != 1:
        raise RuntimeError(
            "immutable runtime lock block must contain exactly one removable adapter line"
        )
    if runtime_block.count(CANONICAL_PROST_LINE) != 1:
        raise RuntimeError(
            "immutable runtime lock block must retain exactly one canonical prost line"
        )
    if VERSIONED_PROST_LINE in runtime_block:
        raise RuntimeError(
            "immutable runtime lock block unexpectedly contains version-qualified prost"
        )
    expected_runtime_block = runtime_block.replace(REMOVED_LOCK_LINE, "", 1)
    return RUNTIME_BLOCK.sub(lambda _match: expected_runtime_block, baseline_text, count=1)


def validate_exact_lock_delta(baseline_text: str, current_text: str) -> None:
    if current_text != expected_lock_text(baseline_text):
        raise RuntimeError(
            "Cargo.lock is not the immutable baseline minus the exact adapter line"
        )

    baseline = tomllib.loads(baseline_text)
    current = tomllib.loads(current_text)
    if baseline.get("version") != current.get("version"):
        raise RuntimeError("Cargo.lock format version changed")

    baseline_packages = package_records(baseline)
    current_packages = package_records(current)
    if len(baseline_packages) != len(current_packages):
        raise RuntimeError("Cargo.lock package count changed")

    baseline_runtime = unique_package(baseline_packages, "crm-application-runtime")
    current_runtime = unique_package(current_packages, "crm-application-runtime")
    expected_runtime = copy.deepcopy(baseline_runtime)
    dependencies = expected_runtime.get("dependencies")
    if not isinstance(dependencies, list):
        raise RuntimeError("immutable runtime lock dependencies are missing")
    if dependencies.count(REMOVED_DEPENDENCY) != 1:
        raise RuntimeError("immutable runtime lock record lacks the exact removable edge")
    if dependencies.count(CANONICAL_PROST_REFERENCE) != 1:
        raise RuntimeError("immutable runtime lock record lacks canonical prost")
    dependencies.remove(REMOVED_DEPENDENCY)
    if current_runtime != expected_runtime:
        raise RuntimeError(
            "current runtime lock record differs from the exact one-edge deletion"
        )

    baseline_without_runtime = [
        package for package in baseline_packages if package is not baseline_runtime
    ]
    current_without_runtime = [
        package for package in current_packages if package is not current_runtime
    ]
    if baseline_without_runtime != current_without_runtime:
        raise RuntimeError(
            "Cargo.lock changed a package record outside crm-application-runtime"
        )

    current_owner = unique_package(
        current_packages, "crm-customer-privacy-production"
    )
    if REMOVED_DEPENDENCY not in current_owner.get("dependencies", []):
        raise RuntimeError("owner production lock record lost the internal adapter")
    unique_package(current_packages, REMOVED_DEPENDENCY)


def materialize_validator() -> None:
    text = VALIDATOR_PATH.read_text(encoding="utf-8")
    if "Cargo.lock still records the removed direct runtime query-adapter edge" in text:
        if "Cargo.lock does not retain the canonical runtime prost reference" not in text:
            raise RuntimeError("partial Step 22C lockfile validator materialization detected")
        return
    if text.count(VALIDATOR_MARKER) != 1:
        raise RuntimeError("validator insertion marker changed")
    VALIDATOR_PATH.write_text(
        text.replace(VALIDATOR_MARKER, VALIDATOR_INSERTION, 1),
        encoding="utf-8",
    )


def main() -> None:
    baseline_text = git_show(f"{BASELINE_COMMIT}:Cargo.lock")
    current_text = expected_lock_text(baseline_text)
    LOCK_PATH.write_text(current_text, encoding="utf-8")
    validate_exact_lock_delta(baseline_text, current_text)
    subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--no-deps"],
        cwd=ROOT,
        check=True,
        stdout=subprocess.DEVNULL,
    )
    if LOCK_PATH.read_text(encoding="utf-8") != current_text:
        raise RuntimeError("cargo metadata --locked changed the accepted lockfile")
    materialize_validator()

    summary = {
        "baseline_commit": BASELINE_COMMIT,
        "changed_package": "crm-application-runtime",
        "package_count": len(package_records(tomllib.loads(current_text))),
        "removed_dependency": REMOVED_DEPENDENCY,
    }
    print(json.dumps(summary, sort_keys=True))


if __name__ == "__main__":
    main()
