"""Base-to-head compatibility invariants for published contract lifecycles."""

from __future__ import annotations

from datetime import date
from pathlib import Path
from typing import Any

if __package__:
    from .contract_lifecycle import (
        MINIMUM_DEPRECATION_DAYS,
        Coordinate,
        iso_date,
        provider_index,
        required_integer,
        validate_policy,
    )
else:
    from contract_lifecycle import (
        MINIMUM_DEPRECATION_DAYS,
        Coordinate,
        iso_date,
        provider_index,
        required_integer,
        validate_policy,
    )


def _external_key(item: dict[str, Any]) -> tuple[str, Coordinate]:
    return item["consumer_id"], Coordinate.read(item, "external consumer")


def _future_date_errors(
    overrides: dict[Coordinate, dict[str, Any]],
    external: list[dict[str, Any]],
    *,
    as_of: date,
    prefix: str,
) -> list[str]:
    errors: list[str] = []
    for coordinate, lifecycle in sorted(overrides.items()):
        dated_values = {
            "deprecated_on": lifecycle["deprecated_on"],
            "migration.completed_on": lifecycle["migration"]["completed_on"],
            "telemetry.zero_since": lifecycle["telemetry"]["zero_since"],
            "retired_on": lifecycle.get("retired_on"),
        }
        for field, value in dated_values.items():
            if value is not None and iso_date(value, field) > as_of:
                errors.append(
                    f"{prefix}{coordinate.display()} {field} must not be later than "
                    f"{as_of.isoformat()}"
                )
    for item in external:
        consumer_id, coordinate = _external_key(item)
        for field in ("migrated_on", "last_seen_on"):
            value = item[field]
            if value is not None and iso_date(value, field) > as_of:
                errors.append(
                    f"{prefix}external consumer {consumer_id} -> {coordinate.display()} "
                    f"{field} must not be later than {as_of.isoformat()}"
                )
    return errors


def _external_transition_errors(
    external: list[dict[str, Any]], base_external: list[dict[str, Any]]
) -> list[str]:
    errors: list[str] = []
    current = {_external_key(item): item for item in external}
    previous = {_external_key(item): item for item in base_external}
    order = lambda entry: (entry[0][1], entry[0][0])
    for key, before in sorted(previous.items(), key=order):
        consumer_id, coordinate = key
        after = current.get(key)
        label = f"external consumer {consumer_id} -> {coordinate.display()}"
        if after is None:
            errors.append(f"{label} lifecycle record must remain permanent")
            continue
        if after["owner"] != before["owner"]:
            errors.append(f"{label} owner cannot change")
        if before["state"] == "migrated":
            if after["state"] != "migrated":
                errors.append(f"migrated {label} cannot return to active")
            for field in ("migration_issue", "migrated_on", "last_seen_on"):
                if after[field] != before[field]:
                    errors.append(f"migrated {label} cannot change {field}")
            continue
        if (
            before["migration_issue"] is not None
            and after["migration_issue"] != before["migration_issue"]
        ):
            errors.append(f"{label} cannot clear or change its migration issue")
        if before["last_seen_on"] is not None and (
            after["last_seen_on"] is None
            or after["last_seen_on"] < before["last_seen_on"]
        ):
            errors.append(f"{label} last_seen_on cannot regress")
    for key, item in sorted(current.items(), key=order):
        if key not in previous and item["state"] == "migrated":
            errors.append(
                f"new migrated external consumer {key[0]} -> {key[1].display()} "
                "lacks active base history"
            )
    return errors


def _lifecycle_transition_errors(
    overrides: dict[Coordinate, dict[str, Any]],
    base_overrides: dict[Coordinate, dict[str, Any]],
) -> list[str]:
    errors: list[str] = []
    for coordinate, before in sorted(base_overrides.items()):
        after = overrides.get(coordinate)
        if after is None:
            continue
        if before["state"] == "retired":
            if after != before:
                errors.append(
                    f"retired lifecycle tombstone is immutable: {coordinate.display()}"
                )
            continue
        stable = {
            "migration.issue": (
                after["migration"]["issue"],
                before["migration"]["issue"],
            ),
            "migration.guide": (
                after["migration"]["guide"],
                before["migration"]["guide"],
            ),
            "telemetry.metric": (
                after["telemetry"]["metric"],
                before["telemetry"]["metric"],
            ),
        }
        for field, (current, previous) in stable.items():
            if current != previous:
                errors.append(
                    f"{coordinate.display()} cannot change lifecycle field {field} "
                    "after deprecation"
                )
        if (
            after["telemetry"]["lookback_days"]
            < before["telemetry"]["lookback_days"]
        ):
            errors.append(
                f"{coordinate.display()} telemetry.lookback_days cannot decrease "
                "after deprecation"
            )
        for section, field in (
            ("migration", "completed_on"),
            ("telemetry", "zero_since"),
        ):
            previous = before[section][field]
            current = after[section][field]
            if previous is not None and current != previous:
                errors.append(
                    f"{coordinate.display()} cannot rewrite {section}.{field} evidence"
                )
    return errors


