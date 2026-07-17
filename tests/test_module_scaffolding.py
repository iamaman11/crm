import json
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest
from unittest.mock import patch

from scripts.scaffold_module import (
    Dependency,
    ModuleSpec,
    ScaffoldError,
    render_manifest,
    scaffold,
)
from scripts.validate_module_manifests import (
    load_schema,
    strict_yaml_load,
    validate_manifest_semantics,
    validate_schema,
)

ROOT = Path(__file__).resolve().parents[1]


class ModuleScaffoldingTests(unittest.TestCase):
    def _workspace(self, temporary_root: Path) -> None:
        (temporary_root / "Cargo.toml").write_text(
            '[workspace]\nresolver = "2"\nmembers = [\n  "modules/crm-existing",\n  "services/crm-api",\n]\n',
            encoding="utf-8",
        )

    def _write_stub_crate(
        self,
        root: Path,
        relative_path: str,
        package_name: str,
        lib_content: str = "#![forbid(unsafe_code)]\n",
    ) -> None:
        crate = root / relative_path
        (crate / "src").mkdir(parents=True)
        (crate / "Cargo.toml").write_text(
            f'''[package]
name = "{package_name}"
version = "0.1.0"
edition = "2024"
publish = false
''',
            encoding="utf-8",
        )
        (crate / "src" / "lib.rs").write_text(lib_content, encoding="utf-8")

    def _compilable_workspace(self, temporary_root: Path) -> None:
        (temporary_root / "Cargo.toml").write_text(
            '''[workspace]
resolver = "2"
members = [
  "crates/crm-application-composition",
  "crates/crm-core-contracts",
  "crates/crm-module-sdk",
]
''',
            encoding="utf-8",
        )
        self._write_stub_crate(
            temporary_root,
            "crates/crm-core-contracts",
            "crm-core-contracts",
        )
        self._write_stub_crate(
            temporary_root,
            "crates/crm-module-sdk",
            "crm-module-sdk",
            lib_content='''#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy)]
pub enum ErrorCategory { Internal }

#[derive(Debug, Clone)]
pub struct SdkError { pub code: String }

impl SdkError {
    pub fn new(
        code: impl Into<String>,
        _category: ErrorCategory,
        _retryable: bool,
        _message: impl Into<String>,
    ) -> Self {
        Self { code: code.into() }
    }

    pub fn with_internal_reference(self, _reference: impl Into<String>) -> Self {
        self
    }
}
''',
        )
        self._write_stub_crate(
            temporary_root,
            "crates/crm-application-composition",
            "crm-application-composition",
            lib_content='''#![forbid(unsafe_code)]

#[derive(Debug, Default)]
pub struct ModuleContributionSet;

impl ModuleContributionSet {
    pub fn new() -> Self { Self }
}
''',
        )

    def test_owner_scaffold_is_schema_valid_and_registered_after_existing_modules(self) -> None:
        spec = ModuleSpec(
            kind="owner",
            module_id="crm.customer",
            display_name="CRM Customer",
            team="customer-platform",
            contact="crm-owner@example.com",
            objects=("customer.party", "customer.contact_point"),
            required_dependencies=(),
        )

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self._workspace(root)
            changed = scaffold(root, spec)

            self.assertIn(Path("modules/crm-customer/module.yaml"), changed)
            self.assertIn(Path("modules/crm-customer/contracts/README.md"), changed)
            self.assertIn(Path("modules/crm-customer/adapters/README.md"), changed)
            self.assertIn(Path("modules/crm-customer/tests/acceptance.rs"), changed)
            self.assertIn(
                Path("crates/crm-customer-composition/src/lib.rs"), changed
            )

            manifest_path = root / "modules/crm-customer/module.yaml"
            manifest = strict_yaml_load(
                manifest_path.read_text(encoding="utf-8"), str(manifest_path)
            )
            self.assertEqual(
                validate_schema(manifest, load_schema(), str(manifest_path)), []
            )
            self.assertEqual(
                validate_manifest_semantics(manifest, str(manifest_path)), []
            )
            self.assertEqual(
                manifest["storage"]["record_types"],
                ["customer.party", "customer.contact_point"],
            )
            self.assertEqual(
                manifest["lifecycle"]["uninstall_policy"],
                "retain_business_records",
            )

            acceptance = (
                root / "modules/crm-customer/tests/acceptance.rs"
            ).read_text(encoding="utf-8")
            self.assertIn("#[ignore =", acceptance)
            self.assertIn("production_acceptance_todo", acceptance)
            production_boundary = (
                root / "crates/crm-customer-composition/src/lib.rs"
            ).read_text(encoding="utf-8")
            self.assertIn("pub fn contribute_to", production_boundary)
            self.assertIn(
                "MODULE_PRODUCTION_CONTRIBUTION_NOT_IMPLEMENTED",
                production_boundary,
            )

            cargo = (root / "Cargo.toml").read_text(encoding="utf-8")
            self.assertLess(
                cargo.index('"modules/crm-existing"'),
                cargo.index('"modules/crm-customer"'),
            )
            self.assertLess(
                cargo.index('"modules/crm-customer"'),
                cargo.index('"crates/crm-customer-composition"'),
            )
            self.assertLess(
                cargo.index('"crates/crm-customer-composition"'),
                cargo.index('"services/crm-api"'),
            )

    def test_generated_owner_module_compiles_and_matches_architecture_policy(self) -> None:
        spec = ModuleSpec(
            kind="owner",
            module_id="crm.customer",
            display_name="CRM Customer",
            team="customer-platform",
            contact="crm-owner@example.com",
            objects=("customer.party",),
            required_dependencies=(),
        )

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self._compilable_workspace(root)
            scaffold(root, spec)

            completed = subprocess.run(
                [
                    "cargo",
                    "check",
                    "--workspace",
                    "--all-targets",
                    "--quiet",
                ],
                cwd=root,
                check=False,
                capture_output=True,
                text=True,
                timeout=60,
            )
            self.assertEqual(
                completed.returncode,
                0,
                msg=f"generated module failed to compile:\n{completed.stderr}",
            )

            cargo = tomllib.loads(
                (root / "modules/crm-customer/Cargo.toml").read_text(
                    encoding="utf-8"
                )
            )
            dependencies = set(cargo.get("dependencies", {})) | set(
                cargo.get("dev-dependencies", {})
            )
            policy = json.loads(
                (ROOT / "architecture-policy.json").read_text(encoding="utf-8")
            )
            self.assertFalse(dependencies & set(policy["forbidden_dependencies"]))
            internal = {
                dependency
                for dependency in dependencies
                if dependency.startswith("crm-")
            }
            self.assertLessEqual(internal, set(policy["allowed_module_prefixes"]))

            composition_cargo = tomllib.loads(
                (root / "crates/crm-customer-composition/Cargo.toml").read_text(
                    encoding="utf-8"
                )
            )
            self.assertEqual(
                set(composition_cargo["dependencies"]),
                {
                    "crm-application-composition",
                    "crm-module-sdk",
                    "crm-customer",
                },
            )

    def test_link_scaffold_requires_two_dependencies_and_owns_no_records(self) -> None:
        invalid = ModuleSpec(
            kind="link",
            module_id="crm.customer-sales-link",
            display_name="Customer Sales Link",
            team="integration-platform",
            contact="crm-owner@example.com",
            objects=(),
            required_dependencies=(Dependency("crm.sales", "^0.2.0"),),
        )
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self._workspace(root)
            with self.assertRaisesRegex(ScaffoldError, "at least two"):
                scaffold(root, invalid)

        valid = ModuleSpec(
            kind="link",
            module_id="crm.customer-sales-link",
            display_name="Customer Sales Link",
            team="integration-platform",
            contact="crm-owner@example.com",
            objects=(),
            required_dependencies=(
                Dependency("crm.customer", "^0.1.0"),
                Dependency("crm.sales", "^0.2.0"),
            ),
        )
        manifest = strict_yaml_load(render_manifest(valid), "generated-link")
        self.assertEqual(validate_schema(manifest, load_schema(), "generated-link"), [])
        self.assertEqual(
            validate_manifest_semantics(manifest, "generated-link"), []
        )
        self.assertEqual(manifest["storage"]["record_types"], [])
        self.assertEqual(manifest["lifecycle"]["retained_record_types"], [])
        self.assertEqual(
            manifest["lifecycle"]["uninstall_policy"], "delete_private_state"
        )

    def test_invalid_dependency_range_is_rejected_before_writing(self) -> None:
        spec = ModuleSpec(
            kind="link",
            module_id="crm.customer-sales-link",
            display_name="Customer Sales Link",
            team="integration-platform",
            contact="crm-owner@example.com",
            objects=(),
            required_dependencies=(
                Dependency("crm.customer", "^0.1.0"),
                Dependency("crm.sales", "not valid!"),
            ),
        )
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self._workspace(root)
            with self.assertRaisesRegex(ScaffoldError, "invalid version range"):
                scaffold(root, spec)
            self.assertFalse((root / "modules/crm-customer-sales-link").exists())

    def test_dry_run_does_not_modify_workspace(self) -> None:
        spec = ModuleSpec(
            kind="owner",
            module_id="crm.customer",
            display_name="CRM Customer",
            team="customer-platform",
            contact="crm-owner@example.com",
            objects=("customer.party",),
            required_dependencies=(),
        )
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self._workspace(root)
            before = (root / "Cargo.toml").read_text(encoding="utf-8")
            changed = scaffold(root, spec, dry_run=True)
            self.assertIn(Path("Cargo.toml"), changed)
            self.assertFalse((root / "modules/crm-customer").exists())
            self.assertFalse(
                (root / "crates/crm-customer-composition").exists()
            )
            self.assertEqual(
                (root / "Cargo.toml").read_text(encoding="utf-8"), before
            )


    def test_existing_composition_directory_is_never_overwritten(self) -> None:
        spec = ModuleSpec(
            kind="owner",
            module_id="crm.customer",
            display_name="CRM Customer",
            team="customer-platform",
            contact="crm-owner@example.com",
            objects=("customer.party",),
            required_dependencies=(),
        )
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self._workspace(root)
            existing = root / "crates/crm-customer-composition"
            existing.mkdir(parents=True)
            marker = existing / "keep.txt"
            marker.write_text("do not replace", encoding="utf-8")
            with self.assertRaisesRegex(ScaffoldError, "already exists"):
                scaffold(root, spec)
            self.assertEqual(marker.read_text(encoding="utf-8"), "do not replace")
            self.assertFalse((root / "modules/crm-customer").exists())

    def test_partial_write_removes_both_scaffold_roots_and_preserves_workspace(self) -> None:
        spec = ModuleSpec(
            kind="owner",
            module_id="crm.customer",
            display_name="CRM Customer",
            team="customer-platform",
            contact="crm-owner@example.com",
            objects=("customer.party",),
            required_dependencies=(),
        )
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self._workspace(root)
            cargo_path = root / "Cargo.toml"
            before = cargo_path.read_text(encoding="utf-8")
            original_write_text = Path.write_text
            failing_path = root / "crates/crm-customer-composition/README.md"

            def fail_composition_readme(path: Path, data: str, *args, **kwargs):
                if path == failing_path:
                    raise OSError("injected scaffold write failure")
                return original_write_text(path, data, *args, **kwargs)

            with patch.object(Path, "write_text", new=fail_composition_readme):
                with self.assertRaisesRegex(OSError, "injected scaffold write failure"):
                    scaffold(root, spec)

            self.assertFalse((root / "modules/crm-customer").exists())
            self.assertFalse((root / "crates/crm-customer-composition").exists())
            self.assertEqual(cargo_path.read_text(encoding="utf-8"), before)

    def test_existing_module_directory_is_never_overwritten(self) -> None:
        spec = ModuleSpec(
            kind="owner",
            module_id="crm.customer",
            display_name="CRM Customer",
            team="customer-platform",
            contact="crm-owner@example.com",
            objects=("customer.party",),
            required_dependencies=(),
        )
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self._workspace(root)
            existing = root / "modules/crm-customer"
            existing.mkdir(parents=True)
            marker = existing / "keep.txt"
            marker.write_text("do not replace", encoding="utf-8")
            with self.assertRaisesRegex(ScaffoldError, "already exists"):
                scaffold(root, spec)
            self.assertEqual(marker.read_text(encoding="utf-8"), "do not replace")


if __name__ == "__main__":
    unittest.main()
