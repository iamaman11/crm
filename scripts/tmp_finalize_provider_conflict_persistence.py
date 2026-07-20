from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    path.write_text(text.replace(old, new, 1))


replace_once(
    Path("crates/crm-customer-enrichment-provider-process-composition/src/conflict_persistence.rs"),
    """#[derive(Debug, Clone)]
pub struct ProviderResponseConflictPersistencePlan {
""",
    """pub struct ProviderResponseConflictPersistencePlan {
""",
    "conflict plan traits",
)

replace_once(
    Path("modules/crm-customer-enrichment/module.yaml"),
    """    - id: customer_enrichment.response.recorded
      version: 1.0.0
      binding:
        kind: protobuf_message
        message: crm.customer_enrichment.v1.ProviderResponseRecordedEvent
    - id: customer_enrichment.provider_usage.recorded
""",
    """    - id: customer_enrichment.response.recorded
      version: 1.0.0
      binding:
        kind: protobuf_message
        message: crm.customer_enrichment.v1.ProviderResponseRecordedEvent
    - id: customer_enrichment.provider_response_conflict.recorded
      version: 1.0.0
      binding:
        kind: protobuf_message
        message: crm.customer_enrichment.v1.ProviderResponseConflictRecordedEvent
    - id: customer_enrichment.provider_usage.recorded
""",
    "manifest conflict event",
)

replace_once(
    Path("crates/crm-proto-contracts/tests/customer_enrichment_contract.rs"),
    """#[test]
fn customer_enrichment_descriptor_identities_are_stable_and_distinct() {
""",
    """#[test]
fn provider_response_conflict_contract_binds_first_receipt_and_semantic_fingerprint() {
    let conflict = enrichment::ProviderResponseConflictRecordedEvent {
        provider_response_conflict: Some(enrichment::ProviderResponseConflict {
            provider_response_conflict_ref: Some(enrichment::ProviderResponseConflictRef {
                provider_response_conflict_id: "enrichment-response-conflict-example".to_owned(),
            }),
            enrichment_request_ref: Some(enrichment::EnrichmentRequestRef {
                enrichment_request_id: "enrichment-request-example".to_owned(),
            }),
            retry_generation: 2,
            first_provider_response_receipt_ref: Some(enrichment::ProviderResponseReceiptRef {
                provider_response_receipt_id: "enrichment-response-example".to_owned(),
            }),
            conflicting_semantic_fingerprint: vec![7; 32],
            detected_at_unix_ms: 500,
        }),
    };

    assert_eq!(
        enrichment::ProviderResponseConflictRecordedEvent::decode(
            conflict.encode_to_vec().as_slice()
        )
        .unwrap(),
        conflict
    );
}

#[test]
fn customer_enrichment_descriptor_identities_are_stable_and_distinct() {
""",
    "conflict contract round trip",
)

print("finalized provider conflict persistence contract")
