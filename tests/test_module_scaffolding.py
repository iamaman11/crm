from pathlib import Path
import tempfile
import unittest

from scripts.scaffold_module import Dependency, ModuleSpec, ScaffoldError, render_manifest, scaffold
from scripts.validate_module_manifests import (
    load_schema,
    strict_yaml_load,
    validate_manifest_semantics,
    validate_schema,
)


class ModuleScaffoldingTests(unittest.TestCase):
    def _workspace(self, temporary_root: Path) -> None:
        (temporary_root / "Cargo.toml").write_text(
            '[workspace]\nresolver = "2"\nmembers = [\n  "modules/crm-existing",\n  "services/crm-api",\n]\n',
            encoding="utf-8",
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
            manifest_path = root / "modules/crm-customer/module.yaml"
            manifest = strict_yaml_load(manifest_path.read_text(encoding="utf-8"), str(manifest_path))
            self.assertEqual(validate_schema(manifest, load_schema(), str(manifest_path)), [])
            self.assertEqual(validate_manifest_semantics(manifest, str(manifest_path)), [])
            self.assertEqual(manifest["storage"]["record_types"], ["customer.party", "customer.contact_point"])
            self.assertEqual(manifest["lifecycle"]["uninstall_policy"], "retain_business_records")

            cargo = (root / "Cargo.toml").read_text(encoding="utf-8")
            self.assertLess(cargo.index('"modules/crm-existing"'), cargo.index('"modules/crm-customer"'))
            self.assertLess(cargo.index('"modules/crm-customer"'), cargo.index('"services/crm-api"'))

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
        self.assertEqual(validate_manifest_semantics(manifest, "generated-link"), [])
        self.assertEqual(manifest["storage"]["record_types"], [])
        self.assertEqual(manifest["lifecycle"]["retained_record_types"], [])
        self.assertEqual(manifest["lifecycle"]["uninstall_policy"], "delete_private_state")

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
            self.assertEqual((root / "Cargo.toml").read_text(encoding="utf-8"), before)

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
