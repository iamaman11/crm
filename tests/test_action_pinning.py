from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from scripts.check_action_pinning import check_workflow


class ActionPinningTests(unittest.TestCase):
    def write_workflow(self, content: str) -> Path:
        temporary = TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        path = Path(temporary.name) / "workflow.yml"
        path.write_text(content, encoding="utf-8")
        return path

    def test_accepts_full_sha_with_version_comment(self) -> None:
        path = self.write_workflow(
            """name: Example
jobs:
  check:
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7
      - uses: ./local-action
      - uses: docker://alpine:3.22
"""
        )

        self.assertEqual(check_workflow(path), ())

    def test_rejects_tag_and_missing_comment(self) -> None:
        path = self.write_workflow(
            """name: Example
jobs:
  check:
    steps:
      - uses: actions/checkout@v7
      - uses: actions/setup-python@ece7cb06caefa5fff74198d8649806c4678c61a1
"""
        )

        failures = check_workflow(path)
        self.assertEqual(len(failures), 3)
        self.assertEqual(
            [failure.message for failure in failures],
            [
                "external Action must be pinned to a full lowercase commit SHA",
                "pinned Action must retain a human-readable version comment",
                "pinned Action must retain a human-readable version comment",
            ],
        )

    def test_rejects_malformed_external_reference(self) -> None:
        path = self.write_workflow(
            """name: Example
jobs:
  check:
    steps:
      - uses: checkout
"""
        )

        failures = check_workflow(path)
        self.assertEqual(len(failures), 1)
        self.assertEqual(failures[0].message, "external Action reference is missing @ref")


if __name__ == "__main__":
    unittest.main()
