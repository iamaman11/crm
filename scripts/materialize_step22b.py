#!/usr/bin/env python3
"""One-shot fail-closed materializer for the Step 22B permanent guards."""

from __future__ import annotations

from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
ARCH_PATH = ROOT / "tests/test_architecture_documentation_consistency.py"
NAV_PATH = ROOT / "tests/test_repository_navigation.py"
IMPORT_LINE = (
    "from scripts.check_step22_runtime_fanin_decisions import validate_decisions\n"
)

ARCH_METHOD = r'''    def test_active_step_22b_runtime_classification_packet_is_exact(self) -> None:
        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(
            self.packet["packet_id"],
            "repository-step-22b-runtime-fanin-classifications",
        )
        self.assertEqual(
            self.packet["baseline"],
            {"ref": "main", "sha": "4642ea39a7c1c8ad78b1d475a3d5391af8414555"},
        )
        self.assertEqual(self.packet["tracking_issues"], [194])
        allowed_paths = set(self.packet["allowed_paths"])
        for path in (
            "affected-scope-policy.json",
            "docs/STEP22_RUNTIME_FANIN_CLASSIFICATION.md",
            "scripts/check_step22_runtime_fanin_decisions.py",
            "step22-runtime-fanin-decisions.json",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_repository_navigation.py",
        ):
            self.assertIn(path, allowed_paths)
        self.assertIn(".github/workflows/**", self.packet["forbidden_paths"])
        self.assertIn("crates/**", self.packet["forbidden_paths"])
        self.assertIn("step22-architecture-inventory.json", self.packet["forbidden_paths"])
        self.assertIn(self.packet["packet_id"], self.active_packet)
        self.assertIn(self.packet["baseline"]["sha"], self.active_packet)

        counts = validate_decisions(ROOT)
        self.assertEqual(
            counts,
            {
                "all": 63,
                "final": 17,
                "platform_generic": 16,
                "test_only": 1,
                "removed": 0,
                "owner_specific_unavoidable": 0,
                "unresolved": 46,
            },
        )
        non_goals = " ".join(self.packet["non_goals"])
        self.assertIn("owner-specific dependency as unavoidable", non_goals)
        self.assertIn("remove move add or otherwise remediate", non_goals)
        self.assertIn("declare all runtime classifications complete", non_goals)

        operations_scope = next(
            scope
            for scope in self.affected_scope_policy["scopes"]
            if scope["id"] == "operations"
        )
        self.assertEqual(operations_scope["owner"], "platform-operations")
        for path in (
            "scripts/check_step22_runtime_fanin_decisions.py",
            "step22-runtime-fanin-decisions.json",
        ):
            self.assertIn(path, operations_scope["path_patterns"])
        self.assertEqual(operations_scope["required_workflows"], ["Governance CI"])

        step22b = read("docs/STEP22_RUNTIME_FANIN_CLASSIFICATION.md")
        for marker in (
            "16",
            "1",
            "46",
            "partial",
            "Step 22 closure remains blocked",
        ):
            self.assertIn(marker, step22b)
        for marker in (
            "PR #298",
            "ffb8c94373c565de00cccd67c38c80bdb3a12405",
            "4642ea39a7c1c8ad78b1d475a3d5391af8414555",
        ):
            self.assertIn(marker, step22b)

'''

