import unittest

from scripts.check_production_route_classifications import load_and_validate


class ProductionRouteClassificationTests(unittest.TestCase):
    def test_exact_classifications_are_well_formed(self) -> None:
        platform, non_runtime, empty_modules = load_and_validate()
        self.assertEqual(len(platform), 7)
        self.assertEqual(
            non_runtime,
            {
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
                *{
                    (
                        "crm.customer-enrichment",
                        capability_id,
                        "1.0.0",
                    )
                    for capability_id in {
                        "customer_enrichment.application.outcome.record",
                        "customer_enrichment.party.display_name.apply",
                        "customer_enrichment.request.cancel",
                        "customer_enrichment.request.dispatch",
                        "customer_enrichment.request.list",
                        "customer_enrichment.response.record",
                        "customer_enrichment.suggestion.accept",
                        "customer_enrichment.suggestion.get",
                        "customer_enrichment.suggestion.list_by_party",
                        "customer_enrichment.suggestion.reject",
                        "customer_enrichment.suggestions.materialize",
                    }
                },
            },
        )
        self.assertEqual(empty_modules, {"crm.sales-activities-link"})
        self.assertIn(("crm.search", "search.global.query", "1.0.0"), platform)
        for capability_id in {
            "customer_enrichment.provider_profile.publish",
            "customer_enrichment.provider_profile.get",
            "customer_enrichment.mapping.publish",
            "customer_enrichment.mapping.get",
            "customer_enrichment.request.create",
            "customer_enrichment.request.get",
        }:
            self.assertNotIn(
                (
                    "crm.customer-enrichment",
                    capability_id,
                    "1.0.0",
                ),
                non_runtime,
            )


if __name__ == "__main__":
    unittest.main()
