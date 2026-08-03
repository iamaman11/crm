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


def published_rows(
    registry: dict[str, Any], plural: str
) -> tuple[list[str], list[list[Any]]]:
    if registry.get("schema_version") != REGISTRY_SCHEMA_VERSION:
        raise ValueError(f"lifecycle registry must use {REGISTRY_SCHEMA_VERSION}")
    if registry.get("policy_schema_version") != POLICY_SCHEMA_VERSION:
        raise ValueError(
            f"lifecycle registry policy schema must use {POLICY_SCHEMA_VERSION}"
        )
    columns = registry.get("published_columns")
    required_columns = ("id", "version", "provider_module_id", "internal_consumers")
    if not isinstance(columns, list) or any(
        column not in columns for column in required_columns
    ):
        raise ValueError(
            "lifecycle registry published_columns must include id, version, "
            "provider_module_id and internal_consumers"
        )
    published = registry.get("published")
    if not isinstance(published, dict):
        raise ValueError("lifecycle registry published must be an object")
    rows = published.get(plural)
    if not isinstance(rows, list):
        raise ValueError(f"lifecycle registry published.{plural} must be a list")
    return columns, rows


def capability_providers(registry: dict[str, Any]) -> dict[tuple[str, str], str]:
    columns, capabilities = published_rows(registry, "capabilities")
    indexes = {column: columns.index(column) for column in columns}
    providers: dict[tuple[str, str], str] = {}
    for index, row in enumerate(capabilities):
        location = f"published.capabilities[{index}]"
        if not isinstance(row, list) or len(row) < len(columns):
            raise ValueError(f"{location} must match published_columns")
        values = {column: row[position] for column, position in indexes.items()}
        capability_id = required_text(values, "id", location)
        version = required_text(values, "version", location)
        provider = required_text(values, "provider_module_id", location)
        consumers = values["internal_consumers"]
        if not isinstance(consumers, list):
            raise ValueError(f"{location}.internal_consumers must be a list")
        coordinate = (capability_id, version)
        if coordinate in providers:
            raise ValueError(f"duplicate published capability {capability_id}@{version}")
        providers[coordinate] = provider
    return providers


def event_delivery_bindings(
    registry: dict[str, Any],
) -> dict[tuple[str, str], tuple[str, tuple[str, ...]]]:
    columns, events = published_rows(registry, "events")
    indexes = {column: columns.index(column) for column in columns}
    bindings: dict[tuple[str, str], tuple[str, tuple[str, ...]]] = {}
    for index, row in enumerate(events):
        location = f"published.events[{index}]"
        if not isinstance(row, list) or len(row) < len(columns):
            raise ValueError(f"{location} must match published_columns")
        values = {column: row[position] for column, position in indexes.items()}
        event_type = required_text(values, "id", location)
        version = required_text(values, "version", location)
        provider = required_text(values, "provider_module_id", location)
        raw_consumers = values["internal_consumers"]
        if not isinstance(raw_consumers, list):
            raise ValueError(f"{location}.internal_consumers must be a list")
        consumers: list[str] = []
        seen_consumers: set[str] = set()
        for consumer_index, raw_consumer in enumerate(raw_consumers):
            consumer = required_text(
                {"consumer": raw_consumer},
                "consumer",
                f"{location}.internal_consumers[{consumer_index}]",
            )
            if consumer in seen_consumers:
                raise ValueError(
                    f"duplicate internal event consumer {consumer} for "
                    f"{event_type}@{version}"
                )
            seen_consumers.add(consumer)
            consumers.append(consumer)
        coordinate = (event_type, version)
        if coordinate in bindings:
            raise ValueError(f"duplicate published event {event_type}@{version}")
        bindings[coordinate] = (provider, tuple(sorted(consumers)))
    return bindings


