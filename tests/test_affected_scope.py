from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

from scripts.affected_scope import (
    POLICY_SCHEMA_VERSION,
    REQUIRED_SCOPE_IDS,
    build_report,
    path_matches,
)


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


def workflow(root: Path, filename: str, name: str, paths: list[str] | None) -> None:
    directory = root / ".github/workflows"
    directory.mkdir(parents=True, exist_ok=True)
    lines = [f"name: {name}", "on:", "  pull_request:"]
    if paths is not None:
        lines.append("    paths:")
        lines.extend(f'      - "{pattern}"' for pattern in paths)
    (directory / filename).write_text("\n".join(lines) + "\n", encoding="utf-8")


def policy(
    root: Path,
    *,
    overrides: dict[str, dict] | None = None,
    neutral: list[str] | None = None,
) -> None:
    overrides = overrides or {}
    scopes = []
    for scope_id in sorted(REQUIRED_SCOPE_IDS):
        raw = {
            "id": scope_id,
            "owner": f"{scope_id}-owner",
            "path_patterns": [f"__never__/{scope_id}/**"],
            "required_workflows": ["Always CI"],
        }
        raw.update(overrides.get(scope_id, {}))
        scopes.append(raw)
    document = {
        "schema_version": POLICY_SCHEMA_VERSION,
        "scopes": scopes,
        "neutral_path_patterns": neutral or ["docs/**", "tests/**"],
    }
    (root / "affected-scope-policy.json").write_text(
        json.dumps(document, indent=2) + "\n",
        encoding="utf-8",
    )


