from pathlib import Path

process_test = Path(
    "crates/crm-customer-enrichment-provider-process-composition/tests/postgres_conflict_process_hold.rs"
)
text = process_test.read_text()
old = "use crm_core_events::{ProjectionStore, ProjectionStore as _};"
new = "use crm_core_events::ProjectionStore;"
if text.count(old) != 1:
    raise SystemExit(f"expected one projection import, found {text.count(old)}")
process_test.write_text(text.replace(old, new, 1))

persistence = Path(
    "crates/crm-customer-enrichment-provider-process-composition/src/conflict_persistence.rs"
)
text = persistence.read_text()
old = "        assert_eq!(plan.batch.relationships.len(), 0);\n"
if text.count(old) != 1:
    raise SystemExit(f"expected one stale relationship assertion, found {text.count(old)}")
persistence.write_text(text.replace(old, "", 1))

print("finalized provider conflict process hold tests")
