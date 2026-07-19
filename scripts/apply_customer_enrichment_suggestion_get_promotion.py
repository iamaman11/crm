from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace(path: str, old: str, new: str) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    if new in text:
        return
    if old not in text:
        raise RuntimeError(f"expected source fragment missing: {path}")
    target.write_text(text.replace(old, new), encoding="utf-8")


replace(
    "crates/crm-customer-enrichment-suggestion-query-adapter/src/lib.rs",
    "    pub(crate) cursor_codec: crm_query_runtime::CursorCodec,",
    "    pub(crate) cursor_codec: Option<crm_query_runtime::CursorCodec>,",
)
replace(
    "crates/crm-customer-enrichment-suggestion-query-adapter/src/lib.rs",
    """        Self {\n            store,\n            visibility,\n            cursor_codec,\n        }\n    }\n\n    async fn execute_get""",
    """        Self {\n            store,\n            visibility,\n            cursor_codec: Some(cursor_codec),\n        }\n    }\n\n    pub fn new_get_only(\n        store: PostgresDataStore,\n        visibility: Arc<dyn QueryVisibilityAuthorizer>,\n    ) -> Self {\n        Self {\n            store,\n            visibility,\n            cursor_codec: None,\n        }\n    }\n\n    pub(crate) fn cursor_codec(&self) -> Result<&crm_query_runtime::CursorCodec, SdkError> {\n        self.cursor_codec.as_ref().ok_or_else(|| {\n            query_configuration_invalid(\"suggestion list cursor codec is not configured\")\n        })\n    }\n\n    async fn execute_get""",
)
replace(
    "crates/crm-customer-enrichment-suggestion-query-adapter/src/lib.rs",
    '.field("cursor_codec", &self.cursor_codec)',
    '.field("cursor_codec_configured", &self.cursor_codec.is_some())',
)
replace(
    "crates/crm-customer-enrichment-suggestion-query-adapter/src/list/cursor.rs",
    ".cursor_codec\n        .decode(token, binding)",
    ".cursor_codec()?\n        .decode(token, binding)",
)
replace(
    "crates/crm-customer-enrichment-suggestion-query-adapter/src/list/cursor.rs",
    ".cursor_codec\n            .encode(",
    ".cursor_codec()?\n            .encode(",
)

replace(
    "crates/crm-application-runtime/src/native_composition.rs",
    """use crm_customer_enrichment_request_list_query_adapter::{\n    CustomerEnrichmentRequestListQueryAdapter,\n    query_capability_definition as customer_enrichment_request_list_query_capability_definition,\n};""",
    """use crm_customer_enrichment_request_list_query_adapter::{\n    CustomerEnrichmentRequestListQueryAdapter,\n    query_capability_definition as customer_enrichment_request_list_query_capability_definition,\n};\nuse crm_customer_enrichment_suggestion_query_adapter::{\n    CustomerEnrichmentSuggestionQueryAdapter, get_suggestion_capability_definition,\n};""",
)
replace(
    "crates/crm-application-runtime/src/native_composition.rs",
    """    definitions.push(customer_enrichment_request_list_query_capability_definition()?);\n    definitions.extend(data_quality_query_capability_definitions()?);""",
    """    definitions.push(customer_enrichment_request_list_query_capability_definition()?);\n    definitions.push(get_suggestion_capability_definition()?);\n    definitions.extend(data_quality_query_capability_definitions()?);""",
)
replace(
    "crates/crm-application-runtime/src/native_composition.rs",
    """    add_activated_queries(\n        &mut contributions,\n        vec![customer_enrichment_request_list_query_capability_definition()?],\n        customer_enrichment_request_list_queries,\n        activation.clone(),\n    )?;\n\n    let data_quality_queries = Arc::new(DataQualityQueryAdapter::new(""",
    """    add_activated_queries(\n        &mut contributions,\n        vec![customer_enrichment_request_list_query_capability_definition()?],\n        customer_enrichment_request_list_queries,\n        activation.clone(),\n    )?;\n\n    let customer_enrichment_suggestion_get_queries =\n        Arc::new(CustomerEnrichmentSuggestionQueryAdapter::new_get_only(\n            store.clone(),\n            visibility_authorizer.clone(),\n        ));\n    add_activated_queries(\n        &mut contributions,\n        vec![get_suggestion_capability_definition()?],\n        customer_enrichment_suggestion_get_queries,\n        activation.clone(),\n    )?;\n\n    let data_quality_queries = Arc::new(DataQualityQueryAdapter::new(""",
)

