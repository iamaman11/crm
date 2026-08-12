#!/usr/bin/env python3
"""Validate the bounded ADR-032 runtime fan-in decision ledger."""

from __future__ import annotations

from collections import Counter
import json
from pathlib import Path
import sys
import tomllib
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DECISIONS_PATH = Path("step22-runtime-fanin-decisions.json")
GOVERNANCE_PATH = Path("architecture-governance.json")
EXPECTED_SCHEMA = "crm.step22-runtime-fanin-decisions/v1"
EXPECTED_INVENTORY_SCHEMA = "crm.step22-architecture-inventory/v1"
EXPECTED_GOVERNANCE_SCHEMA = "crm.architecture-governance/v1"
FINAL_CLASSIFICATIONS = {
    "removed",
    "platform-generic",
    "owner-specific-unavoidable",
    "test-only",
}
REMOVED_PRIVACY_QUERY_STABLE_ID = (
    "crm-application-runtime::dependencies::crm-customer-privacy-query-adapter"
)
REMOVED_CUSTOMER_360_QUERY_STABLE_ID = (
    "crm-application-runtime::dependencies::crm-customer-360-query-adapter"
)
REMOVED_PARTIES_CAPABILITY_STABLE_ID = (
    "crm-application-runtime::dependencies::crm-parties-capability-adapter"
)
EXPECTED_REMOVED_STABLE_IDS = {
    REMOVED_CUSTOMER_360_QUERY_STABLE_ID,
    REMOVED_PRIVACY_QUERY_STABLE_ID,
    REMOVED_PARTIES_CAPABILITY_STABLE_ID,
}
EXPECTED_REGISTRATION = {
    "id": "repository-step-22-runtime-fanin",
    "owner": "architecture-governance",
    "path": DECISIONS_PATH.as_posix(),
    "review_condition": (
        "Review on every accepted runtime fan-in classification or remediation "
        "packet until Repository Step 22 closure."
    ),
    "tracking_issue": "#194",
    "validator": "scripts/check_step22_runtime_fanin_decisions.py",
}
EXPECTED_REMEDIATION = {
    "after": {"all": 60, "production": 59, "test_only": 1},
    "before": {"all": 63, "production": 62, "test_only": 1},
    "removals": [
        {
            "adapter_package": "crm-customer-360-query-adapter",
            "owner_manifest": "crates/crm-first-party-modules/Cargo.toml",
            "owner_sources": ["crates/crm-first-party-modules/src/lib.rs"],
            "replacement_boundary": "crm-first-party-modules",
            "runtime_sources": [
                "crates/crm-application-runtime/src/background.rs",
                "crates/crm-application-runtime/src/bootstrap_visibility/registry.rs",
            ],
            "stable_id": REMOVED_CUSTOMER_360_QUERY_STABLE_ID,
        },
        {
            "adapter_package": "crm-customer-privacy-query-adapter",
            "owner_manifest": "crates/crm-customer-privacy-production/Cargo.toml",
            "owner_sources": [
                "crates/crm-customer-privacy-production/src/legal_hold.rs",
                "crates/crm-customer-privacy-production/src/root.rs",
            ],
            "replacement_boundary": "crm-customer-privacy-production",
            "runtime_sources": [
                "crates/crm-application-runtime/src/bootstrap_visibility/registry.rs",
                "crates/crm-application-runtime/src/customer_privacy_case_create_promotion.rs",
            ],
            "stable_id": REMOVED_PRIVACY_QUERY_STABLE_ID,
        },
        {
            "adapter_package": "crm-parties-capability-adapter",
            "owner_manifest": "crates/crm-party-reference-composition/Cargo.toml",
            "owner_sources": ["crates/crm-party-reference-composition/src/lib.rs"],
            "replacement_boundary": "crm-party-reference-composition",
            "runtime_sources": [
                "crates/crm-application-runtime/src/bootstrap_visibility/registry.rs",
            ],
            "stable_id": REMOVED_PARTIES_CAPABILITY_STABLE_ID,
        },
    ],
    "removed_stable_ids": [
        REMOVED_CUSTOMER_360_QUERY_STABLE_ID,
        REMOVED_PRIVACY_QUERY_STABLE_ID,
        REMOVED_PARTIES_CAPABILITY_STABLE_ID,
    ],
    "runtime_manifest": "crates/crm-application-runtime/Cargo.toml",
}


