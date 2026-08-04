from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from scripts import customer_privacy_operations as operations


class CustomerPrivacyOperationsTests(unittest.TestCase):
    def test_repository_policy_is_valid_and_uses_immutable_inputs(self) -> None:
        policy = operations.load_policy()
        operations.validate_policy(policy)
        self.assertEqual(
            policy["schema_version"],
            "crm.customer-privacy-operations-policy/v1",
        )
        self.assertIn("@sha256:", policy["postgres_image"])
        self.assertGreaterEqual(policy["probe_count"], 10)
        self.assertEqual(policy["allowed_probe_failures"], 0)

    def test_shell_environment_is_deterministic_and_contains_no_credentials(self) -> None:
        rendered = operations.shell_environment(operations.load_policy())
        self.assertIn("export OPS_POSTGRES_IMAGE=", rendered)
        self.assertIn("export OPS_SOURCE_DATABASE=crm_ops_source", rendered)
        self.assertIn("export OPS_RESTORE_DATABASE=crm_ops_restore", rendered)
        self.assertNotIn("PASSWORD", rendered)
        self.assertEqual(rendered, operations.shell_environment(operations.load_policy()))

    def test_report_accepts_exact_restore_slo_observability_and_supply_chain_evidence(
        self,
    ) -> None:
        policy = operations.load_policy()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            latencies = root / "latencies.txt"
            latencies.write_text(
                "\n".join("0.010" for _ in range(policy["probe_count"])) + "\n",
                encoding="utf-8",
            )
            metrics = root / "metrics.txt"
            metrics.write_text(
                "\n".join(policy["required_metric_markers"]) + "\n",
                encoding="utf-8",
            )
            supply_chain = root / "supply-chain.sha256"
            supply_chain.write_text(
                "\n".join(
                    f"{'0' * 64}  {relative}"
                    for relative in policy["supply_chain_inputs"]
                )
                + "\n",
                encoding="utf-8",
            )
            backup = root / "backup.dump"
            backup.write_bytes(b"deterministic-test-backup")
            backup_sha = hashlib.sha256(backup.read_bytes()).hexdigest()
            args = argparse.Namespace(
                startup_seconds="1.25",
                latencies=str(latencies),
                probe_failures="0",
                metrics=str(metrics),
                supply_chain=str(supply_chain),
                backup=str(backup),
                backup_sha256=backup_sha,
                output=str(root / "report.json"),
            )
            report = operations.build_report(args, policy)
            self.assertEqual(
                report["schema_version"],
                "crm.customer-privacy-operations-report/v1",
            )
            self.assertTrue(report["restore_verified"])
            self.assertTrue(report["browser_verified"])
            self.assertEqual(report["readiness_probe_failures"], 0)
            self.assertEqual(report["backup_sha256"], backup_sha)
            json.dumps(report)

    def test_report_rejects_observability_leak(self) -> None:
        policy = operations.load_policy()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            latencies = root / "latencies.txt"
            latencies.write_text(
                "\n".join("0.010" for _ in range(policy["probe_count"])) + "\n",
                encoding="utf-8",
            )
            metrics = root / "metrics.txt"
            metrics.write_text(
                "\n".join(
                    policy["required_metric_markers"]
                    + [policy["forbidden_observability_markers"][0]]
                ),
                encoding="utf-8",
            )
            supply_chain = root / "supply-chain.sha256"
            supply_chain.write_text(
                "\n".join(
                    f"{'0' * 64}  {relative}"
                    for relative in policy["supply_chain_inputs"]
                ),
                encoding="utf-8",
            )
            backup = root / "backup.dump"
            backup.write_bytes(b"backup")
            args = argparse.Namespace(
                startup_seconds="1",
                latencies=str(latencies),
                probe_failures="0",
                metrics=str(metrics),
                supply_chain=str(supply_chain),
                backup=str(backup),
                backup_sha256=hashlib.sha256(backup.read_bytes()).hexdigest(),
                output=str(root / "report.json"),
            )
            with self.assertRaisesRegex(
                operations.OperationsError,
                "leaks forbidden fixture marker",
            ):
                operations.build_report(args, policy)


if __name__ == "__main__":
    unittest.main()
