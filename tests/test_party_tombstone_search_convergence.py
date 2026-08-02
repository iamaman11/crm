import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


class PartyTombstoneSearchConvergenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.packet = json.loads((ROOT / "repository-packet.json").read_text(encoding="utf-8"))
        cls.search_composition = (
            ROOT / "crates/crm-global-search-composition/src/lib.rs"
        ).read_text(encoding="utf-8")
        cls.search_store = (ROOT / "crates/crm-core-data/src/search_store.rs").read_text(
            encoding="utf-8"
        )
        cls.owner_executor = (
            ROOT / "crates/crm-core-data/src/privacy_owner_action.rs"
        ).read_text(encoding="utf-8")
        cls.search_manifest = (
            ROOT / "crates/crm-global-search-composition/Cargo.toml"
        ).read_text(encoding="utf-8")

    def test_packet_is_exactly_bounded_to_the_first_step_15_slice(self) -> None:
        self.assertEqual(
            self.packet["packet_id"],
            "repository-step-15-party-tombstone-search-convergence",
        )
        self.assertEqual(
            self.packet["baseline"]["sha"],
            "5bc885c6a311cbe95eb6e5ba1a85d10aed400650",
        )
        self.assertEqual(
            set(self.packet["allowed_paths"]),
            {
                "crates/crm-core-data/src/search_store.rs",
                "crates/crm-global-search-composition/src/lib.rs",
                "docs/ACTIVE_PACKET.md",
                "repository-packet.json",
                "tests/test_party_tombstone_search_convergence.py",
            },
        )
        objective = self.packet["objective"]
        self.assertIn("does not complete Step 15", objective)
        self.assertTrue(
            any(
                "Customer 360" in criterion and "remain next" in criterion
                for criterion in self.packet["acceptance"]
            )
        )

    def test_fresh_generation_replays_the_existing_party_owner_action_event(self) -> None:
        source = self.search_composition
        self.assertIn(
            'pub const INITIAL_GLOBAL_SEARCH_GENERATION_ID: &str = "g3";', source
        )
        self.assertIn(
            'const PARTY_PRIVACY_ACTION_COMPLETED: &str = '
            '"parties.privacy.action.apply.completed";',
            source,
        )
        self.assertIn("PARTY_PRIVACY_ACTION_COMPLETED,", source)
        self.assertIn("assert_eq!(event_types.len(), 7);", source)
        self.assertIn('assert_eq!(generation.projection_id.as_str(), "search.global.g3")', source)

    def test_party_owner_action_event_is_strictly_bound_without_new_dependencies(self) -> None:
        source = self.search_composition
        required_bindings = (
            "delivery.validate().is_err()",
            "delivery.source_module_id.as_str() != PARTIES_MODULE_ID",
            "delivery.aggregate.record_type.as_str() != PARTY_RESOURCE_TYPE",
            "delivery.payload.schema_id.as_str() != OWNER_ACTION_EVENT_SCHEMA",
            "delivery.payload.descriptor_hash != OWNER_ACTION_EVENT_DESCRIPTOR_HASH",
            "delivery.payload.data_class != DataClass::Restricted",
            "delivery.payload.encoding != PayloadEncoding::Json",
            'require_canonical_json_field(bytes, "tenant_id", delivery.tenant_id.as_str())',
            'require_canonical_json_field(bytes, "owner_module_id", PARTIES_MODULE_ID)',
            '"owner_capability_id"',
            '"owner_capability_version"',
            '"resource_type"',
            '"resource_id"',
            'canonical_json_string_field(bytes, "resource_version")',
            "previous_version.checked_add(1)",
            'canonical_json_string_field(bytes, "action_code")',
        )
        for binding in required_bindings:
            with self.subTest(binding=binding):
                self.assertIn(binding, source)

        self.assertNotIn("crm-customer-privacy", self.search_manifest)
        self.assertNotIn("serde_json", self.search_manifest)

    def test_privacy_action_replaces_personal_search_text_with_non_active_marker(self) -> None:
        source = self.search_composition
        start = source.index("fn party_privacy_action_document(")
        end = source.index("fn validate_party_privacy_action(", start)
        function = source[start:end]

        self.assertIn("PARTY_SEARCH_LIFECYCLE_FIELD", function)
        self.assertIn('"suppressed".to_owned()', function)
        self.assertNotIn("display_name", function)
        self.assertNotIn("kind", function)
        self.assertIn('"delete" => Ok(PARTY_SEARCH_ERASED)', source)
        self.assertIn('"anonymize" => Ok(PARTY_SEARCH_MINIMIZED)', source)
        self.assertIn(
            "PARTY_SEARCH_LIFECYCLE_FIELD.to_owned(),\n                PARTY_SEARCH_ACTIVE.to_owned()",
            source,
        )

    def test_postgres_excludes_non_active_documents_before_text_matching(self) -> None:
        sql_start = self.search_store.index("WITH ranked AS")
        sql_end = self.search_store.index('"#,', sql_start)
        query = self.search_store[sql_start:sql_end]
        lifecycle_filter = query.index("privacy_lifecycle")
        first_match_evaluation = query.index("to_tsvector")

        self.assertLess(lifecycle_filter, first_match_evaluation)
        self.assertIn(
            "COALESCE(\n                      document -> 'display_fields' ->> 'privacy_lifecycle',\n"
            "                      'active'\n                    ) = 'active'",
            query,
        )

    def test_authoritative_party_delete_remains_a_minimized_soft_tombstone(self) -> None:
        source = self.owner_executor
        self.assertIn("PrivacyOwnerRecordAction::Delete", source)
        self.assertIn(
            "deleted_at = CASE WHEN $15::boolean THEN clock_timestamp() ELSE NULL END",
            source,
        )
        self.assertIn("payload_bytes = $11", source)
        self.assertIn("AND deleted_at IS NULL", source)
        self.assertNotIn("DELETE FROM crm.records", source)


if __name__ == "__main__":
    unittest.main()
