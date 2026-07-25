from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from scripts.analyze_workspace import (
    categorize_manifest,
    duplicate_dependency_families,
    package_metrics,
    workflow_metric,
)


class WorkspaceAnalysisTests(unittest.TestCase):
    def temporary_root(self) -> tuple[TemporaryDirectory, Path]:
        temporary = TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        return temporary, Path(temporary.name)

    def test_categorizes_workspace_packages_and_reverse_impact(self) -> None:
        _, root = self.temporary_root()
        module_manifest = root / "modules" / "crm-parties" / "Cargo.toml"
        adapter_manifest = root / "crates" / "crm-parties-adapter" / "Cargo.toml"
        runtime_manifest = root / "crates" / "crm-runtime" / "Cargo.toml"
        for manifest in (module_manifest, adapter_manifest, runtime_manifest):
            manifest.parent.mkdir(parents=True, exist_ok=True)
            manifest.write_text("[package]\nname='fixture'\nversion='0.1.0'\n", encoding="utf-8")

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
        self.assertEqual(categorize_manifest(root, str(module_manifest)), "business-module")

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

    def test_scans_workflow_structure_without_parsing_execution_semantics(self) -> None:
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
      - run: cargo test
""",
            encoding="utf-8",
        )

        metric = workflow_metric(path, root)
        self.assertEqual(metric.name, "Example CI")
        self.assertEqual(metric.job_count, 1)
        self.assertEqual(metric.action_reference_count, 1)
        self.assertEqual(metric.run_step_count, 1)
        self.assertEqual(metric.path_filter_count, 2)
        self.assertEqual(metric.maximum_timeout_minutes, 12)
        self.assertTrue(metric.has_postgres_service)
        self.assertTrue(metric.has_concurrency)
        self.assertTrue(metric.pushes_main_only)
        self.assertTrue(metric.handles_pull_requests)


if __name__ == "__main__":
    unittest.main()