def telemetry(item: dict[str, Any], location: str) -> tuple[str, int]:
    required_text(item, "owner", location)
    value = item.get("telemetry")
    if not isinstance(value, dict):
        raise ValueError(f"{location}.telemetry must be an object")
    metric = required_text(value, "metric", f"{location}.telemetry")
    lookback_days = value.get("lookback_days")
    if (
        isinstance(lookback_days, bool)
        or not isinstance(lookback_days, int)
        or lookback_days <= 0
    ):
        raise ValueError(f"{location}.telemetry.lookback_days must be positive")
    return metric, lookback_days


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
        metric, lookback_days = telemetry(item, location)
        coordinate = (capability_id, version)
        if coordinate in seen:
            raise ValueError(f"duplicate deprecated capability {capability_id}@{version}")
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
    entries.sort(key=lambda entry: (entry["capability_id"], entry["capability_version"]))
    return entries


def deprecated_event_deliveries(
    policy: dict[str, Any], registry: dict[str, Any]
) -> list[dict[str, Any]]:
    if policy.get("schema_version") != POLICY_SCHEMA_VERSION:
        raise ValueError(f"lifecycle policy must use {POLICY_SCHEMA_VERSION}")
    contracts = policy.get("contracts")
    if not isinstance(contracts, list):
        raise ValueError("lifecycle policy contracts must be a list")
    bindings = event_delivery_bindings(registry)
    entries: list[dict[str, Any]] = []
    seen: set[tuple[str, str]] = set()
    for index, item in enumerate(contracts):
        location = f"contracts[{index}]"
        if not isinstance(item, dict):
            raise ValueError(f"{location} must be an object")
        if item.get("kind") != "event" or item.get("state") != "deprecated":
            continue
        event_type = required_text(item, "id", location)
        version = required_text(item, "version", location)
        metric, lookback_days = telemetry(item, location)
        coordinate = (event_type, version)
        if coordinate in seen:
            raise ValueError(f"duplicate deprecated event {event_type}@{version}")
        binding = bindings.get(coordinate)
        if binding is None:
            raise ValueError(f"deprecated event is not published: {event_type}@{version}")
        seen.add(coordinate)
        provider, consumers = binding
        for consumer in consumers:
            entries.append(
                {
                    "event_type": event_type,
                    "event_version": version,
                    "provider_module_id": provider,
                    "consumer_module_id": consumer,
                    "metric": metric,
                    "lookback_days": lookback_days,
                }
            )
    entries.sort(
        key=lambda entry: (
            entry["event_type"],
            entry["event_version"],
            entry["consumer_module_id"],
        )
    )
    return entries


def render(
    capabilities: list[dict[str, Any]], events: list[dict[str, Any]]
) -> bytes:
    lines = [
        "// @generated by scripts/generate_contract_telemetry_catalog.py; do not edit.\n"
    ]
    if capabilities:
        lines.append(
            "const DEPRECATED_CONTRACTS: &[(&str, &str, &str, &str, u32)] = &[\n"
        )
        for entry in capabilities:
            lines.extend(
                [
                    "    (\n",
                    f'        "{entry["capability_id"]}",\n',
                    f'        "{entry["capability_version"]}",\n',
                    f'        "{entry["owner_module_id"]}",\n',
                    f'        "{entry["metric"]}",\n',
                    f'        {entry["lookback_days"]},\n',
                    "    ),\n",
                ]
            )
        lines.append("];\n")
    else:
        lines.append(
            "const DEPRECATED_CONTRACTS: &[(&str, &str, &str, &str, u32)] = &[];\n"
        )
    if events:
        lines.append(
            "const DEPRECATED_EVENT_DELIVERIES: "
            "&[(&str, &str, &str, &str, &str, u32)] = &[\n"
        )
        for entry in events:
            lines.extend(
                [
                    "    (\n",
                    f'        "{entry["event_type"]}",\n',
                    f'        "{entry["event_version"]}",\n',
                    f'        "{entry["provider_module_id"]}",\n',
                    f'        "{entry["consumer_module_id"]}",\n',
                    f'        "{entry["metric"]}",\n',
                    f'        {entry["lookback_days"]},\n',
                    "    ),\n",
                ]
            )
        lines.append("];\n")
    else:
        lines.append(
            "const DEPRECATED_EVENT_DELIVERIES: "
            "&[(&str, &str, &str, &str, &str, u32)] = &[];\n"
        )
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
        policy = load_policy(args.policy)
        registry = load_registry(args.registry)
        expected = render(
            deprecated_capabilities(policy, registry),
            deprecated_event_deliveries(policy, registry),
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
