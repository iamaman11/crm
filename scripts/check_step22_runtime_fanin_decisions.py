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
REMOVED_QUERY_STABLE_ID = (
    "crm-application-runtime::dependencies::crm-customer-privacy-query-adapter"
)
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
    "after": {"all": 62, "production": 61, "test_only": 1},
    "before": {"all": 63, "production": 62, "test_only": 1},
    "owner_manifest": "crates/crm-customer-privacy-production/Cargo.toml",
    "owner_source": "crates/crm-customer-privacy-production/src/legal_hold.rs",
    "removed_stable_ids": [REMOVED_QUERY_STABLE_ID],
    "replacement_boundary": "crm-customer-privacy-production",
    "runtime_manifest": "crates/crm-application-runtime/Cargo.toml",
    "runtime_source": (
        "crates/crm-application-runtime/src/customer_privacy_case_create_promotion.rs"
    ),
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
        raise DecisionLedgerError("Step 22C remediation evidence changed")

    current_ids = current_runtime_direct_ids(
        root, EXPECTED_REMEDIATION["runtime_manifest"]
    )
    removed_ids = {
        stable_id
        for stable_id, (classification, _) in final_by_id.items()
        if classification == "removed"
    }
    if removed_ids != {REMOVED_QUERY_STABLE_ID}:
        raise DecisionLedgerError(
            "Step 22C must record exactly the Customer Privacy query adapter removal"
        )
    if current_ids != accepted_ids - removed_ids:
        added = sorted(current_ids - accepted_ids)
        missing = sorted((accepted_ids - removed_ids) - current_ids)
        raise DecisionLedgerError(
            "current runtime direct dependency surface differs from the accepted "
            f"inventory minus the one removal: added={added}, missing={missing}"
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
            f"current runtime fan-in is not the exact 63 to 62 reduction: {current_counts}"
        )

    owner_manifest = tomllib.loads(
        (root / EXPECTED_REMEDIATION["owner_manifest"]).read_text(encoding="utf-8")
    )
    owner_dependencies = owner_manifest.get("dependencies", {})
    if "crm-customer-privacy-query-adapter" not in owner_dependencies:
        raise DecisionLedgerError(
            "Customer Privacy production must retain the query adapter internally"
        )

    runtime_source = (root / EXPECTED_REMEDIATION["runtime_source"]).read_text(
        encoding="utf-8"
    )
    if "crm_customer_privacy_query_adapter" in runtime_source:
        raise DecisionLedgerError(
            "generic runtime source still imports Customer Privacy query adapter directly"
        )
    for marker in (
        "crm_customer_privacy_production",
        "control_query_capability_definitions",
    ):
        if marker not in runtime_source:
            raise DecisionLedgerError(
                f"generic runtime source is missing replacement boundary marker: {marker}"
            )

    owner_source = (root / EXPECTED_REMEDIATION["owner_source"]).read_text(
        encoding="utf-8"
    )
    expected_reexport = (
        "pub use crm_customer_privacy_query_adapter::"
        "control_query_capability_definitions;"
    )
    if expected_reexport not in owner_source:
        raise DecisionLedgerError(
            "Customer Privacy production does not expose the control query inventory"
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
            "Step 22C must remain partial-classification-and-remediation"
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
            if stable_id != REMOVED_QUERY_STABLE_ID:
                raise DecisionLedgerError(
                    f"Step 22C does not authorize another removal: {stable_id}"
                )
        else:
            raise DecisionLedgerError(
                "Step 22C cannot record owner-specific-unavoidable without the "
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
        raise DecisionLedgerError("Step 22C decision boundary is overstated")
    if not unresolved:
        raise DecisionLedgerError(
            "Step 22C must not claim full classification or Step 22 closure"
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
