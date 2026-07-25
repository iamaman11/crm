from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from scripts.affected_scope import build_report, path_matches


def package(
    root: Path,
    name: str,
    relative: str,
    dependencies: list[str] | None = None,
) -> dict:
    return {
        "id": f"{name} 0.1.0 (path+file://{root / relative})",
        "name": name,
        "manifest_path": str(root / relative / "Cargo.toml"),
        "dependencies": [
            {"name": dependency} for dependency in dependencies or []
        ],
    }


def metadata(root: Path) -> dict:
    packages = [
        package(root, "core", "crates/core"),
        package(root, "owner", "crates/owner", ["core"]),
        package(root, "app", "services/app", ["owner"]),
    ]
    return {
        "packages": packages,
        "workspace_members": [entry["id"] for entry in packages],
    }


def workflow(root: Path, name: str, body: str) -> None:
    directory = root / ".github/workflows"
    directory.mkdir(parents=True, exist_ok=True)
    (directory / f"{name}.yml").write_text(body, encoding="utf-8")


class AffectedScopeTests(unittest.TestCase):
    def test_reverse_dependency_closure_is_explainable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow(
                root,
                "rust",
                """
name: Rust CI
on:
  pull_request:
""",
            )
            report = build_report(
                root,
                "origin/main",
                paths=["crates/core/src/lib.rs"],
                metadata=metadata(root),
                head_sha="abc",
            )
            self.assertEqual(report["direct_packages"], ["core"])
            self.assertEqual(report["affected_packages"], ["app", "core", "owner"])
            self.assertIn(
                "reverse-depends on affected package core",
                report["package_reasons"]["owner"],
            )
            self.assertIn(
                "reverse-depends on affected package owner",
                report["package_reasons"]["app"],
            )
            self.assertFalse(report["broadened"])

    def test_workflow_filters_explain_selection_and_skip(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow(
                root,
                "owner",
                """
name: Owner CI
on:
  pull_request:
    paths:
      - "crates/owner/**"
""",
            )
            workflow(
                root,
                "docs",
                """
name: Docs CI
on:
  pull_request:
    paths:
      - "docs/**"
""",
            )
            workflow(
                root,
                "always",
                """
name: Always CI
on:
  pull_request:
""",
            )
            report = build_report(
                root,
                "origin/main",
                paths=["crates/owner/src/lib.rs"],
                metadata=metadata(root),
                head_sha="abc",
            )
            self.assertEqual(
                [entry["name"] for entry in report["selected_workflows"]],
                ["Always CI", "Owner CI"],
            )
            self.assertEqual(
                [entry["name"] for entry in report["skipped_workflows"]],
                ["Docs CI"],
            )

    def test_unknown_path_broadens_packages_and_workflows(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow(
                root,
                "owner",
                """
name: Owner CI
on:
  pull_request:
    paths:
      - "crates/owner/**"
""",
            )
            workflow(
                root,
                "docs",
                """
name: Docs CI
on:
  pull_request:
    paths:
      - "docs/**"
""",
            )
            report = build_report(
                root,
                "origin/main",
                paths=["mystery/input.bin"],
                metadata=metadata(root),
                head_sha="abc",
            )
            self.assertTrue(report["broadened"])
            self.assertEqual(report["affected_packages"], ["app", "core", "owner"])
            self.assertEqual(
                [entry["name"] for entry in report["selected_workflows"]],
                ["Docs CI", "Owner CI"],
            )
            self.assertFalse(report["skipped_workflows"])

    def test_docs_only_change_selects_no_rust_packages(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow(
                root,
                "governance",
                """
name: Governance CI
on:
  pull_request:
    paths:
      - "docs/**"
""",
            )
            workflow(
                root,
                "rust",
                """
name: Rust CI
on:
  pull_request:
    paths:
      - "crates/**"
""",
            )
            report = build_report(
                root,
                "origin/main",
                paths=["docs/README.md"],
                metadata=metadata(root),
                head_sha="abc",
            )
            self.assertFalse(report["broadened"])
            self.assertFalse(report["affected_packages"])
            self.assertEqual(
                [entry["name"] for entry in report["selected_workflows"]],
                ["Governance CI"],
            )
            self.assertEqual(
                [entry["name"] for entry in report["skipped_workflows"]],
                ["Rust CI"],
            )

    def test_root_workspace_change_broadens(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow(
                root,
                "rust",
                """
name: Rust CI
on:
  pull_request:
""",
            )
            report = build_report(
                root,
                "origin/main",
                paths=["Cargo.toml"],
                metadata=metadata(root),
                head_sha="abc",
            )
            self.assertTrue(report["broadened"])
            self.assertEqual(report["affected_packages"], ["app", "core", "owner"])

    def test_glob_matching_handles_nested_paths(self) -> None:
        self.assertTrue(
            path_matches(
                "crates/crm-customer-enrichment-query-adapter/src/lib.rs",
                "crates/crm-customer-enrichment-*/**",
            )
        )


if __name__ == "__main__":
    unittest.main()
