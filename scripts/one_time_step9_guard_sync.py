from __future__ import annotations

from pathlib import Path


def replace_exact(
    text: str,
    old: str,
    new: str,
    label: str,
    *,
    expected: int = 1,
) -> str:
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{label} replacement count: {count}, expected: {expected}")
    return text.replace(old, new)


architecture = Path("tests/test_architecture_documentation_consistency.py")
text = architecture.read_text(encoding="utf-8")
text = replace_exact(
    text,
    '        self.assertEqual(self.packet["packet_id"], "repository-step-8-evidence-sync")',
    '        self.assertEqual(self.packet["packet_id"], "repository-step-9-affected-scope-expansion")',
    "architecture packet id",
)
text = replace_exact(
    text,
    '        self.assertEqual(self.packet["baseline"]["sha"], "9f21a2b40f6af5ce57045fc4c1fbfc1bd6cb5b90")',
    '        self.assertEqual(self.packet["baseline"]["sha"], "c9f5bd515b2104ea172ca3089b8a0cdd5f152d9c")',
    "architecture baseline",
)
text = replace_exact(
    text,
    '''        for path in (
            "docs/ACTIVE_PACKET.md",
            "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
            "docs/IMPLEMENTATION_ROADMAP.md",
            "docs/PHASE8_DELIVERY_PLAN.md",
            "docs/PROJECT_STATUS.md",
            "repository-packet.json",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_repository_navigation.py",
        ):''',
    '''        for path in (
            ".github/workflows/affected-scope.yml",
            ".github/workflows/product-plane.yml",
            ".github/workflows/rust-generated-sync.yml",
            "affected-scope-policy.json",
            "docs/ACTIVE_PACKET.md",
            "docs/AFFECTED_SCOPE_CI.md",
            "repository-packet.json",
            "scripts/affected_scope.py",
            "tests/test_affected_scope.py",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_repository_navigation.py",
        ):''',
    "architecture allowed paths",
)
text = replace_exact(
    text,
    '        for path in (".github/workflows/**", "Cargo.toml", "Cargo.lock", "crates/**", "database/**", "services/**"):\n',
    '''        for path in (
            "Cargo.lock",
            "Cargo.toml",
            "apps/**",
            "contracts/**",
            "crates/**",
            "database/**",
            "modules/**",
            "packages/**",
            "proto/**",
            "schemas/**",
            "services/**",
        ):
''',
    "architecture forbidden paths",
)
text = replace_exact(
    text,
    '        self.assertIn("repository-step-8-evidence-sync", self.active_packet)',
    '        self.assertIn("repository-step-9-affected-scope-expansion", self.active_packet)',
    "architecture active packet",
)
architecture.write_text(text, encoding="utf-8")

navigation = Path("tests/test_repository_navigation.py")
text = navigation.read_text(encoding="utf-8")
text = replace_exact(
    text,
    '''        for path in (
            ".github/workflows/affected-scope.yml",
            "affected-scope-policy.json",''',
    '''        for path in (
            ".github/workflows/affected-scope.yml",
            ".github/workflows/product-plane.yml",
            ".github/workflows/rust-generated-sync.yml",
            "affected-scope-policy.json",''',
    "navigation workflows",
)
text = replace_exact(
    text,
    '''            "tests/test_affected_scope.py",
            "tests/test_repository_navigation.py",''',
    '''            "tests/test_affected_scope.py",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_repository_navigation.py",''',
    "navigation guards",
    expected=2,
)
navigation.write_text(text, encoding="utf-8")

affected = Path("tests/test_affected_scope.py")
text = affected.read_text(encoding="utf-8")
marker = "    def test_glob_matching_handles_nested_paths(self) -> None:\n"
method = '''    def test_live_policy_representatives_are_covered_by_real_workflows(self) -> None:
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

'''
text = replace_exact(text, marker, method + marker, "live policy test")
affected.write_text(text, encoding="utf-8")