class DecisionLedgerError(RuntimeError):
    """Raised when the Step 22 runtime fan-in ledger is inconsistent."""


def canonical_json(payload: Any) -> str:
    return json.dumps(payload, indent=2, sort_keys=True) + "\n"


def load_json(root: Path, relative: Path) -> tuple[dict[str, Any], str]:
    text = (root / relative).read_text(encoding="utf-8")
    payload = json.loads(text)
    if not isinstance(payload, dict):
        raise DecisionLedgerError(f"{relative} must contain a JSON object")
    return payload, text


def validate_governance_registration(root: Path) -> None:
    registry, _ = load_json(root, GOVERNANCE_PATH)
    if registry.get("schema_version") != EXPECTED_GOVERNANCE_SCHEMA:
        raise DecisionLedgerError("unexpected architecture governance schema")
    registrations = registry.get("decision_ledgers")
    if not isinstance(registrations, list):
        raise DecisionLedgerError("architecture governance decision_ledgers is missing")
    matching = [
        registration
        for registration in registrations
        if isinstance(registration, dict)
        and registration.get("id") == EXPECTED_REGISTRATION["id"]
    ]
    if matching != [EXPECTED_REGISTRATION]:
        raise DecisionLedgerError(
            "architecture governance must contain exactly one canonical Step 22 "
            "runtime fan-in registration"
        )
    for field in ("path", "validator"):
        registered_path = matching[0][field]
        if not (root / registered_path).is_file():
            raise DecisionLedgerError(
                f"registered Step 22 {field} does not exist: {registered_path}"
            )


def inventory_rows(inventory: dict[str, Any]) -> dict[str, dict[str, str]]:
    if inventory.get("schema_version") != EXPECTED_INVENTORY_SCHEMA:
        raise DecisionLedgerError("unexpected Step 22 inventory schema")
    runtime = inventory.get("runtime_fanin")
    if not isinstance(runtime, dict):
        raise DecisionLedgerError("inventory runtime_fanin object is missing")
    columns = runtime.get("columns")
    rows = runtime.get("rows")
    expected_columns = [
        "stable_id",
        "dependency_kind",
        "target_category",
        "target_manifest_path",
    ]
    if columns != expected_columns or not isinstance(rows, list):
        raise DecisionLedgerError("inventory runtime_fanin tabular contract changed")

    result: dict[str, dict[str, str]] = {}
    for raw in rows:
        if not isinstance(raw, list) or len(raw) != len(expected_columns):
            raise DecisionLedgerError("invalid runtime_fanin row")
        row = dict(zip(expected_columns, raw, strict=True))
        stable_id = row["stable_id"]
        if stable_id in result:
            raise DecisionLedgerError(f"duplicate inventory stable_id: {stable_id}")
        result[stable_id] = row
    return result


def current_runtime_direct_ids(root: Path, manifest_path: str) -> set[str]:
    manifest = tomllib.loads((root / manifest_path).read_text(encoding="utf-8"))
    ids: set[str] = set()
    sections = (
        ("dependencies", "dependencies"),
        ("dev-dependencies", "dev-dependencies"),
        ("build-dependencies", "build-dependencies"),
    )
    for section, stable_section in sections:
        dependencies = manifest.get(section, {})
        if not isinstance(dependencies, dict):
            raise DecisionLedgerError(f"runtime manifest {section} must be a table")
        for name in dependencies:
            if name.startswith("crm-"):
                ids.add(f"crm-application-runtime::{stable_section}::{name}")
    return ids


