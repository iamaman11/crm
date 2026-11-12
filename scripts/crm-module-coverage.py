#!/usr/bin/env python3
"""Derive CRM module/API coverage from canonical manifests and actual Worker links.

No checked-in coverage matrix and no fixed count of modules. This is a build
report, NOT another registry or a mechanism that publishes discovered modules.
"""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from validate_module_manifests import (  # noqa: E402
    discover_paths, load_schema, strict_yaml_load, validate_schema,
    validate_manifest_semantics, validate_manifest_set,
)

APP = ROOT / "apps/cloudflare-crm"
WORKER = APP / "rust-party"


def in_repo(path: Path) -> Path:
    resolved = path.resolve()
    if not resolved.is_relative_to(ROOT):
        raise ValueError(f"module path outside repository: {path}")
    return resolved


def rpc_path(binding: dict) -> str | None:
    if binding.get("kind") != "protobuf_rpc":
        return None
    value = binding.get("rpc")
    if not isinstance(value, str) or "." not in value:
        raise ValueError(f"invalid protobuf RPC binding: {value!r}")
    service, method = value.rsplit(".", 1)
    return f"/{service}/{method}"


def build_report(root: Path = ROOT, *, phase: str | None = None) -> dict:
    if root != ROOT:
        raise ValueError("module inventory always uses canonical repository root")
    schema = load_schema()
    entries = []
    errors: list[str] = []
    for path in discover_paths([]):
        manifest = strict_yaml_load(path.read_text(encoding="utf-8"), str(path))
        errors.extend(validate_schema(manifest, schema, str(path)))
        errors.extend(validate_manifest_semantics(manifest, str(path)))
        entries.append((path, manifest))
    if not entries:
        raise ValueError("no canonical module manifests")
    errors.extend(validate_manifest_set(entries))
    known = {x["module_id"] for _, x in entries}
    for path, manifest in entries:
        for dep in manifest["dependencies"]["required"]:
            if dep["module_id"] not in known:
                errors.append(f"{path}: required module missing: {dep['module_id']}")
    if errors:
        raise ValueError("module manifests invalid:\n" + "\n".join(sorted(errors)))

    worker_cargo = tomllib.loads((WORKER / "Cargo.toml").read_text(encoding="utf-8"))
    deps = worker_cargo.get("dependencies", {})
    linked_crates: dict[Path, str] = {}
    for alias, dep in deps.items():
        if isinstance(dep, dict) and "path" in dep:
            target = in_repo(WORKER / dep["path"])
            if not (target / "Cargo.toml").is_file():
                raise ValueError(f"Worker dependency {alias} has no Cargo.toml: {target}")
            if target in linked_crates:
                raise ValueError(f"duplicate linked crate path: {alias}, {linked_crates[target]}")
            linked_crates[target] = alias

    # Single publication source: the exact RPC policy compiled into the Worker.
    # This report is generated at CI time, never committed as a second registry.
    policy = json.loads((APP / "architecture-policy.json").read_text(encoding="utf-8"))
    phase = phase or policy["phase"]
    rust = (WORKER / "src/lib.rs").read_text(encoding="utf-8")
    if "fn rpc_policy(path: &str)" not in rust:
        raise ValueError("native Rust RPC policy is missing")
    entries = re.findall(
        r'^\s*"(\/crm\.[^"]+)"\s*=>\s*\((Read|Write),\s*(Published|Candidate)\),?\s*$',
        rust, re.MULTILINE,
    )
    if not entries:
        raise ValueError("no exact-path Rust RPC policy entries")
    runtime_policy = {}
    for route, permission, activation in entries:
        if route in runtime_policy:
            raise ValueError(f"duplicate Rust RPC policy: {route}")
        runtime_policy[route] = {"permission": permission, "activation": activation}
    active = sorted(
        route for route, value in runtime_policy.items()
        if value["activation"] == "Published"
    )
    if not active:
        raise ValueError("no published native RPC in Rust policy")
    providers: dict[str, str] = {}
    modules = []
    for path, manifest in entries:
        module_id = manifest["module_id"]
        entry = manifest["runtime"].get("entrypoint")
        if manifest["runtime"]["kind"] != "in_process" or not isinstance(entry, str):
            raise ValueError(f"Cloudflare module must have in-process entrypoint: {module_id}")
        crate = in_repo(ROOT / entry)
        crate_file = crate / "Cargo.toml"
        if not crate_file.is_file():
            raise ValueError(f"module {module_id} has missing in-process crate: {entry}")
        cargo = tomllib.loads(crate_file.read_text(encoding="utf-8"))
        crate_name = cargo["package"]["name"]
        link_name = linked_crates.get(crate)
        if link_name is not None and link_name != crate_name:
            declared = deps[link_name]
            if declared.get("package", link_name) != crate_name:
                raise ValueError(f"linked Rust crate package mismatch: {module_id}")
        declared_rpc = []
        binding_specs = []
        for capability in manifest["provides"]["capabilities"]:
            binding = capability.get("binding", {})
            route = rpc_path(binding)
            if route is None:
                continue
            if route in providers:
                raise ValueError(f"duplicate RPC owner {route}: {providers[route]} and {module_id}")
            if not isinstance(binding.get("request"), str) or not isinstance(binding.get("response"), str):
                raise ValueError(f"missing typed Protobuf request/response: {route}")
            providers[route] = module_id
            declared_rpc.append(route)
            binding_specs.append({
                "path": route, "request": binding["request"], "response": binding["response"],
                "capability_id": capability["id"], "capability_version": capability["version"],
            })
        modules.append({
            "module_id": module_id,
            "manifest": path.relative_to(ROOT).as_posix(),
            "crate_path": crate.relative_to(ROOT).as_posix(),
            "crate_name": crate_name,
            "kind": "composition" if entry.startswith("crates/") else "domain",
            "linked": link_name is not None,
            "declared_rpc_paths": sorted(declared_rpc),
            "rpc_bindings": sorted(binding_specs, key=lambda item: item["path"]),
            "active_rpc_paths": sorted(route for route in active if providers.get(route) == module_id),
        })

    # Resolve after every provider is known; identical active RPC ownership
    # cannot be fabricated by adding a status field to the report.
    for mod in modules:
        mod["active_rpc_paths"] = sorted(
            route for route in active if providers.get(route) == mod["module_id"]
        )
    unclaimed = sorted(set(active) - set(providers))
    # An active method is never valid without a linked Rust owner. A manifest
    # alone does not activate a module or authorize publishing a route.
    for mod in modules:
        if mod["active_rpc_paths"] and not mod["linked"]:
            errors.append(f"active RPC belongs to unlinked Rust module: {mod['module_id']}")
    if phase == "cutover":
        if unclaimed:
            errors.append("active RPC is not declared in module.yaml: " + ", ".join(unclaimed))
        for mod in modules:
            if mod["linked"] and mod["declared_rpc_paths"] and not mod["active_rpc_paths"]:
                errors.append(f"linked module lacks accepted Connect RPC: {mod['module_id']}")
    if errors:
        raise ValueError("cutover module acceptance failed:\n" + "\n".join(errors))
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    return {
        "schema_version": 1,
        "source": "modules/*/module.yaml + Worker Cargo.toml + Rust rpc_policy",
        "rpc_policy": dict(sorted(runtime_policy.items())),
        "source_sha": head,
        "phase": phase,
        "modules": sorted(modules, key=lambda x: x["module_id"]),
        "active_rpc_paths": sorted(active),
        "unclaimed_active_rpc_paths": unclaimed,
        "counts": {
            "discovered_modules": len(modules),
            "linked_modules": sum(m["linked"] for m in modules),
            "active_rpc_methods": len(active),
            "declared_rpc_bindings": len(providers),
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        report = build_report()
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(json.dumps({"result": "PASS", "report": str(args.output),
                          "counts": report["counts"], "unclaimed": report["unclaimed_active_rpc_paths"]}))
        return 0
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as exc:
        print(f"CRM_MODULE_COVERAGE_FAIL: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
