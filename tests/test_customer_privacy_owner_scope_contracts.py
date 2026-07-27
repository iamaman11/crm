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
            contributions.count("PrivacyScopeContributionRequestEnvelope contribution = 1;"),
            9,
        )
        self.assertEqual(
            contributions.count("PrivacyScopeContributionResponseEnvelope contribution = 1;"),
            9,
        )
        self.assertNotIn("bytes resource_payload", contributions)
        self.assertNotIn("string resource_value", contributions)
        self.assertIn("PrivacyScopeResourceReference", contributions)
        self.assertIn("bytes page_digest_sha256", contributions)
        self.assertIn("bytes cursor_digest_sha256", contributions)
        self.assertIn("CUSTOMER_DATA_CLASS_RESTRICTED = 9;", types)

    def test_status_sources_freeze_eight_accepted_owners_and_customer_enrichment_next(self) -> None:
        project_status = (ROOT / "docs/PROJECT_STATUS.md").read_text(encoding="utf-8")
        module_catalog = (ROOT / "docs/MODULE_CATALOG.md").read_text(encoding="utf-8")
        roadmap = (ROOT / "docs/IMPLEMENTATION_ROADMAP.md").read_text(
            encoding="utf-8"
        )
        phase_plan = (ROOT / "docs/PHASE8_DELIVERY_PLAN.md").read_text(
            encoding="utf-8"
        )
        identity_packet = (
            ROOT / "docs/IDENTITY_RESOLUTION_PRIVACY_SCOPE_PACKET.md"
        ).read_text(encoding="utf-8")
        customer_data_packet = (
            ROOT / "docs/CUSTOMER_DATA_OPERATIONS_PRIVACY_SCOPE_PACKET.md"
        ).read_text(encoding="utf-8")
        data_quality_packet = (
            ROOT / "docs/DATA_QUALITY_PRIVACY_SCOPE_PACKET.md"
        ).read_text(encoding="utf-8")

        for document in (project_status, module_catalog, roadmap, phase_plan):
            self.assertIn("Customer Data Operations", document)
            self.assertIn("Data Quality", document)
            self.assertIn("Customer Enrichment", document)

        self.assertIn("Eight authoritative owner implementations are accepted", project_status)
        self.assertIn("Eight authoritative implementations are accepted", module_catalog)
        self.assertIn("Next bounded packet — Customer Enrichment", project_status)
        self.assertIn("Customer Enrichment is the next bounded contract-only owner", module_catalog)
        self.assertIn("Next bounded owner slice — Customer Enrichment", roadmap)
        self.assertIn("Next bounded owner packet: Customer Enrichment", phase_plan)

        for document in (project_status, module_catalog, roadmap, phase_plan, customer_data_packet):
            self.assertIn("PR #188", document)
            self.assertIn("07f34786e82fdfa78d263790e9f50541529006f8", document)
            self.assertIn("089be72fa3010b4aa15aff7f9ea55fd86290f8fc", document)

        self.assertIn("26 of 26 permanent workflows", project_status)
        self.assertIn("26 of 26 permanent workflows", module_catalog)
        self.assertIn("26 of 26 permanent workflows", roadmap)
        self.assertIn("26 of 26 permanent workflows", phase_plan)
        self.assertIn("26 of 26 permanent workflows succeeded", customer_data_packet)
        self.assertIn("Accepted historical contract", customer_data_packet)
        self.assertIn("Production discovery remains prohibited", customer_data_packet)

        for document in (project_status, module_catalog, roadmap, phase_plan, data_quality_packet):
            self.assertIn("PR #190", document)
            self.assertIn("dcfe8faebc7462b888f8fc1721cb379a40fea88a", document)
            self.assertIn("deac197c97cddc15bb9916092ca87f6e767ce1de", document)

        self.assertIn("27 of 27 permanent workflows", project_status)
        self.assertIn("27 of 27 permanent workflows", module_catalog)
        self.assertIn("27 of 27 permanent workflows", roadmap)
        self.assertIn("27 of 27 permanent workflows", phase_plan)
        self.assertIn("27 of 27 permanent workflows succeeded", data_quality_packet)
        self.assertIn("Accepted historical contract", data_quality_packet)
        self.assertIn("Production discovery remains forbidden", data_quality_packet)

        stale_claims = (
            "Six authoritative owner implementations are accepted",
            "Six authoritative implementations are accepted",
            "Seven authoritative owner implementations are accepted",
            "Seven authoritative implementations are accepted",
            "Customer Data Operations is the next bounded contract-only owner",
            "Data Quality is the next bounded contract-only owner",
            "Next bounded owner slice — Customer Data Operations",
            "Next bounded owner slice — Data Quality",
            "Next bounded owner packet: Customer Data Operations",
            "Next bounded owner packet: Data Quality",
            "Ready; implementation not started",
            "implementation code has not started",
            "Implementation state: **Not started",
            "PR #188 in progress",
            "PR #190 in progress",
        )
        for stale in stale_claims:
            self.assertNotIn(stale, project_status)
            self.assertNotIn(stale, module_catalog)
            self.assertNotIn(stale, roadmap)
            self.assertNotIn(stale, phase_plan)
            self.assertNotIn(stale, customer_data_packet)
            self.assertNotIn(stale, data_quality_packet)

        required_identity_controls = (
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
        for control in required_identity_controls:
            self.assertIn(control, identity_packet)

        required_customer_data_controls = (
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
        for control in required_customer_data_controls:
            self.assertIn(control, customer_data_packet)

    def test_data_quality_entry_packet_freezes_bounded_direct_subject_scope(self) -> None:
        packet = (ROOT / "docs/DATA_QUALITY_PRIVACY_SCOPE_PACKET.md").read_text(
            encoding="utf-8"
        )

        required_families = (
            "data_quality.party_evaluation_job",
            "data_quality.party_evaluation_input",
            "data_quality.rule_outcome",
            "data_quality.finding",
            "data_quality.finding_observation",
            "data_quality.party_completeness_result",
            "data_quality.remediation_attempt",
        )
        for family in required_families:
            self.assertIn(family, packet)

        required_excluded_definitions = (
            "data_quality.party_rule_set_version",
            "data_quality.party_completeness_profile_version",
            "shared across many Party evaluations",
            "must not be emitted",
        )
        for boundary in required_excluded_definitions:
            self.assertIn(boundary, packet)

        required_bounds = (
            "MAX_PRIVACY_EVALUATION_JOBS_SCANNED = 8_192",
            "MAX_PRIVACY_EVALUATION_INPUTS_SCANNED = 8_192",
            "MAX_PRIVACY_RULE_OUTCOMES_SCANNED = 32_768",
            "MAX_PRIVACY_FINDINGS_SCANNED = 16_384",
            "MAX_PRIVACY_FINDING_OBSERVATIONS_SCANNED = 32_768",
            "MAX_PRIVACY_COMPLETENESS_RESULTS_SCANNED = 8_192",
            "MAX_PRIVACY_REMEDIATION_ATTEMPTS_SCANNED = 8_192",
            "MAX_PRIVACY_DEFINITION_RECORDS_REHYDRATED = 8_192",
            "MAX_PRIVACY_ASSOCIATION_RECORDS_REHYDRATED = 65_536",
            "MAX_PRIVACY_CANONICAL_PARTY_RESOLUTIONS = 65_536",
            "MAX_PRIVACY_OWNER_RECORDS_SCANNED = 65_536",
            "PRIVACY_OWNER_SCAN_BATCH_SIZE = 512",
        )
        for bound in required_bounds:
            self.assertIn(bound, packet)

        required_controls = (
            "There is no provenance-only fallback discovery family",
            "REPEATABLE READ, READ ONLY",
            "page_size + 1",
            "record_id ASC",
            "crm.records (tenant_id, record_type, record_id)",
            "create no Data Quality rows in `crm.relationships`",
            "reference-only",
            "no-write proof",
            "Contract-only/non-runtime",
            "Accepted historical contract",
            "27 of 27 permanent workflows succeeded",
            "dcfe8faebc7462b888f8fc1721cb379a40fea88a",
            "deac197c97cddc15bb9916092ca87f6e767ce1de",
            "Production discovery remains forbidden",
        )
        for control in required_controls:
            self.assertIn(control, packet)

        forbidden_shortcuts = (
            "JSON or byte-payload expression indexes",
            "privacy-only projection table",
            "synthetic `crm.relationships` rows",
            "ungoverned Party-to-quality reverse index",
            "public HTTP or gRPC route",
            "Customer Privacy worker",
            "generic privacy runtime",
            "unbounded tenant scan",
            "selective JSON parsing",
            "runtime promotion",
        )
        for shortcut in forbidden_shortcuts:
            self.assertIn(shortcut, packet)

    def test_customer_enrichment_entry_packet_freezes_relationship_rooted_scope(self) -> None:
        packet = (ROOT / "docs/CUSTOMER_ENRICHMENT_PRIVACY_SCOPE_PACKET.md").read_text(
            encoding="utf-8"
        )

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

        required_excluded_definitions = (
            "customer_enrichment.provider_profile_version",
            "customer_enrichment.mapping_version",
            "shared across many requests",
            "must not be emitted",
        )
        for boundary in required_excluded_definitions:
            self.assertIn(boundary, packet)

        required_relationship_controls = (
            "customer_enrichment.request.party",
            "source record type: `parties.party`",
            "target record type: `customer_enrichment.request`",
            "request is the only subject-discovery root",
            "There is no payload-only or provenance-only fallback discovery family",
            "exactly one authoritative request/Party relationship",
        )
        for control in required_relationship_controls:
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

        required_controls = (
            "customer_enrichment.privacy.scope.contribute@1.0.0",
            "REPEATABLE READ, READ ONLY",
            "page_size + 1",
            "record_id ASC",
            "crm.relationships (tenant_id, relationship_type, source_record_type, source_record_id, target_record_type, target_record_id)",
            "crm.records (tenant_id, record_type, record_id)",
            "reference-only",
            "no-write proof",
            "Contract-only/non-runtime",
            "implementation not started",
            "ninth and final owner",
            "Production discovery remains forbidden",
        )
        for control in required_controls:
            self.assertIn(control, packet)

        forbidden_shortcuts = (
            "JSON or byte-payload expression indexes",
            "privacy-only projection table",
            "duplicate request/Party relationships",
            "synthetic descendant-to-Party relationships",
            "ungoverned Party-to-enrichment reverse index",
            "public HTTP or gRPC route",
            "Customer Privacy worker",
            "generic privacy runtime",
            "full unbounded tenant scan",
            "selective JSON parsing",
            "payload-only or provenance-only fallback discovery",
            "runtime promotion",
        )
        for shortcut in forbidden_shortcuts:
            self.assertIn(shortcut, packet)


if __name__ == "__main__":
    unittest.main()