def validate_remediation(
    root: Path,
    decisions: dict[str, Any],
    accepted_ids: set[str],
    final_by_id: dict[str, tuple[str, str]],
) -> None:
    remediation = decisions.get("remediation_evidence")
    if remediation != EXPECTED_REMEDIATION:
        raise DecisionLedgerError("Step 22 cumulative remediation evidence changed")

    current_ids = current_runtime_direct_ids(
        root, EXPECTED_REMEDIATION["runtime_manifest"]
    )
    removed_ids = {
        stable_id
        for stable_id, (classification, _) in final_by_id.items()
        if classification == "removed"
    }
    if removed_ids != EXPECTED_REMOVED_STABLE_IDS:
        raise DecisionLedgerError(
            "Step 22E must record exactly the accepted Customer Privacy, "
            "Customer 360 and Parties capability-adapter removals"
        )
    if current_ids != accepted_ids - removed_ids:
        added = sorted(current_ids - accepted_ids)
        missing = sorted((accepted_ids - removed_ids) - current_ids)
        raise DecisionLedgerError(
            "current runtime direct dependency surface differs from the accepted "
            f"inventory minus the cumulative removals: added={added}, missing={missing}"
        )

    production = sum("::dependencies::" in stable_id for stable_id in current_ids)
    test_only = sum("::dev-dependencies::" in stable_id for stable_id in current_ids)
    current_counts = {
        "all": len(current_ids),
        "production": production,
        "test_only": test_only,
    }
    if current_counts != EXPECTED_REMEDIATION["after"]:
        raise DecisionLedgerError(
            f"current runtime fan-in is not the exact cumulative 63 to 60 reduction: {current_counts}"
        )

    runtime_manifest = tomllib.loads(
        (root / EXPECTED_REMEDIATION["runtime_manifest"]).read_text(encoding="utf-8")
    )
    runtime_dependencies = runtime_manifest.get("dependencies", {})
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
    if "prost" not in runtime_lock_dependencies:
        raise DecisionLedgerError(
            "Cargo.lock does not retain the canonical runtime prost reference"
        )

    for removal in EXPECTED_REMEDIATION["removals"]:
        adapter = removal["adapter_package"]
        if adapter in runtime_dependencies:
            raise DecisionLedgerError(
                f"runtime manifest still records removed direct edge: {adapter}"
            )
        owner_manifest = tomllib.loads(
            (root / removal["owner_manifest"]).read_text(encoding="utf-8")
        )
        if adapter not in owner_manifest.get("dependencies", {}):
            raise DecisionLedgerError(
                f"owner boundary no longer retains adapter internally: {adapter}"
            )
        if adapter in runtime_lock_dependencies:
            raise DecisionLedgerError(
                f"Cargo.lock still records removed direct runtime edge: {adapter}"
            )
        owner_package = Path(removal["owner_manifest"]).parent.name
        if adapter not in lock_package(owner_package).get("dependencies", []):
            raise DecisionLedgerError(
                f"Cargo.lock no longer records owner-internal adapter: {adapter}"
            )
        lock_package(adapter)
        rust_marker = adapter.replace("-", "_")
        replacement_marker = removal["replacement_boundary"].replace("-", "_")
        runtime_text = "\n".join(
            (root / source).read_text(encoding="utf-8")
            for source in removal["runtime_sources"]
        )
        if rust_marker in runtime_text:
            raise DecisionLedgerError(
                f"generic runtime source still imports removed adapter directly: {adapter}"
            )
        if replacement_marker not in runtime_text:
            raise DecisionLedgerError(
                f"runtime source is missing replacement boundary marker: {replacement_marker}"
            )

    first_party_source = (
        root / "crates/crm-first-party-modules/src/lib.rs"
    ).read_text(encoding="utf-8")
    for marker in (
        "MODULE_ID as CUSTOMER_360_MODULE_ID",
        "query_capability_definitions as customer_360_query_capability_definitions",
    ):
        if marker not in first_party_source:
            raise DecisionLedgerError(
                f"first-party boundary is missing Customer 360 marker: {marker}"
            )
    registry_source = (
        root / "crates/crm-application-runtime/src/bootstrap_visibility/registry.rs"
    ).read_text(encoding="utf-8")
    if "crm_first_party_modules::CUSTOMER_360_MODULE_ID" not in registry_source:
        raise DecisionLedgerError(
            "bootstrap visibility does not consume Customer 360 identity through first-party boundary"
        )
    if "crm_party_reference_composition::parties_runtime_identity" not in registry_source:
        raise DecisionLedgerError(
            "bootstrap visibility does not consume Parties identity through the owner production/reference boundary"
        )

    party_reference_source = (
        root / "crates/crm-party-reference-composition/src/lib.rs"
    ).read_text(encoding="utf-8")
    if "pub fn parties_runtime_identity()" not in party_reference_source:
        raise DecisionLedgerError(
            "Parties production/reference boundary does not expose runtime identity"
        )
    if 'pub const CRATE_NAME: &str = "crm-party-reference-composition"' in party_reference_source:
        raise DecisionLedgerError(
            "Parties runtime identity addition must remain public-surface neutral by retiring the unused CRATE_NAME marker"
        )

    privacy_legal_hold = (
        root / "crates/crm-customer-privacy-production/src/legal_hold.rs"
    ).read_text(encoding="utf-8")
    if (
        "pub use crm_customer_privacy_query_adapter::"
        "control_query_capability_definitions;"
        not in privacy_legal_hold
    ):
        raise DecisionLedgerError(
            "Customer Privacy production does not expose the control query inventory"
        )
    privacy_root = (
        root / "crates/crm-customer-privacy-production/src/root.rs"
    ).read_text(encoding="utf-8")
    if "control_query_visibility_resources" not in privacy_root:
        raise DecisionLedgerError(
            "Customer Privacy production does not expose control visibility resources"
        )


