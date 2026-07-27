from __future__ import annotations

import json
from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[1]
FREEZE_PATH = ROOT / "contracts/customer-privacy-discovery-snapshot-freeze.json"
PARENT_FREEZE_PATH = ROOT / "contracts/customer-privacy-architecture-freeze.json"
SOURCE_PATHS = (
    ROOT / "modules/crm-customer-privacy/src/scope_discovery.rs",
    ROOT / "modules/crm-customer-privacy/src/scope_discovery_state.rs",
    ROOT / "modules/crm-customer-privacy/src/scope_discovery_tests.rs",
)
LIB_PATH = ROOT / "modules/crm-customer-privacy/src/lib.rs"
DOCUMENT_PATH = ROOT / "docs/CUSTOMER_PRIVACY_DISCOVERY_SNAPSHOT_FREEZE.md"
STATUS_PATH = ROOT / "docs/PROJECT_STATUS.md"


class CustomerPrivacyDiscoverySnapshotFreezeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.freeze = json.loads(FREEZE_PATH.read_text(encoding="utf-8"))
        cls.parent = json.loads(PARENT_FREEZE_PATH.read_text(encoding="utf-8"))
        cls.source = "\n".join(
            path.read_text(encoding="utf-8") for path in SOURCE_PATHS
        )
        cls.lib = LIB_PATH.read_text(encoding="utf-8")
        cls.document = DOCUMENT_PATH.read_text(encoding="utf-8")
        cls.status = STATUS_PATH.read_text(encoding="utf-8")

    def test_packet_is_non_runtime_and_anchored_to_parent_freeze(self) -> None:
        packet = self.freeze["packet"]
        self.assertEqual(
            self.freeze["schema_version"],
            "crm.customer-privacy-discovery-snapshot-freeze/v1",
        )
        self.assertEqual(packet["phase"], "8A.11")
        self.assertEqual(packet["issue"], 126)
        self.assertEqual(packet["module_id"], "crm.customer-privacy")
        self.assertEqual(
            packet["state"], "contract_acceptance_frozen_runtime_not_started"
        )
        self.assertEqual(
            packet["parent_schema_version"], self.parent["schema_version"]
        )
        coordinate = self.freeze["discovery_coordinate"]
        self.assertEqual(
            coordinate["coordinate"], "customer_privacy.scope.discover@1.0.0"
        )
        self.assertFalse(coordinate["public_ingress"])
        self.assertEqual(coordinate["runtime_state"], "not_implemented")
        parent_workers = {
            item["coordinate"]: item
            for item in self.parent["worker_internal_coordinates"]
        }
        self.assertIn(coordinate["coordinate"], parent_workers)
        self.assertFalse(parent_workers[coordinate["coordinate"]]["public_ingress"])

    def test_registry_exactly_matches_all_nine_parent_owner_coordinates(self) -> None:
        expected = sorted(
            (
                item["module_id"],
                item["scope"],
            )
            for item in self.parent["owner_contributions"]
        )
        actual = [
            (item["module_id"], item["coordinate"])
            for item in self.freeze["registry"]["owners"]
        ]
        self.assertEqual(actual, expected)
        self.assertEqual(len(actual), 9)
        self.assertEqual(len(actual), len(set(actual)))
        self.assertEqual(
            self.freeze["registry"]["version"],
            "crm.customer-privacy.scope-registry/1.0.0",
        )

    def test_full_lineage_is_immutable_and_snapshot_identity_bound(self) -> None:
        expected_lineage = [
            "privacy_case_id",
            "tenant_id",
            "canonical_party_id",
            "identity_resolution_generation",
            "registry_version",
            "registry_digest_sha256",
            "purpose_code",
            "effective_request_at_unix_ms",
        ]
        lineage = self.freeze["lineage"]
        self.assertEqual(lineage["fields"], expected_lineage)
        self.assertTrue(lineage["immutable"])
        self.assertTrue(lineage["must_match_every_owner_page"])

        identity = self.freeze["snapshot_identity"]
        self.assertTrue(identity["immutable"])
        self.assertFalse(identity["silent_rebase_allowed"])
        for field in expected_lineage:
            self.assertIn(field, identity["fields"])
        self.assertIn("aggregation_snapshot_id", identity["fields"])
        self.assertIn("completeness_digest_sha256", identity["fields"])
        self.assertIn(
            "ordered_bound_owner_contribution_digests", identity["fields"]
        )

    def test_ordering_pagination_and_payload_exclusion_are_exact(self) -> None:
        pagination = self.freeze["pagination"]
        self.assertEqual(pagination["default_page_size"], 64)
        self.assertEqual(pagination["maximum_page_size"], 128)
        self.assertEqual(pagination["cursor_maximum_bytes"], 2048)
        self.assertTrue(pagination["terminal_completeness_required_for_every_owner"])
        projection = self.freeze["resource_projection"]
        self.assertFalse(projection["resource_payloads_allowed"])
        self.assertFalse(projection["owner_private_metadata_allowed"])
        self.assertEqual(
            projection["ordering"],
            [
                "owner_module_id",
                "resource_type",
                "resource_id",
                "resource_version",
                "data_class",
                "evidence_class",
                "retention_policy_id",
            ],
        )
        self.assertEqual(projection["exact_duplicate_policy"], "deduplicate")
        self.assertEqual(
            projection["conflicting_duplicate_policy"], "fail_closed"
        )

    def test_digest_profiles_and_persistence_contract_are_versioned(self) -> None:
        profiles = self.freeze["digest_profiles"]
        self.assertEqual(
            profiles["bound_owner_contribution"],
            "crm.customer-privacy.discovery-owner-contribution/v1",
        )
        self.assertEqual(
            profiles["discovery_lineage"],
            "crm.customer-privacy.discovery-lineage/v1",
        )
        self.assertEqual(
            profiles["authoritative_snapshot_binding"],
            "crm.customer-privacy.discovery-snapshot/v1",
        )
        persistence = self.freeze["persistence_contract"]
        self.assertEqual(persistence["record_type"], "customer-privacy.scope-snapshot")
        self.assertEqual(
            persistence["schema_id"],
            "crm.customer-privacy.discovery_scope_snapshot.state",
        )
        self.assertEqual(persistence["schema_version"], "1.0.0")
        self.assertTrue(persistence["strict_rehydration"])
        self.assertTrue(persistence["append_only_after_finalize"])

    def test_fail_closed_non_effects_and_acceptance_matrix_are_complete(self) -> None:
        failures = set(self.freeze["failure_semantics"]["fail_closed_conditions"])
        for required in (
            "owner_unavailable",
            "owner_disabled",
            "owner_stale",
            "owner_incompatible",
            "registry_version_or_digest_mismatch",
            "identity_topology_generation_drift",
            "missing_owner_contribution",
            "nonterminal_owner_contribution",
            "page_or_cursor_digest_mismatch",
            "snapshot_rehydration_mismatch",
        ):
            self.assertIn(required, failures)

        non_effects = set(self.freeze["non_effects"])
        for required in (
            "no_public_http_or_grpc_route",
            "no_customer_privacy_worker_registration",
            "no_owner_mutation",
            "no_provider_call",
            "no_action_plan",
            "no_destructive_action",
            "no_resource_payload_disclosure",
            "no_generic_runtime_business_switch",
        ):
            self.assertIn(required, non_effects)

        acceptance = set(self.freeze["acceptance_required"])
        for required in (
            "focused_lineage_and_snapshot_digest_tests",
            "exact_nine_owner_registry_parity",
            "permission_aware_snapshot_read_and_audit",
            "clean_postgresql_force_rls_cross_tenant_negatives",
            "complete_schema_removal_and_reapply",
            "repeated_postgresql_acceptance",
            "crash_window_and_idempotent_replay",
            "real_process_discovery_without_planning_or_actions",
            "unchanged_exact_head_ci",
        ):
            self.assertIn(required, acceptance)

    def test_rust_contract_and_documentation_match_machine_freeze(self) -> None:
        required_source = (
            '"customer_privacy.scope.discover@1.0.0"',
            "pub struct ScopeDiscoveryLineage",
            "pub struct DiscoveryOwnerScopeContribution",
            "pub struct DiscoveryScopeSnapshot",
            "purpose_code",
            "effective_request_at_unix_ms",
            'b"crm.customer-privacy.discovery-owner-contribution/v1"',
            'b"crm.customer-privacy.discovery-lineage/v1"',
            'b"crm.customer-privacy.discovery-snapshot/v1"',
            "encode_discovery_scope_snapshot_state",
            "decode_discovery_scope_snapshot_state",
        )
        for text in required_source:
            self.assertIn(text, self.source)
        self.assertIn('include!("scope_discovery.rs");', self.lib)

        for text in (
            "production runtime not started",
            "purpose and effective request time",
            "exactly nine",
            "No new public HTTP/gRPC route",
            "Stage C Customer Privacy golden-package pilot",
        ):
            self.assertIn(text.lower(), self.document.lower())
        self.assertIn("scope discovery and immutable snapshot", self.status.lower())
        self.assertIn("accepted through pr #206", self.status.lower())
        self.assertIn("086b17a95058eee285fcb67a903bd21d9263d357", self.status)
        self.assertIn("95818fd3aeb54a9593a45642583f0b7224d5ecfe", self.status)


if __name__ == "__main__":
    unittest.main()
