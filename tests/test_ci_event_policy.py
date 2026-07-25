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
                "push must be restricted to branch main",
                "missing top-level concurrency block",
            ),
        )

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
