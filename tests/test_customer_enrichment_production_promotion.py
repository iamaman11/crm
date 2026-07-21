import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PROMOTION_PATH = ROOT / "contracts" / "customer-enrichment-production-promotion.json"
CLASSIFICATIONS_PATH = ROOT / "contracts" / "production-route-classifications.json"
MODULE_ID = "crm.customer-enrichment"
ACCEPTED_SOURCE_CHECKPOINT = "f92d101206886e3ceaf94d0e56e52580cec21093"
MERGE_COMMIT = "150e44b95d9dbdc08c1792563de03ec73f34aed1"

EXPECTED_RUNTIME_MUTATIONS = {
    "customer_enrichment.provider_profile.publish@1.0.0",
    "customer_enrichment.mapping.publish@1.0.0",
    "customer_enrichment.request.create@1.0.0",
    "customer_enrichment.request.cancel@1.0.0",
    "customer_enrichment.suggestion.reject@1.0.0",
    "customer_enrichment.suggestion.accept@1.0.0",
}
EXPECTED_RUNTIME_QUERIES = {
    "customer_enrichment.provider_profile.get@1.0.0",
    "customer_enrichment.mapping.get@1.0.0",
    "customer_enrichment.request.get@1.0.0",
    "customer_enrichment.request.list@1.0.0",
    "customer_enrichment.suggestion.get@1.0.0",
    "customer_enrichment.suggestion.list_by_party@1.0.0",
}
EXPECTED_RUNTIME_WORKERS = {
    "customer_enrichment.request.dispatch@1.0.0",
    "customer_enrichment.response.record@1.0.0",
    "customer_enrichment.suggestions.materialize@1.0.0",
    "customer_enrichment.party.display_name.apply@1.0.0",
    "customer_enrichment.application.outcome.record@1.0.0",
}
EXPECTED_PROMOTION = {
    "customer_enrichment.request.dispatch@1.0.0": (
        1,
        "worker_mutation",
        "worker_only",
    ),
    "customer_enrichment.response.record@1.0.0": (
        1,
        "worker_mutation",
        "worker_only",
    ),
    "customer_enrichment.suggestions.materialize@1.0.0": (
        1,
        "worker_mutation",
        "worker_only",
    ),
}
EXTERNAL_DEPENDENCIES = {"parties.party.update@1.0.0"}


def coordinate(entry: dict[str, object]) -> str:
    return f"{entry['id']}@{entry['version']}"


class CustomerEnrichmentProductionPromotionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.plan = json.loads(PROMOTION_PATH.read_text(encoding="utf-8"))
        cls.classifications = json.loads(
            CLASSIFICATIONS_PATH.read_text(encoding="utf-8")
        )

    def test_current_runtime_inventory_is_exact_and_disjoint(self) -> None:
        self.assertEqual(
            self.plan["schema_version"],
            "crm.customer-enrichment.production-promotion/v1",
        )
        self.assertEqual(self.plan["module_id"], MODULE_ID)
        self.assertEqual(
            self.plan["accepted_source_checkpoint"], ACCEPTED_SOURCE_CHECKPOINT
        )
        self.assertEqual(self.plan["merge_commit"], MERGE_COMMIT)
        inventory = self.plan["current_runtime_inventory"]
        mutations = set(inventory["mutations"])
        queries = set(inventory["queries"])
        workers = set(inventory["workers"])
        self.assertEqual(mutations, EXPECTED_RUNTIME_MUTATIONS)
        self.assertEqual(queries, EXPECTED_RUNTIME_QUERIES)
        self.assertEqual(workers, EXPECTED_RUNTIME_WORKERS)
        self.assertTrue(mutations.isdisjoint(queries))
        self.assertTrue(mutations.isdisjoint(workers))
        self.assertTrue(queries.isdisjoint(workers))
        self.assertEqual(len(mutations | queries | workers), 17)

    def test_completed_promotions_match_authoritative_worker_runtime_set(self) -> None:
        entries = [
            entry
            for stage in self.plan["promotion_stages"]
            for entry in stage["coordinates"]
        ]
        promoted = {coordinate(entry) for entry in entries}
        self.assertEqual(promoted, set(EXPECTED_PROMOTION))
        self.assertEqual(len(entries), len(promoted), "promotion coordinates must be unique")

        classified_workers = {
            f"{entry['id']}@{entry['version']}"
            for entry in self.classifications["worker_runtime_routes"]
            if entry["owner_module_id"] == MODULE_ID
        }
        self.assertEqual(classified_workers, EXPECTED_RUNTIME_WORKERS)
        self.assertTrue(promoted.issubset(classified_workers))

        classified_non_runtime = {
            f"{entry['id']}@{entry['version']}"
            for entry in self.classifications["non_runtime_contract_routes"]
            if entry["owner_module_id"] == MODULE_ID
        }
        self.assertEqual(classified_non_runtime, set())

    def test_stages_route_kinds_and_dependencies_are_deterministic(self) -> None:
        stages = self.plan["promotion_stages"]
        self.assertEqual(
            [(stage["stage"], stage["name"], stage["state"]) for stage in stages],
            [(1, "provider_worker_pipeline", "complete")],
        )
        stage_by_coordinate: dict[str, int] = {}
        entries_by_coordinate: dict[str, dict[str, object]] = {}
        for stage in stages:
            for entry in stage["coordinates"]:
                key = coordinate(entry)
                expected_stage, expected_kind, expected_exposure = EXPECTED_PROMOTION[key]
                self.assertEqual(stage["stage"], expected_stage)
                self.assertEqual(entry["route_kind"], expected_kind)
                self.assertEqual(entry["exposure"], expected_exposure)
                self.assertTrue(entry["implementation_crate"].startswith("crm-customer-enrichment-"))
                stage_by_coordinate[key] = stage["stage"]
                entries_by_coordinate[key] = entry

        available_runtime = (
            EXPECTED_RUNTIME_MUTATIONS
            | EXPECTED_RUNTIME_QUERIES
            | EXPECTED_RUNTIME_WORKERS
        )
        for key, entry in entries_by_coordinate.items():
            self.assertIn(key, available_runtime)
            for dependency in entry["depends_on"]:
                if dependency in EXTERNAL_DEPENDENCIES or dependency in available_runtime:
                    continue
                self.assertIn(dependency, stage_by_coordinate)
                self.assertLessEqual(stage_by_coordinate[dependency], stage_by_coordinate[key])

    def test_every_promotion_is_activation_gated_and_acceptance_bound(self) -> None:
        invariants = self.plan["global_invariants"]
        self.assertEqual(invariants["activation_gate"], "crm.module_installations")
        self.assertEqual(invariants["registration_owner"], "module_owned_contribution")
        self.assertFalse(invariants["central_business_route_switches_allowed"])
        self.assertTrue(invariants["production_readiness_requires_single_exact_head"])
        self.assertEqual(invariants["required_exact_head_workflows"], 17)
        self.assertTrue(invariants["retain_provenance_on_uninstall"])

        for stage in self.plan["promotion_stages"]:
            self.assertEqual(stage["state"], "complete")
            for entry in stage["coordinates"]:
                evidence = set(entry["required_evidence"])
                self.assertIn("disable_uninstall", evidence)
                self.assertIn("cross_tenant", evidence)
                self.assertIn("exact_head_17_workflows", evidence)
                self.assertEqual(entry["exposure"], "worker_only")
                self.assertIn("activation_gated_worker_registration", evidence)


if __name__ == "__main__":
    unittest.main()
