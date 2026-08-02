#!/usr/bin/env python3
"""Generate the Rust catalog consumed by production deprecated-contract telemetry."""

from __future__ import annotations

import argparse
import difflib
import json
from pathlib import Path
import re
import sys
from typing import Any

POLICY_SCHEMA_VERSION = "crm.contract-lifecycle-policy/v1"
REGISTRY_SCHEMA_VERSION = "crm.contract-lifecycle/v1"
SAFE_TEXT = re.compile(r"^[A-Za-z0-9_.:+-]+$")


def load_json_object(path: Path, label: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read {label} {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{label} must contain an object")
    return value


def load_policy(path: Path) -> dict[str, Any]:
    return load_json_object(path, "lifecycle policy")


def load_registry(path: Path) -> dict[str, Any]:
    return load_json_object(path, "lifecycle registry")


def required_text(value: dict[str, Any], key: str, location: str) -> str:
    candidate = value.get(key)
    if not isinstance(candidate, str) or not candidate or not SAFE_TEXT.fullmatch(candidate):
        raise ValueError(f"{location}.{key} must be safe non-empty ASCII text")
    return candidate


def capability_providers(registry: dict[str, Any]) -> dict[tuple[str, str], str]:
    if registry.get("schema_version") != REGISTRY_SCHEMA_VERSION:
        raise ValueError(f"lifecycle registry must use {REGISTRY_SCHEMA_VERSION}")
    if registry.get("policy_schema_version") != POLICY_SCHEMA_VERSION:
        raise ValueError(
            f"lifecycle registry policy schema must use {POLICY_SCHEMA_VERSION}"
        )
    columns = registry.get("published_columns")
    required_columns = ("id", "version", "provider_module_id")
    if not isinstance(columns, list) or any(column not in columns for column in required_columns):
        raise ValueError(
            "lifecycle registry published_columns must include id, version and provider_module_id"
        )
    published = registry.get("published")
    if not isinstance(published, dict):
        raise ValueError("lifecycle registry published must be an object")
    capabilities = published.get("capabilities")
    if not isinstance(capabilities, list):
        raise ValueError("lifecycle registry published.capabilities must be a list")
    indexes = {column: columns.index(column) for column in required_columns}
    providers: dict[tuple[str, str], str] = {}
    for index, row in enumerate(capabilities):
        location = f"published.capabilities[{index}]"
        if not isinstance(row, list) or len(row) < len(columns):
            raise ValueError(f"{location} must match published_columns")
        values = {
            column: row[position] for column, position in indexes.items()
        }
        capability_id = required_text(values, "id", location)
        version = required_text(values, "version", location)
        provider = required_text(values, "provider_module_id", location)
        coordinate = (capability_id, version)
        if coordinate in providers:
            raise ValueError(
                f"duplicate published capability {capability_id}@{version}"
            )
        providers[coordinate] = provider
    return providers


def deprecated_capabilities(
    policy: dict[str, Any], registry: dict[str, Any]
) -> list[dict[str, Any]]:
    if policy.get("schema_version") != POLICY_SCHEMA_VERSION:
        raise ValueError(f"lifecycle policy must use {POLICY_SCHEMA_VERSION}")
    contracts = policy.get("contracts")
    if not isinstance(contracts, list):
        raise ValueError("lifecycle policy contracts must be a list")
    providers = capability_providers(registry)
    entries: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    for index, item in enumerate(contracts):
        location = f"contracts[{index}]"
        if not isinstance(item, dict):
            raise ValueError(f"{location} must be an object")
        if item.get("kind") != "capability" or item.get("state") != "deprecated":
            continue
        capability_id = required_text(item, "id", location)
        version = required_text(item, "version", location)
        # This is the governance owner of the lifecycle decision, not the runtime module.
        required_text(item, "owner", location)
        telemetry = item.get("telemetry")
        if not isinstance(telemetry, dict):
            raise ValueError(f"{location}.telemetry must be an object")
        metric = required_text(telemetry, "metric", f"{location}.telemetry")
        lookback_days = telemetry.get("lookback_days")
        if (
            isinstance(lookback_days, bool)
            or not isinstance(lookback_days, int)
            or lookback_days <= 0
        ):
            raise ValueError(
                f"{location}.telemetry.lookback_days must be positive"
            )
        coordinate = (capability_id, version)
        if coordinate in seen:
            raise ValueError(
                f"duplicate deprecated capability {capability_id}@{version}"
            )
        provider = providers.get(coordinate)
        if provider is None:
            raise ValueError(
                f"deprecated capability is not published: {capability_id}@{version}"
            )
        seen.add(coordinate)
        entries.append(
            {
                "capability_id": capability_id,
                "capability_version": version,
                "owner_module_id": provider,
                "metric": metric,
                "lookback_days": lookback_days,
            }
        )
    entries.sort(
        key=lambda entry: (entry["capability_id"], entry["capability_version"])
    )
    return entries


def render(entries: list[dict[str, Any]]) -> bytes:
    lines = [
        "// @generated by scripts/generate_contract_telemetry_catalog.py; do not edit.\n",
        "use crate::contract_usage_telemetry::DeprecatedContractTelemetry;\n\n",
        "pub(crate) const DEPRECATED_CONTRACTS: &[DeprecatedContractTelemetry] = &[\n",
    ]
    for entry in entries:
        lines.extend(
            [
                "    DeprecatedContractTelemetry {\n",
                f'        capability_id: "{entry["capability_id"]}",\n',
                f'        capability_version: "{entry["capability_version"]}",\n',
                f'        owner_module_id: "{entry["owner_module_id"]}",\n',
                f'        metric: "{entry["metric"]}",\n',
                f'        lookback_days: {entry["lookback_days"]},\n',
                "    },\n",
            ]
        )
    lines.append("];\n")
    return "".join(lines).encode("utf-8")


def write_atomic(path: Path, content: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_bytes(content)
    temporary.replace(path)


def check_exact(path: Path, expected: bytes) -> list[str]:
    try:
        actual = path.read_bytes()
    except OSError as error:
        return [f"cannot read generated telemetry catalog {path}: {error}"]
    if actual == expected:
        return []
    diff = "".join(
        difflib.unified_diff(
            actual.decode("utf-8", errors="replace").splitlines(keepends=True),
            expected.decode("utf-8").splitlines(keepends=True),
            fromfile=str(path),
            tofile=f"{path} (generated)",
        )
    )
    return [
        f"{path} is stale; run python scripts/generate_contract_telemetry_catalog.py --write",
        diff.rstrip(),
    ]


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--write", action="store_true")
    parser.add_argument(
        "--policy",
        type=Path,
        default=Path("contracts/contract-lifecycle-policy.json"),
    )
    parser.add_argument(
        "--registry",
        type=Path,
        default=Path("contracts/contract-lifecycle.json"),
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path(
            "crates/crm-application-runtime/src/generated_contract_telemetry.rs"
        ),
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        expected = render(
            deprecated_capabilities(
                load_policy(args.policy), load_registry(args.registry)
            )
        )
    except ValueError as error:
        print(
            f"contract telemetry catalog generation failed: {error}",
            file=sys.stderr,
        )
        return 1
    if args.write:
        write_atomic(args.output, expected)
        print(f"wrote contract telemetry catalog: {args.output}")
        return 0
    errors = check_exact(args.output, expected)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    print("contract telemetry catalog is synchronized")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