replace(
    "crates/crm-application-runtime/src/bootstrap_visibility.rs",
    "use crm_customer_enrichment_request_list_query_adapter::LIST_ENRICHMENT_REQUESTS_CAPABILITY;",
    """use crm_customer_enrichment_request_list_query_adapter::LIST_ENRICHMENT_REQUESTS_CAPABILITY;\nuse crm_customer_enrichment_suggestion_query_adapter::GET_SUGGESTION_CAPABILITY;""",
)
replace(
    "crates/crm-application-runtime/src/bootstrap_visibility.rs",
    'const CUSTOMER_ENRICHMENT_REQUEST_RECORD_TYPE: &str = "customer_enrichment.request";',
    """const CUSTOMER_ENRICHMENT_REQUEST_RECORD_TYPE: &str = \"customer_enrichment.request\";\nconst CUSTOMER_ENRICHMENT_SUGGESTION_RECORD_TYPE: &str = \"customer_enrichment.suggestion\";\nconst CUSTOMER_ENRICHMENT_REVIEW_DECISION_RECORD_TYPE: &str =\n    \"customer_enrichment.review_decision\";""",
)
replace(
    "crates/crm-application-runtime/src/bootstrap_visibility.rs",
    """        _ => Vec::new(),\n    }\n}\n\nfn customer_data_import_job_fields()""",
    """        GET_SUGGESTION_CAPABILITY => vec![\n            resource(\n                CUSTOMER_ENRICHMENT_MODULE_ID,\n                PARTY_RECORD_TYPE,\n                BTreeSet::new(),\n            ),\n            resource(\n                CUSTOMER_ENRICHMENT_MODULE_ID,\n                CUSTOMER_ENRICHMENT_SUGGESTION_RECORD_TYPE,\n                customer_enrichment_suggestion_fields(),\n            ),\n            resource(\n                CUSTOMER_ENRICHMENT_MODULE_ID,\n                CUSTOMER_ENRICHMENT_REVIEW_DECISION_RECORD_TYPE,\n                customer_enrichment_review_decision_fields(),\n            ),\n        ],\n        _ => Vec::new(),\n    }\n}\n\nfn customer_data_import_job_fields()""",
)
replace(
    "crates/crm-application-runtime/src/bootstrap_visibility.rs",
    "\nfn customer_360_party_fields()",
    """\nfn customer_enrichment_suggestion_fields() -> BTreeSet<String> {\n    fields([\n        \"enrichment_request_ref\",\n        \"provider_response_receipt_ref\",\n        \"provider_profile_version_ref\",\n        \"mapping_version_ref\",\n        \"target\",\n        \"proposed_value\",\n        \"proposed_value_digest\",\n        \"observed_at_unix_ms\",\n        \"retrieved_at_unix_ms\",\n        \"effective_at_unix_ms\",\n        \"fresh_until_unix_ms\",\n        \"expires_at_unix_ms\",\n        \"confidence_basis_points\",\n        \"policy_evidence\",\n        \"evidence_references\",\n        \"lifecycle_status\",\n        \"superseded_by_suggestion_ref\",\n    ])\n}\n\nfn customer_enrichment_review_decision_fields() -> BTreeSet<String> {\n    fields([\n        \"suggestion_ref\",\n        \"target_party_resource_version\",\n        \"proposed_value_digest\",\n        \"reviewed_by_actor_id\",\n        \"kind\",\n        \"policy_version\",\n        \"safe_reason_code\",\n        \"approval_evidence_reference\",\n        \"decided_at_unix_ms\",\n        \"expires_at_unix_ms\",\n    ])\n}\n\nfn customer_360_party_fields()""",
)
replace(
    "crates/crm-application-runtime/src/bootstrap_visibility.rs",
    """        }\n    }\n\n    #[test]\n    fn registry_rejects_undeclared_query_owner()""",
    """        }\n\n        let suggestion = registry\n            .resources_for(&definition(\n                CUSTOMER_ENRICHMENT_MODULE_ID,\n                GET_SUGGESTION_CAPABILITY,\n            ))\n            .unwrap();\n        assert_eq!(suggestion.len(), 3);\n        assert_eq!(\n            suggestion[0],\n            resource(\n                CUSTOMER_ENRICHMENT_MODULE_ID,\n                PARTY_RECORD_TYPE,\n                BTreeSet::new(),\n            )\n        );\n        assert_eq!(\n            suggestion[1],\n            resource(\n                CUSTOMER_ENRICHMENT_MODULE_ID,\n                CUSTOMER_ENRICHMENT_SUGGESTION_RECORD_TYPE,\n                customer_enrichment_suggestion_fields(),\n            )\n        );\n        assert_eq!(\n            suggestion[2],\n            resource(\n                CUSTOMER_ENRICHMENT_MODULE_ID,\n                CUSTOMER_ENRICHMENT_REVIEW_DECISION_RECORD_TYPE,\n                customer_enrichment_review_decision_fields(),\n            )\n        );\n    }\n\n    #[test]\n    fn registry_rejects_undeclared_query_owner()""",
)

