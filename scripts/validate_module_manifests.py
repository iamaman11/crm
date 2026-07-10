#!/usr/bin/env python3
"""Validate Ultimate CRM module manifests and emit stable canonical digests."""

from __future__ import annotations

import argparse
from collections import defaultdict
from datetime import date, datetime
import hashlib
import json
import math
from pathlib import Path
import sys
from typing import Any, Iterable

from jsonschema import Draft202012Validator, FormatChecker
from ruamel.yaml import YAML
from ruamel.yaml.tokens import AliasToken, AnchorToken, ScalarToken, TagToken

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_SCHEMA = ROOT / "schemas" / "module.schema.json"
MAX_SAFE_INTEGER = 9_007_199_254_740_991
CANONICALIZATION_PROFILE = "crm.cjson/v1"


class ManifestError(ValueError):
    """A deterministic, user-correctable manifest validation error."""


def _yaml_parser() -> YAML:
    parser = YAML(typ="safe", pure=True)
    parser.version = (1, 2)
    parser.allow_duplicate_keys = False
    return parser


def strict_yaml_load(text: str, source: str = "<memory>") -> dict[str, Any]:
    """Load the approved JSON-compatible YAML 1.2 subset."""
    parser = _yaml_parser()
    try:
        for token in parser.scan(text):
            if isinstance(token, (AnchorToken, AliasToken)):
                raise ManifestError(f"{source}: YAML anchors and aliases are forbidden")
            if isinstance(token, TagToken):
                raise ManifestError(f"{source}: YAML tags are forbidden")
            if isinstance(token, ScalarToken) and token.value == "<<":
                raise ManifestError(f"{source}: YAML merge keys are forbidden")
        loaded = parser.load(text)
    except ManifestError:
        raise
    except Exception as exc:  # parser exceptions vary by ruamel.yaml release
        raise ManifestError(f"{source}: invalid strict YAML: {exc}") from exc

    if not isinstance(loaded, dict):
        raise ManifestError(f"{source}: manifest root must be an object")
    _assert_json_compatible(loaded, source, "$", set())
    return loaded


def _assert_json_compatible(value: Any, source: str, path: str, seen: set[int]) -> None:
    if isinstance(value, (dict, list)):
        object_id = id(value)
        if object_id in seen:
            raise ManifestError(f"{source}: recursive/aliased value at {path}")
        seen.add(object_id)

    if isinstance(value, dict):
        for key, child in value.items():
            if not isinstance(key, str):
                raise ManifestError(f"{source}: non-string object key at {path}")
            if not key.isascii():
                raise ManifestError(f"{source}: canonical object keys must be ASCII at {path}.{key}")
            _assert_json_compatible(child, source, f"{path}.{key}", seen)
        seen.remove(id(value))
        return

    if isinstance(value, list):
        for index, child in enumerate(value):
            _assert_json_compatible(child, source, f"{path}[{index}]", seen)
        seen.remove(id(value))
        return

    if isinstance(value, bool) or value is None or isinstance(value, str):
        return
    if isinstance(value, int):
        if not -MAX_SAFE_INTEGER <= value <= MAX_SAFE_INTEGER:
            raise ManifestError(f"{source}: integer outside safe canonical range at {path}")
        return
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ManifestError(f"{source}: non-finite number at {path}")
        raise ManifestError(f"{source}: floating-point values are forbidden at {path}")
    if isinstance(value, (date, datetime)):
        raise ManifestError(f"{source}: implicit date/time values are forbidden at {path}; quote them")
    raise ManifestError(f"{source}: unsupported YAML value {type(value).__name__} at {path}")


def canonical_json_bytes(value: dict[str, Any]) -> bytes:
    """Serialize the restricted crm.cjson/v1 canonical JSON profile."""
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def canonical_digest(value: dict[str, Any]) -> str:
    return hashlib.sha256(canonical_json_bytes(value)).hexdigest()


