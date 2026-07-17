from __future__ import annotations

from copy import deepcopy
import unittest

from scripts.check_module_compatibility import compare_manifest_sets


def manifest(version: str = "1.0.0") -> dict:
    return {
        "schema_version": "crm.module/v1",
        "module_id": "crm.alpha",
        "version": version,
        "provides": {
            "capabilities": [
                {
                    "id": "alpha.record.get",
                    "version": "1.0.0",
                    "binding": {
                        "kind": "protobuf_rpc",
                        "rpc": "crm.alpha.v1.AlphaService.GetAlpha",
                        "request": "crm.alpha.v1.GetAlphaRequest",
                        "response": "crm.alpha.v1.GetAlphaResponse",
                    },
                }
            ],
            "events": [],
        },
    }


class ModuleCompatibilityTests(unittest.TestCase):
    def test_same_published_version_is_immutable(self) -> None:
        base = manifest()
        changed = deepcopy(base)
        changed["provides"]["capabilities"][0]["id"] = "alpha.record.list"
        errors = compare_manifest_sets({"crm.alpha": base}, {"crm.alpha": changed})
        self.assertTrue(any("without a version bump" in error for error in errors))

    def test_binding_only_change_does_not_churn_runtime_identity(self) -> None:
        base = manifest()
        changed = deepcopy(base)
        changed["provides"]["capabilities"][0]["binding"]["rpc"] = (
            "crm.alpha.v1.AlphaQueryService.GetAlpha"
        )
        self.assertEqual(
            compare_manifest_sets({"crm.alpha": base}, {"crm.alpha": changed}), []
        )

    def test_version_bump_allows_runtime_change(self) -> None:
        base = manifest()
        changed = manifest("1.1.0")
        changed["provides"]["capabilities"].append(
            {
                "id": "alpha.record.list",
                "version": "1.0.0",
                "binding": {
                    "kind": "protobuf_rpc",
                    "rpc": "crm.alpha.v1.AlphaService.ListAlpha",
                    "request": "crm.alpha.v1.ListAlphaRequest",
                    "response": "crm.alpha.v1.ListAlphaResponse",
                },
            }
        )
        self.assertEqual(
            compare_manifest_sets({"crm.alpha": base}, {"crm.alpha": changed}), []
        )

    def test_removal_and_version_regression_are_rejected(self) -> None:
        base = manifest("2.0.0")
        self.assertTrue(compare_manifest_sets({"crm.alpha": base}, {}))
        regressed = manifest("1.9.9")
        errors = compare_manifest_sets(
            {"crm.alpha": base}, {"crm.alpha": regressed}
        )
        self.assertTrue(any("regressed" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
