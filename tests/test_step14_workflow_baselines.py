"""Regression guards for Repository Step 14 permanent workflow baselines."""

from pathlib import Path
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = {
    "approval": ROOT / ".github/workflows/customer-privacy-approval.yml",
    "discovery": ROOT / ".github/workflows/customer-privacy-discovery.yml",
    "planning": ROOT / ".github/workflows/customer-privacy-planning.yml",
}


class Step14WorkflowBaselineTests(unittest.TestCase):
    def test_workspace_has_exact_accepted_package_count(self) -> None:
        workspace = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        self.assertEqual(len(workspace["workspace"]["members"]), 112)

    def test_permanent_workflows_assert_the_exact_reduced_workspace(self) -> None:
        approval = WORKFLOWS["approval"].read_text(encoding="utf-8")
        discovery = WORKFLOWS["discovery"].read_text(encoding="utf-8")
        planning = WORKFLOWS["planning"].read_text(encoding="utf-8")

        self.assertIn('test "${package_count}" = "112"', approval)
        self.assertIn('if [ "${package_count}" != "112" ]; then', discovery)
        self.assertIn('test "${package_count}" = "112"', planning)

        for name, workflow in {
            "approval": approval,
            "discovery": discovery,
            "planning": planning,
        }.items():
            with self.subTest(workflow=name):
                self.assertNotIn('test "${package_count}" = "113"', workflow)
                self.assertNotIn('if [ "${package_count}" != "113" ]; then', workflow)

    def test_existing_behavioral_acceptance_steps_remain_present(self) -> None:
        approval = WORKFLOWS["approval"].read_text(encoding="utf-8")
        for marker in (
            "Verify approval libraries and production inventory",
            "Verify real crm-api approval process",
            "Verify approval invariants and active packet boundaries",
            "Roll back authoritative schema",
            "Reapply authoritative schema and fixtures",
            "Repeat approval library and real-process acceptance",
        ):
            self.assertIn(marker, approval)

        planning = WORKFLOWS["planning"].read_text(encoding="utf-8")
        for marker in (
            "Verify planning and read packages and production inventory",
            "Verify frozen non-effects",
            "Roll back authoritative schema",
            "Reapply and repeat acceptance",
        ):
            self.assertIn(marker, planning)

        discovery = WORKFLOWS["discovery"].read_text(encoding="utf-8")
        for marker in (
            "Verify discovery packages and production inventory",
            "Verify frozen non-effects",
            "Roll back authoritative schema",
            "Reapply and repeat acceptance",
        ):
            self.assertIn(marker, discovery)


if __name__ == "__main__":
    unittest.main()
