#!/usr/bin/env python3
"""Deterministic repository explanation, packet checks, and generated navigation."""

from __future__ import annotations

from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import subprocess
import tomllib
from typing import Any, Iterable

from ruamel.yaml import YAML

try:
    from affected_scope import build_report, path_matches
except ModuleNotFoundError:  # Imported as scripts.repository_navigation in tests.
    from scripts.affected_scope import build_report, path_matches

PACKET_SCHEMA = "crm.repository-packet/v1"
NAVIGATION_SCHEMA = "crm.repository-navigation/v1"
PACKET_PATH = Path("repository-packet.json")
ROUTES_PATH = Path("contracts/production-route-classifications.json")
ACTIVE_PACKET_PATH = Path("docs/ACTIVE_PACKET.md")
REPOSITORY_MAP_PATH = Path("docs/generated/REPOSITORY_MAP.md")
TEXT_SUFFIXES = {
    ".json", ".md", ".proto", ".py", ".rs", ".sql", ".toml", ".ts",
    ".tsx", ".yaml", ".yml",
}
SKIP_DIRS = {".git", "build", "node_modules", "target"}


class NavigationError(RuntimeError):
    """Raised when repository navigation cannot be produced safely."""


def _canonical(value: Any) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode()


def digest(value: Any) -> str:
    return f"sha256:{hashlib.sha256(_canonical(value)).hexdigest()}"


