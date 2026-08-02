"""Deterministic contract lifecycle inventory and retirement enforcement."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import date
import json
from pathlib import Path
import re
from typing import Any, Iterable

SCHEMA_VERSION = "crm.contract-lifecycle/v1"
POLICY_SCHEMA_VERSION = "crm.contract-lifecycle-policy/v1"
MINIMUM_DEPRECATION_DAYS = 30
KINDS = {"capability", "event"}
CONSUMER_STATES = {"active", "migrated"}
ID_RE = re.compile(r"^[a-z][a-z0-9]*(?:[._-][a-z][a-z0-9]*)+$")
SEMVER_RE = re.compile(
    r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    r"(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$"
)
METRIC_RE = re.compile(r"^[a-zA-Z_:][a-zA-Z0-9_:]*$")


@dataclass(frozen=True, order=True)
class Coordinate:
    kind: str
    contract_id: str
    version: str

    @classmethod
    def read(cls, value: dict[str, Any], location: str, kind: str | None = None) -> "Coordinate":
        resolved_kind = kind or required_string(value, "kind", location)
        if resolved_kind not in KINDS:
            raise ValueError(f"{location}.kind must be one of {sorted(KINDS)}")
        contract_id = required_string(value, "id", location)
        version = required_string(value, "version", location)
        if len(contract_id) > 180 or not ID_RE.fullmatch(contract_id):
            raise ValueError(f"{location}.id must be a namespaced contract id")
        if len(version) > 80 or not SEMVER_RE.fullmatch(version):
            raise ValueError(f"{location}.version must be semantic version text")
        return cls(resolved_kind, contract_id, version)

    def display(self) -> str:
        return f"{self.kind}:{self.contract_id}@{self.version}"

    def mapping(self) -> dict[str, str]:
        return {"kind": self.kind, "id": self.contract_id, "version": self.version}


@dataclass(frozen=True)
class Provider:
    module_id: str
    binding: dict[str, str]


def required_string(value: dict[str, Any], key: str, location: str) -> str:
    candidate = value.get(key)
    if not isinstance(candidate, str) or not candidate.strip():
        raise ValueError(f"{location}.{key} must be a non-empty string")
    return candidate.strip()


def required_integer(value: dict[str, Any], key: str, location: str, minimum: int) -> int:
    candidate = value.get(key)
    if isinstance(candidate, bool) or not isinstance(candidate, int) or candidate < minimum:
        raise ValueError(f"{location}.{key} must be an integer >= {minimum}")
    return candidate


def iso_date(value: Any, location: str) -> date:
    if not isinstance(value, str):
        raise ValueError(f"{location} must be an ISO date string")
    try:
        parsed = date.fromisoformat(value)
    except ValueError as error:
        raise ValueError(f"{location} must use YYYY-MM-DD") from error
    if parsed.isoformat() != value:
        raise ValueError(f"{location} must use canonical YYYY-MM-DD")
    return parsed


def optional_date(value: Any, location: str) -> date | None:
    return None if value is None else iso_date(value, location)


def load_json_object(path: Path, label: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read {label} {path}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{label} {path} must contain an object")
    return value


def provider_index(bindings: dict[str, Any]) -> tuple[dict[Coordinate, Provider], list[str]]:
    errors: list[str] = []
    providers: dict[Coordinate, Provider] = {}
    if bindings.get("schema_version") != "crm.contract-bindings/v1":
        errors.append("contract bindings must use crm.contract-bindings/v1")
    modules = bindings.get("modules")
    if not isinstance(modules, list):
        return providers, errors + ["contract bindings modules must be a list"]
    for module_index, module in enumerate(modules):
        location = f"bindings.modules[{module_index}]"
        if not isinstance(module, dict):
            errors.append(f"{location} must be an object")
            continue
        try:
            module_id = required_string(module, "module_id", location)
        except ValueError as error:
            errors.append(str(error))
            continue
        for plural, kind in (("capabilities", "capability"), ("events", "event")):
            entries = module.get(plural)
            if not isinstance(entries, list):
                errors.append(f"{location}.{plural} must be a list")
                continue
            for index, entry in enumerate(entries):
                item_location = f"{location}.{plural}[{index}]"
                if not isinstance(entry, dict):
                    errors.append(f"{item_location} must be an object")
                    continue
                try:
                    coordinate = Coordinate.read(entry, item_location, kind)
                    binding = (
                        {
                            "rpc": required_string(entry, "rpc", item_location),
                            "request": required_string(entry, "request", item_location),
                            "response": required_string(entry, "response", item_location),
                        }
                        if kind == "capability"
                        else {"message": required_string(entry, "message", item_location)}
                    )
                except ValueError as error:
                    errors.append(str(error))
                    continue
                if coordinate in providers:
                    errors.append(f"duplicate published contract {coordinate.display()}")
                else:
                    providers[coordinate] = Provider(module_id, binding)
    return providers, sorted(set(errors))


def consumer_index(
    manifests: Iterable[dict[str, Any]],
) -> tuple[dict[Coordinate, set[str]], list[str]]:
    errors: list[str] = []
    consumers: dict[Coordinate, set[str]] = {}
    for manifest_index, manifest in enumerate(manifests):
        location = f"manifests[{manifest_index}]"
        try:
            module_id = required_string(manifest, "module_id", location)
            consumes = manifest["consumes"]
            if not isinstance(consumes, dict):
                raise ValueError(f"{location}.consumes must be an object")
        except (KeyError, ValueError) as error:
            errors.append(str(error))
            continue
        for plural, kind in (("capabilities", "capability"), ("events", "event")):
            entries = consumes.get(plural)
            if not isinstance(entries, list):
                errors.append(f"{location}.consumes.{plural} must be a list")
                continue
            seen: set[Coordinate] = set()
            for index, entry in enumerate(entries):
                item_location = f"{location}.consumes.{plural}[{index}]"
                try:
                    if not isinstance(entry, dict):
                        raise ValueError(f"{item_location} must be an object")
                    coordinate = Coordinate.read(entry, item_location, kind)
                except ValueError as error:
                    errors.append(str(error))
                    continue
                if coordinate in seen:
                    errors.append(f"{module_id} consumes {coordinate.display()} more than once")
                else:
                    seen.add(coordinate)
                    consumers.setdefault(coordinate, set()).add(module_id)
    return consumers, sorted(set(errors))


def _normalize_override(
    entry: dict[str, Any], location: str, minimum_days: int
) -> tuple[Coordinate, dict[str, Any]]:
    coordinate = Coordinate.read(entry, location)
    state = required_string(entry, "state", location)
    if state not in {"deprecated", "retired"}:
        raise ValueError(f"{location}.state must be deprecated or retired")
    owner = required_string(entry, "owner", location)
    deprecated_on = iso_date(entry.get("deprecated_on"), f"{location}.deprecated_on")
    removal_not_before = iso_date(
        entry.get("removal_not_before"), f"{location}.removal_not_before"
    )
    if (removal_not_before - deprecated_on).days < minimum_days:
        raise ValueError(
            f"{location}.removal_not_before must be at least {minimum_days} days after deprecated_on"
        )
    replacement_value = entry.get("replacement")
    if not isinstance(replacement_value, dict):
        raise ValueError(f"{location}.replacement must be an object")
    replacement = Coordinate.read(replacement_value, f"{location}.replacement")
    if replacement.kind != coordinate.kind or replacement == coordinate:
        raise ValueError(f"{location}.replacement must be a different {coordinate.kind}")
    migration = entry.get("migration")
    telemetry = entry.get("telemetry")
    if not isinstance(migration, dict) or not isinstance(telemetry, dict):
        raise ValueError(f"{location}.migration and telemetry must be objects")
    issue = required_integer(migration, "issue", f"{location}.migration", 1)
    guide = required_string(migration, "guide", f"{location}.migration")
    guide_path = Path(guide)
    if guide_path.is_absolute() or ".." in guide_path.parts or not guide.startswith("docs/"):
        raise ValueError(f"{location}.migration.guide must be a safe docs/ path")
    completed_on = optional_date(
        migration.get("completed_on"), f"{location}.migration.completed_on"
    )
    metric = required_string(telemetry, "metric", f"{location}.telemetry")
    if not METRIC_RE.fullmatch(metric):
        raise ValueError(f"{location}.telemetry.metric must be a Prometheus metric name")
    lookback_days = required_integer(telemetry, "lookback_days", f"{location}.telemetry", 1)
    zero_since = optional_date(telemetry.get("zero_since"), f"{location}.telemetry.zero_since")
    retired_on = optional_date(entry.get("retired_on"), f"{location}.retired_on")
    if state == "deprecated" and retired_on is not None:
        raise ValueError(f"{location}.retired_on must be absent while deprecated")
    if state == "retired":
        if retired_on is None or retired_on < removal_not_before:
            raise ValueError(f"{location}.retired_on must be on or after removal_not_before")
        if completed_on is None or not deprecated_on <= completed_on <= retired_on:
            raise ValueError(
                f"{location}.migration.completed_on must be between deprecated_on and retired_on"
            )
        if zero_since is None or (retired_on - zero_since).days < lookback_days:
            raise ValueError(
                f"{location}.telemetry.zero_since must prove at least lookback_days of zero usage"
            )
    normalized: dict[str, Any] = {
        **coordinate.mapping(),
        "state": state,
        "owner": owner,
        "deprecated_on": deprecated_on.isoformat(),
        "removal_not_before": removal_not_before.isoformat(),
        "replacement": replacement.mapping(),
        "migration": {
            "issue": issue,
            "guide": guide,
            "completed_on": completed_on.isoformat() if completed_on else None,
        },
        "telemetry": {
            "metric": metric,
            "lookback_days": lookback_days,
            "zero_since": zero_since.isoformat() if zero_since else None,
        },
    }
    if retired_on:
        normalized["retired_on"] = retired_on.isoformat()
    return coordinate, normalized


def _normalize_external(entry: dict[str, Any], location: str) -> tuple[Coordinate, dict[str, Any]]:
    coordinate = Coordinate.read(entry, location)
    consumer_id = required_string(entry, "consumer_id", location)
    owner = required_string(entry, "owner", location)
    state = required_string(entry, "state", location)
    if state not in CONSUMER_STATES:
        raise ValueError(f"{location}.state must be one of {sorted(CONSUMER_STATES)}")
    issue = entry.get("migration_issue")
    if issue is not None and (
        isinstance(issue, bool) or not isinstance(issue, int) or issue <= 0
    ):
        raise ValueError(f"{location}.migration_issue must be positive or null")
    migrated_on = optional_date(entry.get("migrated_on"), f"{location}.migrated_on")
    last_seen_on = optional_date(entry.get("last_seen_on"), f"{location}.last_seen_on")
    if state == "active" and migrated_on is not None:
        raise ValueError(f"{location}.migrated_on must be absent while active")
    if state == "migrated" and (issue is None or migrated_on is None):
        raise ValueError(f"{location} migrated consumer requires issue and migrated_on")
    if migrated_on and last_seen_on and last_seen_on > migrated_on:
        raise ValueError(f"{location}.last_seen_on must not follow migrated_on")
    return coordinate, {
        "consumer_id": consumer_id,
        "owner": owner,
        **coordinate.mapping(),
        "state": state,
        "migration_issue": issue,
        "migrated_on": migrated_on.isoformat() if migrated_on else None,
        "last_seen_on": last_seen_on.isoformat() if last_seen_on else None,
    }


def validate_policy(
    policy: dict[str, Any],
) -> tuple[dict[Coordinate, dict[str, Any]], list[dict[str, Any]], list[str]]:
    errors: list[str] = []
    overrides: dict[Coordinate, dict[str, Any]] = {}
    external: list[dict[str, Any]] = []
    if policy.get("schema_version") != POLICY_SCHEMA_VERSION:
        errors.append(f"contract lifecycle policy must use {POLICY_SCHEMA_VERSION}")
    try:
        minimum_days = required_integer(
            policy, "minimum_deprecation_days", "policy", MINIMUM_DEPRECATION_DAYS
        )
    except ValueError as error:
        errors.append(str(error))
        minimum_days = MINIMUM_DEPRECATION_DAYS
    entries = policy.get("contracts")
    if not isinstance(entries, list):
        errors.append("policy.contracts must be a list")
        entries = []
    for index, entry in enumerate(entries):
        try:
            if not isinstance(entry, dict):
                raise ValueError(f"policy.contracts[{index}] must be an object")
            coordinate, normalized = _normalize_override(
                entry, f"policy.contracts[{index}]", minimum_days
            )
            if coordinate in overrides:
                raise ValueError(f"duplicate lifecycle override {coordinate.display()}")
            overrides[coordinate] = normalized
        except ValueError as error:
            errors.append(str(error))
    entries = policy.get("external_consumers")
    if not isinstance(entries, list):
        errors.append("policy.external_consumers must be a list")
        entries = []
    seen: set[tuple[str, Coordinate]] = set()
    for index, entry in enumerate(entries):
        try:
            if not isinstance(entry, dict):
                raise ValueError(f"policy.external_consumers[{index}] must be an object")
            coordinate, normalized = _normalize_external(
                entry, f"policy.external_consumers[{index}]"
            )
            key = (normalized["consumer_id"], coordinate)
            if key in seen:
                raise ValueError(
                    f"duplicate external consumer binding {key[0]} -> {coordinate.display()}"
                )
            seen.add(key)
            external.append(normalized)
        except ValueError as error:
            errors.append(str(error))
    external.sort(key=lambda item: (item["kind"], item["id"], item["version"], item["consumer_id"]))
    return overrides, external, sorted(set(errors))


def _validate_transitions(
    providers: dict[Coordinate, Provider],
    overrides: dict[Coordinate, dict[str, Any]],
    base_bindings: dict[str, Any],
    base_policy: dict[str, Any] | None,
) -> list[str]:
    errors: list[str] = []
    base_providers, base_errors = provider_index(base_bindings)
    errors.extend(f"base: {error}" for error in base_errors)
    base_overrides: dict[Coordinate, dict[str, Any]] = {}
    if base_policy is not None:
        base_overrides, _, policy_errors = validate_policy(base_policy)
        errors.extend(f"base: {error}" for error in policy_errors)
    for coordinate, previous in sorted(base_overrides.items()):
        current = overrides.get(coordinate)
        if previous["state"] == "deprecated":
            if coordinate in providers and current is None:
                errors.append(f"deprecated contract cannot silently return to active: {coordinate.display()}")
            if current:
                for field in ("owner", "deprecated_on", "removal_not_before", "replacement"):
                    if current.get(field) != previous.get(field):
                        errors.append(
                            f"{coordinate.display()} cannot change lifecycle field {field} after deprecation"
                        )
        elif current is None or current.get("state") != "retired":
            errors.append(f"retired lifecycle tombstone must remain permanent: {coordinate.display()}")
        if previous["state"] == "retired" and coordinate in providers:
            errors.append(f"retired contract cannot be republished: {coordinate.display()}")
    for coordinate in sorted(set(base_providers) - set(providers)):
        current = overrides.get(coordinate)
        previous = base_overrides.get(coordinate)
        if current is None or current.get("state") != "retired":
            errors.append(
                f"removed published contract lacks a current retired lifecycle tombstone: {coordinate.display()}"
            )
        elif previous is None or previous.get("state") != "deprecated":
            errors.append(
                f"removed published contract was not deprecated in the base policy: {coordinate.display()}"
            )
    for coordinate, current in sorted(overrides.items()):
        previous = base_overrides.get(coordinate)
        if (
            current["state"] == "retired"
            and coordinate not in base_providers
            and (previous is None or previous.get("state") != "retired")
        ):
            errors.append(
                f"new retired tombstone has no published or retired base history: {coordinate.display()}"
            )
    return errors


def build_registry(
    bindings: dict[str, Any],
    manifests: Iterable[dict[str, Any]],
    policy: dict[str, Any],
    *,
    base_bindings: dict[str, Any] | None = None,
    base_policy: dict[str, Any] | None = None,
) -> tuple[dict[str, Any], list[str]]:
    providers, errors = provider_index(bindings)
    consumers, consumer_errors = consumer_index(manifests)
    overrides, external, policy_errors = validate_policy(policy)
    errors.extend(consumer_errors + policy_errors)
    for coordinate in consumers:
        if coordinate not in providers:
            errors.append(f"live internal consumer references unpublished {coordinate.display()}")
    external_by_contract: dict[Coordinate, list[dict[str, Any]]] = {}
    for item in external:
        coordinate = Coordinate.read(item, "external consumer")
        external_by_contract.setdefault(coordinate, []).append(item)
        if coordinate not in providers and coordinate not in overrides:
            errors.append(
                f"external consumer {item['consumer_id']} references unknown {coordinate.display()}"
            )
    for coordinate, lifecycle in overrides.items():
        replacement = Coordinate.read(lifecycle["replacement"], "replacement")
        if replacement not in providers:
            errors.append(
                f"{coordinate.display()} replacement is not currently published: {replacement.display()}"
            )
        if replacement in overrides:
            errors.append(f"{coordinate.display()} replacement must be active: {replacement.display()}")
        state = lifecycle["state"]
        if state == "deprecated" and coordinate not in providers:
            errors.append(f"deprecated contract must remain published: {coordinate.display()}")
        if state == "retired" and coordinate in providers:
            errors.append(f"retired contract must no longer be published: {coordinate.display()}")
        bound_external = external_by_contract.get(coordinate, [])
        if state == "deprecated":
            unmanaged = sorted(
                item["consumer_id"]
                for item in bound_external
                if item["state"] == "active" and item["migration_issue"] is None
            )
            if unmanaged:
                errors.append(
                    f"deprecated {coordinate.display()} has active external consumers without migration issues: {unmanaged}"
                )
        else:
            live_internal = sorted(consumers.get(coordinate, set()))
            live_external = sorted(
                item["consumer_id"] for item in bound_external if item["state"] == "active"
            )
            if live_internal:
                errors.append(
                    f"cannot retire {coordinate.display()}; live internal consumers remain: {live_internal}"
                )
            if live_external:
                errors.append(
                    f"cannot retire {coordinate.display()}; live external consumers remain: {live_external}"
                )
    if base_bindings is not None:
        errors.extend(_validate_transitions(providers, overrides, base_bindings, base_policy))
    contracts: list[dict[str, Any]] = []
    for coordinate in sorted(set(providers) | set(overrides)):
        provider = providers.get(coordinate)
        lifecycle = overrides.get(coordinate)
        entry: dict[str, Any] = {
            **coordinate.mapping(),
            "state": lifecycle["state"] if lifecycle else "active",
            "provider_module_id": provider.module_id if provider else None,
            "binding": provider.binding if provider else None,
            "internal_consumers": sorted(consumers.get(coordinate, set())),
            "external_consumers": [
                {
                    key: item[key]
                    for key in (
                        "consumer_id", "owner", "state", "migration_issue",
                        "migrated_on", "last_seen_on",
                    )
                }
                for item in external_by_contract.get(coordinate, [])
            ],
        }
        if lifecycle:
            entry["lifecycle"] = {
                key: value
                for key, value in lifecycle.items()
                if key not in {"kind", "id", "version", "state"}
            }
        contracts.append(entry)
    registry = {
        "schema_version": SCHEMA_VERSION,
        "policy_schema_version": POLICY_SCHEMA_VERSION,
        "contracts": contracts,
    }
    return registry, sorted(set(errors))


def render_registry(registry: dict[str, Any]) -> bytes:
    return (json.dumps(registry, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def registry_counts(registry: dict[str, Any]) -> tuple[int, int, int, int]:
    contracts = registry["contracts"]
    return (
        len(contracts),
        sum(item["state"] == "active" for item in contracts),
        sum(item["state"] == "deprecated" for item in contracts),
        sum(item["state"] == "retired" for item in contracts),
    )
