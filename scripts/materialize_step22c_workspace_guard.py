#!/usr/bin/env python3
"""One-shot fail-closed materializer for the Step 22C workspace inventory guard."""

from __future__ import annotations

from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
TEST_PATH = ROOT / "tests/test_workspace_analysis.py"
OLD_METHOD = "test_committed_step22_ledger_matches_fresh_structural_inventory"
NEW_METHOD = r'''    def test_committed_step22_baseline_is_immutable_and_current_remediation_is_exact(self) -> None:
        path = ROOT / "step22-architecture-inventory.json"
        committed_text = path.read_text(encoding="utf-8")
        committed = json.loads(committed_text)
        self.assertEqual(
            committed_text,
            json.dumps(committed, indent=2, sort_keys=True) + "\n",
        )
        self.assertRegex(
            committed["measurement_source_commit"], r"^[0-9a-f]{40}$"
        )
        self.assertEqual(committed["phase"], "inventory-only")
        self.assertEqual(committed["decision_state"], "unresolved")
        self.assertEqual(
            committed["runtime_fanin"]["counts"],
            {"all": 63, "production": 62, "test_only": 1, "build": 0},
        )
        self.assertEqual(
            committed["permanent_gates"]["counts"],
            {"workflows": 41, "jobs": 42},
        )

        accepted_runtime_ids = {
            row[0] for row in committed["runtime_fanin"]["rows"]
        }
        accepted_workflow_ids = [
            row[0] for row in committed["permanent_gates"]["workflow_rows"]
        ]
        accepted_job_ids = [
            row[0] for row in committed["permanent_gates"]["job_rows"]
        ]
        self.assertEqual(len(accepted_runtime_ids), 63)
        self.assertEqual(
            len(accepted_workflow_ids), len(set(accepted_workflow_ids))
        )
        self.assertEqual(len(accepted_job_ids), len(set(accepted_job_ids)))
        self.assertTrue(
            all(
                identifier.startswith(".github/workflows/")
                for identifier in accepted_job_ids
            )
        )
        self.assertTrue(all("#" in identifier for identifier in accepted_job_ids))
        self.assertFalse(
            committed["decision_boundary"]["final_classifications_recorded"]
        )
        self.assertFalse(
            committed["decision_boundary"]["gate_dispositions_recorded"]
        )
        self.assertFalse(committed["decision_boundary"]["remediation_performed"])
        self.assertFalse(committed["decision_boundary"]["step22_complete"])

        fresh = json.loads(
            json.dumps(self.review_ledger(build_report(ROOT)["step22_inventory"]))
        )
        self.assertRegex(fresh["measurement_source_commit"], r"^[0-9a-f]{40}$")
        self.assertEqual(fresh["schema_version"], committed["schema_version"])
        self.assertEqual(fresh["phase"], "inventory-only")
        self.assertEqual(fresh["decision_state"], "unresolved")
        self.assertEqual(fresh["adr"], committed["adr"])
        self.assertEqual(
            fresh["runtime_fanin"]["package"],
            committed["runtime_fanin"]["package"],
        )
        self.assertEqual(
            fresh["runtime_fanin"]["manifest_path"],
            committed["runtime_fanin"]["manifest_path"],
        )
        self.assertEqual(
            fresh["runtime_fanin"]["columns"],
            committed["runtime_fanin"]["columns"],
        )
        self.assertEqual(
            fresh["runtime_fanin"]["counts"],
            {"all": 62, "production": 61, "test_only": 1, "build": 0},
        )
        self.assertEqual(
            fresh["permanent_gates"], committed["permanent_gates"]
        )

        current_runtime_ids = {
            row[0] for row in fresh["runtime_fanin"]["rows"]
        }
        decisions = json.loads(
            (ROOT / "step22-runtime-fanin-decisions.json").read_text(
                encoding="utf-8"
            )
        )
        removed_ids = {
            stable_id
            for stable_id, classification, _ in decisions["final_rows"]
            if classification == "removed"
        }
        self.assertEqual(
            removed_ids,
            {
                "crm-application-runtime::dependencies::"
                "crm-customer-privacy-query-adapter"
            },
        )
        self.assertEqual(accepted_runtime_ids - current_runtime_ids, removed_ids)
        self.assertEqual(current_runtime_ids - accepted_runtime_ids, set())
        self.assertEqual(len(current_runtime_ids), 62)
        self.assertEqual(
            decisions["remediation_evidence"]["before"],
            {"all": 63, "production": 62, "test_only": 1},
        )
        self.assertEqual(
            decisions["remediation_evidence"]["after"],
            {"all": 62, "production": 61, "test_only": 1},
        )
        self.assertTrue(
            decisions["decision_boundary"]["remediation_performed"]
        )
        self.assertFalse(decisions["decision_boundary"]["step22_complete"])

'''


def main() -> None:
    text = TEST_PATH.read_text(encoding="utf-8")
    if "test_committed_step22_baseline_is_immutable_and_current_remediation_is_exact" in text:
        if OLD_METHOD in text:
            raise RuntimeError("both old and new Step 22 workspace guards are present")
        return
    pattern = re.compile(
        rf"(?ms)^    def {OLD_METHOD}\(self\) -> None:\n.*?(?=^\nif __name__ == \"__main__\":)"
    )
    updated, count = pattern.subn(NEW_METHOD, text)
    if count != 1:
        raise RuntimeError(
            f"expected one replacement for {OLD_METHOD}, got {count}"
        )
    if OLD_METHOD in updated:
        raise RuntimeError("stale Step 22A fresh-equality guard remains")
    TEST_PATH.write_text(updated, encoding="utf-8")


if __name__ == "__main__":
    main()
