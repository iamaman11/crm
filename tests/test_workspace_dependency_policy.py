"""Focused tests for calibrated workspace dependency inheritance policies."""

import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from scripts.check_workspace_dependency_policy import validate_policy_document


class WorkspaceDependencyPolicyTests(unittest.TestCase):
    def temporary_root(self) -> tuple[TemporaryDirectory, Path]:
        temporary = TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        return temporary, Path(temporary.name)

    def write_policy(self, root: Path) -> None:
        (root / "workspace-dependency-policy.json").write_text(
            json.dumps(
                {
                    "schema_version": "crm.workspace-dependency-policy/v1",
                    "policies": [
                        {
                            "id": "owner-module-common-dependencies",
                            "owner": "architecture-governance",
                            "scope_glob": "modules/*/Cargo.toml",
                            "dependencies": ["serde", "serde_json", "sha2"],
                            "enforcement": "blocking",
                            "allow_local_features": False,
                            "reason": "fixture",
                            "tracking_issue": "#194",
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )

    def write_root_manifest(self, root: Path) -> None:
        (root / "Cargo.toml").write_text(
            """[workspace]
members = []

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
""",
            encoding="utf-8",
        )

    def test_accepts_exact_inheritance_and_does_not_require_unused_dependencies(self) -> None:
        _, root = self.temporary_root()
        self.write_root_manifest(root)
        self.write_policy(root)
        first = root / "modules" / "first" / "Cargo.toml"
        first.parent.mkdir(parents=True)
        first.write_text(
            """[package]
name = "first"
version = "0.1.0"

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
sha2 = { workspace = true }
""",
            encoding="utf-8",
        )
        second = root / "modules" / "second" / "Cargo.toml"
        second.parent.mkdir(parents=True)
        second.write_text(
            """[package]
name = "second"
version = "0.1.0"

[dependencies]
other = "1"
""",
            encoding="utf-8",
        )
        report = validate_policy_document(root)
        self.assertEqual(report["blocking_errors"], [])
        self.assertEqual(report["policies"][0]["matched_manifest_count"], 2)
        self.assertEqual(report["policies"][0]["governed_declaration_count"], 3)

    def test_rejects_direct_version_and_local_feature_override(self) -> None:
        _, root = self.temporary_root()
        self.write_root_manifest(root)
        self.write_policy(root)
        manifest = root / "modules" / "bad" / "Cargo.toml"
        manifest.parent.mkdir(parents=True)
        manifest.write_text(
            """[package]
name = "bad"
version = "0.1.0"

[dependencies]
serde = { workspace = true, features = ["rc"] }
serde_json = "1"
""",
            encoding="utf-8",
        )
        report = validate_policy_document(root)
        self.assertTrue(
            any(
                "must not add local features" in error
                for error in report["blocking_errors"]
            )
        )
        self.assertTrue(
            any(
                "must use workspace = true" in error
                for error in report["blocking_errors"]
            )
        )


if __name__ == "__main__":
    unittest.main()
