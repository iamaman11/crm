import json
from pathlib import Path
import unittest

from scripts.validate_module_manifests import strict_yaml_load


ROOT = Path(__file__).resolve().parents[1]


class CustomerPrivacyOwnerScopeContractTests(unittest.TestCase):
    def test_exact_nine_owner_contracts_match_freeze_manifests_and_classification(self) -> None:
        packet = json.loads(
            (ROOT / "contracts/customer-privacy-owner-scope-contracts.json").read_text(
                encoding="utf-8"
            )
        )
        freeze = json.loads(
            (ROOT / "contracts/customer-privacy-architecture-freeze.json").read_text(
                encoding="utf-8"
            )
        )
        classifications = json.loads(
            (ROOT / "contracts/production-route-classifications.json").read_text(
                encoding="utf-8"
            )
        )

        self.assertEqual(packet["state"], "contract_only_non_runtime")
        self.assertEqual(packet["wire"]["digest_algorithm"], "sha256")
        self.assertEqual(packet["wire"]["digest_bytes"], 32)
        self.assertEqual(
            packet["wire"]["request_envelope"],
            "crm.customer_privacy.v1.PrivacyScopeContributionRequestEnvelope",
        )
        self.assertEqual(
            packet["wire"]["response_envelope"],
            "crm.customer_privacy.v1.PrivacyScopeContributionResponseEnvelope",
        )
        self.assertEqual(packet["bounded_page"]["default_page_size"], 64)
        self.assertEqual(packet["bounded_page"]["maximum_page_size"], 128)
        self.assertEqual(packet["bounded_page"]["maximum_cursor_bytes"], 2048)

        owners = packet["owners"]
        self.assertEqual(len(owners), 9)
        self.assertEqual(len({entry["module_id"] for entry in owners}), 9)
        self.assertEqual(len({entry["capability_id"] for entry in owners}), 9)
        self.assertEqual(len({entry["rpc"] for entry in owners}), 9)
        self.assertEqual(len({entry["request"] for entry in owners}), 9)
        self.assertEqual(len({entry["response"] for entry in owners}), 9)

        frozen = {
            entry["module_id"]: entry["scope"].rsplit("@", 1)
            for entry in freeze["owner_contributions"]
        }
        actual = {
            entry["module_id"]: [entry["capability_id"], entry["version"]]
            for entry in owners
        }
        self.assertEqual(actual, frozen)

        expected_non_runtime = set()
        for entry in owners:
            manifest_path = ROOT / entry["manifest"]
            manifest = strict_yaml_load(
                manifest_path.read_text(encoding="utf-8"), str(manifest_path)
            )
            self.assertEqual(manifest["module_id"], entry["module_id"])
            capabilities = {
                (capability["id"], capability["version"]): capability
                for capability in manifest["provides"]["capabilities"]
            }
            coordinate = (entry["capability_id"], entry["version"])
            self.assertIn(coordinate, capabilities)
            binding = capabilities[coordinate]["binding"]
            self.assertEqual(binding["kind"], "protobuf_rpc")
            self.assertEqual(binding["rpc"], entry["rpc"])
            self.assertEqual(binding["request"], entry["request"])
            self.assertEqual(binding["response"], entry["response"])
            expected_non_runtime.add(
                (entry["module_id"], entry["capability_id"], entry["version"])
            )

        non_runtime = {
            (route["owner_module_id"], route["id"], route["version"])
            for route in classifications["non_runtime_contract_routes"]
        }
        worker_runtime = {
            (route["owner_module_id"], route["id"], route["version"])
            for route in classifications["worker_runtime_routes"]
        }
        platform_runtime = {
            (route["owner_module_id"], route["id"], route["version"])
            for route in classifications["platform_runtime_routes"]
        }
        self.assertTrue(expected_non_runtime <= non_runtime)
        self.assertTrue(expected_non_runtime.isdisjoint(worker_runtime))
        self.assertTrue(expected_non_runtime.isdisjoint(platform_runtime))

        privacy_manifest_path = ROOT / "modules/crm-customer-privacy/module.yaml"
        privacy_manifest = strict_yaml_load(
            privacy_manifest_path.read_text(encoding="utf-8"),
            str(privacy_manifest_path),
        )
        privacy_consumes = {
            (capability["id"], capability["version"])
            for capability in privacy_manifest["consumes"]["capabilities"]
        }
        self.assertTrue(
            privacy_consumes.isdisjoint(
                {(entry["capability_id"], entry["version"]) for entry in owners}
            )
        )

    def test_wire_contract_is_reference_only_and_represents_every_data_class(self) -> None:
        packet = json.loads(
            (ROOT / "contracts/customer-privacy-owner-scope-contracts.json").read_text(
                encoding="utf-8"
            )
        )
        contributions = (
            ROOT / "proto/crm/customer_privacy/v1/contributions.proto"
        ).read_text(encoding="utf-8")
        types = (ROOT / "proto/crm/customer_privacy/v1/types.proto").read_text(
            encoding="utf-8"
        )

        self.assertEqual(contributions.count("  rpc "), 9)
        for owner in packet["owners"]:
            service_and_method = owner["rpc"].removeprefix(
                "crm.customer_privacy.v1."
            )
            service, method = service_and_method.rsplit(".", 1)
            request = owner["request"].rsplit(".", 1)[1]
            response = owner["response"].rsplit(".", 1)[1]
            self.assertIn(f"service {service} {{", contributions)
            self.assertIn(
                f"rpc {method}({request}) returns ({response});",
                contributions,
            )
            self.assertTrue(request.startswith(method))
            self.assertTrue(response.startswith(method))
        self.assertEqual(
            contributions.count(
                "PrivacyScopeContributionRequestEnvelope contribution = 1;"
            ),
            9,
        )
        self.assertEqual(
            contributions.count(
                "PrivacyScopeContributionResponseEnvelope contribution = 1;"
            ),
            9,
        )
        self.assertNotIn("bytes resource_payload", contributions)
        self.assertNotIn("string resource_value", contributions)
        self.assertIn("PrivacyScopeResourceReference", contributions)
        self.assertIn("bytes page_digest_sha256", contributions)
        self.assertIn("bytes cursor_digest_sha256", contributions)
        self.assertIn("CUSTOMER_DATA_CLASS_RESTRICTED = 9;", types)

    def test_status_sources_track_nine_accepted_owners_and_phase8a_closure(self) -> None:
        project_status = (ROOT / "docs/PROJECT_STATUS.md").read_text(encoding="utf-8")
        module_catalog = (ROOT / "docs/MODULE_CATALOG.md").read_text(encoding="utf-8")
        roadmap = (ROOT / "docs/IMPLEMENTATION_ROADMAP.md").read_text(
            encoding="utf-8"
        )
        phase_plan = (ROOT / "docs/PHASE8_DELIVERY_PLAN.md").read_text(
            encoding="utf-8"
        )
        customer_data_packet = (
            ROOT / "docs/CUSTOMER_DATA_OPERATIONS_PRIVACY_SCOPE_PACKET.md"
        ).read_text(encoding="utf-8")
        data_quality_packet = (
            ROOT / "docs/DATA_QUALITY_PRIVACY_SCOPE_PACKET.md"
        ).read_text(encoding="utf-8")
        enrichment_packet = (
            ROOT / "docs/CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_PACKET.md"
        ).read_text(encoding="utf-8")

        authoritative = (project_status, module_catalog, roadmap, phase_plan)
        for document in authoritative:
            self.assertIn("Customer Data Operations", document)
            self.assertIn("Data Quality", document)
            self.assertIn("Customer Enrichment", document)
            self.assertIn("scope discovery and immutable snapshot", document.lower())
            self.assertIn("Current product-complete expert modules: **1**", document)

        self.assertIn(
            "All nine authoritative owner implementations are accepted", project_status
        )
        self.assertIn(
            "All nine authoritative implementations are accepted", module_catalog
        )
        self.assertIn("Nine-owner set complete", phase_plan)
        self.assertIn("Phase 8A.11 / issue #126 is complete", project_status)
        self.assertIn("Phase 8A is **Complete through PR #296**", phase_plan)

        for document in (*authoritative, enrichment_packet):
            self.assertIn("PR #192", document)
            self.assertIn("e90e36027de18a07be68e43327ea732810ff332a", document)
            self.assertIn("e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c", document)
            self.assertIn("28 of 28 permanent workflows", document)

        self.assertIn("Status: **Accepted historical contract**", enrichment_packet)
        self.assertIn("28 of 28 permanent workflows succeeded", enrichment_packet)
        self.assertIn("Production discovery remains forbidden", enrichment_packet)
        self.assertIn("Planning and action execution remain prohibited", enrichment_packet)
        self.assertIn("Accepted through PR #206", phase_plan)
        self.assertIn("086b17a95058eee285fcb67a903bd21d9263d357", phase_plan)
        self.assertIn("95818fd3aeb54a9593a45642583f0b7224d5ecfe", phase_plan)
        self.assertIn("PR #206", module_catalog)
        self.assertIn("planning and action execution remain not started", module_catalog)

        for document in (*authoritative, customer_data_packet):
            self.assertIn("PR #188", document)
            self.assertIn("07f34786e82fdfa78d263790e9f50541529006f8", document)
            self.assertIn("089be72fa3010b4aa15aff7f9ea55fd86290f8fc", document)
        self.assertIn("26 of 26 permanent workflows succeeded", customer_data_packet)
        self.assertIn("Accepted historical contract", customer_data_packet)

        for document in (*authoritative, data_quality_packet):
            self.assertIn("PR #190", document)
            self.assertIn("dcfe8faebc7462b888f8fc1721cb379a40fea88a", document)
            self.assertIn("deac197c97cddc15bb9916092ca87f6e767ce1de", document)
        self.assertIn("27 of 27 permanent workflows succeeded", data_quality_packet)
        self.assertIn("Accepted historical contract", data_quality_packet)

        stale_claims = (
            "Six authoritative owner implementations are accepted",
            "Seven authoritative owner implementations are accepted",
            "Eight authoritative owner implementations are accepted",
            "Six authoritative implementations are accepted",
            "Seven authoritative implementations are accepted",
            "Eight authoritative implementations are accepted",
            "Customer Data Operations is the next bounded contract-only owner",
            "Data Quality is the next bounded contract-only owner",
            "Customer Enrichment is the next bounded contract-only owner",
            "Next bounded owner slice — Customer Data Operations",
            "Next bounded owner slice — Data Quality",
            "Next bounded owner slice — Customer Enrichment",
            "Next bounded owner packet: Customer Data Operations",
            "Next bounded owner packet: Data Quality",
            "Next bounded owner packet: Customer Enrichment",
            "Customer Enrichment remains the final owner before",
            "Customer Enrichment implementation not started",
            "Implementation remains not started in this commit",
            "PR #192 in progress",
        )
        for stale in stale_claims:
            for document in (*authoritative, enrichment_packet):
                self.assertNotIn(stale, document)

        temporary_paths = (
            ".github/workflows/temp-enrichment-governance-sync.yml",
            ".github/workflows/temp-enrichment-governance-runner.yml",
            ".ci/enrichment-governance-sync-trigger",
        )
        for path in temporary_paths:
            self.assertFalse((ROOT / path).exists(), path)

    def test_identity_customer_data_and_data_quality_historical_controls_remain_frozen(self) -> None:
        identity_packet = (
            ROOT / "docs/IDENTITY_RESOLUTION_PRIVACY_SCOPE_PACKET.md"
        ).read_text(encoding="utf-8")
        customer_data_packet = (
            ROOT / "docs/CUSTOMER_DATA_OPERATIONS_PRIVACY_SCOPE_PACKET.md"
        ).read_text(encoding="utf-8")
        data_quality_packet = (
            ROOT / "docs/DATA_QUALITY_PRIVACY_SCOPE_PACKET.md"
        ).read_text(encoding="utf-8")

        identity_controls = (
            "MAX_PRIVACY_ALIAS_HOPS = 64",
            "MAX_PRIVACY_ALIAS_NODES = 4_096",
            "MAX_PRIVACY_ACTIVE_REDIRECT_EDGES = 4_095",
            "MAX_PRIVACY_RELATIONSHIP_CANDIDATES = 16_384",
            "MAX_PRIVACY_CANDIDATE_RECORDS_REHYDRATED = 8_192",
            "MAX_PRIVACY_MERGE_RECORDS_REHYDRATED = 8_192",
            "MAX_PRIVACY_OWNER_RECORDS_SCANNED = 16_384",
            "Provenance-only fallback discovery",
            "page_size + 1",
            "terminal completeness",
        )
        for control in identity_controls:
            self.assertIn(control, identity_packet)

        customer_data_controls = (
            "MAX_PRIVACY_IMPORT_ROWS_SCANNED = 16_384",
            "MAX_PRIVACY_EXPORT_SELECTION_ITEMS_SCANNED = 16_384",
            "MAX_PRIVACY_ASSOCIATED_EXPORT_RECORDS_REHYDRATED = 32_768",
            "MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS = 32_768",
            "MAX_PRIVACY_OWNER_RECORDS_SCANNED = 65_536",
            "customer_data.import_row",
            "customer_data.export_selection_item",
            "customer_data.export_execution_stage",
            "customer_data.export_execution_outcome",
            "multi-subject container",
            "page_size + 1",
            "terminal completeness",
            "REPEATABLE READ, READ ONLY",
            "reference-only",
            "no query-side changes",
        )
        for control in customer_data_controls:
            self.assertIn(control, customer_data_packet)

        data_quality_controls = (
            "data_quality.party_evaluation_job",
            "data_quality.party_evaluation_input",
            "data_quality.rule_outcome",
            "data_quality.finding",
            "data_quality.finding_observation",
            "data_quality.party_completeness_result",
            "data_quality.remediation_attempt",
            "data_quality.party_rule_set_version",
            "data_quality.party_completeness_profile_version",
            "MAX_PRIVACY_EVALUATION_JOBS_SCANNED = 8_192",
            "MAX_PRIVACY_RULE_OUTCOMES_SCANNED = 32_768",
            "MAX_PRIVACY_ASSOCIATION_RECORDS_REHYDRATED = 65_536",
            "There is no provenance-only fallback discovery family",
            "REPEATABLE READ, READ ONLY",
            "record_id ASC",
            "page_size + 1",
            "reference-only",
            "no-write proof",
            "Accepted historical contract",
        )
        for control in data_quality_controls:
            self.assertIn(control, data_quality_packet)

    def test_customer_enrichment_historical_contract_and_typed_relationship_are_frozen(self) -> None:
        packet = (ROOT / "docs/CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_PACKET.md").read_text(
            encoding="utf-8"
        )
        postgres = (
            ROOT / "crates/crm-customer-enrichment-privacy-scope-adapter/src/postgres.rs"
        ).read_text(encoding="utf-8")
        workflow = (
            ROOT / ".github/workflows/customer-enrichment-privacy-scope.yml"
        ).read_text(encoding="utf-8")

        required_families = (
            "customer_enrichment.request",
            "customer_enrichment.provider_response_receipt",
            "customer_enrichment.provider_response_conflict",
            "customer_enrichment.suggestion",
            "customer_enrichment.review_decision",
            "customer_enrichment.application_attempt",
            "customer_enrichment.provider_usage_entry",
        )
        for family in required_families:
            self.assertIn(family, packet)

        required_contract = (
            "customer_enrichment.provider_profile_version",
            "customer_enrichment.mapping_version",
            "shared across many requests",
            "must not be emitted",
            "customer_enrichment.request.party",
            "source record type: `parties.party`",
            "target record type: `customer_enrichment.request`",
            "request is the only subject-discovery root",
            "There is no payload-only or provenance-only fallback discovery family",
            "one exact authoritative `customer_enrichment.request.party` relationship",
            "REPEATABLE READ, READ ONLY",
            "page_size + 1",
            "record_id ASC",
            "crm.relationships (tenant_id, relationship_type, source_record_type, source_record_id, target_record_type, target_record_id)",
            "crm.records (tenant_id, record_type, record_id)",
            "reference-only",
            "no-write",
            "Contract-only/non-runtime",
            "Accepted historical contract",
            "28 of 28 permanent workflows succeeded",
            "Scope discovery and immutable snapshot",
        )
        for control in required_contract:
            self.assertIn(control, packet)

        required_bounds = (
            "MAX_PRIVACY_ALIAS_HOPS = 64",
            "MAX_PRIVACY_ALIAS_NODES = 4_096",
            "MAX_PRIVACY_ACTIVE_REDIRECT_EDGES = 4_095",
            "MAX_PRIVACY_REQUEST_RELATIONSHIPS_SCANNED = 16_384",
            "MAX_PRIVACY_REQUEST_RECORDS_REHYDRATED = 16_384",
            "MAX_PRIVACY_RESPONSE_RECEIPTS_SCANNED = 32_768",
            "MAX_PRIVACY_RESPONSE_CONFLICTS_SCANNED = 16_384",
            "MAX_PRIVACY_SUGGESTIONS_SCANNED = 65_536",
            "MAX_PRIVACY_REVIEW_DECISIONS_SCANNED = 65_536",
            "MAX_PRIVACY_APPLICATION_ATTEMPTS_SCANNED = 65_536",
            "MAX_PRIVACY_PROVIDER_USAGE_ENTRIES_SCANNED = 65_536",
            "MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED = 8_192",
            "MAX_PRIVACY_ASSOCIATION_RECORDS_REHYDRATED = 131_072",
            "MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS = 16_384",
            "MAX_PRIVACY_OWNER_RECORDS_SCANNED = 131_072",
            "PRIVACY_RELATIONSHIP_SCAN_BATCH_SIZE = 512",
            "PRIVACY_OWNER_SCAN_BATCH_SIZE = 512",
        )
        for bound in required_bounds:
            self.assertIn(bound, packet)

        for typed_field in (
            "owner_module_id",
            "schema_id",
            "schema_version",
            "descriptor_hash",
            "data_class",
            "payload_encoding",
            "maximum_payload_size",
            "retention_policy_id",
            "payload_bytes",
        ):
            self.assertIn(typed_field, postgres)
        self.assertNotIn("relationships.attributes", postgres)
        self.assertNotIn("attributes_json", postgres)
        self.assertIn("canonical typed link contract", postgres)

        self.assertIn("name: Customer Enrichment Privacy Scope CI", workflow)
        self.assertIn("push:", workflow)
        self.assertIn("pull_request:", workflow)
        self.assertIn("complete rollback", packet)
        self.assertIn("absence of schema `crm`", packet)
        self.assertIn("repeated PostgreSQL acceptance", packet)


if __name__ == "__main__":
    unittest.main()
