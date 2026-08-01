"""Tests for repository-step-13 suppression enforcement."""

from datetime import date
import json
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from scripts.check_step13_suppression_baseline import evaluate


class Step13SuppressionBaselineTests(unittest.TestCase):
    def write_baseline(self, root: Path, registrations: list[dict], direct: int = 0) -> None:
        (root / "step13-suppression-baseline.json").write_text(
            json.dumps(
                {
                    "accepted_evidence": {"entry_count": sum(item["n"] for item in registrations)},
                    "enforcement": {
                        "required_current_direct_lint_table_count": direct,
                    },
                    "registrations": registrations,
                }
            ),
            encoding="utf-8",
        )

    def test_registered_reduction_is_allowed(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "crate" / "src" / "lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text("#![allow(dead_code)]\n", encoding="utf-8")
            self.write_baseline(
                root,
                [
                    {
                        "k": "rust-allow",
                        "p": "crate/src/lib.rs",
                        "d": "dead_code",
                        "n": 2,
                    }
                ],
            )

            report = evaluate(root, today=date(2026, 8, 1))

        self.assertTrue(report["ok"])
        self.assertEqual(len(report["reductions"]), 1)
        self.assertEqual(report["unregistered"], [])
        self.assertEqual(report["growth"], [])

    def test_new_key_and_occurrence_growth_are_blocking(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "crate" / "src" / "lib.rs"
            source.parent.mkdir(parents=True)
            source.write_text(
                "#![allow(dead_code)]\n#![allow(dead_code)]\n#![allow(unused_imports)]\n",
                encoding="utf-8",
            )
            self.write_baseline(
                root,
                [
                    {
                        "k": "rust-allow",
                        "p": "crate/src/lib.rs",
                        "d": "dead_code",
                        "n": 1,
                    }
                ],
            )

            report = evaluate(root, today=date(2026, 8, 1))

        self.assertFalse(report["ok"])
        self.assertEqual(len(report["unregistered"]), 1)
        self.assertEqual(len(report["growth"]), 1)

    def test_direct_lint_target_and_expiry_are_blocking(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = root / "crate" / "Cargo.toml"
            manifest.parent.mkdir(parents=True)
            manifest.write_text(
                "[package]\nname='example'\nversion='0.1.0'\n\n"
                "[lints.clippy]\ntoo_many_arguments='allow'\n",
                encoding="utf-8",
            )
            self.write_baseline(
                root,
                [
                    {
                        "k": "direct-lint-table",
                        "p": "crate/Cargo.toml",
                        "d": "package-local [lints] table",
                        "n": 1,
                        "x": "2026-07-31",
                    }
                ],
                direct=0,
            )

            report = evaluate(root, today=date(2026, 8, 1))

        self.assertFalse(report["ok"])
        self.assertEqual(report["direct_lint_table_count"], 1)
        self.assertEqual(len(report["expired"]), 1)


if __name__ == "__main__":
    unittest.main()
