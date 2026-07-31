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

    def test_relationship_owner_batch_is_aggregated_without_behavior_reordering(self) -> None:
        parties = (ROOT / "crates/crm-party-reference-composition/src/lib.rs").read_text(
            encoding="utf-8"
        )
        consents = (
            ROOT / "crates/crm-consents-capability-composition/src/lib.rs"
        ).read_text(encoding="utf-8")
        contact_points = (
            ROOT / "crates/crm-contact-points-capability-composition/src/lib.rs"
        ).read_text(encoding="utf-8")
        relationships = (
            ROOT / "crates/crm-party-relationships-capability-composition/src/lib.rs"
        ).read_text(encoding="utf-8")
        aggregate = (ROOT / "crates/crm-first-party-modules/src/lib.rs").read_text(
            encoding="utf-8"
        )
        runtime = (
            ROOT / "crates/crm-application-runtime/src/native_composition.rs"
        ).read_text(encoding="utf-8")
        runtime_cargo = (ROOT / "crates/crm-application-runtime/Cargo.toml").read_text(
            encoding="utf-8"
        )

        for owner in (parties, consents, contact_points, relationships):
            self.assertIn("pub fn mutation_capability_definitions()", owner)
            self.assertIn("pub fn query_capability_definitions()", owner)
            self.assertIn("pub fn build_contribution(", owner)
            self.assertIn("ActivationGatedMutationValidator::new", owner)
            self.assertIn("ActivationGatedQueryValidator::new", owner)

        order = [
            "build_parties_contribution(",
            "build_customer_accounts_contribution(",
            "build_consents_contribution(",
            "build_contact_points_contribution(",
            "build_party_relationships_contribution(",
        ]
        positions = [aggregate.index(marker) for marker in order]
        self.assertEqual(positions, sorted(positions))
        self.assertIn("PostgresPartyReferenceReader::new(store.clone())", aggregate)
        self.assertNotIn("@1.0.0", aggregate)
        self.assertNotIn("crm.parties", aggregate)
        self.assertNotIn("crm.contact-points", aggregate)
        self.assertNotIn("crm.party-relationships", aggregate)

        self.assertNotIn("ContactPointCapabilityPlanner", runtime)
        self.assertNotIn("ContactPointPartyReferenceSemanticValidator", runtime)
        self.assertNotIn("ContactPointQueryAdapter::new", runtime)
        self.assertNotIn("PartyRelationshipCapabilityPlanner", runtime)
        self.assertNotIn("PartyRelationshipReferenceSemanticValidator", runtime)
        self.assertNotIn("PartyRelationshipQueryAdapter::new", runtime)
        self.assertNotIn("use crm_consents_capability_adapter", runtime)
        self.assertNotIn("PostgresPartyReferenceReader::new", runtime)
        self.assertIn(
            "parties_mutation_capability_definitions as party_capability_definitions",
            runtime,
        )
        self.assertIn(
            "contact_points_mutation_capability_definitions as contact_point_capability_definitions",
            runtime,
        )
        self.assertIn(
            "party_relationships_query_capability_definitions as party_relationship_query_capability_definitions",
            runtime,
        )

        for dependency in (
            "crm-contact-points-query-adapter =",
            "crm-customer-accounts-query-adapter =",
            "crm-party-relationships-capability-composition =",
            "crm-party-relationships-query-adapter =",
        ):
            self.assertNotIn(dependency, runtime_cargo)

        for dependency in (
            "crm-consents-capability-adapter =",
            "crm-consents-query-adapter =",
            "crm-contact-points-capability-adapter =",
            "crm-contact-points-capability-composition =",
            "crm-customer-accounts-capability-adapter =",
            "crm-parties-capability-adapter =",
            "crm-parties-query-adapter =",
            "crm-party-reference-composition =",
            "crm-party-relationships-capability-adapter =",
            "crm-party-relationships-projection =",
        ):
            self.assertIn(dependency, runtime_cargo)


    def test_identity_data_operations_and_quality_batch_is_owner_aggregated(self) -> None:
        identity = (
            ROOT
            / "crates/crm-identity-resolution-capability-composition/src/production_contribution.rs"
        ).read_text(encoding="utf-8")
        data_operations = (
            ROOT
            / "crates/crm-customer-data-operations-execution-composition/src/production_contribution.rs"
        ).read_text(encoding="utf-8")
        data_quality = (
            ROOT
            / "crates/crm-data-quality-source-composition/src/production_contribution.rs"
        ).read_text(encoding="utf-8")
        aggregate = (ROOT / "crates/crm-first-party-modules/src/lib.rs").read_text(
            encoding="utf-8"
        )
        runtime = (
            ROOT / "crates/crm-application-runtime/src/native_composition.rs"
        ).read_text(encoding="utf-8")

        for owner in (identity, data_operations, data_quality):
            self.assertIn("pub fn mutation_capability_definitions()", owner)
            self.assertIn("pub fn query_capability_definitions()", owner)
            self.assertIn("pub fn build_contribution(", owner)
            self.assertIn("ActivationGatedMutationValidator::new", owner)
            self.assertIn("ActivationGatedQueryValidator::new", owner)

        order = [
            "build_party_relationships_contribution(",
            "build_identity_resolution_contribution(",
            "build_customer_data_operations_contribution(",
            "build_data_quality_contribution(",
        ]
        positions = [aggregate.index(marker) for marker in order]
        self.assertEqual(positions, sorted(positions))

        for marker in (
            "IdentityResolutionCapabilityPlanner",
            "IdentityResolutionQueryAdapter::new",
            "CustomerDataOperationsCapabilityPlanner",
            "CustomerDataOperationsQueryAdapter::new",
            "DataQualityCapabilityExecutor::new",
            "DataQualityQueryAdapter::new",
        ):
            self.assertNotIn(marker, runtime)

        self.assertFalse(
            (ROOT / "crates/crm-application-runtime/src/data_quality_capability_execution.rs").exists()
        )
        self.assertFalse(
            (ROOT / "crates/crm-application-runtime/src/data_quality_registration.rs").exists()
        )
        self.assertTrue(
            (ROOT / "crates/crm-data-quality-source-composition/src/capability_execution.rs").exists()
        )
        self.assertTrue(
            (ROOT / "crates/crm-data-quality-source-composition/src/registration.rs").exists()
        )

    def test_final_owner_batch_is_aggregated(self) -> None:
        sales = (
            ROOT
            / "crates/crm-sales-activities-capability-composition/src/production_contribution.rs"
        ).read_text(encoding="utf-8")
        customer_360 = (
            ROOT / "crates/crm-customer-360-query-adapter/src/production_contribution.rs"
        ).read_text(encoding="utf-8")
        enrichment = (
            ROOT
            / "crates/crm-customer-enrichment-capability-composition/src/production_contribution.rs"
        ).read_text(encoding="utf-8")
        aggregate = (ROOT / "crates/crm-first-party-modules/src/lib.rs").read_text(
            encoding="utf-8"
        )
        runtime = (
            ROOT / "crates/crm-application-runtime/src/native_composition.rs"
        ).read_text(encoding="utf-8")

        self.assertIn("pub fn mutation_capability_definitions()", sales)
        self.assertIn("pub fn production_query_capability_definitions()", sales)
        self.assertIn("pub fn build_contribution(", sales)
        self.assertIn("ActivationGatedMutationValidator::new", sales)
        self.assertIn("ActivationGatedQueryValidator::new", sales)
        self.assertIn("pub fn mutation_capability_definitions()", enrichment)
        self.assertIn("pub fn query_capability_definitions()", enrichment)
        self.assertIn("pub fn build_contribution(", enrichment)
        self.assertIn("ActivationGatedMutationValidator::new", enrichment)
        self.assertIn("ActivationGatedQueryValidator::new", enrichment)
        self.assertIn("pub fn production_query_capability_definitions()", customer_360)
        self.assertIn("pub fn build_contribution(", customer_360)
        self.assertIn("ActivationGatedQueryValidator::new", customer_360)

        for marker in (
            "build_sales_activities_contribution(",
            "build_customer_360_contribution(",
            "build_customer_enrichment_contribution(",
        ):
            self.assertIn(marker, aggregate)
        for marker in (
            "SalesActivitiesCapabilityPlannerRouter",
            "SalesActivitiesQueryAdapter::new",
            "Customer360QueryAdapter::new",
            "CustomerEnrichmentCapabilityExecutor::new",
            "CustomerEnrichmentQueryAdapter::new",
        ):
            self.assertNotIn(marker, runtime)

        self.assertFalse(
            (
                ROOT
                / "crates/crm-application-runtime/src/native_composition/customer_enrichment_reference_guards.rs"
            ).exists()
        )
        self.assertTrue(
            (
                ROOT
                / "crates/crm-customer-enrichment-capability-composition/src/reference_guards.rs"
            ).exists()
        )

if __name__ == "__main__":
    unittest.main()
