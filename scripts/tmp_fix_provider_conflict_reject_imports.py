from pathlib import Path

module = Path("crates/crm-customer-enrichment-provider-process-composition/src/conflict_rejection.rs")
text = module.read_text()
text = text.replace(
    "    IdempotencyKey, ModuleExecutionContext, RecordId, RequestId, SchemaVersion, SdkError, TraceId,\n",
    "    IdempotencyKey, ModuleExecutionContext, RequestId, SchemaVersion, SdkError, TraceId,\n",
    1,
)
module.write_text(text)

test = Path("crates/crm-customer-enrichment-provider-process-composition/tests/postgres_conflict_reject_process.rs")
text = test.read_text()
text = text.replace(
    "    ENRICHMENT_REQUEST_STATUS_CHANGED_EVENT_TYPE, MODULE_ID, enrichment_request_from_snapshot,\n",
    "    MODULE_ID, enrichment_request_from_snapshot,\n",
    1,
)
test.write_text(text)