def validate_payload(
    root: Path,
    decisions: dict[str, Any],
    inventory: dict[str, Any],
) -> dict[str, int]:
    if decisions.get("schema_version") != EXPECTED_SCHEMA:
        raise DecisionLedgerError("unexpected runtime fan-in decision schema")
    if decisions.get("phase") != "partial-classification-and-remediation":
        raise DecisionLedgerError(
            "Step 22E must remain partial-classification-and-remediation"
        )
    if set(decisions.get("allowed_final_classifications", [])) != FINAL_CLASSIFICATIONS:
        raise DecisionLedgerError("ADR-032 final classification enum changed")

    source = decisions.get("inventory_source")
    if not isinstance(source, dict):
        raise DecisionLedgerError("inventory_source is required")
    if source.get("path") != "step22-architecture-inventory.json":
        raise DecisionLedgerError("decision ledger must bind the accepted inventory path")
    if source.get("accepted_source") != "ffb8c94373c565de00cccd67c38c80bdb3a12405":
        raise DecisionLedgerError("decision ledger accepted source is not PR #298")
    if source.get("merge_commit") != "4642ea39a7c1c8ad78b1d475a3d5391af8414555":
        raise DecisionLedgerError("decision ledger baseline is not the Step 22A merge")

    inventory_by_id = inventory_rows(inventory)
    final_columns = decisions.get("final_row_columns")
    if final_columns != ["stable_id", "classification", "boundary_id"]:
        raise DecisionLedgerError("final decision row columns changed")
    raw_final_rows = decisions.get("final_rows")
    if not isinstance(raw_final_rows, list):
        raise DecisionLedgerError("final_rows must be a list")

    boundaries = decisions.get("boundary_definitions")
    if not isinstance(boundaries, dict) or not boundaries:
        raise DecisionLedgerError("boundary_definitions must be non-empty")

    final_by_id: dict[str, tuple[str, str]] = {}
    for raw in raw_final_rows:
        if not isinstance(raw, list) or len(raw) != 3:
            raise DecisionLedgerError("invalid final decision row")
        stable_id, classification, boundary_id = raw
        if stable_id in final_by_id:
            raise DecisionLedgerError(f"duplicate final stable_id: {stable_id}")
        if stable_id not in inventory_by_id:
            raise DecisionLedgerError(f"unknown final stable_id: {stable_id}")
        if classification not in FINAL_CLASSIFICATIONS:
            raise DecisionLedgerError(f"invalid classification for {stable_id}")
        boundary = boundaries.get(boundary_id)
        if not isinstance(boundary, dict):
            raise DecisionLedgerError(f"unknown boundary definition: {boundary_id}")
        if boundary.get("classification") != classification:
            raise DecisionLedgerError(
                f"boundary classification mismatch for {stable_id}"
            )
        for field in ("owner", "prevented_boundary", "review_condition"):
            value = boundary.get(field)
            if not isinstance(value, str) or not value.strip():
                raise DecisionLedgerError(
                    f"boundary {boundary_id} requires non-empty {field}"
                )
        evidence_paths = boundary.get("evidence_paths")
        if not isinstance(evidence_paths, list) or not evidence_paths:
            raise DecisionLedgerError(
                f"boundary {boundary_id} requires evidence_paths"
            )
        for relative in evidence_paths:
            if not isinstance(relative, str) or not (root / relative).is_file():
                raise DecisionLedgerError(
                    f"boundary {boundary_id} has missing evidence path: {relative}"
                )

        inventory_row = inventory_by_id[stable_id]
        if classification == "platform-generic":
            if inventory_row["dependency_kind"] != "production":
                raise DecisionLedgerError(
                    f"platform-generic entry must be production: {stable_id}"
                )
            if inventory_row["target_category"] != "technical-crate":
                raise DecisionLedgerError(
                    f"platform-generic entry must target a technical crate: {stable_id}"
                )
        elif classification == "test-only":
            if inventory_row["dependency_kind"] != "test-only":
                raise DecisionLedgerError(
                    f"test-only entry is not isolated in dev-dependencies: {stable_id}"
                )
        elif classification == "removed":
            if stable_id not in EXPECTED_REMOVED_STABLE_IDS:
                raise DecisionLedgerError(
                    f"Step 22E does not authorize another removal: {stable_id}"
                )
        else:
            raise DecisionLedgerError(
                "Step 22E cannot record owner-specific-unavoidable without the "
                "complete ADR-032 evidence contract"
            )
        final_by_id[stable_id] = (classification, boundary_id)

    unresolved = set(inventory_by_id) - set(final_by_id)
    if set(final_by_id) | unresolved != set(inventory_by_id):
        raise DecisionLedgerError("decision coverage does not match inventory")

    computed = Counter(classification for classification, _ in final_by_id.values())
    computed_counts = {
        "all": len(inventory_by_id),
        "final": len(final_by_id),
        "platform_generic": computed["platform-generic"],
        "test_only": computed["test-only"],
        "removed": computed["removed"],
        "owner_specific_unavoidable": computed["owner-specific-unavoidable"],
        "unresolved": len(unresolved),
    }
    if decisions.get("counts") != computed_counts:
        raise DecisionLedgerError(
            f"decision counts changed: expected {computed_counts}, "
            f"got {decisions.get('counts')}"
        )

    expected_boundary = {
        "all_dependencies_present": True,
        "final_classifications_recorded": False,
        "gate_dispositions_recorded": False,
        "owner_specific_unavoidable_recorded": False,
        "remediation_performed": True,
        "step22_complete": False,
    }
    if decisions.get("decision_boundary") != expected_boundary:
        raise DecisionLedgerError("Step 22E decision boundary is overstated")
    if not unresolved:
        raise DecisionLedgerError(
            "Step 22E must not claim full classification or Step 22 closure"
        )

    validate_remediation(root, decisions, set(inventory_by_id), final_by_id)
    return computed_counts


def validate_decisions(root: Path = ROOT) -> dict[str, int]:
    validate_governance_registration(root)
    decisions, decisions_text = load_json(root, DECISIONS_PATH)
    if decisions_text != canonical_json(decisions):
        raise DecisionLedgerError(
            f"{DECISIONS_PATH} is not canonical sorted JSON"
        )
    inventory_path = Path(decisions["inventory_source"]["path"])
    inventory, _ = load_json(root, inventory_path)
    return validate_payload(root, decisions, inventory)


def main() -> int:
    try:
        counts = validate_decisions(ROOT)
    except (DecisionLedgerError, OSError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(
        "Step 22 runtime fan-in decisions passed: "
        f"{counts['final']} final "
        f"({counts['platform_generic']} platform-generic, "
        f"{counts['test_only']} test-only, "
        f"{counts['removed']} removed), "
        f"{counts['unresolved']} unresolved."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