def _text(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise NavigationError(f"{field} must be a non-empty string")
    return value.strip()


def _strings(value: Any, field: str) -> list[str]:
    if not isinstance(value, list) or not value:
        raise NavigationError(f"{field} must be a non-empty list")
    result = [_text(item, f"{field}[{index}]") for index, item in enumerate(value)]
    if len(result) != len(set(result)):
        raise NavigationError(f"{field} must not contain duplicates")
    return result


def _json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise NavigationError(f"cannot read {path}: {error}") from error
    if not isinstance(value, dict):
        raise NavigationError(f"{path} must contain an object")
    return value


def load_packet(root: Path) -> dict[str, Any]:
    packet = _json(root / PACKET_PATH)
    if packet.get("schema_version") != PACKET_SCHEMA:
        raise NavigationError(f"{PACKET_PATH} must use {PACKET_SCHEMA}")
    for field in ("packet_id", "title", "status", "objective"):
        _text(packet.get(field), field)
    baseline = packet.get("baseline")
    if not isinstance(baseline, dict):
        raise NavigationError("baseline must be an object")
    _text(baseline.get("ref"), "baseline.ref")
    sha = _text(baseline.get("sha"), "baseline.sha")
    if len(sha) != 40 or any(character not in "0123456789abcdef" for character in sha):
        raise NavigationError("baseline.sha must be a lowercase 40-character Git SHA")
    issues = packet.get("tracking_issues")
    if not isinstance(issues, list) or not issues or any(
        not isinstance(issue, int) or issue <= 0 for issue in issues
    ):
        raise NavigationError("tracking_issues must contain positive integers")
    for field in (
        "allowed_paths", "forbidden_paths", "deliverables", "required_checks",
        "acceptance", "non_goals",
    ):
        _strings(packet.get(field), field)
    overlap = set(packet["allowed_paths"]) & set(packet["forbidden_paths"])
    if overlap:
        raise NavigationError(f"allowed and forbidden paths overlap: {sorted(overlap)}")
    return packet


def load_modules(root: Path) -> list[dict[str, Any]]:
    yaml = YAML(typ="safe")
    modules: list[dict[str, Any]] = []
    module_ids: set[str] = set()
    coordinates: set[str] = set()
    for path in sorted((root / "modules").glob("*/module.yaml")):
        manifest = yaml.load(path.read_text(encoding="utf-8")) or {}
        if not isinstance(manifest, dict):
            raise NavigationError(f"{path} must contain an object")
        module_id = _text(manifest.get("module_id"), f"{path}: module_id")
        version = _text(manifest.get("version"), f"{path}: version")
        if module_id in module_ids:
            raise NavigationError(f"duplicate module {module_id}")
        module_ids.add(module_id)
        for capability in (manifest.get("provides") or {}).get("capabilities") or []:
            coordinate = f"{_text(capability.get('id'), 'capability.id')}@{_text(capability.get('version'), 'capability.version')}"
            if coordinate in coordinates:
                raise NavigationError(f"duplicate capability {coordinate}")
            coordinates.add(coordinate)
        modules.append({
            "path": path.relative_to(root).as_posix(),
            "module_id": module_id,
            "version": version,
            "manifest": manifest,
        })
    if not modules:
        raise NavigationError("no module manifests found")
    return sorted(modules, key=lambda item: item["module_id"])


def load_routes(root: Path) -> dict[str, Any]:
    routes = _json(root / ROUTES_PATH)
    if routes.get("schema_version") != "crm.production-route-classifications/v1":
        raise NavigationError("unsupported route classification schema")
    for field in (
        "platform_runtime_routes", "worker_runtime_routes",
        "non_runtime_contract_routes", "empty_runtime_modules",
    ):
        if not isinstance(routes.get(field), list):
            raise NavigationError(f"{field} must be a list")
    return routes


def load_workspace_members(root: Path) -> list[str]:
    try:
        document = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise NavigationError(f"cannot read Cargo.toml: {error}") from error
    members = document.get("workspace", {}).get("members")
    if not isinstance(members, list) or not members:
        raise NavigationError("workspace.members must be a non-empty list")
    return [_text(member, "workspace.members") for member in members]


def _capabilities(modules: Iterable[dict[str, Any]]) -> list[dict[str, Any]]:
    result = []
    for module in modules:
        for capability in (module["manifest"].get("provides") or {}).get("capabilities") or []:
            result.append({
                "module": module,
                "capability": capability,
                "id": str(capability["id"]),
                "version": str(capability["version"]),
                "coordinate": f"{capability['id']}@{capability['version']}",
            })
    return sorted(result, key=lambda item: item["coordinate"])


def _route_index(routes: dict[str, Any]) -> dict[str, dict[str, str]]:
    result: dict[str, dict[str, str]] = {}
    fields = {
        "platform_runtime_routes": "platform_runtime",
        "worker_runtime_routes": "worker_runtime",
        "non_runtime_contract_routes": "non_runtime",
    }
    for field, classification in fields.items():
        for route in routes[field]:
            coordinate = f"{route['id']}@{route['version']}"
            if coordinate in result:
                raise NavigationError(f"route is classified twice: {coordinate}")
            result[coordinate] = {
                "classification": classification,
                "owner_module_id": str(route["owner_module_id"]),
                "reason": str(route["reason"]),
            }
    return result


def _route(capability: dict[str, Any], index: dict[str, dict[str, str]]) -> dict[str, str]:
    return index.get(capability["coordinate"], {
        "classification": "public_runtime",
        "owner_module_id": capability["module"]["module_id"],
        "reason": "Published module capability not classified as worker or non-runtime.",
    })


def _list(values: Iterable[str]) -> str:
    items = list(values)
    return "\n".join(f"- {item}" for item in items) if items else "- None"


def render_active_packet(packet: dict[str, Any]) -> str:
    source_digest = digest(packet)
    baseline = packet["baseline"]
    lines = [
        "<!-- Generated by scripts/generate_repository_navigation.py; do not edit. -->",
        f"<!-- schema: {NAVIGATION_SCHEMA}; source-digest: {source_digest} -->",
        "", "# Active Repository Packet", "",
        f"- **Packet:** `{packet['packet_id']}` — {packet['title']}",
        f"- **Status:** `{packet['status']}`",
        f"- **Tracking:** {', '.join(f'#{issue}' for issue in packet['tracking_issues'])}",
        f"- **Baseline:** `{baseline['ref']}` / `{baseline['sha']}`",
        f"- **Source digest:** `{source_digest}`", "",
        "## Objective", "", str(packet["objective"]), "",
    ]
    for heading, field, code in (
        ("Deliverables", "deliverables", True),
        ("Allowed paths", "allowed_paths", True),
        ("Forbidden paths", "forbidden_paths", True),
        ("Required checks", "required_checks", False),
        ("Acceptance", "acceptance", False),
        ("Explicit non-goals", "non_goals", False),
    ):
        values = (f"`{value}`" if code else value for value in packet[field])
        lines.extend([f"## {heading}", "", _list(values), ""])
    lines.extend([
        "This generated file is orientation only. `repository-packet.json` and the normative architecture plan remain authoritative.",
        "",
    ])
    return "\n".join(lines)


def _package_category(path: str) -> str:
    for prefix, category in (
        ("crates/", "crate"), ("modules/", "module"),
        ("services/", "service"), ("packages/", "package"),
    ):
        if path.startswith(prefix):
            return category
    return "other"


def render_repository_map(
    modules: list[dict[str, Any]], routes: dict[str, Any], members: list[str]
) -> str:
    capabilities = _capabilities(modules)
    route_index = _route_index(routes)
    source_digest = digest({"members": members, "modules": modules, "routes": routes})
    categories = Counter(_package_category(member) for member in members)
    event_count = sum(
        len((module["manifest"].get("provides") or {}).get("events") or [])
        for module in modules
    )
    lines = [
        "<!-- Generated by scripts/generate_repository_navigation.py; do not edit. -->",
        f"<!-- schema: {NAVIGATION_SCHEMA}; source-digest: {source_digest} -->",
        "", "# Repository Map", "",
        f"- **Workspace packages:** {len(members)}",
        f"- **Business manifests:** {len(modules)}",
        f"- **Published capability coordinates:** {len(capabilities)}",
        f"- **Published event coordinates:** {event_count}",
        f"- **Source digest:** `{source_digest}`", "",
        "## Workspace package categories", "",
        "| Category | Count |", "|---|---:|",
    ]
    for category in ("crate", "module", "service", "package", "other"):
        if categories[category]:
            lines.append(f"| {category} | {categories[category]} |")
    lines.extend([
        "", "## Business module inventory", "",
        "| Module | Version | Owner | Runtime | Capabilities | Events | Storage | Public | Worker | Non-runtime |",
        "|---|---:|---|---|---:|---:|---:|---:|---:|---:|",
    ])
    by_module = defaultdict(Counter)
    for capability in capabilities:
        by_module[capability["module"]["module_id"]][
            _route(capability, route_index)["classification"]
        ] += 1
    for module in modules:
        manifest = module["manifest"]
        owner = manifest.get("owner") or {}
        runtime = manifest.get("runtime") or {}
        provides = manifest.get("provides") or {}
        storage = manifest.get("storage") or {}
        counts = by_module[module["module_id"]]
        lines.append(
            f"| `{module['module_id']}` | `{module['version']}` | "
            f"{owner.get('team') or '—'} | {runtime.get('kind') or '—'} | "
            f"{len(provides.get('capabilities') or [])} | "
            f"{len(provides.get('events') or [])} | "
            f"{len(storage.get('record_types') or [])} | "
            f"{counts['public_runtime'] + counts['platform_runtime']} | "
            f"{counts['worker_runtime']} | {counts['non_runtime']} |"
        )
    lines.extend([
        "", "## Route classification totals", "",
        f"- Platform runtime routes: {len(routes['platform_runtime_routes'])}",
        f"- Worker runtime routes: {len(routes['worker_runtime_routes'])}",
        f"- Non-runtime contract routes: {len(routes['non_runtime_contract_routes'])}",
        f"- Route-less modules: {len(routes['empty_runtime_modules'])}",
        "", "## Workspace members", "",
        _list(f"`{member}`" for member in members), "",
        "This generated map is orientation only. Module manifests, route classifications, and `Cargo.toml` remain authoritative.",
        "",
    ])
    return "\n".join(lines)


def generated_documents(root: Path) -> dict[Path, str]:
    root = root.resolve()
    return {
        ACTIVE_PACKET_PATH: render_active_packet(load_packet(root)),
        REPOSITORY_MAP_PATH: render_repository_map(
            load_modules(root), load_routes(root), load_workspace_members(root)
        ),
    }


def write_generated_documents(root: Path) -> list[str]:
    changed = []
    for relative, content in generated_documents(root).items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        current = path.read_text(encoding="utf-8") if path.exists() else None
        if current != content:
            path.write_text(content, encoding="utf-8")
            changed.append(relative.as_posix())
    return changed


def stale_generated_documents(root: Path) -> list[str]:
    stale = []
    for relative, expected in generated_documents(root).items():
        path = root / relative
        if not path.exists() or path.read_text(encoding="utf-8") != expected:
            stale.append(relative.as_posix())
    return stale


def _searchable(path: Path, root: Path) -> bool:
    relative = path.relative_to(root)
    return (
        path.is_file()
        and path.suffix in TEXT_SUFFIXES
        and not any(part in SKIP_DIRS for part in relative.parts)
        and path.stat().st_size <= 1_000_000
    )


def _path_category(path: str) -> str:
    if path.startswith(("contracts/", "proto/", "schemas/")):
        return "contracts"
    if path.startswith("database/migrations/") or "postgres" in path:
        return "persistence"
    if "composition" in path or "production" in path:
        return "production_composition"
    if path.startswith("services/") or "ingress" in path or "worker" in path:
        return "ingress_or_worker"
    if path.startswith("tests/") or "/tests/" in path or path.startswith(".github/"):
        return "tests_and_workflows"
    if "adapter" in path:
        return "adapters"
    if "application" in path:
        return "application"
    return "other"


def search_repository(root: Path, needles: Iterable[str]) -> dict[str, list[str]]:
    needles = tuple(sorted({needle for needle in needles if needle}))
    result: dict[str, list[str]] = defaultdict(list)
    for path in sorted(root.rglob("*")):
        try:
            if not _searchable(path, root):
                continue
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        if any(needle in text for needle in needles):
            relative = path.relative_to(root).as_posix()
            result[_path_category(relative)].append(relative)
    return {category: sorted(paths) for category, paths in sorted(result.items())}


def explain_target(root: Path, target: str) -> dict[str, Any]:
    root = root.resolve()
    target = _text(target, "target")
    modules = load_modules(root)
    capabilities = _capabilities(modules)
    route_index = _route_index(load_routes(root))
    module_matches = [module for module in modules if module["module_id"] == target]
    capability_matches = [item for item in capabilities if item["coordinate"] == target]
    if not capability_matches and "@" not in target:
        id_matches = [item for item in capabilities if item["id"] == target]
        if len(id_matches) == 1:
            capability_matches = id_matches
        elif len(id_matches) > 1:
            raise NavigationError(
                "ambiguous capability; use one of: "
                + ", ".join(item["coordinate"] for item in id_matches)
            )
    if module_matches:
        module = module_matches[0]
        manifest = module["manifest"]
        module_capabilities = [
            item for item in capabilities if item["module"]["module_id"] == target
        ]
        result = {
            "schema_version": NAVIGATION_SCHEMA,
            "kind": "module",
            "target": target,
            "module_id": target,
            "version": module["version"],
            "manifest_path": module["path"],
            "display_name": manifest.get("display_name"),
            "description": manifest.get("description"),
            "owner": manifest.get("owner") or {},
            "runtime": manifest.get("runtime") or {},
            "dependencies": manifest.get("dependencies") or {},
            "storage": manifest.get("storage") or {},
            "lifecycle": manifest.get("lifecycle") or {},
            "capabilities": [
                {
                    "coordinate": item["coordinate"],
                    "binding": item["capability"].get("binding") or {},
                    "route": _route(item, route_index),
                }
                for item in module_capabilities
            ],
            "events": sorted(
                f"{event['id']}@{event['version']}"
                for event in (manifest.get("provides") or {}).get("events") or []
            ),
            "references": search_repository(root, [target]),
        }
    elif capability_matches:
        item = capability_matches[0]
        binding = item["capability"].get("binding") or {}
        result = {
            "schema_version": NAVIGATION_SCHEMA,
            "kind": "capability",
            "target": target,
            "coordinate": item["coordinate"],
            "capability_id": item["id"],
            "version": item["version"],
            "owner_module_id": item["module"]["module_id"],
            "manifest_path": item["module"]["path"],
            "binding": binding,
            "route": _route(item, route_index),
            "references": search_repository(
                root, [item["id"], item["coordinate"], str(binding.get("rpc") or "")]
            ),
        }
    else:
        raise NavigationError(f"unknown module or capability: {target}")
    result["source_digest"] = digest(result)
    return result


def render_explanation(value: dict[str, Any]) -> str:
    lines = [
        f"# Repository Explanation: `{value['target']}`", "",
        f"- **Kind:** {value['kind']}",
        f"- **Source digest:** `{value['source_digest']}`",
    ]
    if value["kind"] == "module":
        lines.extend([
            f"- **Module:** `{value['module_id']}@{value['version']}`",
            f"- **Manifest:** `{value['manifest_path']}`",
            f"- **Owner team:** {value['owner'].get('team') or '—'}",
            f"- **Runtime:** {value['runtime'].get('kind') or '—'} / `{value['runtime'].get('entrypoint') or '—'}`",
            "", "## Capabilities", "",
            "| Coordinate | Classification | RPC |", "|---|---|---|",
        ])
        for capability in value["capabilities"]:
            lines.append(
                f"| `{capability['coordinate']}` | {capability['route']['classification']} | "
                f"`{capability['binding'].get('rpc') or '—'}` |"
            )
        lines.extend(["", "## Events", "", _list(f"`{event}`" for event in value["events"])])
    else:
        route = value["route"]
        binding = value["binding"]
        lines.extend([
            f"- **Coordinate:** `{value['coordinate']}`",
            f"- **Owner:** `{value['owner_module_id']}`",
            f"- **Manifest:** `{value['manifest_path']}`",
            f"- **Classification:** {route['classification']}",
            f"- **Reason:** {route['reason']}",
            f"- **RPC:** `{binding.get('rpc') or '—'}`",
            f"- **Request:** `{binding.get('request') or '—'}`",
            f"- **Response:** `{binding.get('response') or '—'}`",
        ])
    lines.extend(["", "## Repository references", ""])
    for category, paths in (value.get("references") or {}).items():
        lines.extend([f"### {category.replace('_', ' ').title()}", "", _list(f"`{path}`" for path in paths), ""])
    if not value.get("references"):
        lines.append("- None")
    return "\n".join(lines).rstrip() + "\n"


def _git(root: Path, *arguments: str) -> str:
    completed = subprocess.run(
        ["git", *arguments], cwd=root, text=True, capture_output=True, check=False
    )
    if completed.returncode:
        raise NavigationError(completed.stdout + completed.stderr)
    return completed.stdout.strip()


def evaluate_path_policy(
    paths: Iterable[str], allowed: Iterable[str], forbidden: Iterable[str]
) -> tuple[list[str], list[str]]:
    forbidden_paths, disallowed_paths = [], []
    allowed, forbidden = tuple(allowed), tuple(forbidden)
    for path in sorted(set(paths)):
        if any(path_matches(path, pattern) for pattern in forbidden):
            forbidden_paths.append(path)
        elif not any(path_matches(path, pattern) for pattern in allowed):
            disallowed_paths.append(path)
    return forbidden_paths, disallowed_paths


def _changed_categories(paths: Iterable[str]) -> dict[str, list[str]]:
    result: dict[str, list[str]] = defaultdict(list)
    for path in sorted(set(paths)):
        if path.startswith(("contracts/", "proto/", "schemas/")):
            result["contracts"].append(path)
        if path.startswith("database/migrations/"):
            result["migrations"].append(path)
        if path == ROUTES_PATH.as_posix() or "route" in Path(path).name:
            result["routes"].append(path)
        if "worker" in path:
            result["workers"].append(path)
    return dict(sorted(result.items()))


def packet_check(root: Path, base_ref: str) -> dict[str, Any]:
    root = root.resolve()
    packet = load_packet(root)
    base_ref = _text(base_ref, "base_ref")
    base_sha = _git(root, "rev-parse", base_ref)
    affected = build_report(root, base_ref)
    paths = list(affected.get("changed_paths") or [])
    forbidden, disallowed = evaluate_path_policy(
        paths, packet["allowed_paths"], packet["forbidden_paths"]
    )
    stale = stale_generated_documents(root)
    blockers = []
    if base_sha != packet["baseline"]["sha"]:
        blockers.append(
            f"base {base_ref} resolves to {base_sha}; packet declares {packet['baseline']['sha']}"
        )
    blockers.extend(f"forbidden path changed: {path}" for path in forbidden)
    blockers.extend(f"path outside packet: {path}" for path in disallowed)
    blockers.extend(f"generated navigation is stale: {path}" for path in stale)
    report = {
        "schema_version": NAVIGATION_SCHEMA,
        "packet_id": packet["packet_id"],
        "packet_status": packet["status"],
        "base_ref": base_ref,
        "base_sha": base_sha,
        "declared_baseline_sha": packet["baseline"]["sha"],
        "head_sha": affected.get("head_sha"),
        "changed_paths": paths,
        "forbidden_changed_paths": forbidden,
        "disallowed_changed_paths": disallowed,
        "changed_categories": _changed_categories(paths),
        "affected_packages": affected.get("affected_packages") or [],
        "selected_workflows": affected.get("selected_workflows") or [],
        "generated_stale_paths": stale,
        "blockers": blockers,
        "ok": not blockers,
    }
    report["source_digest"] = digest(report)
    return report


def render_packet_check(report: dict[str, Any]) -> str:
    lines = [
        f"# Packet Check: `{report['packet_id']}`", "",
        f"- **Result:** {'PASS' if report['ok'] else 'FAIL'}",
        f"- **Base:** `{report['base_ref']}` / `{report['base_sha']}`",
        f"- **Declared baseline:** `{report['declared_baseline_sha']}`",
        f"- **Head:** `{report['head_sha']}`",
        f"- **Changed paths:** {len(report['changed_paths'])}",
        f"- **Affected packages:** {len(report['affected_packages'])}",
        f"- **Selected workflows:** {len(report['selected_workflows'])}",
        f"- **Source digest:** `{report['source_digest']}`", "",
        "## Blockers", "", _list(report["blockers"]), "",
        "## Changed paths", "", _list(f"`{path}`" for path in report["changed_paths"]), "",
        "## Affected packages", "", _list(f"`{name}`" for name in report["affected_packages"]), "",
        "## Selected workflows", "",
    ]
    if report["selected_workflows"]:
        for workflow in report["selected_workflows"]:
            lines.append(f"- **{workflow['name']}** — `{workflow['path']}`")
            lines.extend(f"  - {reason}" for reason in workflow.get("reasons") or [])
    else:
        lines.append("- None")
    return "\n".join(lines).rstrip() + "\n"
