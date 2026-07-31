#!/usr/bin/env python3
"""One-run materializer for the step-10/step-11 evidence correction."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys

from repository_navigation import NavigationError, stale_generated_documents, write_generated_documents


WRONG = "Repository step 10 is accepted through PR #244 / accepted source `405d2dbb97bb371b51cfb1d4ffb5549a57262878` / squash merge `4b08202fe9dd0c0df83567e24e6b9d86fb79c9db` / 34 of 34 applicable permanent workflows on one unchanged exact head. It implements"
RIGHT = "Repository step 10 is accepted through PR #241 / accepted source `2bb3a671deb18a6ae3bcea228ed01ed287b9de6a` / squash merge `19232f6f3e2ae87aabeb080257c1aac5477a6616` / 34 of 34 applicable permanent workflows on one unchanged exact head. It implements"
MARKER = '        self.assertIn("customer_privacy.access_export.request@1.0.0", self.catalog)\n'
ASSERTIONS = '''        self.assertIn("customer_privacy.access_export.request@1.0.0", self.catalog)
        self.assertIn(
            "Repository step 10 is accepted through PR #241 / accepted source `2bb3a671deb18a6ae3bcea228ed01ed287b9de6a` / squash merge `19232f6f3e2ae87aabeb080257c1aac5477a6616` / 34 of 34 applicable permanent workflows on one unchanged exact head.",
            self.status,
        )
        self.assertNotIn("Repository step 10 is accepted through PR #244", self.status)
'''


def parser() -> argparse.ArgumentParser:
    value = argparse.ArgumentParser()
    value.add_argument("--root", type=Path, default=Path.cwd())
    mode = value.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true")
    mode.add_argument("--check", action="store_true")
    return value


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise NavigationError(f"evidence correction expected one {label}, found {count}")
    return text.replace(old, new, 1)


def commit(root: Path) -> None:
    branch = os.environ.get("GITHUB_HEAD_REF")
    if not branch:
        raise NavigationError("evidence correction requires GITHUB_HEAD_REF")
    subprocess.run(["git", "config", "user.name", "github-actions[bot]"], cwd=root, check=True)
    subprocess.run(["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"], cwd=root, check=True)
    subprocess.run(
        ["git", "add", "docs/PROJECT_STATUS.md", "tests/test_architecture_documentation_consistency.py", "docs/ACTIVE_PACKET.md"],
        cwd=root,
        check=True,
    )
    subprocess.run(["git", "commit", "-m", "Correct repository step evidence attribution"], cwd=root, check=True)
    subprocess.run(["git", "push", "origin", f"HEAD:{branch}"], cwd=root, check=True)


def main() -> int:
    args = parser().parse_args()
    try:
        if args.write:
            status_path = args.root / "docs/PROJECT_STATUS.md"
            status = replace_once(status_path.read_text(encoding="utf-8"), WRONG, RIGHT, "step-10 status claim")
            status_path.write_text(status, encoding="utf-8")

            test_path = args.root / "tests/test_architecture_documentation_consistency.py"
            tests = test_path.read_text(encoding="utf-8")
            if "Repository step 10 is accepted through PR #244" in tests:
                raise NavigationError("permanent test already contains the forbidden attribution")
            tests = replace_once(tests, MARKER, ASSERTIONS, "step-10 permanent assertion marker")
            test_path.write_text(tests, encoding="utf-8")

            write_generated_documents(args.root)
            commit(args.root)
            return 0

        stale = stale_generated_documents(args.root)
    except NavigationError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    if stale:
        for path in stale:
            print(f"STALE {path}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
