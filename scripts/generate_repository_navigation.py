#!/usr/bin/env python3
"""One-run materializer restoring repository-step-12 behavior neutrality."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys

from repository_navigation import NavigationError, stale_generated_documents, write_generated_documents


LOCK_REPLACEMENTS = (
    (
        '''name = "http"\nversion = "1.5.0"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "918d3568bebf352712bc2ef3d46a8bcf1a75b373be6539de198e9105cbbf9ce0"''',
        '''name = "http"\nversion = "1.4.2"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "6970f50e31d6fc17d3fa27329444bfa74e196cf62e95052a3f6fee181dba6425"''',
        "http",
    ),
    (
        '''name = "hybrid-array"\nversion = "0.4.14"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "707114b52a152fa7bdb290cd7cd5912d9467273b6d74e21b8d81aca1f8533f6b"''',
        '''name = "hybrid-array"\nversion = "0.4.13"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "818356c5132c1fede50f837ca96afbe78ff42413047f4abb886217845e1b6c8c"''',
        "hybrid-array",
    ),
    (
        '''name = "rustls"\nversion = "0.23.43"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "0283386ce02abc0151e1761d08802dfe86c173b0b494af5cbc086574e453da06"''',
        '''name = "rustls"\nversion = "0.23.42"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "3c54fcab019b409d04215d3a17cb438fd7fbf192ee61461f20f4fe18704bc138"''',
        "rustls",
    ),
    (
        '''name = "tokio-macros"\nversion = "2.7.2"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "78773a2a397f451582ce068015985c33193cf6dea8b74d2a639fe457b2f07b0e"\ndependencies = [\n "proc-macro2",\n "quote",\n "syn 3.0.3",\n]''',
        '''name = "tokio-macros"\nversion = "2.7.1"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "6328af13490e73a9b4694030fafd93f8c8c6a9dede33e821c3fc63eddf8042ba"\ndependencies = [\n "proc-macro2",\n "quote",\n "syn 2.0.119",\n]''',
        "tokio-macros",
    ),
)

DUPLICATE_PARTY_QUERY_CONSTRUCTION = '''    let _party_queries = Arc::new(PartyQueryAdapter::new(\n        store.clone(),\n        cursor(cursor_key)?,\n        visibility_authorizer.clone(),\n    )?);\n\n'''


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    return parser


def restore_lockfile(root: Path) -> bool:
    path = root / "Cargo.lock"
    text = path.read_text(encoding="utf-8")
    changed = False
    for current, baseline, label in LOCK_REPLACEMENTS:
        if baseline in text:
            continue
        count = text.count(current)
        if count != 1:
            raise NavigationError(
                f"lockfile materializer expected one current {label} record, found {count}"
            )
        text = text.replace(current, baseline, 1)
        changed = True
    if changed:
        path.write_text(text, encoding="utf-8")
    return changed


def remove_duplicate_party_query_construction(root: Path) -> bool:
    path = root / "crates/crm-application-runtime/src/native_composition.rs"
    text = path.read_text(encoding="utf-8")
    count = text.count(DUPLICATE_PARTY_QUERY_CONSTRUCTION)
    if count == 0:
        return False
    if count != 1:
        raise NavigationError(
            f"runtime materializer expected one duplicate Party query constructor, found {count}"
        )
    path.write_text(
        text.replace(DUPLICATE_PARTY_QUERY_CONSTRUCTION, "", 1),
        encoding="utf-8",
    )
    return True


def commit(root: Path) -> None:
    branch = os.environ.get("GITHUB_HEAD_REF") or os.environ.get("GITHUB_REF_NAME")
    if not branch:
        raise NavigationError("behavior-neutrality materializer requires a branch ref")
    subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1", "--no-deps"],
        cwd=root,
        check=True,
        stdout=subprocess.DEVNULL,
    )
    subprocess.run(["cargo", "fmt", "--all", "--", "--check"], cwd=root, check=True)
    subprocess.run(["git", "config", "user.name", "github-actions[bot]"], cwd=root, check=True)
    subprocess.run(
        ["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"],
        cwd=root,
        check=True,
    )
    subprocess.run(
        ["git", "add", "Cargo.lock", "crates/crm-application-runtime/src/native_composition.rs"],
        cwd=root,
        check=True,
    )
    subprocess.run(
        ["git", "commit", "-m", "Restore repository-step-12 behavior neutrality"],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "push", "origin", f"HEAD:{branch}"], cwd=root, check=True)


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        if args.write:
            changed = restore_lockfile(args.root)
            changed = remove_duplicate_party_query_construction(args.root) or changed
            write_generated_documents(args.root)
            if changed:
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
