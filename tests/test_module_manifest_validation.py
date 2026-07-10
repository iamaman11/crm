from __future__ import annotations

from pathlib import Path
import unittest

from scripts.validate_module_manifests import (
    ManifestError,
    canonical_digest,
    strict_yaml_load,
    validate_manifest_set,
)


class StrictYamlTests(unittest.TestCase):
    def test_rejects_duplicate_keys(self) -> None:
        with self.assertRaises(ManifestError):
            strict_yaml_load("module_id: crm.sales\nmodule_id: crm.other\n")

    def test_rejects_anchor_and_alias(self) -> None:
        with self.assertRaises(ManifestError):
            strict_yaml_load("first: &shared value\nsecond: *shared\n")

    def test_rejects_custom_tag(self) -> None:
        with self.assertRaises(ManifestError):
            strict_yaml_load("value: !custom data\n")

    def test_rejects_merge_key(self) -> None:
        with self.assertRaises(ManifestError):
            strict_yaml_load("base: {a: 1}\nvalue: {<<: {a: 1}}\n")

    def test_rejects_float(self) -> None:
        with self.assertRaisesRegex(ManifestError, "floating-point"):
            strict_yaml_load("value: 1.25\n")

    def test_rejects_implicit_date(self) -> None:
        with self.assertRaisesRegex(ManifestError, "date/time"):
            strict_yaml_load("value: 2026-07-10\n")

    def test_digest_is_independent_of_mapping_order(self) -> None:
        left = strict_yaml_load("a: 1\nb: text\n")
        right = strict_yaml_load("b: text\na: 1\n")
        self.assertEqual(canonical_digest(left), canonical_digest(right))


class DependencyGraphTests(unittest.TestCase):
    @staticmethod
    def manifest(module_id: str, dependencies: list[str]) -> dict:
        return {
            "module_id": module_id,
            "dependencies": {
                "required": [
                    {"module_id": dependency, "version_range": "^1.0.0"}
                    for dependency in dependencies
                ]
            },
            "provides": {
                "capabilities": [],
                "events": [],
                "objects": [],
            },
        }

    def test_detects_required_dependency_cycle(self) -> None:
        entries = [
            (Path("a/module.yaml"), self.manifest("crm.a", ["crm.b"])),
            (Path("b/module.yaml"), self.manifest("crm.b", ["crm.a"])),
        ]
        errors = validate_manifest_set(entries)
        self.assertTrue(any("dependency cycle" in error for error in errors))

    def test_detects_duplicate_object_owner(self) -> None:
        first = self.manifest("crm.a", [])
        second = self.manifest("crm.b", [])
        first["provides"]["objects"] = ["shared.record"]
        second["provides"]["objects"] = ["shared.record"]
        errors = validate_manifest_set(
            [(Path("a/module.yaml"), first), (Path("b/module.yaml"), second)]
        )
        self.assertTrue(any("already owned" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
