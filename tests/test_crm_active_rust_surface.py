"""Negative proof for the active Cloudflare CRM Rust exception gate."""
from __future__ import annotations

from collections import Counter
import importlib.util
from pathlib import Path
import json
import unittest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "crm_active_rust_surface", ROOT / "scripts/crm-active-rust-surface.py"
)
assert SPEC and SPEC.loader
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)


class ActiveRustSurfaceTests(unittest.TestCase):
    def test_existing_suppressions_are_not_silently_reclassified(self) -> None:
        accepted = gate.suppressions("#[allow(dead_code)]\n#[expect(clippy::needless_return)]")
        self.assertEqual(gate.growth(accepted, accepted), {})

    def test_added_allow_is_blocked(self) -> None:
        accepted = gate.suppressions("#[allow(dead_code)]")
        candidate = gate.suppressions(
            "#[allow(dead_code)]\n#[allow(unused_imports)]"
        )
        self.assertEqual(
            gate.growth(candidate, accepted),
            {"#[allow(unused_imports)]": 1},
        )

    def test_added_duplicate_allow_is_blocked(self) -> None:
        accepted = gate.suppressions("#[allow(dead_code)]")
        candidate = gate.suppressions(
            "#[allow(dead_code)]\n#[allow(dead_code)]"
        )
        self.assertEqual(gate.growth(candidate, accepted), {"#[allow(dead_code)]": 1})

    def test_ignored_test_is_blocked(self) -> None:
        self.assertEqual(
            gate.growth(gate.suppressions('#[ignore = "unstable"]'), Counter()),
            {'#[ignore = "unstable"]': 1},
        )

    def test_cfg_attr_allow_is_blocked(self) -> None:
        candidate = gate.suppressions("#[cfg_attr(test, allow(dead_code))]")
        self.assertEqual(candidate["#[cfg_attr(test, allow(dead_code))]"], 1)

    def test_normal_cfg_attr_is_not_a_suppression(self) -> None:
        self.assertFalse(gate.suppressions('#[cfg_attr(test, derive(Debug))]'))

    def test_removed_exception_is_allowed(self) -> None:
        accepted = gate.suppressions("#[allow(dead_code)]")
        self.assertEqual(gate.growth(Counter(), accepted), {})

    def test_new_manifest_lint_table_is_blocked(self) -> None:
        now = gate.lint_sections('[package]\nname = "example"\n[lints.rust]\nwarnings = "allow"\n')
        self.assertTrue(gate.growth(now, Counter()))

    def test_exact_original_worker_owner_coverage(self) -> None:
        roots, paths = gate.scope()
        report = json.loads(gate.REPORT_PATH.read_text(encoding="utf-8"))
        linked = {row["crate_path"] for row in report["modules"] if row["linked"]}
        self.assertTrue(roots[0].endswith("rust-party"))
        self.assertEqual(set(roots[1:]), linked)
        self.assertEqual(len(roots), len(set(roots)))
        self.assertTrue(any(f.endswith("modules/crm-parties/src/lib.rs") for f in paths))


if __name__ == "__main__":
    unittest.main()
