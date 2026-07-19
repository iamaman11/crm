import re
from pathlib import Path


path = Path("crates/crm-application-runtime/src/bootstrap_visibility.rs")
text = path.read_text(encoding="utf-8")

constants = """const CUSTOMER_ENRICHMENT_REQUEST_RECORD_TYPE: &str = "customer_enrichment.request";
const CUSTOMER_ENRICHMENT_SUGGESTION_RECORD_TYPE: &str = "customer_enrichment.suggestion";
const CUSTOMER_ENRICHMENT_REVIEW_DECISION_RECORD_TYPE: &str =
    "customer_enrichment.review_decision";

"""
text, constant_replacements = re.subn(
    r'const CUSTOMER_ENRICHMENT_REQUEST_RECORD_TYPE: &str = "customer_enrichment\.request";\n(?:const CUSTOMER_ENRICHMENT_SUGGESTION_RECORD_TYPE:.*?\n|const CUSTOMER_ENRICHMENT_REVIEW_DECISION_RECORD_TYPE:.*?\n|    "customer_enrichment\.review_decision";\n)*\n',
    constants,
    text,
    count=1,
    flags=re.DOTALL,
)
if constant_replacements != 1:
    raise RuntimeError("expected Customer Enrichment visibility constants are missing")

visibility = """fn customer_enrichment_visibility(
    definition: &CapabilityDefinition,
) -> Vec<BootstrapVisibilityResource> {
    match definition.capability_id.as_str() {
        GET_PROVIDER_PROFILE_CAPABILITY | GET_MAPPING_CAPABILITY => vec![resource(
            CUSTOMER_ENRICHMENT_MODULE_ID,
            CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_RECORD_TYPE,
            fields(["definition"]),
        )],
        GET_ENRICHMENT_REQUEST_CAPABILITY | LIST_ENRICHMENT_REQUESTS_CAPABILITY => vec![
            // Live visibility keys are scoped by the query owner. These routes use Party visibility
            // only as a resource-existence gate and disclose no Party fields.
            resource(
                CUSTOMER_ENRICHMENT_MODULE_ID,
                PARTY_RECORD_TYPE,
                BTreeSet::new(),
            ),
            resource(
                CUSTOMER_ENRICHMENT_MODULE_ID,
                CUSTOMER_ENRICHMENT_REQUEST_RECORD_TYPE,
                customer_enrichment_request_fields(),
            ),
        ],
        GET_SUGGESTION_CAPABILITY => vec![
            resource(
                CUSTOMER_ENRICHMENT_MODULE_ID,
                PARTY_RECORD_TYPE,
                BTreeSet::new(),
            ),
            resource(
                CUSTOMER_ENRICHMENT_MODULE_ID,
                CUSTOMER_ENRICHMENT_SUGGESTION_RECORD_TYPE,
                customer_enrichment_suggestion_fields(),
            ),
            resource(
                CUSTOMER_ENRICHMENT_MODULE_ID,
                CUSTOMER_ENRICHMENT_REVIEW_DECISION_RECORD_TYPE,
                customer_enrichment_review_decision_fields(),
            ),
        ],
        _ => Vec::new(),
    }
}

fn data_quality_visibility"""
text, visibility_replacements = re.subn(
    r"fn customer_enrichment_visibility\(.*?\n\}\n\nfn data_quality_visibility",
    visibility,
    text,
    count=1,
    flags=re.DOTALL,
)
if visibility_replacements != 1:
    raise RuntimeError("expected customer_enrichment_visibility function is missing")

path.write_text(text, encoding="utf-8")