replace(
    "crates/crm-application-runtime/tests/customer_enrichment_registration_contract.rs",
    "use crm_customer_enrichment_request_list_query_adapter::LIST_ENRICHMENT_REQUESTS_CAPABILITY;",
    """use crm_customer_enrichment_request_list_query_adapter::LIST_ENRICHMENT_REQUESTS_CAPABILITY;\nuse crm_customer_enrichment_suggestion_query_adapter::GET_SUGGESTION_CAPABILITY;""",
)
replace(
    "crates/crm-application-runtime/tests/customer_enrichment_registration_contract.rs",
    """    assert_eq!(enrichment_definitions.len(), 4);\n    assert_eq!(\n        enrichment_definitions\n            .iter()\n            .map(|definition| definition.capability_id.as_str())\n            .collect::<BTreeSet<_>>(),\n        [\n            GET_PROVIDER_PROFILE_CAPABILITY,""",
    """    assert_eq!(enrichment_definitions.len(), 5);\n    assert_eq!(\n        enrichment_definitions\n            .iter()\n            .map(|definition| definition.capability_id.as_str())\n            .collect::<BTreeSet<_>>(),\n        [\n            GET_PROVIDER_PROFILE_CAPABILITY,""",
)
replace(
    "crates/crm-application-runtime/tests/customer_enrichment_registration_contract.rs",
    """            GET_ENRICHMENT_REQUEST_CAPABILITY,\n            LIST_ENRICHMENT_REQUESTS_CAPABILITY,\n        ]""",
    """            GET_ENRICHMENT_REQUEST_CAPABILITY,\n            LIST_ENRICHMENT_REQUESTS_CAPABILITY,\n            GET_SUGGESTION_CAPABILITY,\n        ]""",
)

replace(
    "crates/crm-application-runtime/Cargo.toml",
    'crm-customer-enrichment-request-list-query-adapter = { path = "../crm-customer-enrichment-request-list-query-adapter" }',
    """crm-customer-enrichment-request-list-query-adapter = { path = \"../crm-customer-enrichment-request-list-query-adapter\" }\ncrm-customer-enrichment-suggestion-query-adapter = { path = \"../crm-customer-enrichment-suggestion-query-adapter\" }""",
)
replace(
    "crates/crm-application-runtime/Cargo.toml",
    "\n[build-dependencies]\n",
    """\n[dev-dependencies]\ncrm-customer-enrichment = { path = \"../../modules/crm-customer-enrichment\" }\ncrm-customer-enrichment-review-adapter = { path = \"../crm-customer-enrichment-review-adapter\" }\nsqlx = { version = \"0.9\", default-features = false, features = [\"postgres\", \"runtime-tokio\"] }\n\n[build-dependencies]\n""",
)

process_test = ROOT / "crates/crm-application-runtime/tests/postgres_customer_enrichment_suggestion_get.rs"
if not process_test.exists():
    process_test.write_text(
        (ROOT / "scripts/customer_enrichment_suggestion_get_process.rs").read_text(encoding="utf-8"),
        encoding="utf-8",
    )

replace(
    "scripts/run_customer_enrichment_review_process.sh",
    """bash scripts/prepare_customer_enrichment_worker_process_database.sh\ncargo test -p crm-customer-enrichment-application-composition --features postgres-integration --test postgres_application_orchestration_process -- --nocapture\n""",
    """bash scripts/prepare_customer_enrichment_worker_process_database.sh\ncargo test -p crm-customer-enrichment-application-composition --features postgres-integration --test postgres_application_orchestration_process -- --nocapture\n\nbash scripts/prepare_customer_enrichment_worker_process_database.sh\ncargo test -p crm-application-runtime --test postgres_customer_enrichment_suggestion_get -- --nocapture\n""",
)