def load_schema(path: Path = DEFAULT_SCHEMA) -> dict[str, Any]:
    schema = json.loads(path.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(schema)
    return schema


def validate_schema(manifest: dict[str, Any], schema: dict[str, Any], source: str) -> list[str]:
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    errors: list[str] = []
    for error in sorted(validator.iter_errors(manifest), key=lambda item: list(item.absolute_path)):
        location = "$"
        if error.absolute_path:
            location += "." + ".".join(str(part) for part in error.absolute_path)
        errors.append(f"{source}: {location}: {error.message}")
    return errors


def _contract_keys(items: Iterable[dict[str, Any]]) -> list[tuple[str, str]]:
    return [(item["id"], item["version"]) for item in items]


def validate_manifest_semantics(manifest: dict[str, Any], source: str) -> list[str]:
    errors: list[str] = []
    module_id = manifest["module_id"]
    dependencies = manifest["dependencies"]

    required_ids = [item["module_id"] for item in dependencies["required"]]
    optional_ids = [item["module_id"] for item in dependencies["optional"]]
    conflict_ids = dependencies["conflicts"]

    for label, values in (
        ("required dependencies", required_ids),
        ("optional dependencies", optional_ids),
        ("conflicts", conflict_ids),
    ):
        duplicates = sorted(value for value, count in _counts(values).items() if count > 1)
        if duplicates:
            errors.append(f"{source}: duplicate {label}: {duplicates}")

    if module_id in set(required_ids + optional_ids + conflict_ids):
        errors.append(f"{source}: module must not depend on or conflict with itself")
    overlap = sorted(set(required_ids) & set(optional_ids))
    if overlap:
        errors.append(f"{source}: dependencies cannot be both required and optional: {overlap}")

    for category in ("capabilities", "events"):
        keys = _contract_keys(manifest["provides"][category])
        duplicates = sorted(key for key, count in _counts(keys).items() if count > 1)
        if duplicates:
            errors.append(f"{source}: duplicate provided {category}: {duplicates}")

    object_types = set(manifest["provides"]["objects"])
    record_types = set(manifest["storage"]["record_types"])
    retained_types = set(manifest["lifecycle"]["retained_record_types"])
    undeclared_storage = sorted(record_types - object_types)
    if undeclared_storage:
        errors.append(f"{source}: storage record types not owned by module: {undeclared_storage}")
    invalid_retained = sorted(retained_types - record_types)
    if invalid_retained:
        errors.append(f"{source}: retained record types are not declared storage: {invalid_retained}")

    private_namespaces = manifest["storage"]["private_state_namespaces"]
    if any(not namespace.startswith(module_id) for namespace in private_namespaces):
        errors.append(f"{source}: private state namespace must start with module_id '{module_id}'")

    return errors


def _counts(values: Iterable[Any]) -> dict[Any, int]:
    counts: dict[Any, int] = defaultdict(int)
    for value in values:
        counts[value] += 1
    return counts


def validate_manifest_set(entries: list[tuple[Path, dict[str, Any]]]) -> list[str]:
    errors: list[str] = []
    by_module: dict[str, tuple[Path, dict[str, Any]]] = {}
    providers: dict[tuple[str, str, str], str] = {}
    object_owners: dict[str, str] = {}

    for path, manifest in entries:
        module_id = manifest["module_id"]
        if module_id in by_module:
            errors.append(f"{path}: duplicate module_id '{module_id}', also in {by_module[module_id][0]}")
        else:
            by_module[module_id] = (path, manifest)

        for category in ("capabilities", "events"):
            for contract in manifest["provides"][category]:
                key = (category, contract["id"], contract["version"])
                previous = providers.get(key)
                if previous and previous != module_id:
                    errors.append(
                        f"{path}: {category[:-1]} {contract['id']}@{contract['version']} "
                        f"already provided by {previous}"
                    )
                providers[key] = module_id

        for object_type in manifest["provides"]["objects"]:
            previous = object_owners.get(object_type)
            if previous and previous != module_id:
                errors.append(f"{path}: object '{object_type}' already owned by {previous}")
            object_owners[object_type] = module_id

    graph: dict[str, set[str]] = {}
    known = set(by_module)
    for module_id, (_, manifest) in by_module.items():
        graph[module_id] = {
            item["module_id"]
            for item in manifest["dependencies"]["required"]
            if item["module_id"] in known
        }
    errors.extend(_dependency_cycle_errors(graph))
    return errors


def _dependency_cycle_errors(graph: dict[str, set[str]]) -> list[str]:
    visiting: set[str] = set()
    visited: set[str] = set()
    stack: list[str] = []
    errors: list[str] = []

    def visit(node: str) -> None:
        if node in visited:
            return
        if node in visiting:
            start = stack.index(node)
            cycle = stack[start:] + [node]
            errors.append("required module dependency cycle: " + " -> ".join(cycle))
            return
        visiting.add(node)
        stack.append(node)
        for dependency in sorted(graph.get(node, set())):
            visit(dependency)
        stack.pop()
        visiting.remove(node)
        visited.add(node)

    for module_id in sorted(graph):
        visit(module_id)
    return errors


def discover_paths(arguments: list[str]) -> list[Path]:
    if arguments:
        paths = [Path(item) for item in arguments]
    else:
        paths = sorted(ROOT.glob("modules/*/module.yaml"))
    return [path.resolve() for path in paths]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("paths", nargs="*", help="module.yaml paths; defaults to modules/*/module.yaml")
    parser.add_argument("--schema", type=Path, default=DEFAULT_SCHEMA)
    args = parser.parse_args(argv)

    paths = discover_paths(args.paths)
    if not paths:
        print("No module manifests found", file=sys.stderr)
        return 2

    try:
        schema = load_schema(args.schema)
    except Exception as exc:
        print(f"Schema load failed: {exc}", file=sys.stderr)
        return 2

    entries: list[tuple[Path, dict[str, Any]]] = []
    errors: list[str] = []
    for path in paths:
        try:
            manifest = strict_yaml_load(path.read_text(encoding="utf-8"), str(path))
        except (OSError, ManifestError) as exc:
            errors.append(str(exc))
            continue
        schema_errors = validate_schema(manifest, schema, str(path))
        errors.extend(schema_errors)
        if schema_errors:
            continue
        errors.extend(validate_manifest_semantics(manifest, str(path)))
        entries.append((path, manifest))

    errors.extend(validate_manifest_set(entries))
    if errors:
        print("Module manifest validation FAILED:")
        for error in errors:
            print(f"- {error}")
        return 1

    for path, manifest in entries:
        print(
            f"PASS {manifest['module_id']}@{manifest['version']} "
            f"{CANONICALIZATION_PROFILE}:sha256:{canonical_digest(manifest)} "
            f"({path.relative_to(ROOT) if path.is_relative_to(ROOT) else path})"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
