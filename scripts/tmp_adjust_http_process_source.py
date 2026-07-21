from pathlib import Path

path = Path("crates/crm-application-runtime/tests/postgres_customer_enrichment_provider_http_process.rs")
text = path.read_text()
changes = (
    (
        "use crm_customer_enrichment_capability_adapter::provider_response_capability_definition;",
        "use crm_customer_enrichment_capability_adapter::{enrichment_request_from_snapshot, provider_response_capability_definition};",
    ),
    (
        "if tenant_id != *self.snapshot.request.tenant_id()",
        "if &tenant_id != self.snapshot.request.tenant_id()",
    ),
)
for old, new in changes:
    if text.count(old) != 1:
        raise SystemExit(f"marker mismatch: {old}")
    text = text.replace(old, new, 1)
path.write_text(text)
