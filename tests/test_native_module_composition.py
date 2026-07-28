from __future__ import annotations

from pathlib import Path
import tempfile
import unittest

from scripts.check_native_module_composition import (
    LEGACY_MARKERS,
    find_legacy_composition_violations,
)

ROOT = Path(__file__).resolve().parents[1]


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

    def test_customer_accounts_registration_is_aggregated_without_reordering(self) -> None:
        owner = (
            ROOT
            / "crates/crm-customer-accounts-capability-composition/src/lib.rs"
        ).read_text(encoding="utf-8")
        aggregate = (ROOT / "crates/crm-first-party-modules/src/lib.rs").read_text(
            encoding="utf-8"
        )
        runtime = (
            ROOT / "crates/crm-application-runtime/src/native_composition.rs"
        ).read_text(encoding="utf-8")

        self.assertIn("pub fn mutation_capability_definitions()", owner)
        self.assertIn("adapter_mutation_capability_definitions()", owner)
        self.assertIn("pub fn query_capability_definitions()", owner)
        self.assertIn("adapter_query_capability_definitions()", owner)
        self.assertIn(
            "mutation_capability_definitions as "
            "customer_accounts_mutation_capability_definitions",
            aggregate,
        )
        self.assertIn(
            "query_capability_definitions as "
            "customer_accounts_query_capability_definitions",
            aggregate,
        )
        self.assertNotIn("customer_accounts.", aggregate)
        self.assertNotIn("@1.0.0", aggregate)

        self.assertNotIn("use crm_customer_accounts_capability_adapter", runtime)
        self.assertNotIn("use crm_customer_accounts_query_adapter", runtime)
        self.assertIn(
            "customer_accounts_mutation_capability_definitions as "
            "account_capability_definitions",
            runtime,
        )
        self.assertIn(
            "customer_accounts_query_capability_definitions as "
            "account_query_capability_definitions",
            runtime,
        )

        mutation_order = [
            "definitions.extend(party_capability_definitions()?);",
            "definitions.extend(account_capability_definitions()?);",
            "definitions.extend(contact_point_capability_definitions()?);",
        ]
        query_order = [
            "definitions.extend(party_query_capability_definitions()?);",
            "definitions.extend(account_query_capability_definitions()?);",
            "definitions.extend(contact_point_query_capability_definitions()?);",
        ]
        for sequence in (mutation_order, query_order):
            positions = [runtime.index(marker) for marker in sequence]
            self.assertEqual(positions, sorted(positions))

        account_contribution = aggregate.index(
            "build_customer_accounts_contribution("
        )
        consent_contribution = aggregate.index("build_consents_contribution(")
        self.assertLess(account_contribution, consent_contribution)
        self.assertIn("activation: activation.clone(),", aggregate)
        self.assertIn("ActivationGatedMutationValidator::new", owner)
        self.assertIn("ActivationGatedQueryValidator::new", owner)


if __name__ == "__main__":
    unittest.main()
