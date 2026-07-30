from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def replace_exact(path: str, old: str, new: str, *, expected: int = 1) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{path}: expected {expected} exact matches, found {count}")
    target.write_text(text.replace(old, new), encoding="utf-8")


replace_exact(
    "tests/test_repository_navigation.py",
    '''        for path in (
            "Cargo.lock",
            "Cargo.toml",
            "affected-scope-policy.json",
            "contracts/**",
            "database/migrations/**",
            "packages/**",
            "proto/**",
            "schemas/**",
            "services/crm-api/src/**",
        ):
''',
    '''        for path in (
            ".github/workflows/**",
            "Cargo.lock",
            "Cargo.toml",
            "affected-scope-policy.json",
            "apps/**",
            "contracts/**",
            "crates/**",
            "database/**",
            "modules/**",
            "packages/**",
            "proto/**",
            "schemas/**",
            "scripts/**",
            "services/**",
        ):
''',
)
replace_exact(
    "tests/test_repository_navigation.py",
    '''            "selected_workflows": [
                {
                    "name": "Affected Scope CI",
                    "path": ".github/workflows/affected-scope.yml",
                    "selected": True,
                    "reasons": ["test fixture"],
                }
            ],
''',
    '''            "selected_workflows": [
                {
                    "name": name,
                    "path": path,
                    "selected": True,
                    "reasons": ["test fixture"],
                }
                for name, path in (
                    ("Affected Scope CI", ".github/workflows/affected-scope.yml"),
                    ("Governance CI", ".github/workflows/governance.yml"),
                    ("Rust CI", ".github/workflows/rust.yml"),
                    ("Rust Generated Sync", ".github/workflows/rust-generated-sync.yml"),
                )
            ],
''',
)
replace_exact(
    "tests/test_repository_navigation.py",
    '''                return_value=(
                    "4e0077fbf09d94e5fd7e4c69e238d6d3878252b0"
                ),
''',
    '''                return_value=(
                    "19232f6f3e2ae87aabeb080257c1aac5477a6616"
                ),
''',
)

replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '            "10. governed Customer Privacy access/export assembly — **Next**;",\n',
    '            "10. governed Customer Privacy access/export assembly — **Complete through PR #241**;",\n'
    '            "11. owner-specific deletion, anonymization and supported crypto-shred execution — **Next**;",\n',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '''        self.assertIn(
            "10. repository step 10 — governed access/export assembly — **next**;",
            self.phase8,
        )
        self.assertNotIn(
            "9. repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product checks — **next**;",
            self.phase8,
        )
        self.assertIn(
            "A later step must not start while repository step 10 is unfinished.",
            self.phase8,
        )
''',
    '''        self.assertIn(
            "10. repository step 10 — governed access/export assembly — **complete through PR #241**;",
            self.phase8,
        )
        self.assertIn(
            "11. repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — **next**;",
            self.phase8,
        )
        self.assertNotIn(
            "10. repository step 10 — governed access/export assembly — **next**;",
            self.phase8,
        )
        self.assertIn(
            "A later step must not start while repository step 11 is unfinished.",
            self.phase8,
        )
''',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '''        for path in (
            "Cargo.lock",
            "Cargo.toml",
            "contracts/**",
            "database/migrations/**",
            "proto/**",
        ):
''',
    '''        for path in (
            ".github/workflows/**",
            "Cargo.lock",
            "Cargo.toml",
            "affected-scope-policy.json",
            "apps/**",
            "contracts/**",
            "crates/**",
            "database/**",
            "modules/**",
            "packages/**",
            "proto/**",
            "schemas/**",
            "scripts/**",
            "services/**",
        ):
''',
)
