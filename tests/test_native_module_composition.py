from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from scripts.check_native_module_composition import (
    LEGACY_MARKERS,
    find_legacy_composition_violations,
)


class NativeModuleCompositionReadinessTests(unittest.TestCase):
    def test_clean_tree_passes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            self.assertEqual(
                find_legacy_composition_violations(Path(temporary)),
                [],
            )

    def test_every_governed_legacy_marker_is_detected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            expected = set()
            grouped: dict[str, list[str]] = {}
            for marker in LEGACY_MARKERS:
                grouped.setdefault(marker.path, []).append(marker.needle)
                expected.add(marker.needle)
            for relative, needles in grouped.items():
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("\n".join(needles), encoding="utf-8")

            violations = find_legacy_composition_violations(root)
            for needle in expected:
                self.assertTrue(
                    any(needle in violation for violation in violations),
                    msg=f"missing violation for {needle}",
                )

    def test_similarly_named_text_outside_governed_paths_is_ignored(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "docs/example.txt"
            path.parent.mkdir(parents=True)
            path.write_text(LEGACY_MARKERS[0].needle, encoding="utf-8")
            self.assertEqual(find_legacy_composition_violations(root), [])


if __name__ == "__main__":
    unittest.main()
