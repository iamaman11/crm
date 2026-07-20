from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected exactly one match, found {count}")
    path.write_text(text.replace(old, new, 1))


replace_once(
    Path("crates/crm-customer-enrichment-provider-process-composition/src/lib.rs"),
    'assert_eq!(error.code, "CUSTOMER_ENRICHMENT_PARTY_VERSION_MISMATCH");',
    'assert_eq!(error.code, "CUSTOMER_ENRICHMENT_DISPATCH_TARGET_CONFLICT");',
    "provider recovery error assertion",
)

pure_conflict = "- [x] Add pure durable provider-response conflict evidence: deterministic identity binds tenant, request, retry generation, immutable first receipt and conflicting semantic fingerprint; strict canonical persistence rejects corruption; operator resolution permits only retain-first or reject-request with exact actor, policy version, reason, approval and causation lineage; exact replay is a no-op and a different second decision fails closed."
atomic_persistence = "- [x] Add atomic provider-response conflict persistence under the existing non-runtime `customer_enrichment.response.record@1.0.0` scope: one confidential immutable conflict record, one typed internal outbox event, one audit, one exact idempotency claim and one business transaction; fresh-PostgreSQL replay preserves version 1 and creates no duplicate evidence while public route inventory remains unchanged."
replace_once(
    Path("modules/crm-customer-enrichment/ACCEPTANCE.md"),
    pure_conflict + "\n",
    pure_conflict + "\n" + atomic_persistence + "\n",
    "accepted conflict persistence gate",
)

replace_once(
    Path("modules/crm-customer-enrichment/ACCEPTANCE.md"),
    "- [ ] Complete remaining reconciliation/materialization conflict scenarios, including provider-process conflict persistence, live operator authorization, checkpoint resumption and materialization gating after an approved canonical choice.",
    "- [ ] Complete remaining reconciliation/materialization conflict scenarios, including provider-process integration of persisted conflicts, live operator authorization, deterministic checkpoint resumption and materialization gating after an approved canonical choice.",
    "remaining conflict process gate",
)

print("fixed provider recovery assertion and synchronized conflict acceptance")
