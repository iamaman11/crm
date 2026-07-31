#!/usr/bin/env python3
"""One-run materializer for step-12 batch-1 compile and packet corrections."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys

from repository_navigation import NavigationError, stale_generated_documents, write_generated_documents


SHORT_NON_GOAL = '            "complete repository step 12 for Identity Resolution",\n'
FULL_NON_GOAL = (
    '            "complete repository step 12 for Identity Resolution, Customer Data Operations, "\n'
    '            "Data Quality, Customer Enrichment, Sales/Activities, Customer 360 or other "\n'
    '            "remaining owners in this batch",\n'
)
TEST_PATHS = (
    "tests/test_repository_navigation.py",
    "tests/test_architecture_documentation_consistency.py",
)
DEPENDENCY_INSERTIONS = (
    (
        'crm-capability-runtime = { path = "../crm-capability-runtime" }\n'
        'crm-consents-query-adapter = { path = "../crm-consents-query-adapter" }\n',
        'crm-capability-runtime = { path = "../crm-capability-runtime" }\n'
        'crm-consents-capability-adapter = { path = "../crm-consents-capability-adapter" }\n'
        'crm-consents-query-adapter = { path = "../crm-consents-query-adapter" }\n',
        "Consents capability dependency",
    ),
    (
        'crm-consents-query-adapter = { path = "../crm-consents-query-adapter" }\n'
        'crm-core-data = { path = "../crm-core-data" }\n',
        'crm-consents-query-adapter = { path = "../crm-consents-query-adapter" }\n'
        'crm-contact-points-capability-adapter = { path = "../crm-contact-points-capability-adapter" }\n'
        'crm-contact-points-capability-composition = { path = "../crm-contact-points-capability-composition" }\n'
        'crm-core-data = { path = "../crm-core-data" }\n',
        "Contact Points runtime dependencies",
    ),
    (
        'crm-customer-360-query-adapter = { path = "../crm-customer-360-query-adapter" }\n'
        'crm-customer-data-operations-capability-adapter = { path = "../crm-customer-data-operations-capability-adapter" }\n',
        'crm-customer-360-query-adapter = { path = "../crm-customer-360-query-adapter" }\n'
        'crm-customer-accounts-capability-adapter = { path = "../crm-customer-accounts-capability-adapter" }\n'
        'crm-customer-data-operations-capability-adapter = { path = "../crm-customer-data-operations-capability-adapter" }\n',
        "Customer Accounts bootstrap dependency",
    ),
    (
        'crm-parties-query-adapter = { path = "../crm-parties-query-adapter" }\n'
        'crm-party-relationships-projection = { path = "../crm-party-relationships-projection" }\n',
        'crm-parties-query-adapter = { path = "../crm-parties-query-adapter" }\n'
        'crm-party-reference-composition = { path = "../crm-party-reference-composition" }\n'
        'crm-party-relationships-capability-adapter = { path = "../crm-party-relationships-capability-adapter" }\n'
        'crm-party-relationships-projection = { path = "../crm-party-relationships-projection" }\n',
        "Party reference and relationship bootstrap dependencies",
    ),
)


def parser() -> argparse.ArgumentParser:
    value = argparse.ArgumentParser()
    value.add_argument("--root", type=Path, default=Path.cwd())
    mode = value.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    return value


def replace_once(text: str, old: str, new: str, label: str) -> tuple[str, bool]:
    if new in text:
        return text, False
    count = text.count(old)
    if count != 1:
        raise NavigationError(f"step-12 correction expected one {label}, found {count}")
    return text.replace(old, new, 1), True


def materialize(root: Path) -> bool:
    changed = False
    for relative in TEST_PATHS:
        path = root / relative
        text = path.read_text(encoding="utf-8")
        text, updated = replace_once(
            text,
            SHORT_NON_GOAL,
            FULL_NON_GOAL,
            f"shortened non-goal in {relative}",
        )
        if updated:
            path.write_text(text, encoding="utf-8")
            changed = True

    cargo_path = root / "crates/crm-application-runtime/Cargo.toml"
    cargo = cargo_path.read_text(encoding="utf-8")
    for old, new, label in DEPENDENCY_INSERTIONS:
        cargo, updated = replace_once(cargo, old, new, label)
        changed = changed or updated
    cargo_path.write_text(cargo, encoding="utf-8")

    write_generated_documents(root)
    subprocess.run([sys.executable, "scripts/repo.py", "lock"], cwd=root, check=True)
    subprocess.run(["cargo", "fmt", "--all"], cwd=root, check=True)
    return changed


def commit(root: Path) -> None:
    branch = os.environ.get("GITHUB_HEAD_REF") or os.environ.get("GITHUB_REF_NAME")
    if not branch:
        raise NavigationError("step-12 correction materializer requires a branch ref")
    subprocess.run(["git", "config", "user.name", "github-actions[bot]"], cwd=root, check=True)
    subprocess.run(
        ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
        cwd=root,
        check=True,
    )
    subprocess.run(
        [
            "git",
            "add",
            "Cargo.lock",
            "crates/crm-application-runtime/Cargo.toml",
            "docs/ACTIVE_PACKET.md",
            *TEST_PATHS,
        ],
        cwd=root,
        check=True,
    )
    subprocess.run(
        ["git", "commit", "-m", "Correct step 12 runtime edges and packet assertions"],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "push", "origin", f"HEAD:{branch}"], cwd=root, check=True)


def main() -> int:
    args = parser().parse_args()
    try:
        if args.write:
            if materialize(args.root):
                commit(args.root)
            return 0
        stale = stale_generated_documents(args.root)
    except (NavigationError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    if stale:
        for path in stale:
            print(f"STALE {path}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