classification_path = ROOT / "contracts/production-route-classifications.json"
classifications = json.loads(classification_path.read_text(encoding="utf-8"))
classifications["non_runtime_contract_routes"] = [
    entry
    for entry in classifications["non_runtime_contract_routes"]
    if not (
        entry["owner_module_id"] == "crm.customer-enrichment"
        and entry["id"] == "customer_enrichment.suggestion.get"
        and entry["version"] == "1.0.0"
    )
]
classification_path.write_text(json.dumps(classifications, indent=2) + "\n", encoding="utf-8")

promotion_path = ROOT / "contracts/customer-enrichment-production-promotion.json"
promotion = json.loads(promotion_path.read_text(encoding="utf-8"))
query_coordinate = "customer_enrichment.suggestion.get@1.0.0"
if query_coordinate not in promotion["current_runtime_inventory"]["queries"]:
    promotion["current_runtime_inventory"]["queries"].append(query_coordinate)
for stage in promotion["promotion_stages"]:
    stage["coordinates"] = [
        entry
        for entry in stage["coordinates"]
        if not (
            entry["id"] == "customer_enrichment.suggestion.get"
            and entry["version"] == "1.0.0"
        )
    ]
promotion_path.write_text(json.dumps(promotion, indent=2) + "\n", encoding="utf-8")

replace(
    "tests/test_production_route_classifications.py",
    '                        "customer_enrichment.suggestion.get",\n',
    "",
)
replace(
    "tests/test_production_route_classifications.py",
    '            "customer_enrichment.request.list",\n',
    '            "customer_enrichment.request.list",\n            "customer_enrichment.suggestion.get",\n',
)
replace(
    "tests/test_customer_enrichment_production_promotion.py",
    '    "customer_enrichment.request.list@1.0.0",\n}',
    '    "customer_enrichment.request.list@1.0.0",\n    "customer_enrichment.suggestion.get@1.0.0",\n}',
)
replace(
    "tests/test_customer_enrichment_production_promotion.py",
    '    "customer_enrichment.suggestion.get@1.0.0": (1, "query", "public"),\n',
    "",
)
replace(
    "tests/test_customer_enrichment_production_promotion.py",
    "        self.assertEqual(len(mutations | queries), 8)",
    "        self.assertEqual(len(mutations | queries), 9)",
)

replace(
    "modules/crm-customer-enrichment/ACCEPTANCE.md",
    "Current production route inventory: **4 mutations + 4 permission-aware queries**; the remaining 9 published coordinates stay individually non-runtime.",
    "Current production route inventory: **4 mutations + 5 permission-aware queries**; the remaining 8 published coordinates stay individually non-runtime.",
)
replace(
    "modules/crm-customer-enrichment/ACCEPTANCE.md",
    """- [x] Freeze a machine-readable production-promotion contract for the exact 4-mutation + 4-query runtime inventory and all nine non-runtime coordinates. CI validates unique coordinates, deterministic stages and dependencies, route kind/exposure, module-owned activation gating, disable/uninstall, cross-tenant and one-exact-head 17-workflow requirements before any coordinate can be promoted.\n""",
    """- [x] Freeze a machine-readable production-promotion contract for the exact runtime inventory and all remaining non-runtime coordinates. CI validates unique coordinates, deterministic stages and dependencies, route kind/exposure, module-owned activation gating, disable/uninstall, cross-tenant and one-exact-head 17-workflow requirements before any coordinate can be promoted.\n- [x] Promote activation-gated `customer_enrichment.suggestion.get@1.0.0` through the exact production composition with get-only adapter construction, Party-first hiding, suggestion/review visibility, field redaction and a fresh-PostgreSQL HTTP process proving success, live authorization denial, cross-tenant rejection, suspended/uninstalling shutdown and side-effect-free reads.\n""",
)
replace(
    "modules/crm-customer-enrichment/production/CONTRIBUTION.md",
    "current 4-mutation + 4-query runtime inventory, all nine individually non-runtime coordinates",
    "current 4-mutation + 5-query runtime inventory, all eight individually non-runtime coordinates",
)