NAV_METHOD = r'''    def test_active_step_22b_runtime_classification_packet_is_exact(self) -> None:
        packet = load_packet(ROOT)
        self.assertEqual(packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(
            packet["packet_id"],
            "repository-step-22b-runtime-fanin-classifications",
        )
        self.assertEqual(packet["status"], "active")
        self.assertEqual(
            packet["baseline"],
            {"ref": "main", "sha": "4642ea39a7c1c8ad78b1d475a3d5391af8414555"},
        )
        self.assertEqual(packet["tracking_issues"], [194])
        allowed_paths = set(packet["allowed_paths"])
        self.assertTrue(
            {
                "affected-scope-policy.json",
                "docs/STEP22_RUNTIME_FANIN_CLASSIFICATION.md",
                "scripts/check_step22_runtime_fanin_decisions.py",
                "step22-runtime-fanin-decisions.json",
                "tests/test_architecture_documentation_consistency.py",
                "tests/test_repository_navigation.py",
            }.issubset(allowed_paths)
        )
        self.assertNotIn(".github/workflows/rust-generated-sync.yml", allowed_paths)
        self.assertFalse(any(path.startswith("crates/") for path in allowed_paths))
        self.assertIn(".github/workflows/**", packet["forbidden_paths"])
        self.assertIn("crates/**", packet["forbidden_paths"])
        self.assertIn("step22-architecture-inventory.json", packet["forbidden_paths"])
        deliverables = " ".join(packet["deliverables"])
        non_goals = " ".join(packet["non_goals"])
        self.assertIn("sixteen platform-generic", deliverables)
        self.assertIn("forty-six", deliverables)
        self.assertIn("owner-specific dependency as unavoidable", non_goals)
        self.assertIn("declare all runtime classifications complete", non_goals)
        self.assertEqual(
            validate_decisions(ROOT),
            {
                "all": 63,
                "final": 17,
                "platform_generic": 16,
                "test_only": 1,
                "removed": 0,
                "owner_specific_unavoidable": 0,
                "unresolved": 46,
            },
        )

'''


def add_import(text: str, marker: str) -> str:
    if IMPORT_LINE in text:
        return text
    if text.count(marker) != 1:
        raise RuntimeError(f"expected one import marker {marker!r}")
    return text.replace(marker, marker + IMPORT_LINE, 1)


def replace_method(text: str, method_name: str, replacement: str) -> str:
    pattern = re.compile(
        rf"(?ms)^    def {re.escape(method_name)}\(self\) -> None:\n"
        rf".*?(?=^    def test_)"
    )
    updated, count = pattern.subn(replacement, text)
    if count != 1:
        raise RuntimeError(
            f"expected one method replacement for {method_name}, got {count}"
        )
    return updated


def replace_once(text: str, old: str, new: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"expected one replacement for {old!r}, got {count}")
    return text.replace(old, new, 1)


def main() -> None:
    arch = ARCH_PATH.read_text(encoding="utf-8")
    arch = add_import(arch, "import unittest\n\n")
    arch = replace_method(
        arch,
        "test_active_step_22a_inventory_packet_is_exact",
        ARCH_METHOD,
    )
    if "repository-step-22a-remeasurement-inventories" in arch:
        raise RuntimeError(
            "stale Step 22A active-packet assertion remains in architecture test"
        )
    ARCH_PATH.write_text(arch, encoding="utf-8")

    nav = NAV_PATH.read_text(encoding="utf-8")
    nav = add_import(nav, "from unittest.mock import patch\n\n")
    nav = replace_method(
        nav,
        "test_active_step_22a_inventory_packet_declaration_is_exact",
        NAV_METHOD,
    )
    nav = replace_once(
        nav,
        '"repository-step-22a-remeasurement-inventories",\n'
        '            first[ACTIVE_PACKET_PATH],',
        '"repository-step-22b-runtime-fanin-classifications",\n'
        '            first[ACTIVE_PACKET_PATH],',
    )
    nav = replace_once(
        nav,
        '"4167bd530b91e3a8fc9bfaaf0d02fcdc1f7a20f3",\n'
        '            first[ACTIVE_PACKET_PATH],',
        '"4642ea39a7c1c8ad78b1d475a3d5391af8414555",\n'
        '            first[ACTIVE_PACKET_PATH],',
    )
    nav = replace_once(
        nav,
        'return_value="4167bd530b91e3a8fc9bfaaf0d02fcdc1f7a20f3",',
        'return_value="4642ea39a7c1c8ad78b1d475a3d5391af8414555",',
    )
    nav = replace_once(
        nav,
        'reasons=["Step 22A exact remeasurement inventory"],',
        'reasons=["Step 22B bounded runtime fan-in classifications"],',
    )
    if "repository-step-22a-remeasurement-inventories" in nav:
        raise RuntimeError(
            "stale Step 22A active-packet assertion remains in navigation test"
        )
    NAV_PATH.write_text(nav, encoding="utf-8")


if __name__ == "__main__":
    main()