class AffectedScopeTests(unittest.TestCase):
    def test_reverse_dependency_closure_is_explainable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow(root, "always.yml", "Always CI", None)
            policy(root)
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
            self.assertFalse(report["selected_scopes"])

    def test_workflow_filters_explain_selection_and_skip(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow(root, "owner.yml", "Owner CI", ["crates/owner/**"])
            workflow(root, "docs.yml", "Docs CI", ["docs/**"])
            workflow(root, "always.yml", "Always CI", None)
            policy(root)
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

    def test_unknown_path_fails_closed_until_policy_classifies_it(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow(root, "owner.yml", "Owner CI", ["crates/owner/**"])
            workflow(root, "docs.yml", "Docs CI", ["docs/**"])
            workflow(root, "always.yml", "Always CI", None)
            policy(root)
            with self.assertRaisesRegex(
                RuntimeError,
                "unknown affected scope cannot prove a safe non-Rust workflow closure",
            ):
                build_report(
                    root,
                    "origin/main",
                    paths=["mystery/input.bin"],
                    metadata=metadata(root),
                    head_sha="abc",
                )

    def test_docs_only_change_selects_no_rust_or_non_rust_scope(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow(root, "governance.yml", "Governance CI", ["docs/**"])
            workflow(root, "rust.yml", "Rust CI", ["crates/**"])
            workflow(root, "always.yml", "Always CI", None)
            policy(root)
            report = build_report(
                root,
                "origin/main",
                paths=["docs/README.md"],
                metadata=metadata(root),
                head_sha="abc",
            )
            self.assertFalse(report["broadened"])
            self.assertFalse(report["affected_packages"])
            self.assertFalse(report["selected_scopes"])
            self.assertEqual(
                [entry["name"] for entry in report["selected_workflows"]],
                ["Always CI", "Governance CI"],
            )
            self.assertEqual(
                [entry["name"] for entry in report["skipped_workflows"]],
                ["Rust CI"],
            )

    def test_root_workspace_change_broadens(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow(root, "rust.yml", "Rust CI", None)
            workflow(root, "always.yml", "Always CI", None)
            policy(root)
            report = build_report(
                root,
                "origin/main",
                paths=["Cargo.toml"],
                metadata=metadata(root),
                head_sha="abc",
            )
            self.assertTrue(report["broadened"])
            self.assertEqual(report["affected_packages"], ["app", "core", "owner"])

    def test_contract_scope_requires_governance(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow(root, "governance.yml", "Governance CI", ["contracts/**"])
            workflow(root, "always.yml", "Always CI", None)
            policy(
                root,
                overrides={
                    "contracts": {
                        "path_patterns": ["contracts/**"],
                        "required_workflows": ["Governance CI"],
                    }
                },
            )
            report = build_report(
                root,
                "origin/main",
                paths=["contracts/example.json"],
                metadata=metadata(root),
                head_sha="abc",
            )
            self.assertFalse(report["broadened"])
            self.assertEqual(
                [scope["id"] for scope in report["selected_scopes"]],
                ["contracts"],
            )
            self.assertIn(
                "Governance CI",
                [workflow["name"] for workflow in report["selected_workflows"]],
            )

    def test_migration_scope_selects_database_process_and_product(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for filename, name in (
                ("application.yml", "Application Runtime CI"),
                ("database.yml", "Database CI"),
                ("product.yml", "Product Plane CI"),
            ):
                workflow(root, filename, name, ["database/**"])
            workflow(root, "always.yml", "Always CI", None)
            policy(
                root,
                overrides={
                    "database_migrations": {
                        "path_patterns": ["database/migrations/**"],
                        "required_workflows": [
                            "Application Runtime CI",
                            "Database CI",
                            "Product Plane CI",
                        ],
                    },
                    "postgresql_acceptance": {
                        "path_patterns": ["database/**"],
                        "required_workflows": ["Database CI"],
                    },
                },
            )
            report = build_report(
                root,
                "origin/main",
                paths=["database/migrations/9999_example.up.sql"],
                metadata=metadata(root),
                head_sha="abc",
            )
            self.assertFalse(report["broadened"])
            self.assertEqual(
                [scope["id"] for scope in report["selected_scopes"]],
                ["database_migrations", "postgresql_acceptance"],
            )
            selected = {
                workflow["name"] for workflow in report["selected_workflows"]
            }
            self.assertTrue(
                {
                    "Application Runtime CI",
                    "Database CI",
                    "Product Plane CI",
                }.issubset(selected)
            )

    def test_undercovered_required_workflow_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow(root, "governance.yml", "Governance CI", ["docs/**"])
            workflow(root, "owner.yml", "Owner CI", ["contracts/**"])
            workflow(root, "always.yml", "Always CI", None)
            policy(
                root,
                overrides={
                    "contracts": {
                        "path_patterns": ["contracts/**"],
                        "required_workflows": ["Governance CI"],
                    }
                },
            )
            with self.assertRaisesRegex(
                RuntimeError,
                "scope contracts requires Governance CI.*path filters did not select it",
            ):
                build_report(
                    root,
                    "origin/main",
                    paths=["contracts/example.json"],
                    metadata=metadata(root),
                    head_sha="abc",
                )

    def test_missing_required_permanent_workflow_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow(root, "always.yml", "Always CI", None)
            policy(
                root,
                overrides={
                    "contracts": {
                        "path_patterns": ["contracts/**"],
                        "required_workflows": ["Missing CI"],
                    }
                },
            )
            with self.assertRaisesRegex(
                RuntimeError,
                "requires missing permanent PR workflows",
            ):
                build_report(
                    root,
                    "origin/main",
                    paths=["contracts/example.json"],
                    metadata=metadata(root),
                    head_sha="abc",
                )

    def test_frontend_and_product_scopes_are_cumulative(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow(root, "product.yml", "Product Plane CI", ["apps/web/**"])
            workflow(root, "always.yml", "Always CI", None)
            policy(
                root,
                overrides={
                    "frontend": {
                        "path_patterns": ["apps/web/**"],
                        "required_workflows": ["Product Plane CI"],
                    },
                    "product_plane": {
                        "path_patterns": ["apps/web/**"],
                        "required_workflows": ["Product Plane CI"],
                    },
                },
            )
            report = build_report(
                root,
                "origin/main",
                paths=["apps/web/src/app.tsx"],
                metadata=metadata(root),
                head_sha="abc",
            )
            self.assertFalse(report["broadened"])
            self.assertEqual(
                [scope["id"] for scope in report["selected_scopes"]],
                ["frontend", "product_plane"],
            )

    def test_operations_scope_is_owned_and_enforced(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            workflow(root, "governance.yml", "Governance CI", ["ops/**"])
            workflow(root, "always.yml", "Always CI", None)
            policy(
                root,
                overrides={
                    "operations": {
                        "path_patterns": ["ops/**"],
                        "required_workflows": ["Governance CI"],
                    }
                },
            )
            report = build_report(
                root,
                "origin/main",
                paths=["ops/restore/runbook.yaml"],
                metadata=metadata(root),
                head_sha="abc",
            )
            self.assertFalse(report["broadened"])
            self.assertEqual(
                report["selected_scopes"][0]["owner"],
                "operations-owner",
            )

    def test_live_policy_representatives_are_covered_by_real_workflows(self) -> None:
        representatives = {
            "contracts": (
                "contracts/example.json",
                "schemas/module.schema.json",
                "modules/crm-sales/module.yaml",
            ),
            "protobuf_api_compatibility": (
                "proto/crm/example/v1/example.proto",
                "buf.yaml",
                "buf.gen.web.yaml",
                "crates/crm-proto-contracts/src/lib.rs",
                "packages/client/src/contract_hashes.ts",
                "scripts/contract_bindings.py",
            ),
            "database_migrations": ("database/migrations/9999_example.up.sql",),
            "postgresql_acceptance": (
                "database/tests/0001_platform_foundation.sql",
                "crates/crm-core-data/src/lib.rs",
            ),
            "process_runtime_acceptance": (
                "services/crm-api/src/main.rs",
                "crates/crm-application-runtime/src/lib.rs",
                "scripts/prepare_customer_enrichment_worker_process_database.sh",
            ),
            "product_plane": (
                "package.json",
                "apps/web/src/app.tsx",
                "scripts/run_e2e.sh",
            ),
            "frontend": ("apps/web/src/app.tsx", "buf.gen.web.yaml"),
            "operations": (
                ".github/workflows/governance.yml",
                "docs/CI_TELEMETRY_BASELINE.md",
                "scripts/check_native_module_composition.py",
                "scripts/prepare_isolated_process_database.sh",
            ),
        }
        empty_metadata = {"packages": [], "workspace_members": []}
        for scope_id, paths in representatives.items():
            for path in paths:
                with self.subTest(scope=scope_id, path=path):
                    report = build_report(
                        Path(__file__).resolve().parents[1],
                        "origin/main",
                        paths=[path],
                        metadata=empty_metadata,
                        head_sha="representative",
                    )
                    selected = {
                        scope["id"]: scope for scope in report["selected_scopes"]
                    }
                    self.assertIn(scope_id, selected)
                    selected_workflows = {
                        workflow["name"] for workflow in report["selected_workflows"]
                    }
                    self.assertTrue(
                        set(selected[scope_id]["required_workflows"]).issubset(
                            selected_workflows
                        )
                    )

    def test_native_composition_guard_has_exact_operations_scope(self) -> None:
        root = Path(__file__).resolve().parents[1]
        empty_metadata = {"packages": [], "workspace_members": []}
        report = build_report(
            root,
            "origin/main",
            paths=["scripts/check_native_module_composition.py"],
            metadata=empty_metadata,
            head_sha="native-composition-guard",
        )
        self.assertEqual(
            [scope["id"] for scope in report["selected_scopes"]],
            ["operations"],
        )
        self.assertIn(
            "Governance CI",
            [workflow["name"] for workflow in report["selected_workflows"]],
        )
        with self.assertRaisesRegex(
            RuntimeError,
            "unknown affected scope cannot prove a safe non-Rust workflow closure",
        ):
            build_report(
                root,
                "origin/main",
                paths=["scripts/check_unclassified_native_guard.py"],
                metadata=empty_metadata,
                head_sha="unclassified-native-guard",
            )

    def test_glob_matching_handles_nested_paths(self) -> None:
        self.assertTrue(
            path_matches(
                "crates/crm-customer-enrichment-query-adapter/src/lib.rs",
                "crates/crm-customer-enrichment-*/**",
            )
        )


if __name__ == "__main__":
    unittest.main()
