#!/usr/bin/env python3
"""Materialize the bounded Repository Step 22D packet."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASELINE = "9b2495c9a594f5539aa586f6d775a8ea12442a48"
PRIVACY_ID = "crm-application-runtime::dependencies::crm-customer-privacy-query-adapter"
CUSTOMER_360_ID = "crm-application-runtime::dependencies::crm-customer-360-query-adapter"


def canonical(payload: object) -> str:
    return json.dumps(payload, indent=2, sort_keys=True) + "\n"


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content, encoding="utf-8")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


def replace_between(text: str, start: str, end: str, replacement: str, label: str) -> str:
    start_index = text.find(start)
    if start_index < 0:
        raise RuntimeError(f"{label}: start marker missing")
    end_index = text.find(end, start_index)
    if end_index < 0:
        raise RuntimeError(f"{label}: end marker missing")
    return text[:start_index] + replacement + text[end_index:]


def remediation_evidence() -> dict[str, object]:
    return {
        "after": {"all": 61, "production": 60, "test_only": 1},
        "before": {"all": 63, "production": 62, "test_only": 1},
        "removals": [
            {
                "adapter_package": "crm-customer-360-query-adapter",
                "owner_manifest": "crates/crm-first-party-modules/Cargo.toml",
                "owner_sources": ["crates/crm-first-party-modules/src/lib.rs"],
                "replacement_boundary": "crm-first-party-modules",
                "runtime_sources": [
                    "crates/crm-application-runtime/src/bootstrap_visibility/registry.rs"
                ],
                "stable_id": CUSTOMER_360_ID,
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
                "stable_id": PRIVACY_ID,
            },
        ],
        "removed_stable_ids": [CUSTOMER_360_ID, PRIVACY_ID],
        "runtime_manifest": "crates/crm-application-runtime/Cargo.toml",
    }


def materialize_decisions() -> None:
    path = ROOT / "step22-runtime-fanin-decisions.json"
    payload = json.loads(path.read_text(encoding="utf-8"))
    boundary_id = "removed-customer-360-query-adapter"
    payload["boundary_definitions"][boundary_id] = {
        "classification": "removed",
        "evidence_paths": [
            "crates/crm-application-runtime/Cargo.toml",
            "crates/crm-application-runtime/src/bootstrap_visibility/registry.rs",
            "crates/crm-first-party-modules/Cargo.toml",
            "crates/crm-first-party-modules/src/lib.rs",
        ],
        "owner": "customer-360",
        "prevented_boundary": (
            "Keeps Customer 360 query-adapter identity and query inventory behind the "
            "existing first-party owner aggregation boundary instead of the generic process host."
        ),
        "review_condition": (
            "Fail if crm-application-runtime regains a direct Customer 360 query-adapter "
            "dependency or source import."
        ),
    }
    rows = [row for row in payload["final_rows"] if row[0] != CUSTOMER_360_ID]
    rows.append([CUSTOMER_360_ID, "removed", boundary_id])
    payload["final_rows"] = sorted(rows, key=lambda row: row[0])
    payload["counts"] = {
        "all": 63,
        "final": 19,
        "owner_specific_unavoidable": 0,
        "platform_generic": 16,
        "removed": 2,
        "test_only": 1,
        "unresolved": 44,
    }
    payload["remediation_evidence"] = remediation_evidence()
    path.write_text(canonical(payload), encoding="utf-8")


def materialize_validator() -> None:
    path = "scripts/check_step22_runtime_fanin_decisions.py"
    text = read(path)
    constants = '''REMOVED_PRIVACY_QUERY_STABLE_ID = (\n    "crm-application-runtime::dependencies::crm-customer-privacy-query-adapter"\n)\nREMOVED_CUSTOMER_360_QUERY_STABLE_ID = (\n    "crm-application-runtime::dependencies::crm-customer-360-query-adapter"\n)\nEXPECTED_REMOVED_STABLE_IDS = {\n    REMOVED_CUSTOMER_360_QUERY_STABLE_ID,\n    REMOVED_PRIVACY_QUERY_STABLE_ID,\n}\nEXPECTED_REGISTRATION = {\n    "id": "repository-step-22-runtime-fanin",\n    "owner": "architecture-governance",\n    "path": DECISIONS_PATH.as_posix(),\n    "review_condition": (\n        "Review on every accepted runtime fan-in classification or remediation "\n        "packet until Repository Step 22 closure."\n    ),\n    "tracking_issue": "#194",\n    "validator": "scripts/check_step22_runtime_fanin_decisions.py",\n}\nEXPECTED_REMEDIATION = {\n    "after": {"all": 61, "production": 60, "test_only": 1},\n    "before": {"all": 63, "production": 62, "test_only": 1},\n    "removals": [\n        {\n            "adapter_package": "crm-customer-360-query-adapter",\n            "owner_manifest": "crates/crm-first-party-modules/Cargo.toml",\n            "owner_sources": ["crates/crm-first-party-modules/src/lib.rs"],\n            "replacement_boundary": "crm-first-party-modules",\n            "runtime_sources": [\n                "crates/crm-application-runtime/src/bootstrap_visibility/registry.rs"\n            ],\n            "stable_id": REMOVED_CUSTOMER_360_QUERY_STABLE_ID,\n        },\n        {\n            "adapter_package": "crm-customer-privacy-query-adapter",\n            "owner_manifest": "crates/crm-customer-privacy-production/Cargo.toml",\n            "owner_sources": [\n                "crates/crm-customer-privacy-production/src/legal_hold.rs",\n                "crates/crm-customer-privacy-production/src/root.rs",\n            ],\n            "replacement_boundary": "crm-customer-privacy-production",\n            "runtime_sources": [\n                "crates/crm-application-runtime/src/bootstrap_visibility/registry.rs",\n                "crates/crm-application-runtime/src/customer_privacy_case_create_promotion.rs",\n            ],\n            "stable_id": REMOVED_PRIVACY_QUERY_STABLE_ID,\n        },\n    ],\n    "removed_stable_ids": [\n        REMOVED_CUSTOMER_360_QUERY_STABLE_ID,\n        REMOVED_PRIVACY_QUERY_STABLE_ID,\n    ],\n    "runtime_manifest": "crates/crm-application-runtime/Cargo.toml",\n}\n'''
    text = replace_between(
        text,
        "REMOVED_QUERY_STABLE_ID = (",
        "\n\n\nclass DecisionLedgerError",
        constants,
        "validator constants",
    )
    validation = '''def validate_remediation(\n    root: Path,\n    decisions: dict[str, Any],\n    accepted_ids: set[str],\n    final_by_id: dict[str, tuple[str, str]],\n) -> None:\n    remediation = decisions.get("remediation_evidence")\n    if remediation != EXPECTED_REMEDIATION:\n        raise DecisionLedgerError("Step 22 cumulative remediation evidence changed")\n\n    current_ids = current_runtime_direct_ids(\n        root, EXPECTED_REMEDIATION["runtime_manifest"]\n    )\n    removed_ids = {\n        stable_id\n        for stable_id, (classification, _) in final_by_id.items()\n        if classification == "removed"\n    }\n    if removed_ids != EXPECTED_REMOVED_STABLE_IDS:\n        raise DecisionLedgerError(\n            "Step 22D must record exactly the accepted Customer Privacy and "\n            "Customer 360 query-adapter removals"\n        )\n    if current_ids != accepted_ids - removed_ids:\n        added = sorted(current_ids - accepted_ids)\n        missing = sorted((accepted_ids - removed_ids) - current_ids)\n        raise DecisionLedgerError(\n            "current runtime direct dependency surface differs from the accepted "\n            f"inventory minus the cumulative removals: added={added}, missing={missing}"\n        )\n\n    production = sum("::dependencies::" in stable_id for stable_id in current_ids)\n    test_only = sum("::dev-dependencies::" in stable_id for stable_id in current_ids)\n    current_counts = {\n        "all": len(current_ids),\n        "production": production,\n        "test_only": test_only,\n    }\n    if current_counts != EXPECTED_REMEDIATION["after"]:\n        raise DecisionLedgerError(\n            f"current runtime fan-in is not the exact cumulative 63 to 61 reduction: {current_counts}"\n        )\n\n    runtime_manifest = tomllib.loads(\n        (root / EXPECTED_REMEDIATION["runtime_manifest"]).read_text(encoding="utf-8")\n    )\n    runtime_dependencies = runtime_manifest.get("dependencies", {})\n    lockfile = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))\n    packages = lockfile.get("package", [])\n    if not isinstance(packages, list):\n        raise DecisionLedgerError("Cargo.lock package inventory is missing")\n\n    def lock_package(name: str) -> dict[str, Any]:\n        matching = [\n            package\n            for package in packages\n            if isinstance(package, dict) and package.get("name") == name\n        ]\n        if len(matching) != 1:\n            raise DecisionLedgerError(\n                f"Cargo.lock must contain exactly one {name} package record"\n            )\n        return matching[0]\n\n    runtime_lock_dependencies = lock_package("crm-application-runtime").get(\n        "dependencies", []\n    )\n    if "prost" not in runtime_lock_dependencies:\n        raise DecisionLedgerError(\n            "Cargo.lock does not retain the canonical runtime prost reference"\n        )\n\n    for removal in EXPECTED_REMEDIATION["removals"]:\n        adapter = removal["adapter_package"]\n        if adapter in runtime_dependencies:\n            raise DecisionLedgerError(\n                f"runtime manifest still records removed direct edge: {adapter}"\n            )\n        owner_manifest = tomllib.loads(\n            (root / removal["owner_manifest"]).read_text(encoding="utf-8")\n        )\n        if adapter not in owner_manifest.get("dependencies", {}):\n            raise DecisionLedgerError(\n                f"owner boundary no longer retains adapter internally: {adapter}"\n            )\n        if adapter in runtime_lock_dependencies:\n            raise DecisionLedgerError(\n                f"Cargo.lock still records removed direct runtime edge: {adapter}"\n            )\n        owner_package = Path(removal["owner_manifest"]).parent.name\n        if adapter not in lock_package(owner_package).get("dependencies", []):\n            raise DecisionLedgerError(\n                f"Cargo.lock no longer records owner-internal adapter: {adapter}"\n            )\n        lock_package(adapter)\n        rust_marker = adapter.replace("-", "_")\n        replacement_marker = removal["replacement_boundary"].replace("-", "_")\n        runtime_text = "\\n".join(\n            (root / source).read_text(encoding="utf-8")\n            for source in removal["runtime_sources"]\n        )\n        if rust_marker in runtime_text:\n            raise DecisionLedgerError(\n                f"generic runtime source still imports removed adapter directly: {adapter}"\n            )\n        if replacement_marker not in runtime_text:\n            raise DecisionLedgerError(\n                f"runtime source is missing replacement boundary marker: {replacement_marker}"\n            )\n\n    first_party_source = (\n        root / "crates/crm-first-party-modules/src/lib.rs"\n    ).read_text(encoding="utf-8")\n    for marker in (\n        "MODULE_ID as CUSTOMER_360_MODULE_ID",\n        "query_capability_definitions as customer_360_query_capability_definitions",\n    ):\n        if marker not in first_party_source:\n            raise DecisionLedgerError(\n                f"first-party boundary is missing Customer 360 marker: {marker}"\n            )\n    registry_source = (\n        root / "crates/crm-application-runtime/src/bootstrap_visibility/registry.rs"\n    ).read_text(encoding="utf-8")\n    if "crm_first_party_modules::CUSTOMER_360_MODULE_ID" not in registry_source:\n        raise DecisionLedgerError(\n            "bootstrap visibility does not consume Customer 360 identity through first-party boundary"\n        )\n\n    privacy_legal_hold = (\n        root / "crates/crm-customer-privacy-production/src/legal_hold.rs"\n    ).read_text(encoding="utf-8")\n    if (\n        "pub use crm_customer_privacy_query_adapter::"\n        "control_query_capability_definitions;"\n        not in privacy_legal_hold\n    ):\n        raise DecisionLedgerError(\n            "Customer Privacy production does not expose the control query inventory"\n        )\n    privacy_root = (\n        root / "crates/crm-customer-privacy-production/src/root.rs"\n    ).read_text(encoding="utf-8")\n    if "control_query_visibility_resources" not in privacy_root:\n        raise DecisionLedgerError(\n            "Customer Privacy production does not expose control visibility resources"\n        )\n'''
    text = replace_between(
        text,
        "def validate_remediation(",
        "\n\ndef validate_payload(",
        validation,
        "validator remediation",
    )
    old_removed = '''        elif classification == "removed":\n            if stable_id != REMOVED_QUERY_STABLE_ID:\n                raise DecisionLedgerError(\n                    f"Step 22C does not authorize another removal: {stable_id}"\n                )\n'''
    new_removed = '''        elif classification == "removed":\n            if stable_id not in EXPECTED_REMOVED_STABLE_IDS:\n                raise DecisionLedgerError(\n                    f"Step 22D does not authorize another removal: {stable_id}"\n                )\n'''
    text = replace_once(text, old_removed, new_removed, "validator removed enum")
    text = text.replace("Step 22C must remain partial-classification-and-remediation", "Step 22D must remain partial-classification-and-remediation")
    text = text.replace("Step 22C cannot record owner-specific-unavoidable", "Step 22D cannot record owner-specific-unavoidable")
    text = text.replace("Step 22C decision boundary is overstated", "Step 22D decision boundary is overstated")
    text = text.replace("Step 22C must not claim full classification or Step 22 closure", "Step 22D must not claim full classification or Step 22 closure")
    write(path, text)


def packet_payload() -> dict[str, object]:
    return {
        "acceptance": [
            f"the branch is based exactly on main commit {BASELINE}",
            "the accepted Step 22A inventory remains immutable with exact baseline counts sixty-three total sixty-two production one test-only forty-one workflows and forty-two jobs",
            "the cumulative removed stable-ID set contains exactly the Customer Privacy and Customer 360 query-adapter edges",
            "the current runtime direct stable-ID set equals the accepted inventory minus exactly those two removals with exact counts sixty-one total sixty production and one test-only",
            "crm-application-runtime Cargo.toml source and Cargo.lock no longer name crm-customer-360-query-adapter while crm-first-party-modules retains it internally and exposes CUSTOMER_360_MODULE_ID",
            f"Cargo.lock equals immutable baseline commit {BASELINE} byte-for-byte except for deletion of exactly one crm-customer-360-query-adapter dependency line from the crm-application-runtime package record",
            "the Customer 360 query inventory contribution visibility resources routes persistence and authorization behavior remain unchanged",
            "the decision ledger contains exactly nineteen final classifications two removals zero owner-specific-unavoidable and forty-four unresolved dependencies",
            "the conservative public Rust surface remains exactly 5377",
            "generated contracts schemas migrations workflows job topology and permanent-gate dispositions remain unchanged",
            "the final diff changes only declared lockfile runtime first-party decision documentation packet generated-navigation validator and guard paths",
            "one unchanged exact head passes every applicable permanent workflow with zero unresolved comments reviews or review threads",
        ],
        "allowed_paths": [
            "Cargo.lock",
            "crates/crm-application-runtime/Cargo.toml",
            "crates/crm-application-runtime/src/bootstrap_visibility/registry.rs",
            "crates/crm-first-party-modules/src/lib.rs",
            "docs/ACTIVE_PACKET.md",
            "docs/STEP22_CUSTOMER_360_QUERY_FANIN_REDUCTION.md",
            "docs/generated/REPOSITORY_MAP.md",
            "repository-packet.json",
            "scripts/check_step22_runtime_fanin_decisions.py",
            "step22-runtime-fanin-decisions.json",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_repository_navigation.py",
            "tests/test_workspace_analysis.py",
        ],
        "baseline": {"ref": "main", "sha": BASELINE},
        "deliverables": [
            "publicly expose CUSTOMER_360_MODULE_ID through crm-first-party-modules which already owns the Customer 360 query adapter and query inventory",
            "consume Customer 360 module identity through crm_first_party_modules and remove every direct crm_customer_360_query_adapter source import from crm-application-runtime",
            "remove only crm-customer-360-query-adapter from crm-application-runtime Cargo.toml without adding or moving another direct dependency",
            f"synchronize Cargo.lock from immutable baseline {BASELINE} by deleting exactly the crm-customer-360-query-adapter line from the crm-application-runtime dependency list while every other byte remains unchanged",
            "record the second exact removed stable ID with fan-in evidence of 62 to 61 total and 61 to 60 production dependencies",
            "generalize the permanent validator to prove the accepted inventory minus the exact cumulative removal set while retaining each adapter behind its owner boundary",
            "record exactly nineteen final classifications with sixteen platform-generic one test-only two removed zero owner-specific-unavoidable and forty-four unresolved",
            "update permanent packet architecture documentation guards and regenerate active packet navigation",
            "preserve Customer 360 query composition visibility resources runtime behavior and all workflows contracts schemas migrations and permanent-gate dispositions",
        ],
        "forbidden_paths": [
            ".github/workflows/**",
            "AGENTS.md",
            "README.md",
            "affected-scope-policy.json",
            "apps/**",
            "architecture-governance.json",
            "contracts/**",
            "customer-privacy-operations-policy.json",
            "database/**",
            "evidence/**",
            "modules/**",
            "package.json",
            "packages/**",
            "pnpm-lock.yaml",
            "pnpm-workspace.yaml",
            "proto/**",
            "requirements-dev.txt",
            "rust-toolchain.toml",
            "schemas/**",
            "services/**",
            "step13-complexity-policy.json",
            "step13-suppression-baseline.json",
            "step22-architecture-inventory.json",
            "tsconfig.base.json",
            "workspace-dependency-policy.json",
        ],
        "non_goals": [
            "remove the crm-customer-360-query-adapter package or remove it from crm-first-party-modules",
            "classify crm-first-party-modules or another owner-specific dependency as unavoidable",
            "remove move add or otherwise remediate another crm-application-runtime dependency",
            "change any Cargo.lock byte package identity version source checksum or package record outside the exact runtime Customer 360 dependency-line deletion",
            "change Customer 360 capability IDs schemas query results composition visibility authorization or persistence behavior",
            "assign retain simplify merge or remove dispositions to permanent workflows jobs or repository gates",
            "add remove rename or modify a permanent workflow job or repository gate",
            "rewrite the accepted Step 22A inventory snapshot to match the remediated current state",
            "declare all runtime classifications complete complete Repository Step 22 start Phase 8B raise an architecture score or declare architecture 10/10",
        ],
        "objective": "Execute one bounded ADR-032 Step 22 remediation by exposing the existing Customer 360 module identity through the first-party owner aggregation boundary and removing the redundant direct crm-customer-360-query-adapter dependency and source import from crm-application-runtime without changing behavior or the owner-internal dependency graph.",
        "packet_id": "repository-step-22d-customer-360-query-fanin-reduction",
        "required_checks": [
            "Affected Scope CI",
            "Application Runtime CI",
            "Complexity Baseline CI",
            "Governance CI",
            "Rust Generated Sync",
            "Rust CI",
        ],
        "schema_version": "crm.repository-packet/v1",
        "status": "active",
        "title": "Remove redundant Customer 360 query-adapter runtime fan-in",
        "tracking_issues": [194],
    }


def materialize_packet_and_document() -> None:
    write("repository-packet.json", canonical(packet_payload()))
    document = f'''# Repository Step 22D — Customer 360 Query Fan-In Reduction

Status: **Active bounded remediation packet**  
Tracking issue: #194  
Binding decision: `docs/adr/ADR-032-step-22-runtime-fanin-and-permanent-gate-value.md`  
Baseline: PR #300 squash merge `{BASELINE}`

## Purpose

Step 22A measured 63 internal direct dependencies of `crm-application-runtime`. Step 22C removed the Customer Privacy query-adapter edge and reduced the current surface to 62. Step 22D removes one more redundant owner-adapter edge.

`crm-first-party-modules` already owns `crm-customer-360-query-adapter`, builds its production contribution and exposes its query inventory. The generic runtime used the adapter directly only to read `MODULE_ID` while registering bootstrap visibility.

Step 22D exposes that module identity through the existing first-party boundary and removes the direct process-host dependency.

## Exact before and after

| Metric | Step 22C merged state | Step 22D candidate | Delta |
|---|---:|---:|---:|
| Internal direct dependencies | 62 | 61 | -1 |
| Production internal direct dependencies | 61 | 60 | -1 |
| Test-only internal direct dependencies | 1 | 1 | 0 |
| Conservative public Rust surface | 5,377 | 5,377 | 0 |
| Final ADR-032 classifications | 18 | 19 | +1 |
| Cumulative `removed` | 1 | 2 | +1 |
| Unresolved accepted inventory dependencies | 45 | 44 | -1 |

Removed stable ID:

`{CUSTOMER_360_ID}`

Replacement boundary:

`crm-first-party-modules`

## Boundary-preserving implementation

1. `crm-first-party-modules` re-exports `MODULE_ID` as `CUSTOMER_360_MODULE_ID` alongside its existing Customer 360 query inventory export.
2. `bootstrap_visibility/registry.rs` imports that constant through `crm_first_party_modules`.
3. `crm-customer-360-query-adapter` is removed only from `crm-application-runtime/Cargo.toml`.
4. `crm-first-party-modules` retains the adapter internally and continues to build the same production contribution.
5. The grouped re-export replaces an existing public re-export line, keeping the conservative public Rust surface at exactly 5,377.

No capability coordinate, query inventory, visibility resource, persistence path, authorization rule or runtime route changes.

## Exact lockfile synchronization

The lockfile proof is pinned to immutable baseline `{BASELINE}`. The only accepted change is deletion of:

`"crm-customer-360-query-adapter",`

from the `crm-application-runtime` dependency list. Registry package versions, sources, checksums and all other package records must remain byte-identical to the baseline.

## Mechanical proof

Run:

```bash
python scripts/check_step22_runtime_fanin_decisions.py
```

The validator requires the current direct dependency set to equal the accepted 63-row Step 22A inventory minus exactly the Customer Privacy and Customer 360 query-adapter edges. Current counts must be 61 total, 60 production and 1 test-only. Both adapter packages must remain present behind their owner boundaries.

## Decision boundary

This packet does not remove either adapter package, remediate another dependency, classify any owner-specific edge as unavoidable, change a workflow or gate disposition, complete Repository Step 22, start Phase 8B or declare architecture 10/10.

## Next Step 22 work

After acceptance, 44 dependencies from the original inventory remain unresolved. Each further reduction remains a separate measured packet.
'''
    write("docs/STEP22_CUSTOMER_360_QUERY_FANIN_REDUCTION.md", document)


def materialize_navigation_test() -> None:
    path = "tests/test_repository_navigation.py"
    text = read(path)
    method = '''    def test_active_step_22d_customer_360_query_fanin_packet_is_exact(self) -> None:\n        packet = load_packet(ROOT)\n        self.assertEqual(packet["schema_version"], "crm.repository-packet/v1")\n        self.assertEqual(\n            packet["packet_id"],\n            "repository-step-22d-customer-360-query-fanin-reduction",\n        )\n        self.assertEqual(packet["status"], "active")\n        self.assertEqual(\n            packet["baseline"],\n            {"ref": "main", "sha": "9b2495c9a594f5539aa586f6d775a8ea12442a48"},\n        )\n        self.assertEqual(packet["tracking_issues"], [194])\n        allowed_paths = set(packet["allowed_paths"])\n        self.assertTrue(\n            {\n                "Cargo.lock",\n                "crates/crm-application-runtime/Cargo.toml",\n                "crates/crm-application-runtime/src/bootstrap_visibility/registry.rs",\n                "crates/crm-first-party-modules/src/lib.rs",\n                "docs/STEP22_CUSTOMER_360_QUERY_FANIN_REDUCTION.md",\n                "scripts/check_step22_runtime_fanin_decisions.py",\n                "step22-runtime-fanin-decisions.json",\n                "tests/test_architecture_documentation_consistency.py",\n                "tests/test_repository_navigation.py",\n                "tests/test_workspace_analysis.py",\n            }.issubset(allowed_paths)\n        )\n        self.assertNotIn(".github/workflows/rust-generated-sync.yml", allowed_paths)\n        forbidden_paths = set(packet["forbidden_paths"])\n        self.assertIn(".github/workflows/**", forbidden_paths)\n        self.assertNotIn("Cargo.toml", allowed_paths)\n        self.assertNotIn("Cargo.toml", forbidden_paths)\n        self.assertIn("step22-architecture-inventory.json", forbidden_paths)\n        deliverables = " ".join(packet["deliverables"])\n        non_goals = " ".join(packet["non_goals"])\n        self.assertIn("62 to 61", deliverables)\n        self.assertIn("61 to 60", deliverables)\n        self.assertIn("remediate another crm-application-runtime dependency", non_goals)\n        self.assertIn("declare all runtime classifications complete", non_goals)\n        self.assertEqual(\n            validate_decisions(ROOT),\n            {\n                "all": 63,\n                "final": 19,\n                "platform_generic": 16,\n                "test_only": 1,\n                "removed": 2,\n                "owner_specific_unavoidable": 0,\n                "unresolved": 44,\n            },\n        )\n\n'''
    text = replace_between(
        text,
        "    def test_active_step_22c_customer_privacy_query_fanin_packet_is_exact",
        "    def test_generated_navigation_is_deterministic_and_current",
        method,
        "navigation packet method",
    )
    text = text.replace(
        "repository-step-22c-customer-privacy-query-fanin-reduction",
        "repository-step-22d-customer-360-query-fanin-reduction",
    )
    text = text.replace(
        "6fe0e8e7702b01a78f5db3f174c09b686de27402",
        BASELINE,
    )
    text = text.replace(
        "Step 22C Customer Privacy query fan-in reduction",
        "Step 22D Customer 360 query fan-in reduction",
    )
    write(path, text)


def materialize_architecture_test() -> None:
    path = "tests/test_architecture_documentation_consistency.py"
    text = read(path)
    method = '''    def test_active_step_22d_customer_360_query_fanin_packet_is_exact(self) -> None:\n        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")\n        self.assertEqual(\n            self.packet["packet_id"],\n            "repository-step-22d-customer-360-query-fanin-reduction",\n        )\n        self.assertEqual(\n            self.packet["baseline"],\n            {"ref": "main", "sha": "9b2495c9a594f5539aa586f6d775a8ea12442a48"},\n        )\n        self.assertEqual(self.packet["tracking_issues"], [194])\n        allowed_paths = set(self.packet["allowed_paths"])\n        for path in (\n            "Cargo.lock",\n            "crates/crm-application-runtime/Cargo.toml",\n            "crates/crm-application-runtime/src/bootstrap_visibility/registry.rs",\n            "crates/crm-first-party-modules/src/lib.rs",\n            "docs/STEP22_CUSTOMER_360_QUERY_FANIN_REDUCTION.md",\n            "scripts/check_step22_runtime_fanin_decisions.py",\n            "step22-runtime-fanin-decisions.json",\n            "tests/test_architecture_documentation_consistency.py",\n            "tests/test_repository_navigation.py",\n            "tests/test_workspace_analysis.py",\n        ):\n            self.assertIn(path, allowed_paths)\n        forbidden_paths = set(self.packet["forbidden_paths"])\n        self.assertIn(".github/workflows/**", forbidden_paths)\n        self.assertNotIn("Cargo.toml", allowed_paths)\n        self.assertNotIn("Cargo.toml", forbidden_paths)\n        self.assertIn("step22-architecture-inventory.json", forbidden_paths)\n        self.assertIn(self.packet["packet_id"], self.active_packet)\n        self.assertIn(self.packet["baseline"]["sha"], self.active_packet)\n\n        counts = validate_decisions(ROOT)\n        self.assertEqual(\n            counts,\n            {\n                "all": 63,\n                "final": 19,\n                "platform_generic": 16,\n                "test_only": 1,\n                "removed": 2,\n                "owner_specific_unavoidable": 0,\n                "unresolved": 44,\n            },\n        )\n        non_goals = " ".join(self.packet["non_goals"])\n        self.assertIn("classify crm-first-party-modules", non_goals)\n        self.assertIn("remediate another crm-application-runtime dependency", non_goals)\n        self.assertIn("declare all runtime classifications complete", non_goals)\n\n        operations_scope = next(\n            scope\n            for scope in self.affected_scope_policy["scopes"]\n            if scope["id"] == "operations"\n        )\n        self.assertEqual(operations_scope["owner"], "platform-operations")\n        for path in (\n            "scripts/check_step22_runtime_fanin_decisions.py",\n            "step22-runtime-fanin-decisions.json",\n        ):\n            self.assertIn(path, operations_scope["path_patterns"])\n        self.assertEqual(operations_scope["required_workflows"], ["Governance CI"])\n\n        step22c = read("docs/STEP22_CUSTOMER_PRIVACY_QUERY_FANIN_REDUCTION.md")\n        for marker in ("63", "62", "18", "45", "Customer Privacy"):\n            self.assertIn(marker, step22c)\n\n        step22d = read("docs/STEP22_CUSTOMER_360_QUERY_FANIN_REDUCTION.md")\n        for marker in (\n            "62",\n            "61",\n            "19",\n            "44",\n            "crm-application-runtime::dependencies::crm-customer-360-query-adapter",\n            "9b2495c9a594f5539aa586f6d775a8ea12442a48",\n            "Repository Step 22",\n        ):\n            self.assertIn(marker, step22d)\n\n'''
    text = replace_between(
        text,
        "    def test_active_step_22c_customer_privacy_query_fanin_packet_is_exact",
        "    def test_repository_map_and_product_inventory_remain_exact",
        method,
        "architecture packet method",
    )
    write(path, text)


def materialize_workspace_test() -> None:
    path = "tests/test_workspace_analysis.py"
    text = read(path)
    text = replace_once(
        text,
        '{"all": 62, "production": 61, "test_only": 1, "build": 0},',
        '{"all": 61, "production": 60, "test_only": 1, "build": 0},',
        "workspace fresh counts",
    )
    old_set = '''            {\n                "crm-application-runtime::dependencies::"\n                "crm-customer-privacy-query-adapter"\n            },'''
    new_set = '''            {\n                "crm-application-runtime::dependencies::"\n                "crm-customer-360-query-adapter",\n                "crm-application-runtime::dependencies::"\n                "crm-customer-privacy-query-adapter",\n            },'''
    text = replace_once(text, old_set, new_set, "workspace removed set")
    text = replace_once(
        text,
        "        self.assertEqual(len(current_runtime_ids), 62)",
        "        self.assertEqual(len(current_runtime_ids), 61)",
        "workspace current length",
    )
    text = replace_once(
        text,
        '{"all": 62, "production": 61, "test_only": 1},',
        '{"all": 61, "production": 60, "test_only": 1},',
        "workspace remediation after",
    )
    write(path, text)


def main() -> None:
    materialize_decisions()
    materialize_validator()
    materialize_packet_and_document()
    materialize_navigation_test()
    materialize_architecture_test()
    materialize_workspace_test()


if __name__ == "__main__":
    main()
