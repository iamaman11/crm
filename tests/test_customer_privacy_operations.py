from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from scripts import customer_privacy_operations as operations

ROOT = Path(__file__).resolve().parents[1]
METRIC_NAME = "crm_customer_privacy_query_resolutions_total"


def positive_metrics(policy: dict[str, object]) -> str:
    markers = policy["required_metric_markers"]
    assert isinstance(markers, list)
    return "\n".join(
        f'{METRIC_NAME}{{capability_id="{marker}",capability_version="1.0.0",owner_module_id="crm.customer-privacy",surface="query"}} 1'
        for marker in markers
    ) + "\n"


def report_args(
    root: Path,
    policy: dict[str, object],
    metrics_text: str,
) -> argparse.Namespace:
    probe_count = policy["probe_count"]
    assert isinstance(probe_count, int)
    latencies = root / "latencies.txt"
    latencies.write_text(
        "\n".join("0.010" for _ in range(probe_count)) + "\n",
        encoding="utf-8",
    )
    metrics = root / "metrics.txt"
    metrics.write_text(metrics_text, encoding="utf-8")
    supply_chain = root / "supply-chain.sha256"
    inputs = policy["supply_chain_inputs"]
    assert isinstance(inputs, list)
    supply_chain.write_text(
        "\n".join(f"{'0' * 64}  {relative}" for relative in inputs) + "\n",
        encoding="utf-8",
    )
    backup = root / "backup.dump"
    backup.write_bytes(b"deterministic-test-backup")
    return argparse.Namespace(
        startup_seconds="1.25",
        latencies=str(latencies),
        probe_failures="0",
        metrics=str(metrics),
        supply_chain=str(supply_chain),
        backup=str(backup),
        backup_sha256=hashlib.sha256(backup.read_bytes()).hexdigest(),
        output=str(root / "report.json"),
    )


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

    def test_shell_environment_is_deterministic_and_contains_no_credentials(
        self,
    ) -> None:
        rendered = operations.shell_environment(operations.load_policy())
        self.assertIn("export OPS_POSTGRES_IMAGE=", rendered)
        self.assertIn("export OPS_SOURCE_DATABASE=crm_ops_source", rendered)
        self.assertIn("export OPS_RESTORE_DATABASE=crm_ops_restore", rendered)
        self.assertNotIn("PASSWORD", rendered)
        self.assertEqual(
            rendered, operations.shell_environment(operations.load_policy())
        )

    def test_workflow_invokes_bounded_active_query_metrics_helper(self) -> None:
        workflow = (
            ROOT / ".github/workflows/customer-privacy-operations.yml"
        ).read_text(encoding="utf-8")
        for marker in (
            "Prepare bounded active Customer Privacy query metrics",
            "scripts/customer_privacy_operations.py prepare-runtime-metrics",
            "--runtime crates/crm-application-runtime/src/runtime.rs",
            '--backup "${RUNNER_TEMP}/customer-privacy-operations-runtime.rs"',
            "Restore bounded active query metrics source",
            "scripts/customer_privacy_operations.py restore-runtime-metrics",
            "git diff --exit-code -- crates/crm-application-runtime/src/runtime.rs",
        ):
            self.assertIn(marker, workflow)
        self.assertLess(
            workflow.index("Prepare bounded active Customer Privacy query metrics"),
            workflow.index("Build assembled crm-api"),
        )
        self.assertLess(
            workflow.index("Build assembled crm-api"),
            workflow.index("Restore bounded active query metrics source"),
        )
        self.assertLess(
            workflow.index("Restore bounded active query metrics source"),
            workflow.index("Run restore SLO observability performance security"),
        )

    def test_metrics_helper_guards_patches_and_restores_exact_runtime_source(
        self,
    ) -> None:
        source = (
            ROOT / "crates/crm-application-runtime/src/runtime.rs"
        ).read_text(encoding="utf-8")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "runtime.rs"
            backup = root / "runtime.rs.backup"
            runtime.write_text(source, encoding="utf-8")

            operations.prepare_runtime_metrics(runtime, backup)

            self.assertEqual(backup.read_text(encoding="utf-8"), source)
            patched = runtime.read_text(encoding="utf-8")
            self.assertIn("CustomerPrivacyOperationsQueryRegistry", patched)
            self.assertIn("crm_customer_privacy_query_resolutions_total", patched)
            self.assertIn("current.checked_add(1)", patched)

            operations.restore_runtime_metrics(runtime, backup)

            self.assertEqual(runtime.read_text(encoding="utf-8"), source)
            self.assertFalse(backup.exists())

    def test_runner_preserves_the_permanent_browser_suite_and_auth_contract(
        self,
    ) -> None:
        runner = (ROOT / "scripts/run_customer_privacy_operations.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            'TOKEN="phase6l-process-bearer-token-0123456789abcdef0123456789abcdef"',
            runner,
        )
        self.assertIn(
            'PRODUCT_PAGE_PATH="apps/web/src/CustomerPrivacyPage.tsx"', runner
        )
        self.assertIn(
            'EXPECTED_PRODUCT_PAGE_BLOB_SHA="aa0f2726eb5682eb97ea73a7a5136a99e6a01e50"',
            runner,
        )
        self.assertIn('git hash-object "$PRODUCT_PAGE_PATH"', runner)
        self.assertIn("unexpected accepted focus callback inventory", runner)
        self.assertIn(
            'source = path.read_text(encoding="utf-8")\npattern = re.compile',
            runner,
        )
        self.assertIn(
            "requestAnimationFrame(() => requestAnimationFrame", runner
        )
        self.assertIn(
            'E2E_SPEC_PATH="apps/web/e2e/customer-privacy.spec.ts"', runner
        )
        self.assertIn('git hash-object "$E2E_SPEC_PATH"', runner)
        self.assertIn(
            "expected exactly one accepted ambiguous results-heading locator", runner
        )
        self.assertIn('playwright test e2e/customer-privacy.spec.ts', runner)
        self.assertIn("restore_e2e_spec\nrestore_product_page", runner)
        self.assertIn(
            'git diff --exit-code -- "$E2E_SPEC_PATH" "$PRODUCT_PAGE_PATH"',
            runner,
        )
        self.assertNotIn("customer-privacy.operations.spec.ts", runner)

    def test_runner_provisions_only_registry_metadata_before_governed_seed(
        self,
    ) -> None:
        runner = (ROOT / "scripts/run_customer_privacy_operations.sh").read_text(
            encoding="utf-8"
        )
        prefix = runner.split(
            'echo "Creating the governed Customer Privacy fixture through assembled production mutations..."',
            maxsplit=1,
        )[0]
        self.assertIn("INSERT INTO crm.module_versions", prefix)
        self.assertIn("INSERT INTO crm.capability_registry", prefix)
        self.assertNotIn("INSERT INTO crm.records", prefix)
        self.assertNotIn("INSERT INTO crm.customer_privacy_cases", prefix)
        self.assertIn("cargo test -p crm-api --test seed_e2e_fixture", runner)

    def test_positive_metric_sample_requires_non_comment_finite_positive_value(
        self,
    ) -> None:
        marker = "customer_privacy.case.list"
        self.assertFalse(operations.has_positive_metric_sample("", marker))
        self.assertFalse(
            operations.has_positive_metric_sample(f"# HELP x {marker}\n", marker)
        )
        self.assertFalse(
            operations.has_positive_metric_sample(
                f'x{{capability_id="{marker}"}} 0\n', marker
            )
        )
        self.assertFalse(
            operations.has_positive_metric_sample(
                f'x{{capability_id="{marker}"}} NaN\n', marker
            )
        )
        self.assertTrue(
            operations.has_positive_metric_sample(
                f'x{{capability_id="{marker}"}} 2\n', marker
            )
        )

    def test_report_accepts_exact_restore_slo_observability_and_supply_chain_evidence(
        self,
    ) -> None:
        policy = operations.load_policy()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            args = report_args(root, policy, positive_metrics(policy))
            report = operations.build_report(args, policy)
            self.assertEqual(
                report["schema_version"],
                "crm.customer-privacy-operations-report/v1",
            )
            self.assertTrue(report["restore_verified"])
            self.assertTrue(report["browser_verified"])
            self.assertTrue(report["active_query_metrics_verified"])
            self.assertEqual(report["readiness_probe_failures"], 0)
            self.assertEqual(report["backup_sha256"], args.backup_sha256)
            json.dumps(report)

    def test_report_rejects_zero_required_query_metric(self) -> None:
        policy = operations.load_policy()
        markers = policy["required_metric_markers"]
        assert isinstance(markers, list)
        metrics = positive_metrics(policy).replace(
            f'capability_id="{markers[0]}"',
            f'capability_id="{markers[0]}"',
            1,
        ).replace(
            f'capability_id="{markers[0]}",capability_version="1.0.0",owner_module_id="crm.customer-privacy",surface="query"}} 1',
            f'capability_id="{markers[0]}",capability_version="1.0.0",owner_module_id="crm.customer-privacy",surface="query"}} 0',
            1,
        )
        with tempfile.TemporaryDirectory() as directory:
            args = report_args(Path(directory), policy, metrics)
            with self.assertRaisesRegex(
                operations.OperationsError,
                f"missing positive sample: {markers[0]}",
            ):
                operations.build_report(args, policy)

    def test_report_rejects_observability_leak(self) -> None:
        policy = operations.load_policy()
        forbidden = policy["forbidden_observability_markers"]
        assert isinstance(forbidden, list)
        metrics = positive_metrics(policy) + f"# forbidden {forbidden[0]}\n"
        with tempfile.TemporaryDirectory() as directory:
            args = report_args(Path(directory), policy, metrics)
            with self.assertRaisesRegex(
                operations.OperationsError,
                "leaks forbidden fixture marker",
            ):
                operations.build_report(args, policy)


if __name__ == "__main__":
    unittest.main()
