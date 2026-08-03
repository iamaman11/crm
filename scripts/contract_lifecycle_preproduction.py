"""Pre-production contract retirement policy extension.

The core lifecycle model intentionally requires a full deprecation and zero-usage
window. This extension adds one narrower fail-closed path for coordinates that
were never externally released. It is installed only by the canonical lifecycle
generator and is backed by immutable release evidence.
"""

from __future__ import annotations

from copy import deepcopy
from pathlib import Path
import re
from typing import Any, Callable

if __package__:
    from . import contract_lifecycle as core
else:
    import contract_lifecycle as core

NEVER_RELEASED_MODE = "never_externally_released"
EVIDENCE_ID_RE = re.compile(r"^[a-z0-9][a-z0-9._-]{2,199}$")

OriginalValidator = Callable[
    [dict[str, Any]],
    tuple[dict[core.Coordinate, dict[str, Any]], list[dict[str, Any]], list[str]],
]

_ORIGINAL_VALIDATE_POLICY: OriginalValidator = core.validate_policy


def _preproduction_entry(entry: Any) -> bool:
    return (
        isinstance(entry, dict)
        and isinstance(entry.get("retirement"), dict)
        and entry["retirement"].get("mode") == NEVER_RELEASED_MODE
    )


def _normalize_preproduction(
    entry: dict[str, Any], location: str, minimum_days: int
) -> tuple[core.Coordinate, dict[str, Any]]:
    coordinate = core.Coordinate.read(entry, location)
    state = core.required_string(entry, "state", location)
    if state != "retired":
        raise ValueError(
            f"{location}.retirement.mode {NEVER_RELEASED_MODE} requires state retired"
        )
    owner = core.required_string(entry, "owner", location)
    deprecated_on = core.iso_date(entry.get("deprecated_on"), f"{location}.deprecated_on")
    removal_not_before = core.iso_date(
        entry.get("removal_not_before"), f"{location}.removal_not_before"
    )
    if (removal_not_before - deprecated_on).days < minimum_days:
        raise ValueError(
            f"{location}.removal_not_before must preserve the normal {minimum_days}-day boundary"
        )
    retired_on = core.iso_date(entry.get("retired_on"), f"{location}.retired_on")
    if retired_on < deprecated_on:
        raise ValueError(f"{location}.retired_on must not precede deprecated_on")

    replacement_value = entry.get("replacement")
    if not isinstance(replacement_value, dict):
        raise ValueError(f"{location}.replacement must be an object")
    replacement = core.Coordinate.read(replacement_value, f"{location}.replacement")
    if replacement.kind != coordinate.kind or replacement == coordinate:
        raise ValueError(f"{location}.replacement must be a different {coordinate.kind}")

    migration = entry.get("migration")
    telemetry = entry.get("telemetry")
    retirement = entry.get("retirement")
    if not isinstance(migration, dict) or not isinstance(telemetry, dict):
        raise ValueError(f"{location}.migration and telemetry must be objects")
    if not isinstance(retirement, dict):
        raise ValueError(f"{location}.retirement must be an object")

    issue = core.required_integer(migration, "issue", f"{location}.migration", 1)
    guide = core.required_string(migration, "guide", f"{location}.migration")
    guide_path = Path(guide)
    if guide_path.is_absolute() or ".." in guide_path.parts or not guide.startswith("docs/"):
        raise ValueError(f"{location}.migration.guide must be a safe docs/ path")
    completed_on = core.optional_date(
        migration.get("completed_on"), f"{location}.migration.completed_on"
    )
    if completed_on is None or not deprecated_on <= completed_on <= retired_on:
        raise ValueError(
            f"{location}.migration.completed_on must be between deprecated_on and retired_on"
        )

    metric = core.required_string(telemetry, "metric", f"{location}.telemetry")
    if not core.METRIC_RE.fullmatch(metric):
        raise ValueError(f"{location}.telemetry.metric must be a Prometheus metric name")
    lookback_days = core.required_integer(
        telemetry, "lookback_days", f"{location}.telemetry", 1
    )
    zero_since = core.optional_date(
        telemetry.get("zero_since"), f"{location}.telemetry.zero_since"
    )
    if zero_since is not None:
        raise ValueError(
            f"{location}.telemetry.zero_since must remain null for never-released retirement"
        )

    mode = core.required_string(retirement, "mode", f"{location}.retirement")
    if mode != NEVER_RELEASED_MODE:
        raise ValueError(f"{location}.retirement.mode must be {NEVER_RELEASED_MODE}")
    evidence_id = core.required_string(
        retirement, "evidence_id", f"{location}.retirement"
    )
    if not EVIDENCE_ID_RE.fullmatch(evidence_id):
        raise ValueError(
            f"{location}.retirement.evidence_id must use 3-200 lowercase identifier characters"
        )

    return coordinate, {
        **coordinate.mapping(),
        "state": state,
        "owner": owner,
        "deprecated_on": deprecated_on.isoformat(),
        "removal_not_before": removal_not_before.isoformat(),
        "replacement": replacement.mapping(),
        "migration": {
            "issue": issue,
            "guide": guide,
            "completed_on": completed_on.isoformat(),
        },
        "telemetry": {
            "metric": metric,
            "lookback_days": lookback_days,
            "zero_since": None,
        },
        "retirement": {
            "mode": mode,
            "evidence_id": evidence_id,
        },
        "retired_on": retired_on.isoformat(),
    }


def validate_policy(
    policy: dict[str, Any],
) -> tuple[dict[core.Coordinate, dict[str, Any]], list[dict[str, Any]], list[str]]:
    """Validate the normal policy plus the never-externally-released exception."""

    entries = policy.get("contracts")
    if not isinstance(entries, list):
        return _ORIGINAL_VALIDATE_POLICY(policy)

    ordinary = [entry for entry in entries if not _preproduction_entry(entry)]
    extended = [entry for entry in entries if _preproduction_entry(entry)]
    ordinary_policy = deepcopy(policy)
    ordinary_policy["contracts"] = ordinary
    overrides, external, errors = _ORIGINAL_VALIDATE_POLICY(ordinary_policy)

    try:
        minimum_days = core.required_integer(
            policy,
            "minimum_deprecation_days",
            "policy",
            core.MINIMUM_DEPRECATION_DAYS,
        )
    except ValueError as error:
        errors.append(str(error))
        minimum_days = core.MINIMUM_DEPRECATION_DAYS

    for index, entry in enumerate(extended):
        location = f"policy.contracts[preproduction:{index}]"
        try:
            coordinate, normalized = _normalize_preproduction(
                entry, location, minimum_days
            )
            if coordinate in overrides:
                raise ValueError(f"duplicate lifecycle override {coordinate.display()}")
            overrides[coordinate] = normalized
        except ValueError as error:
            errors.append(str(error))
    return overrides, external, sorted(set(errors))


def install() -> None:
    """Install the extension into the canonical lifecycle module for this process."""

    core.validate_policy = validate_policy
