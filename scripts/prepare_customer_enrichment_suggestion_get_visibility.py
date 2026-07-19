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

lib_path = Path("crates/crm-application-runtime/src/lib.rs")
lib_text = lib_path.read_text(encoding="utf-8")
if "mod bootstrap_visibility;\n" not in lib_text:
    lib_text, marker_replacements = re.subn(
        r"// Staged repair[^\n]*\n",
        "mod bootstrap_visibility;\n",
        lib_text,
        count=1,
    )
    if marker_replacements != 1:
        raise RuntimeError("expected staged bootstrap module marker is missing")
lib_path.write_text(lib_text, encoding="utf-8")

patch_path = Path("scripts/apply_customer_enrichment_suggestion_get_promotion.py")
patch_text = patch_path.read_text(encoding="utf-8")
old_guard = '    if old not in text:\n        raise RuntimeError(f"expected source fragment missing: {path}")\n'
new_guard = '''    if old not in text:
        if (
            path == "crates/crm-application-runtime/src/bootstrap_visibility.rs"
            and "GET_SUGGESTION_CAPABILITY => vec![" in text
        ):
            return
        raise RuntimeError(f"expected source fragment missing: {path}")
'''
if old_guard in patch_text:
    patch_text = patch_text.replace(old_guard, new_guard, 1)
elif new_guard not in patch_text:
    raise RuntimeError("expected staged promotion guard is missing")
patch_path.write_text(patch_text, encoding="utf-8")
