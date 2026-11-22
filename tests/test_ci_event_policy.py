from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from scripts.check_ci_event_policy import check_workflow


class CiEventPolicyTests(unittest.TestCase):
    def write_workflow(self, content: str) -> Path:
        temporary = TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        path = Path(temporary.name) / "workflow.yml"
        path.write_text(content, encoding="utf-8")
        return path

    def test_accepts_main_only_push_and_pr_only_cancellation(self) -> None:
        path = self.write_workflow(
            """name: Example

on:
  push:
    branches:
      - main
  pull_request:
    branches:
      - main

concurrency:
  group: ${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}

jobs:
  test:
    runs-on: ubuntu-latest
"""
        )

        self.assertEqual(check_workflow(path).errors, ())

    def test_rejects_unrestricted_push_and_missing_concurrency(self) -> None:
        path = self.write_workflow(
            """name: Example

on:
  push:
  pull_request:

jobs:
  test:
    runs-on: ubuntu-latest
"""
        )

        self.assertEqual(
            check_workflow(path).errors,
            (
                "pull_request must be restricted to branch main",
                "push must be restricted to branch main",
                "missing top-level concurrency block",
            ),
        )

    def test_only_crm_release_workflow_may_push_dev(self) -> None:
        temporary = TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        directory = Path(temporary.name) / ".github" / "workflows"
        directory.mkdir(parents=True)
        path = directory / "cloudflare-crm-spike.yml"
        path.write_text(
            """name: Cloudflare CRM Spike CI
on:
  push:
    branches:
      - dev
    paths-ignore:
      - '*.md'
      - '**/*.md'
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: false
jobs:
  gate:
    runs-on: ubuntu-latest
""",
            encoding="utf-8",
        )
        self.assertEqual(check_workflow(path).errors, ())
        path.write_text(
            path.read_text(encoding="utf-8").replace(
                "  push:\n    branches:\n      - dev", "  push:\n    branches:\n      - main"
            ),
            encoding="utf-8",
        )
        self.assertEqual(
            check_workflow(path).errors,
            ("push must be restricted to branch dev",),
        )

    def test_other_workflows_cannot_push_dev(self) -> None:
        path = self.write_workflow(
            """name: Other Project
on:
  push:
    branches:
      - dev
  pull_request:
    branches:
      - main
concurrency:
  group: ${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: ${{ github.event_name == 'pull_request' }}
jobs:
  test:
    runs-on: ubuntu-latest
"""
        )
        self.assertEqual(
            check_workflow(path).errors,
            ("push must be restricted to branch main",),
        )

    def test_repository_workflows_never_select_markdown_inputs(self) -> None:
        workflows = Path(__file__).resolve().parents[1] / ".github" / "workflows"
        for path in sorted(workflows.glob("*.yml")):
            with self.subTest(workflow=path.name):
                self.assertNotIn(
                    "CI must not select Markdown documentation inputs",
                    check_workflow(path).errors,
                )

    def test_crm_actual_workflow_and_regressions(self) -> None:
        source = Path(__file__).resolve().parents[1] / ".github/workflows/cloudflare-crm-spike.yml"
        original = source.read_text(encoding="utf-8")
        self.assertEqual(check_workflow(source).errors, ())
        temporary = TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        path = Path(temporary.name) / ".github/workflows/cloudflare-crm-spike.yml"
        path.parent.mkdir(parents=True)
        cases = (
            (original.replace("'**/*.md'", "'apps/**'"), "CRM may ignore only **/*.md documentation"),
            (original.replace("      - '*.md'", "      - 'docs/PROJECT_STATUS.md'\n      - '*.md'"),
             "CRM may ignore only **/*.md documentation"),
            (original.replace("    paths-ignore:", "    paths:"), "CRM may ignore only **/*.md documentation"),
            (original.replace("cancel-in-progress: false", "cancel-in-progress: true"), "CRM must not cancel an active release"),
            (original.replace("python scripts/crm-module-coverage.py",
                              "python scripts/generate_repository_navigation.py --check\n          python scripts/crm-module-coverage.py"),
             "CRM CI must not validate Markdown documentation"),
            (original.replace("concurrency:\n", "  pull_request:\n    branches:\n      - dev\nconcurrency:\n", 1), "CRM uses one dev pipeline, not duplicate pull_request builds"),
            (original.replace("      - dev\n", "      - dev\n      - experimental\n", 1), "push must be restricted to branch dev"),
        )
        for contents, expected_error in cases:
            with self.subTest(expected_error=expected_error):
                self.assertNotEqual(contents, original, "negative fixture must change the workflow")
                path.write_text(contents, encoding="utf-8")
                self.assertIn(expected_error, check_workflow(path).errors)

    def test_crm_uses_cross_commit_compiler_cache_without_splitting_acceptance(self) -> None:
        workflow = (Path(__file__).resolve().parents[1]
                    / ".github/workflows/cloudflare-crm-spike.yml").read_text(encoding="utf-8")
        self.assertIn("sccache-action@fc920bf0ec8de6ee65d409111f7ec508035751ba", workflow)
        self.assertIn("RUSTC_WRAPPER=$SCCACHE_PATH", workflow)
        self.assertIn("SCCACHE_GHA_ENABLED=true", workflow)
        self.assertIn("crm-rust1100-wasm-v3-", workflow)
        key = next(line for line in workflow.splitlines() if "key:" in line and "crm-rust1100-wasm-v3-" in line)
        self.assertNotIn("github.sha", key, "Cargo target cache must be reusable across pushes")
        self.assertIn("needs: cloudflare-crm-spike", workflow)
        self.assertIn("worker-build --release --locked --features native", workflow)
        self.assertIn("worker-build --dev --locked --features native-ci", workflow)
        self.assertIn("node rust-party/test/native.mjs", workflow)
        self.assertIn("node apps/cloudflare-crm/e2e/live.mjs", workflow)
        self.assertIn("node apps/cloudflare-crm/scripts/deploy.mjs verify-data", workflow)

    def test_rejects_legacy_pr_into_dev_even_for_pr_only_workflows(self) -> None:
        path = self.write_workflow("""name: Legacy
on:
  pull_request:
    branches:
      - dev
jobs:
  test:
    runs-on: ubuntu-latest
""")
        self.assertEqual(
            check_workflow(path).errors,
            ("pull_request must be restricted to branch main",),
        )

    def test_rejects_unrestricted_pr_only_workflow(self) -> None:
        path = self.write_workflow("""name: Legacy
on:
  pull_request:
jobs:
  test:
    runs-on: ubuntu-latest
""")
        self.assertEqual(
            check_workflow(path).errors,
            ("pull_request must be restricted to branch main",),
        )

    def test_accepts_main_pr_with_additional_path_filters(self) -> None:
        path = self.write_workflow("""name: Legacy
on:
  pull_request:
    branches:
      - main
    types: [opened, synchronize]
    paths:
      - "modules/**"
      - "proto/**"
jobs:
  test:
    runs-on: ubuntu-latest
""")
        self.assertEqual(check_workflow(path).errors, ())

    def test_ignores_non_pr_workflows(self) -> None:
        path = self.write_workflow(
            """name: Scheduled

on:
  schedule:
    - cron: '0 0 * * *'
  workflow_dispatch:

jobs:
  test:
    runs-on: ubuntu-latest
"""
        )

        self.assertEqual(check_workflow(path).errors, ())


if __name__ == "__main__":
    unittest.main()
