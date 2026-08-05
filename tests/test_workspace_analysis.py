"""Focused tests for deterministic measurement-only workspace analysis."""

from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from scripts.analyze_workspace import (
    build_step22_inventory,
    categorize_manifest,
    duplicate_dependency_families,
    package_metrics,
    runtime_dependency_metrics,
    workflow_metric,
)


class WorkspaceAnalysisTests(unittest.TestCase):
    def temporary_root(self) -> tuple[TemporaryDirectory, Path]:
        temporary = TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        return temporary, Path(temporary.name)

    def write_manifest(self, root: Path, relative: str, name: str) -> Path:
        manifest = root / relative / "Cargo.toml"
        manifest.parent.mkdir(parents=True, exist_ok=True)
        manifest.write_text(
            f"[package]\nname='{name}'\nversion='0.1.0'\n", encoding="utf-8"
        )
        return manifest

    def test_categorizes_workspace_packages_and_reverse_impact(self) -> None:
        _, root = self.temporary_root()
        module_manifest = self.write_manifest(
            root, "modules/crm-parties", "crm-parties"
        )
        adapter_manifest = self.write_manifest(
            root, "crates/crm-parties-adapter", "crm-parties-adapter"
        )
        runtime_manifest = self.write_manifest(
            root, "crates/crm-runtime", "crm-runtime"
        )

        metadata = {
            "workspace_members": ["parties-id", "adapter-id", "runtime-id"],
            "packages": [
                {
                    "id": "parties-id",
                    "name": "crm-parties",
                    "manifest_path": str(module_manifest),
                    "dependencies": [],
                },
                {
                    "id": "adapter-id",
                    "name": "crm-parties-adapter",
                    "manifest_path": str(adapter_manifest),
                    "dependencies": [{"name": "crm-parties"}],
                },
                {
                    "id": "runtime-id",
                    "name": "crm-runtime",
                    "manifest_path": str(runtime_manifest),
                    "dependencies": [{"name": "crm-parties-adapter"}],
                },
            ],
        }

        metrics, categories = package_metrics(root, metadata)
        by_name = {metric.name: metric for metric in metrics}
        self.assertEqual(categories, {"business-module": 1, "technical-crate": 2})
        self.assertEqual(by_name["crm-parties"].direct_internal_dependents, 1)
        self.assertEqual(by_name["crm-parties"].transitive_reverse_impact, 2)
        self.assertEqual(by_name["crm-runtime"].transitive_reverse_impact, 0)
        self.assertEqual(
            categorize_manifest(root, str(module_manifest)), "business-module"
        )

    def test_reports_only_external_duplicate_dependency_families(self) -> None:
        lockfile = {
            "package": [
                {"name": "crm-runtime", "version": "0.1.0"},
                {"name": "serde", "version": "1.0.200"},
                {"name": "serde", "version": "1.0.210"},
                {"name": "tokio", "version": "1.45.0"},
            ]
        }

        self.assertEqual(
            duplicate_dependency_families(lockfile, {"crm-runtime"}),
            [{"name": "serde", "versions": ["1.0.200", "1.0.210"]}],
        )

    def test_inventories_runtime_dependencies_by_manifest_scope(self) -> None:
        _, root = self.temporary_root()
        manifests = {
            "runtime": self.write_manifest(
                root, "crates/crm-application-runtime", "crm-application-runtime"
            ),
            "platform": self.write_manifest(
                root, "crates/crm-platform-runtime", "crm-platform-runtime"
            ),
            "owner": self.write_manifest(
                root, "modules/crm-owner", "crm-owner"
            ),
            "test": self.write_manifest(
                root, "crates/crm-test-support", "crm-test-support"
            ),
            "build": self.write_manifest(
                root, "crates/crm-build-support", "crm-build-support"
            ),
        }
        metadata = {
            "workspace_members": [
                "runtime-id",
                "platform-id",
                "owner-id",
                "test-id",
                "build-id",
            ],
            "packages": [
                {
                    "id": "runtime-id",
                    "name": "crm-application-runtime",
                    "manifest_path": str(manifests["runtime"]),
                    "dependencies": [
                        {"name": "crm-platform-runtime", "kind": None},
                        {"name": "crm-owner", "kind": None},
                        {"name": "crm-test-support", "kind": "dev"},
                        {"name": "crm-build-support", "kind": "build"},
                        {"name": "serde", "kind": None},
                    ],
                },
                {
                    "id": "platform-id",
                    "name": "crm-platform-runtime",
                    "manifest_path": str(manifests["platform"]),
                    "dependencies": [],
                },
                {
                    "id": "owner-id",
                    "name": "crm-owner",
                    "manifest_path": str(manifests["owner"]),
                    "dependencies": [],
                },
                {
                    "id": "test-id",
                    "name": "crm-test-support",
                    "manifest_path": str(manifests["test"]),
                    "dependencies": [],
                },
                {
                    "id": "build-id",
                    "name": "crm-build-support",
                    "manifest_path": str(manifests["build"]),
                    "dependencies": [],
                },
            ],
        }

        metrics = runtime_dependency_metrics(root, metadata)
        self.assertEqual(len(metrics), 4)
        by_name = {metric.dependency_name: metric for metric in metrics}
        self.assertEqual(by_name["crm-platform-runtime"].dependency_kind, "production")
        self.assertEqual(by_name["crm-owner"].target_category, "business-module")
        self.assertEqual(by_name["crm-test-support"].manifest_section, "dev-dependencies")
        self.assertEqual(by_name["crm-build-support"].dependency_kind, "build")
        self.assertEqual(
            by_name["crm-test-support"].stable_id,
            "crm-application-runtime::dev-dependencies::crm-test-support",
        )
        self.assertTrue(
            all(metric.decision_state == "unresolved" for metric in metrics)
        )

    def test_scans_workflow_and_job_structure_without_execution_inference(self) -> None:
        _, root = self.temporary_root()
        path = root / ".github" / "workflows" / "example.yml"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            """name: Example CI

on:
  push:
    branches:
      - main
    paths:
      - "Cargo.toml"
  pull_request:
    paths:
      - "Cargo.toml"

concurrency:
  group: example
  cancel-in-progress: true

jobs:
  check:
    runs-on: ubuntu-latest
    timeout-minutes: 12
    services:
      postgres:
        image: postgres:17
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7
      - run: docker version
      - run: cargo test
  browser:
    runs-on: ubuntu-latest
    timeout-minutes: 20
    steps:
      - run: npx playwright install chromium
      - run: cargo run -p crm-api
""",
            encoding="utf-8",
        )

        metric = workflow_metric(path, root)
        self.assertEqual(metric.stable_id, ".github/workflows/example.yml")
        self.assertEqual(metric.name, "Example CI")
        self.assertEqual(metric.job_count, 2)
        self.assertEqual(metric.action_reference_count, 1)
        self.assertEqual(metric.run_step_count, 4)
        self.assertEqual(metric.path_filter_count, 2)
        self.assertEqual(metric.maximum_timeout_minutes, 20)
        self.assertTrue(metric.has_postgres_service)
        self.assertTrue(metric.has_concurrency)
        self.assertTrue(metric.pushes_main_only)
        self.assertTrue(metric.handles_pull_requests)
        jobs = {job.job_id: job for job in metric.jobs}
        self.assertEqual(
            jobs["check"].stable_id, ".github/workflows/example.yml#check"
        )
        self.assertEqual(jobs["check"].run_step_count, 2)
        self.assertEqual(
            jobs["check"].environment_signals,
            ("postgres-service", "docker"),
        )
        self.assertEqual(
            jobs["browser"].environment_signals,
            ("browser", "process-runtime"),
        )
        self.assertTrue(
            all(job.decision_state == "unresolved" for job in metric.jobs)
        )

    def test_step22_inventory_is_unique_and_explicitly_inventory_only(self) -> None:
        _, root = self.temporary_root()
        runtime_manifest = self.write_manifest(
            root, "crates/crm-application-runtime", "crm-application-runtime"
        )
        dependency_manifest = self.write_manifest(
            root, "crates/crm-platform-runtime", "crm-platform-runtime"
        )
        metadata = {
            "workspace_members": ["runtime-id", "dependency-id"],
            "packages": [
                {
                    "id": "runtime-id",
                    "name": "crm-application-runtime",
                    "manifest_path": str(runtime_manifest),
                    "dependencies": [
                        {"name": "crm-platform-runtime", "kind": None}
                    ],
                },
                {
                    "id": "dependency-id",
                    "name": "crm-platform-runtime",
                    "manifest_path": str(dependency_manifest),
                    "dependencies": [],
                },
            ],
        }
        workflow_path = root / ".github" / "workflows" / "check.yml"
        workflow_path.parent.mkdir(parents=True, exist_ok=True)
        workflow_path.write_text(
            """name: Check CI
on:
  pull_request:
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - run: cargo test
""",
            encoding="utf-8",
        )
        dependencies = runtime_dependency_metrics(root, metadata)
        workflow = workflow_metric(workflow_path, root)
        inventory = build_step22_inventory("a" * 40, dependencies, [workflow])

        self.assertEqual(inventory["phase"], "inventory-only")
        self.assertEqual(
            inventory["runtime_fanin"]["unresolved_decision_count"], 1
        )
        self.assertEqual(inventory["permanent_gates"]["workflow_count"], 1)
        self.assertEqual(inventory["permanent_gates"]["job_count"], 1)
        self.assertFalse(
            inventory["decision_boundary"]["final_classifications_recorded"]
        )
        self.assertFalse(inventory["decision_boundary"]["step22_complete"])

        with self.assertRaisesRegex(ValueError, "duplicate permanent workflow"):
            build_step22_inventory("a" * 40, dependencies, [workflow, workflow])


if __name__ == "__main__":
    unittest.main()
