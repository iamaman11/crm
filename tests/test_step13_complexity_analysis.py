"""Tests for ADR-031 current-main complexity measurement."""

import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from scripts.analyze_step13_complexity import dependency_depths, suppression_inventory


ROOT = Path(__file__).resolve().parents[1]


class Step13ComplexityAnalysisTests(unittest.TestCase):
    def test_dependency_depths_are_deterministic(self) -> None:
        depths = dependency_depths(
            {
                "contracts": set(),
                "application": {"contracts"},
                "runtime": {"application", "contracts"},
            }
        )
        self.assertEqual(depths["contracts"], 0)
        self.assertEqual(depths["application"], 1)
        self.assertEqual(depths["runtime"], 2)

    def test_inventory_covers_manifest_and_source_equivalents(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = root / "crates" / "example" / "Cargo.toml"
            source = root / "crates" / "example" / "src" / "lib.rs"
            manifest.parent.mkdir(parents=True)
            source.parent.mkdir(parents=True)
            manifest.write_text(
                "[package]\nname='example'\nversion='0.1.0'\n\n"
                "[lints.clippy]\ntoo_many_arguments='allow'\n",
                encoding="utf-8",
            )
            source.write_text(
                "#![allow(dead_code)]\n"
                "#[expect(clippy::too_many_arguments)]\n"
                "#[ignore = \"requires external service\"]\n"
                "fn fixture() {}\n",
                encoding="utf-8",
            )

            inventory = suppression_inventory(root)

        self.assertEqual(inventory["entry_count"], 4)
        self.assertEqual(
            inventory["counts_by_kind"],
            {
                "direct-lint-table": 1,
                "ignored-test": 1,
                "rust-allow": 1,
                "rust-expect": 1,
            },
        )

    def test_measurement_files_use_existing_fail_closed_operations_scope(self) -> None:
        policy = json.loads(
            (ROOT / "affected-scope-policy.json").read_text(encoding="utf-8")
        )
        scopes = {scope["id"]: scope for scope in policy["scopes"]}
        self.assertEqual(
            set(scopes),
            {
                "contracts",
                "protobuf_api_compatibility",
                "database_migrations",
                "postgresql_acceptance",
                "process_runtime_acceptance",
                "product_plane",
                "frontend",
                "operations",
            },
        )
        operations = scopes["operations"]
        self.assertIn("scripts/analyze_step13_complexity.py", operations["path_patterns"])
        self.assertIn("step13-complexity-policy.json", operations["path_patterns"])
        self.assertEqual(operations["required_workflows"], ["Governance CI"])

        workflow = (ROOT / ".github/workflows/complexity-baseline.yml").read_text(
            encoding="utf-8"
        )
        for path in (
            '      - "scripts/analyze_step13_complexity.py"',
            '      - "step13-complexity-policy.json"',
        ):
            self.assertGreaterEqual(workflow.count(path), 2)


if __name__ == "__main__":
    unittest.main()
