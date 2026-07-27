"""Focused tests for workspace dependency, crate and exception governance."""

from datetime import date
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

from scripts.analyze_workspace_governance import (
    dependency_depths,
    dependency_metrics,
    package_graph,
    public_items,
    validate_governance,
)


class WorkspaceGovernanceTests(unittest.TestCase):
    def temporary_root(self) -> tuple[TemporaryDirectory, Path]:
        temporary = TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        return temporary, Path(temporary.name)

    def test_dependency_graph_depth_and_conservative_public_surface(self) -> None:
        _, root = self.temporary_root()
        manifests = {}
        packages = []
        for name in ("domain", "adapter", "runtime"):
            manifest = root / "crates" / name / "Cargo.toml"
            manifest.parent.mkdir(parents=True)
            manifest.write_text(
                f'[package]\nname = "{name}"\nversion = "0.1.0"\n',
                encoding="utf-8",
            )
            source = manifest.parent / "src" / "lib.rs"
            source.parent.mkdir()
            source.write_text(
                "pub struct Public;\npub(crate) struct Internal;\n",
                encoding="utf-8",
            )
            manifests[name] = manifest
            packages.append(
                {
                    "id": f"{name}-id",
                    "name": name,
                    "manifest_path": str(manifest),
                    "dependencies": [],
                }
            )
        packages[1]["dependencies"] = [{"name": "domain"}]
        packages[2]["dependencies"] = [{"name": "adapter"}]
        dependencies, dependents = package_graph(packages)
        self.assertEqual(dependency_depths(dependencies)["runtime"], 2)
        self.assertEqual(dependents["domain"], {"adapter"})
        self.assertEqual(public_items(manifests["domain"]), 1)

    def test_reports_version_feature_and_inheritance_divergence(self) -> None:
        _, root = self.temporary_root()
        first = root / "crates" / "first" / "Cargo.toml"
        second = root / "crates" / "second" / "Cargo.toml"
        first.parent.mkdir(parents=True)
        second.parent.mkdir(parents=True)
        first.write_text(
            """[package]
name = "first"
version = "0.1.0"

[dependencies]
serde = { workspace = true }
tracing = { version = "0.1", features = ["log"] }
""",
            encoding="utf-8",
        )
        second.write_text(
            """[package]
name = "second"
version = "0.1.0"

[dependencies]
serde = { version = "1", features = ["derive"] }
tracing = { version = "0.2", features = ["attributes", "log"] }
""",
            encoding="utf-8",
        )
        packages = [
            {"name": "first", "manifest_path": str(first)},
            {"name": "second", "manifest_path": str(second)},
        ]
        report = dependency_metrics(
            root,
            packages,
            {"serde": {"version": "1", "features": ["derive"]}},
        )
        self.assertEqual(report["version_divergence"][0]["name"], "tracing")
        self.assertEqual(report["feature_divergence"][0]["name"], "tracing")
        self.assertEqual(
            report["non_inheriting_workspace_dependencies"][0]["name"],
            "serde",
        )

    def test_rejects_expired_exception(self) -> None:
        _, root = self.temporary_root()
        (root / "architecture-governance.json").write_text(
            json.dumps(
                {
                    "schema_version": "crm.architecture-governance/v1",
                    "exceptions": [
                        {
                            "id": "ARCH-001",
                            "owner": "platform",
                            "rule": "fixture",
                            "reason_and_risk": "fixture risk",
                            "scope": "crates/fixture",
                            "created_date": "2026-01-01",
                            "expiry_date": "2026-02-01",
                            "removal_condition": "remove fixture",
                            "compensating_checks": ["fixture test"],
                            "tracking_issue": "#194",
                        }
                    ],
                    "new_crate_justifications": [],
                }
            ),
            encoding="utf-8",
        )
        errors, warnings, new_members = validate_governance(
            root,
            {"crates/existing"},
            base_ref=None,
            today=date(2026, 7, 27),
        )
        self.assertTrue(any("expired" in error for error in errors))
        self.assertEqual(warnings, [])
        self.assertEqual(new_members, [])

    def test_requires_complete_justification_for_new_member(self) -> None:
        _, root = self.temporary_root()
        (root / "architecture-governance.json").write_text(
            json.dumps(
                {
                    "schema_version": "crm.architecture-governance/v1",
                    "exceptions": [],
                    "new_crate_justifications": [],
                }
            ),
            encoding="utf-8",
        )
        with patch(
            "scripts.analyze_workspace_governance.run",
            return_value='[workspace]\nmembers = ["crates/existing"]\n',
        ):
            errors, _, new_members = validate_governance(
                root,
                {"crates/existing", "crates/new"},
                base_ref="origin/main",
                today=date(2026, 7, 27),
            )
        self.assertEqual(new_members, ["crates/new"])
        self.assertTrue(any("no complete" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
