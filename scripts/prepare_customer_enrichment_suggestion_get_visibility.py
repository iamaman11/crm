from pathlib import Path


path = Path("crates/crm-application-runtime/src/bootstrap_visibility.rs")
text = path.read_text(encoding="utf-8")
new = """        GET_SUGGESTION_CAPABILITY => vec![
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
if new not in text:
    old = """        ],
        _ => Vec::new(),
    }
}

fn data_quality_visibility"""
    if old not in text:
        raise RuntimeError("expected customer_enrichment_visibility tail is missing")
    text = text.replace(old, new, 1)
    path.write_text(text, encoding="utf-8")
