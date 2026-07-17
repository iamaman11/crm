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
            },
        )
        self.assertEqual(empty_modules, {"crm.sales-activities-link"})
        self.assertIn(("crm.search", "search.global.query", "1.0.0"), platform)


if __name__ == "__main__":
    unittest.main()
