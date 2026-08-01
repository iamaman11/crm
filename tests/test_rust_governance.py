"""Focused tests for pinned Rust toolchain and workspace lint governance."""

from datetime import date
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import tomllib
import unittest

from scripts.check_rust_governance import (
    active_rust_exceptions,
    compiler_messages,
    package_adoption,
    workspace_members_from_text,
)


ROOT = Path(__file__).resolve().parents[1]
RETIRED_DIRECT_LINT_MANIFESTS = (
    "crates/crm-application-runtime/Cargo.toml",
    "crates/crm-customer-data-operations-execution-composition/Cargo.toml",
    "services/crm-api/Cargo.toml",
)


class RustGovernanceTests(unittest.TestCase):
    def temporary_root(self) -> tuple[TemporaryDirectory, Path]:
        temporary = TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        return temporary, Path(temporary.name)

    def test_measures_workspace_direct_and_missing_adoption(self) -> None:
        _, root = self.temporary_root()
        packages = []
        manifests = {
            "inherited": """[package]
name = "inherited"
version = "0.1.0"
rust-version.workspace = true

[lints]
workspace = true
""",
            "direct": """[package]
name = "direct"
version = "0.1.0"
rust-version = "1.97.1"

[lints.rust]
warnings = "deny"
""",
            "missing": """[package]
name = "missing"
version = "0.1.0"
""",
        }
        for name, content in manifests.items():
            manifest = root / "crates" / name / "Cargo.toml"
            manifest.parent.mkdir(parents=True)
            manifest.write_text(content, encoding="utf-8")
            packages.append({"name": name, "manifest_path": str(manifest)})

        counts, rows = package_adoption(root, packages)

        self.assertEqual(counts["rust_version_inherited"], 1)
        self.assertEqual(counts["rust_version_direct"], 1)
        self.assertEqual(counts["rust_version_missing"], 1)
        self.assertEqual(counts["lints_inherited"], 1)
        self.assertEqual(counts["lints_direct"], 1)
        self.assertEqual(counts["lints_missing"], 1)
        self.assertEqual({row["package"] for row in rows}, set(manifests))

    def test_compiler_measurement_counts_only_workspace_packages(self) -> None:
        _, root = self.temporary_root()
        diagnostics = root / "diagnostics.jsonl"
        workspace_id = "workspace 0.1.0 (path+file:///workspace)"
        dependency_id = "dependency 1.0.0 (registry+https://example.invalid)"
        messages = [
            {
                "reason": "compiler-message",
                "package_id": workspace_id,
                "message": {"level": "warning"},
            },
            {
                "reason": "compiler-message",
                "package_id": workspace_id,
                "message": {"level": "error"},
            },
            {
                "reason": "compiler-message",
                "package_id": dependency_id,
                "message": {"level": "warning"},
            },
            {"reason": "build-finished", "success": False},
        ]
        diagnostics.write_text(
            "\n".join(json.dumps(message) for message in messages) + "\nnot-json\n",
            encoding="utf-8",
        )

        self.assertEqual(
            compiler_messages(diagnostics, {workspace_id}),
            {"warnings": 1, "errors": 1},
        )

    def test_rust_exceptions_are_exact_scoped_and_time_bounded(self) -> None:
        _, root = self.temporary_root()
        (root / "architecture-governance.json").write_text(
            json.dumps(
                {
                    "exceptions": [
                        {
                            "id": "RUST-VALID",
                            "rule": "rust-governance",
                            "scope": "crates/valid/Cargo.toml",
                            "expiry_date": "2026-08-31",
                        },
                        {
                            "id": "RUST-EXPIRED",
                            "rule": "rust-governance",
                            "scope": "crates/expired/Cargo.toml",
                            "expiry_date": "2026-07-01",
                        },
                        {
                            "id": "RUST-BROAD",
                            "rule": "rust-governance",
                            "scope": "crates/broad",
                            "expiry_date": "2026-08-31",
                        },
                    ]
                }
            ),
            encoding="utf-8",
        )

        active, errors = active_rust_exceptions(root, today=date(2026, 7, 28))

        self.assertEqual(set(active), {"crates/valid/Cargo.toml"})
        self.assertTrue(any("expired" in error for error in errors))
        self.assertTrue(any("exact Cargo.toml" in error for error in errors))

    def test_repository_has_zero_direct_lint_exceptions(self) -> None:
        registry = json.loads(
            (ROOT / "architecture-governance.json").read_text(encoding="utf-8")
        )
        rust_exceptions = [
            item
            for item in registry["exceptions"]
            if item.get("rule") == "rust-governance"
        ]
        self.assertEqual(rust_exceptions, [])

        for relative in RETIRED_DIRECT_LINT_MANIFESTS:
            with self.subTest(manifest=relative):
                manifest = tomllib.loads(
                    (ROOT / relative).read_text(encoding="utf-8")
                )
                self.assertEqual(manifest.get("lints"), {"workspace": True})

        policy = json.loads(
            (ROOT / "rust-governance-policy.json").read_text(encoding="utf-8")
        )
        baseline = policy["measured_baseline"]
        self.assertEqual(baseline["maximum_direct_lint_tables"], 0)
        self.assertEqual(baseline["maximum_missing_workspace_lints_packages"], 109)
        self.assertEqual(baseline["active_rust_governance_exceptions"], 0)

    def test_workspace_member_parser_uses_only_declared_members(self) -> None:
        text = """[workspace]
members = ["crates/one", "modules/two"]
exclude = ["crates/ignored"]
"""
        self.assertEqual(
            workspace_members_from_text(text),
            {"crates/one", "modules/two"},
        )


if __name__ == "__main__":
    unittest.main()
