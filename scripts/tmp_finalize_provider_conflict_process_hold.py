from pathlib import Path

path = Path(
    "crates/crm-customer-enrichment-provider-process-composition/tests/postgres_conflict_process_hold.rs"
)
text = path.read_text()
old = "use crm_core_events::{ProjectionStore, ProjectionStore as _};"
new = "use crm_core_events::ProjectionStore;"
if text.count(old) != 1:
    raise SystemExit(f"expected one projection import, found {text.count(old)}")
path.write_text(text.replace(old, new, 1))
print("finalized provider conflict process hold test")
