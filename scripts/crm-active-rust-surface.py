#!/usr/bin/env python3
"""Fail-closed Rust source exception gate for the *deployed* Cloudflare CRM.

The historical ADR-031 Step 13 budget belongs to the unrelated 112-crate
PostgreSQL process host. This guard scopes to the native Worker and the
13 original Rust owner crates actually linked by its Cargo manifest.
It blocks new source-level lint suppressions and ignored tests relative to
immutable accepted baselines. Rust compilation, Clippy and E2E are additional
required gates, not replaced by this source inventory.
"""
from __future__ import annotations

import argparse
from collections import Counter
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
POLICY_PATH = ROOT / "apps/cloudflare-crm/architecture-policy.json"
COVERAGE_PATH = ROOT / "apps/cloudflare-crm/contract-coverage.json"
WORKER = ROOT / "apps/cloudflare-crm/rust-party"
ATTRIBUTE = re.compile(r"#\s*!?\s*\[\s*(?:allow|expect|ignore|cfg_attr)\b[^\]]*\]", re.S)
SUPPRESSION_IN_CFG = re.compile(r"\b(?:allow|expect|ignore)\s*(?:\(|\])")
PUBLIC = re.compile(
    r"^\s*pub(?:\s*\([^)]*\))?\s+(?:async\s+)?"
    r"(?:const|enum|extern|fn|mod|static|struct|trait|type|union|use)\b"
)
LOCAL_LINTS = re.compile(r"(?m)^\s*\[lints(?:\.[^]]+)?\]\s*$")


def git(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(["git", *args], cwd=ROOT, text=True,
                          capture_output=True, check=check)


def accepted_file(sha: str, path: str) -> str:
    result = git("show", f"{sha}:{path}", check=False)
    if result.returncode == 0:
        return result.stdout
    # An added source did not exist in the accepted baseline, so it starts
    # with zero permitted exceptions.
    missing = git("cat-file", "-e", f"{sha}:{path}", check=False)
    if missing.returncode != 0:
        return ""
    raise RuntimeError(f"could not read accepted file {sha}:{path}: {result.stderr}")


def suppressions(text: str) -> Counter[str]:
    found: Counter[str] = Counter()
    for match in ATTRIBUTE.finditer(text):
        attr = match.group(0)
        if re.match(r"#\s*!?\s*\[\s*cfg_attr\b", attr) and not SUPPRESSION_IN_CFG.search(attr):
            continue
        # Normalize formatting, but do not conflate different exceptions.
        found[" ".join(attr.split())] += 1
    return found


def lint_sections(text: str) -> Counter[str]:
    found: Counter[str] = Counter()
    for m in LOCAL_LINTS.finditer(text):
        remainder = text[m.start():]
        next_section = re.search(r"(?m)^\s*\[[^]]+\]\s*$", remainder[len(m.group(0)):])
        section = remainder[:len(m.group(0)) + next_section.start()] if next_section else remainder
        found[" ".join(section.split())] += 1
    return found


def growth(current: Counter[str], baseline: Counter[str]) -> dict[str, int]:
    return dict(current - baseline)


def scope() -> tuple[list[str], list[str]]:
    coverage = json.loads(COVERAGE_PATH.read_text(encoding="utf-8"))
    manifest = tomllib.loads((WORKER / "Cargo.toml").read_text(encoding="utf-8"))
    owner_packages = {row["domain_module"] for row in coverage["owners"]}
    if len(owner_packages) != 13:
        raise RuntimeError(f"expected 13 original Rust owners, got {len(owner_packages)}")
    deps = manifest.get("dependencies", {})
    owner_paths = []
    for name in sorted(owner_packages):
        entry = deps.get(name)
        if not isinstance(entry, dict) or not entry.get("path"):
            raise RuntimeError(f"original owner is not linked by native Worker: {name}")
        path = (WORKER / entry["path"]).resolve()
        try:
            path.relative_to(ROOT)
        except ValueError as error:
            raise RuntimeError(f"Rust owner escapes canonical repository: {name}") from error
        if not (path / "Cargo.toml").is_file() or not (path / "src").is_dir():
            raise RuntimeError(f"native Worker owner Cargo crate missing: {name}")
        owner_paths.append(path.relative_to(ROOT).as_posix())
    roots = ["apps/cloudflare-crm/rust-party", *owner_paths]
    files = sorted({
        f.relative_to(ROOT).as_posix()
        for rel in roots
        for f in (ROOT / rel).rglob("*.rs")
        if "target" not in f.relative_to(ROOT / rel).parts
    })
    for rel in roots:
        files.append(rel + "/Cargo.toml")
    return roots, sorted(files)


def evaluate() -> dict:
    policy = json.loads(POLICY_PATH.read_text(encoding="utf-8"))
    worker_base = policy.get("accepted_runtime_sha")
    owner_base = policy.get("accepted_owner_source_sha")
    for sha in (worker_base, owner_base):
        if not isinstance(sha, str) or not re.fullmatch(r"[a-f0-9]{40}", sha):
            raise RuntimeError("accepted Rust source baseline must be pinned to full SHA")
        git("cat-file", "-e", f"{sha}^{{commit}}")
    roots, files = scope()
    errors = []
    counts = {"public_current": 0, "public_accepted": 0,
              "suppression_current": 0, "suppression_accepted": 0}
    for relative in files:
        base = worker_base if relative.startswith("apps/cloudflare-crm/rust-party/") else owner_base
        current = (ROOT / relative).read_text(encoding="utf-8")
        accepted = accepted_file(base, relative)
        if relative.endswith(".rs"):
            now, old = suppressions(current), suppressions(accepted)
            counts["public_current"] += sum(bool(PUBLIC.match(line)) for line in current.splitlines())
            counts["public_accepted"] += sum(bool(PUBLIC.match(line)) for line in accepted.splitlines())
            counts["suppression_current"] += sum(now.values())
            counts["suppression_accepted"] += sum(old.values())
        else:
            now, old = lint_sections(current), lint_sections(accepted)
        for attribute, number in sorted(growth(now, old).items()):
            errors.append({
                "file": relative, "added": number, "attribute": attribute,
                "detail": "new lint/ignored-test exception beyond accepted active Rust baseline",
            })
    return {
        "schema": "crm.active-rust-surface/v1",
        "result": "FAIL" if errors else "PASS",
        "owners": len(roots) - 1,
        "scoped_crates": roots,
        "files_checked": len(files),
        "worker_baseline_sha": worker_base,
        "owner_baseline_sha": owner_base,
        "counts": counts,
        "errors": errors,
        "note": "Rust pub declarations are measured, not frozen: API changes are governed by proto, compiler, owner tests and actual RPC/E2E coverage. Historical ADR-031 global budgets remain on main.",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json-output", type=Path)
    args = parser.parse_args()
    try:
        report = evaluate()
    except (OSError, KeyError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"CRM_ACTIVE_RUST_GATE_ERROR: {error}", file=sys.stderr)
        return 1
    serialized = json.dumps(report, indent=2, sort_keys=True)
    if args.json_output:
        args.json_output.parent.mkdir(parents=True, exist_ok=True)
        args.json_output.write_text(serialized + "\n", encoding="utf-8")
    print(json.dumps({key: report[key] for key in (
        "schema", "result", "owners", "files_checked", "counts", "errors"
    )}, ensure_ascii=False))
    return 0 if report["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
