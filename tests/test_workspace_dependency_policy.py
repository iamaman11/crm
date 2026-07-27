"""Focused tests for workspace dependency inheritance and no-growth policies."""

from datetime import date
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from scripts.check_workspace_dependency_policy import validate_policy_document


FAMILIES = ("prost", "serde", "serde_json", "sha2")


def accepted_spec(version: str, features: list[str] | None = None) -> dict[str, object]:
    return {
        "workspace": False,
        "version": version,
        "features": features or [],
        "default_features": True,
        "path": None,
        "git": None,
        "registry": None,
        "package": None,
        "branch": None,
        "rev": None,
        "tag": None,
    }


class WorkspaceDependencyPolicyTests(unittest.TestCase):
    def temporary_root(self) -> tuple[TemporaryDirectory, Path]:
        temporary = TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        (root / "Cargo.toml").write_text(
            """[workspace]
members = []

[workspace.dependencies]
prost = "0.14"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
""",
            encoding="utf-8",
        )
        self.write_registry(root, [])
        return temporary, root

    def write_registry(self, root: Path, exceptions: list[dict[str, object]]) -> None:
        (root / "architecture-governance.json").write_text(
            json.dumps(
                {
                    "schema_version": "crm.architecture-governance/v1",
                    "exceptions": exceptions,
                    "new_crate_justifications": [],
                }
            ),
            encoding="utf-8",
        )

    def write_policy(
        self,
        root: Path,
        *,
        accepted_consumers: dict[str, list[str]] | None = None,
        calibrated: bool = False,
        families: list[str] | None = None,
    ) -> None:
        consumers = {name: [] for name in FAMILIES}
        if accepted_consumers:
            consumers.update(accepted_consumers)
        policies = []
        if calibrated:
            policies.append(
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
            )
        (root / "workspace-dependency-policy.json").write_text(
            json.dumps(
                {
                    "schema_version": "crm.workspace-dependency-policy/v1",
                    "policies": policies,
                    "no_growth": {
                        "id": "root-workspace-dependency-direct-debt-no-growth",
                        "owner": "architecture-governance",
                        "reason": "fixture",
                        "tracking_issue": "#194",
                        "exception_rule": "workspace-dependency-no-growth",
                        "dependency_families": families or list(FAMILIES),
                        "accepted_specs": {
                            "prost": accepted_spec("0.14"),
                            "serde": accepted_spec("1", ["derive"]),
                            "serde_json": accepted_spec("1"),
                            "sha2": accepted_spec("0.10"),
                        },
                        "accepted_direct_consumers": consumers,
                    },
                }
            ),
            encoding="utf-8",
        )

    def write_manifest(self, root: Path, relative: str, dependencies: str) -> Path:
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        package = path.parent.name.replace("_", "-")
        path.write_text(
            f"""[package]
name = "{package}"
version = "0.1.0"

[dependencies]
{dependencies}
""",
            encoding="utf-8",
        )
        return path

    def validate(self, root: Path, manifests: list[Path]):
        return validate_policy_document(
            root,
            manifest_paths=manifests,
            today=date(2026, 7, 27),
        )

    def test_accepts_exact_inheritance_and_does_not_require_unused_dependencies(self) -> None:
        _, root = self.temporary_root()
        self.write_policy(root, calibrated=True)
        first = self.write_manifest(
            root,
            "modules/first/Cargo.toml",
            """serde = { workspace = true }
serde_json = { workspace = true }
sha2 = { workspace = true }""",
        )
        second = self.write_manifest(
            root, "modules/second/Cargo.toml", 'other = "1"'
        )
        report = self.validate(root, [first, second])
        self.assertEqual(report["blocking_errors"], [])
        self.assertEqual(report["policies"][0]["matched_manifest_count"], 2)
        self.assertEqual(report["policies"][0]["governed_declaration_count"], 3)

    def test_rejects_calibrated_direct_version_and_local_feature_override(self) -> None:
        _, root = self.temporary_root()
        self.write_policy(root, calibrated=True)
        manifest = self.write_manifest(
            root,
            "modules/bad/Cargo.toml",
            '''serde = { workspace = true, features = ["rc"] }
serde_json = "1"''',
        )
        report = self.validate(root, [manifest])
        self.assertTrue(
            any("must not add local features" in error for error in report["blocking_errors"])
        )
        self.assertTrue(
            any("must use workspace = true" in error for error in report["blocking_errors"])
        )

    def test_rejects_new_direct_consumer(self) -> None:
        _, root = self.temporary_root()
        self.write_policy(root)
        manifest = self.write_manifest(
            root, "crates/new/Cargo.toml", 'prost = "0.14"'
        )
        report = self.validate(root, [manifest])
        self.assertTrue(
            any("new direct consumer" in error for error in report["blocking_errors"])
        )

    def test_rejects_direct_version_drift(self) -> None:
        _, root = self.temporary_root()
        path = "crates/existing/Cargo.toml"
        self.write_policy(root, accepted_consumers={"prost": [path]})
        manifest = self.write_manifest(root, path, 'prost = "0.15"')
        report = self.validate(root, [manifest])
        self.assertTrue(
            any("changed its accepted direct declaration" in error for error in report["blocking_errors"])
        )

    def test_rejects_feature_drift(self) -> None:
        _, root = self.temporary_root()
        path = "crates/existing/Cargo.toml"
        self.write_policy(root, accepted_consumers={"prost": [path]})
        manifest = self.write_manifest(
            root, path, 'prost = { version = "0.14", features = ["derive"] }'
        )
        report = self.validate(root, [manifest])
        self.assertTrue(
            any("changed its accepted direct declaration" in error for error in report["blocking_errors"])
        )

    def test_rejects_source_override(self) -> None:
        _, root = self.temporary_root()
        path = "crates/existing/Cargo.toml"
        self.write_policy(root, accepted_consumers={"prost": [path]})
        manifest = self.write_manifest(
            root, path, 'prost = { git = "https://example.invalid/prost" }'
        )
        report = self.validate(root, [manifest])
        self.assertTrue(
            any("changed its accepted direct declaration" in error for error in report["blocking_errors"])
        )

    def test_accepts_new_clean_workspace_inheritance(self) -> None:
        _, root = self.temporary_root()
        self.write_policy(root)
        manifest = self.write_manifest(
            root, "crates/new/Cargo.toml", "prost = { workspace = true }"
        )
        report = self.validate(root, [manifest])
        self.assertEqual(report["blocking_errors"], [])
        self.assertEqual(report["no_growth"]["current_direct_consumer_count"], 0)

    def test_allows_existing_debt_reduction_without_baseline_edit(self) -> None:
        _, root = self.temporary_root()
        path = "crates/existing/Cargo.toml"
        self.write_policy(root, accepted_consumers={"prost": [path]})
        manifest = self.write_manifest(
            root, path, "prost = { workspace = true }"
        )
        report = self.validate(root, [manifest])
        self.assertEqual(report["blocking_errors"], [])
        self.assertEqual(report["no_growth"]["reduced_direct_consumer_count"], 1)

    def test_valid_exception_allows_exact_scoped_deviation(self) -> None:
        _, root = self.temporary_root()
        self.write_policy(root)
        path = "crates/new/Cargo.toml"
        self.write_registry(
            root,
            [
                {
                    "id": "ARCH-DEP-001",
                    "owner": "platform",
                    "rule": "workspace-dependency-no-growth",
                    "reason_and_risk": "Temporary direct dependency is required; drift risk is bounded.",
                    "scope": f"{path}:prost",
                    "created_date": "2026-07-27",
                    "expiry_date": "2026-08-27",
                    "removal_condition": "Migrate the consumer to workspace inheritance.",
                    "compensating_checks": ["focused dependency policy test"],
                    "tracking_issue": "#194",
                }
            ],
        )
        manifest = self.write_manifest(root, path, 'prost = "0.15"')
        report = self.validate(root, [manifest])
        self.assertEqual(report["blocking_errors"], [])
        self.assertEqual(report["no_growth"]["exception_count"], 1)

    def test_rejects_expired_ownerless_or_incomplete_exception(self) -> None:
        invalid = {
            "expired": {
                "id": "ARCH-DEP-EXPIRED",
                "owner": "platform",
                "rule": "workspace-dependency-no-growth",
                "reason_and_risk": "fixture",
                "scope": "crates/new/Cargo.toml:prost",
                "created_date": "2026-01-01",
                "expiry_date": "2026-02-01",
                "removal_condition": "fixture",
                "compensating_checks": ["fixture"],
                "tracking_issue": "#194",
            },
            "ownerless": {
                "id": "ARCH-DEP-OWNERLESS",
                "owner": "",
                "rule": "workspace-dependency-no-growth",
                "reason_and_risk": "fixture",
                "scope": "crates/new/Cargo.toml:prost",
                "created_date": "2026-07-27",
                "expiry_date": "2026-08-27",
                "removal_condition": "fixture",
                "compensating_checks": ["fixture"],
                "tracking_issue": "#194",
            },
            "incomplete": {
                "id": "ARCH-DEP-INCOMPLETE",
                "owner": "platform",
                "rule": "workspace-dependency-no-growth",
                "reason_and_risk": "fixture",
                "scope": "crates/new/Cargo.toml:prost",
                "created_date": "2026-07-27",
                "expiry_date": "2026-08-27",
                "removal_condition": "fixture",
                "compensating_checks": [],
                "tracking_issue": "#194",
            },
        }
        for label, exception in invalid.items():
            with self.subTest(label=label):
                _, root = self.temporary_root()
                self.write_policy(root)
                self.write_registry(root, [exception])
                manifest = self.write_manifest(
                    root, "crates/new/Cargo.toml", 'prost = "0.15"'
                )
                report = self.validate(root, [manifest])
                self.assertTrue(report["blocking_errors"])
                self.assertEqual(report["no_growth"]["exception_count"], 0)

    def test_rejects_unknown_dependency_family(self) -> None:
        _, root = self.temporary_root()
        self.write_policy(root, families=[*FAMILIES, "unknown"])
        report = self.validate(root, [])
        self.assertTrue(
            any(
                "must exactly match root" in error
                or "accepted_specs keys" in error
                or "accepted_direct_consumers keys" in error
                for error in report["blocking_errors"]
            )
        )


if __name__ == "__main__":
    unittest.main()
