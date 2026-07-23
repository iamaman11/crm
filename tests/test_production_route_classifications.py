import unittest

from scripts.check_production_route_classifications import load_and_validate


class ProductionRouteClassificationTests(unittest.TestCase):
    def test_exact_classifications_are_well_formed(self) -> None:
        platform, workers, non_runtime, empty_modules = load_and_validate()
        self.assertEqual(len(platform), 7)
        self.assertEqual(
            workers,
            {
                (
                    "crm.customer-enrichment",
                    "customer_enrichment.request.dispatch",
                    "1.0.0",
                ),
                (
                    "crm.customer-enrichment",
                    "customer_enrichment.response.record",
                    "1.0.0",
                ),
                (
                    "crm.customer-enrichment",
                    "customer_enrichment.suggestions.materialize",
                    "1.0.0",
                ),
                (
                    "crm.customer-enrichment",
                    "customer_enrichment.party.display_name.apply",
                    "1.0.0",
                ),
                (
                    "crm.customer-enrichment",
                    "customer_enrichment.application.outcome.record",
                    "1.0.0",
                ),
            },
        )
        privacy_contract_only = {
            ("crm.customer-privacy", "customer_privacy.case.approve", "1.0.0"),
            (
                "crm.customer-privacy",
                "customer_privacy.case.owner_outcomes.list",
                "1.0.0",
            ),
            ("crm.customer-privacy", "customer_privacy.case.plan.get", "1.0.0"),
            ("crm.customer-privacy", "customer_privacy.legal_hold.get", "1.0.0"),
            (
                "crm.customer-privacy",
                "customer_privacy.legal_hold.list_by_subject",
                "1.0.0",
            ),
            ("crm.customer-privacy", "customer_privacy.legal_hold.place", "1.0.0"),
            ("crm.customer-privacy", "customer_privacy.legal_hold.release", "1.0.0"),
            ("crm.customer-privacy", "customer_privacy.restriction.get", "1.0.0"),
            ("crm.customer-privacy", "customer_privacy.restriction.place", "1.0.0"),
            ("crm.customer-privacy", "customer_privacy.restriction.release", "1.0.0"),
        }
        owner_scope_contract_only = {
            ("crm.parties", "parties.privacy.scope.contribute", "1.0.0"),
            (
                "crm.customer-accounts",
                "customer_accounts.privacy.scope.contribute",
                "1.0.0",
            ),
            (
                "crm.contact-points",
                "contact_points.privacy.scope.contribute",
                "1.0.0",
            ),
            (
                "crm.party-relationships",
                "party_relationships.privacy.scope.contribute",
                "1.0.0",
            ),
            ("crm.consents", "consents.privacy.scope.contribute", "1.0.0"),
            (
                "crm.identity-resolution",
                "identity_resolution.privacy.scope.contribute",
                "1.0.0",
            ),
            (
                "crm.customer-data-operations",
                "customer_data.privacy.scope.contribute",
                "1.0.0",
            ),
            (
                "crm.data-quality",
                "data_quality.privacy.scope.contribute",
                "1.0.0",
            ),
            (
                "crm.customer-enrichment",
                "customer_enrichment.privacy.scope.contribute",
                "1.0.0",
            ),
        }
        legacy_import_contract_only = {
            (
                "crm.customer-data-operations",
                "customer_data.import.party.create",
                "1.0.0",
            ),
            (
                "crm.customer-data-operations",
                "customer_data.import.party.rows.validate",
                "1.0.0",
            ),
        }
        self.assertEqual(
            non_runtime,
            legacy_import_contract_only
            | privacy_contract_only
            | owner_scope_contract_only,
        )
        for runtime_id in {
            "customer_privacy.case.create",
            "customer_privacy.case.submit",
            "customer_privacy.case.subject.verify",
            "customer_privacy.case.cancel",
            "customer_privacy.case.get",
            "customer_privacy.case.list",
        }:
            self.assertNotIn(
                ("crm.customer-privacy", runtime_id, "1.0.0"),
                non_runtime,
            )
        self.assertEqual(empty_modules, {"crm.sales-activities-link"})
        self.assertIn(("crm.search", "search.global.query", "1.0.0"), platform)
        completed_enrichment = {
            "customer_enrichment.provider_profile.publish",
            "customer_enrichment.provider_profile.get",
            "customer_enrichment.mapping.publish",
            "customer_enrichment.mapping.get",
            "customer_enrichment.request.create",
            "customer_enrichment.request.cancel",
            "customer_enrichment.request.get",
            "customer_enrichment.request.list",
            "customer_enrichment.suggestion.get",
            "customer_enrichment.suggestion.list_by_party",
            "customer_enrichment.suggestion.reject",
            "customer_enrichment.suggestion.accept",
            "customer_enrichment.request.dispatch",
            "customer_enrichment.response.record",
            "customer_enrichment.suggestions.materialize",
            "customer_enrichment.party.display_name.apply",
            "customer_enrichment.application.outcome.record",
        }
        for capability_id in completed_enrichment:
            coordinate = (
                "crm.customer-enrichment",
                capability_id,
                "1.0.0",
            )
            self.assertNotIn(coordinate, non_runtime)
        self.assertEqual(
            {
                coordinate
                for coordinate in non_runtime
                if coordinate[0] == "crm.customer-enrichment"
            },
            {
                (
                    "crm.customer-enrichment",
                    "customer_enrichment.privacy.scope.contribute",
                    "1.0.0",
                )
            },
        )
        self.assertFalse(
            any(owner == "crm.customer-privacy" for owner, _, _ in workers),
            "Customer Privacy workers are not published by the public-contract slice",
        )


if __name__ == "__main__":
    unittest.main()