def _retirement_order_errors(
    overrides: dict[Coordinate, dict[str, Any]], external: list[dict[str, Any]]
) -> list[str]:
    errors: list[str] = []
    by_contract: dict[Coordinate, list[dict[str, Any]]] = {}
    for item in external:
        by_contract.setdefault(Coordinate.read(item, "external consumer"), []).append(item)
    for coordinate, lifecycle in sorted(overrides.items()):
        if lifecycle["state"] != "retired":
            continue
        retired_on = iso_date(lifecycle["retired_on"], "retired_on")
        migrated_late = sorted(
            item["consumer_id"]
            for item in by_contract.get(coordinate, [])
            if item["state"] == "migrated"
            and iso_date(item["migrated_on"], "migrated_on") > retired_on
        )
        if migrated_late:
            errors.append(
                f"cannot retire {coordinate.display()}; external consumers migrated "
                f"after retirement: {migrated_late}"
            )
    return errors


def _guide_errors(
    overrides: dict[Coordinate, dict[str, Any]], repository_root: Path | None
) -> list[str]:
    if repository_root is None:
        return []
    errors: list[str] = []
    root = repository_root.resolve()
    for coordinate, lifecycle in sorted(overrides.items()):
        guide = root / lifecycle["migration"]["guide"]
        if not guide.is_file():
            errors.append(
                f"{coordinate.display()} migration guide does not exist: "
                f"{lifecycle['migration']['guide']}"
            )
    return errors


def validate_transition_integrity(
    bindings: dict[str, Any],
    policy: dict[str, Any],
    *,
    base_bindings: dict[str, Any] | None = None,
    base_policy: dict[str, Any] | None = None,
    today: date | None = None,
    repository_root: Path | None = None,
) -> list[str]:
    """Return fail-closed compatibility and evidence-transition violations."""

    errors: list[str] = []
    as_of = today or date.today()
    providers, provider_errors = provider_index(bindings)
    overrides, external, policy_errors = validate_policy(policy)
    errors.extend(provider_errors)
    errors.extend(policy_errors)
    errors.extend(
        _future_date_errors(overrides, external, as_of=as_of, prefix="")
    )
    errors.extend(_retirement_order_errors(overrides, external))
    errors.extend(_guide_errors(overrides, repository_root))

    if base_bindings is None:
        if base_policy is None:
            errors.extend(_external_transition_errors(external, []))
        return sorted(set(errors))

    base_providers, base_provider_errors = provider_index(base_bindings)
    errors.extend(f"base: {error}" for error in base_provider_errors)
    for coordinate in sorted(set(providers) & set(base_providers)):
        current = providers[coordinate]
        previous = base_providers[coordinate]
        if current.module_id != previous.module_id:
            errors.append(f"published provider cannot change for {coordinate.display()}")
        if current.binding != previous.binding:
            errors.append(f"published binding cannot change for {coordinate.display()}")

    base_overrides: dict[Coordinate, dict[str, Any]] = {}
    base_external: list[dict[str, Any]] = []
    if base_policy is not None:
        base_overrides, base_external, base_policy_errors = validate_policy(base_policy)
        errors.extend(f"base: {error}" for error in base_policy_errors)
        errors.extend(
            _future_date_errors(
                base_overrides, base_external, as_of=as_of, prefix="base: "
            )
        )
        try:
            current_minimum = required_integer(
                policy,
                "minimum_deprecation_days",
                "policy",
                MINIMUM_DEPRECATION_DAYS,
            )
            base_minimum = required_integer(
                base_policy,
                "minimum_deprecation_days",
                "base policy",
                MINIMUM_DEPRECATION_DAYS,
            )
            if current_minimum < base_minimum:
                errors.append(
                    "minimum_deprecation_days cannot decrease from "
                    f"{base_minimum} to {current_minimum}"
                )
        except ValueError as error:
            errors.append(str(error))
    errors.extend(_external_transition_errors(external, base_external))
    errors.extend(_lifecycle_transition_errors(overrides, base_overrides))
    return sorted(set(errors))
