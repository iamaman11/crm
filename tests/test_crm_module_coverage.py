"""Prove module discovery, activation and RPC ownership without a fixed count."""
from __future__ import annotations

import importlib.util
from pathlib import Path
import shutil
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "crm_module_coverage", ROOT / "scripts/crm-module-coverage.py"
)
assert SPEC and SPEC.loader
coverage = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(coverage)


class ModuleDiscoveryTests(unittest.TestCase):
    def test_every_canonical_manifest_is_discovered(self) -> None:
        report = coverage.build_report()
        discovered = {m["manifest"] for m in report["modules"]}
        expected = {p.relative_to(ROOT).as_posix()
                    for p in ROOT.glob("modules/*/module.yaml")}
        self.assertEqual(discovered, expected)
        self.assertEqual(report["counts"]["discovered_modules"], len(expected))
        self.assertEqual(report["counts"]["linked_modules"],
                         sum(m["linked"] for m in report["modules"]))

    def test_new_unlinked_module_is_discovered_but_not_published(self) -> None:
        directory = ROOT / "modules/crm-ci-expansion-probe"
        self.assertFalse(directory.exists(), "test module directory must be disposable")
        baseline = coverage.build_report()
        try:
            (directory / "src").mkdir(parents=True)
            (directory / "Cargo.toml").write_text(
                '[package]\nname="crm-ci-expansion-probe"\nversion="0.1.0"\nedition="2024"\n',
                encoding="utf-8",
            )
            (directory / "src/lib.rs").write_text("pub fn example() {}\n", encoding="utf-8")
            (directory / "module.yaml").write_text(
                """schema_version: crm.module/v1
module_id: crm.ci-expansion-probe
version: 0.1.0
owner:
  team: customer-platform
  contact: crm-owner@example.com
  codeowners: ['@iamaman11']
runtime:
  kind: in_process
  entrypoint: modules/crm-ci-expansion-probe
platform:
  minimum_version: 0.1.0
dependencies: {required: [], optional: [], conflicts: []}
provides: {capabilities: [], events: [], objects: [], ui_extensions: []}
consumes: {capabilities: [], events: []}
storage: {record_types: [], private_state_namespaces: []}
security: {data_classes: [], network_egress: [], secret_handles: []}
lifecycle:
  upgrade_policy: manual
  rollback_policy: supported
  uninstall_policy: delete_private_state
  retained_record_types: []
""",
                encoding="utf-8",
            )
            expanded = coverage.build_report()
            self.assertEqual(expanded["counts"]["discovered_modules"],
                             baseline["counts"]["discovered_modules"] + 1)
            module = next(m for m in expanded["modules"]
                          if m["module_id"] == "crm.ci-expansion-probe")
            self.assertFalse(module["linked"], "new module cannot self-activate")
            self.assertEqual(module["active_rpc_paths"], [])
        finally:
            shutil.rmtree(directory, ignore_errors=True)

    def test_unclaimed_active_rpc_blocks_cutover_not_contract_only(self) -> None:
        current = coverage.build_report(phase="contract-only")
        self.assertIn("/crm.parties.v2.PartyService/GetParty",
                      current["active_rpc_paths"])
        # Existing Party manifest still describes v1. This mismatch must not
        # be hidden or accepted in the final cutover.
        if current["unclaimed_active_rpc_paths"]:
            with self.assertRaisesRegex(ValueError, "active RPC is not declared"):
                coverage.build_report(phase="cutover")


if __name__ == "__main__":
    unittest.main()
