#!/usr/bin/env python3
"""Validate exact production-route and route-less module classifications."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BINDINGS = ROOT / "contracts/module-contract-bindings.json"
CLASSIFICATIONS = ROOT / "contracts/production-route-classifications.json"

Route = tuple[str, str, str]


def route_coordinate(route: dict[str, object]) -> Route:
    coordinate = tuple(
        str(route.get(key, "")).strip()
        for key in ("owner_module_id", "id", "version")
    )
    if not all(coordinate):
        raise ValueError(f"route classification has an empty coordinate: {route!r}")
    return coordinate  # type: ignore[return-value]


def exact_routes(entries: object, label: str) -> set[Route]:
    if not isinstance(entries, list):
        raise ValueError(f"{label} must be a list")
    coordinates: list[Route] = []
    for entry in entries:
        if not isinstance(entry, dict) or not str(entry.get("reason", "")).strip():
            raise ValueError(f"{label} entries require a non-empty reason")
        coordinates.append(route_coordinate(entry))
    if len(coordinates) != len(set(coordinates)):
        raise ValueError(f"duplicate exact coordinate in {label}")
    return set(coordinates)


def exact_empty_modules(entries: object) -> set[str]:
    if not isinstance(entries, list):
        raise ValueError("empty_runtime_modules must be a list")
    module_ids: list[str] = []
    for entry in entries:
        if not isinstance(entry, dict) or not str(entry.get("reason", "")).strip():
            raise ValueError("empty_runtime_modules entries require a non-empty reason")
        module_id = str(entry.get("module_id", "")).strip()
        if not module_id:
            raise ValueError("empty runtime module classification lacks module_id")
        module_ids.append(module_id)
    if len(module_ids) != len(set(module_ids)):
        raise ValueError("duplicate exact module in empty_runtime_modules")
    return set(module_ids)


def load_and_validate() -> tuple[set[Route], set[Route], set[str]]:
    bindings = json.loads(BINDINGS.read_text(encoding="utf-8"))
    classifications = json.loads(CLASSIFICATIONS.read_text(encoding="utf-8"))
    if bindings.get("schema_version") != "crm.contract-bindings/v1":
        raise ValueError("unexpected contract binding schema version")
    if classifications.get("schema_version") != "crm.production-route-classifications/v1":
        raise ValueError("unexpected production route classification schema version")

    governed = {
        (module["module_id"], capability["id"], capability["version"])
        for module in bindings["modules"]
        for capability in module["capabilities"]
    }
    governed_modules = {module["module_id"] for module in bindings["modules"]}
    bound_empty_modules = {
        module["module_id"]
        for module in bindings["modules"]
        if not module["capabilities"]
    }

    platform = exact_routes(
        classifications.get("platform_runtime_routes"), "platform_runtime_routes"
    )
    non_runtime = exact_routes(
        classifications.get("non_runtime_contract_routes"),
        "non_runtime_contract_routes",
    )
    empty_modules = exact_empty_modules(classifications.get("empty_runtime_modules"))

    if platform & non_runtime:
        raise ValueError("platform and non-runtime route classifications overlap")
    if any(owner in governed_modules for owner, _, _ in platform):
        raise ValueError(
            "platform runtime classifications must not use governed business module owners"
        )
    if not non_runtime <= governed:
        raise ValueError(
            "non-runtime classifications must name governed binding coordinates"
        )
    if empty_modules != bound_empty_modules:
        raise ValueError(
            "route-less module classifications must exactly match modules with no bound capabilities"
        )
    return platform, non_runtime, empty_modules


def main() -> int:
    platform, non_runtime, empty_modules = load_and_validate()
    print(
        "Production route classifications valid: "
        f"{len(platform)} platform routes, {len(non_runtime)} non-runtime routes, "
        f"{len(empty_modules)} route-less modules."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
