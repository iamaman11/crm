# Acceptance gates for `crm.customer-enrichment`

Foundation state: **In progress — not a production vertical slice**. These gates block any completion or readiness claim.

Current production route inventory: **4 mutations + 4 permission-aware queries**; the remaining 9 published coordinates stay individually non-runtime. This inventory is authoritative only on a canonical Generated Sync state and a green exact-head workflow matrix.

- [x] Freeze immutable module identity, owned evidence records and retain-on-uninstall semantics.
- [x] Freeze provider infrastructure, secret-handle, mapping, provenance, review and exact owner-capability boundaries.
- [x] Publish compatible `crm.customer_enrichment.v1` Protobuf contracts, typed descriptor tests, generated manifest bindings and client descriptor hashes.
- [x] Implement immutable provider-profile and mapping-version invariants with deterministic content-derived identities, bounded metadata and focused unit coverage.
- [x] Implement deterministic request, response-receipt, suggestion, review and application-attempt domain behavior with strict state transitions, replay conflict detection, freshness/expiry semantics, approval binding and target idempotency planning.
- [x] Implement immutable provider-usage, billable-unit and quota-snapshot evidence with deterministic identities and bounded semantic validation.
- [x] Add strict bounded canonical persisted-state conversion, schema descriptors, exact re-encoding and corruption rejection for all eight manifest-owned enrichment record types.
- [x] Add pure-core Party snapshot, versioned policy/Consent, sanitized provider-dispatch and exact owner-application port contracts.
- [x] Add activation-gated native `customer_enrichment.provider_profile.publish@1.0.0` production composition with exact wire/domain conversion and atomic immutable record/idempotency/outbox/audit evidence.
- [x] Add activation-gated permission-aware `customer_enrichment.provider_profile.get@1.0.0` with tenant-scoped lookup, strict persisted-state validation, fail-closed resource visibility and `definition` field redaction.
- [x] Add activation-gated native `customer_enrichment.mapping.publish@1.0.0` with atomic governed provider-profile `MustExist` locking, persisted identity and target-field support validation, immutable mapping persistence, idempotency, outbox and audit evidence.
- [x] Add activation-gated permission-aware `customer_enrichment.mapping.get@1.0.0` with tenant-scoped lookup, strict mapping-state rehydration, live referenced-provider-profile visibility, not-found hiding and declarative `definition` redaction.
- [x] Add activation-gated native `customer_enrichment.request.create@1.0.0` with deterministic identity, canonical Personal persisted state, immutable mapping/profile validation, versioned purpose/legal-basis policy, governed Party and optional Consent reads, exact Party row/version locking, Party access-path relationship and atomic idempotency/outbox/audit evidence.
- [x] Add activation-gated permission-aware `customer_enrichment.request.get@1.0.0` with strict Personal request-state rehydration, live target-Party and request-record visibility, not-found hiding and declarative field redaction.
- [x] Add activation-gated native `customer_enrichment.request.cancel@1.0.0` with live Party pre-authorization, exact request-row locking, terminal-state rejection, optimistic version update and atomic Personal status-change/idempotency/audit evidence.
- [x] Add activation-gated permission-aware `customer_enrichment.request.list@1.0.0` with exact Party/provider/status filters, tenant/actor/capability-version/filter/page-size-bound signed cursor, stable updated-at ordering, bounded visibility scanning, strict Personal rehydration, live Party/request visibility, hidden-Party empty-page semantics and declarative field redaction.
- [x] Add non-runtime deterministic worker foundation for `customer_enrichment.request.dispatch@1.0.0` and `customer_enrichment.response.record@1.0.0`: exact status and retry-generation expectations, deterministic dispatch transitions, immutable sanitized receipt creation and exact request binding.
- [x] Add a non-runtime atomic response batch planner: exact request lock, `expected_retry_generation`, request update, immutable receipt, ResponseReceived and optional BillableUnits evidence, idempotency, outbox and per-record audits. Integration tests prove metered and zero-meter batches, exact record/event/audit counts, stale generation rejection and invalid-digest rejection.
- [x] Add non-runtime dispatch recovery foundation with exact adapter kind/version, exact registry boundary, generation-bound deterministic provider key, profile and Party version validation, durable Dispatched state before provider invocation, and recovery that rebuilds the same request. Focused tests cover recovery identity, retries, stale inputs and closed deadlines.
- [x] Add immutable exact-coordinate provider registry and durable non-runtime worker composition with commit-before-I/O ordering, sanitized response validation, deterministic response identity and crash-safe replay.
- [x] Add fresh-PostgreSQL Customer Enrichment worker process acceptance proving seed → dispatch → provider → response and repeated replay with one request, one receipt, three usage rows, seven events, seven audits, three exact idempotency rows and three transactions without duplicates.
- [x] Add pure deterministic suggestion materialization over exact request/receipt/profile/mapping lineage with response-class rules, mapping count/confidence constraints, exact provider-policy evidence, protected-evidence linkage, deterministic suggestion ordering/deduplication and no partial request mutation.
- [x] Add the atomic non-runtime `customer_enrichment.suggestions.materialize@1.0.0` planner and immutable-dependency PostgreSQL worker composition. Integration and fresh-PostgreSQL process evidence prove exact dependency reads, one atomic request update plus two immutable suggestions, and repeat replay without duplicate records, events, audits, idempotency rows or transactions.
- [ ] Add governed public and worker-only capability/query adapters and production composition for every remaining published coordinate.
- [ ] Implement the remaining Party/Consent semantic port adapters plus final live authorization and declarative field visibility.
- [ ] Add concrete provider infrastructure adapters outside the pure module core with sanitized errors and no credential/raw-payload leakage.
- [ ] Add tenant-scoped PostgreSQL persistence with FORCE RLS, deterministic uniqueness, atomic idempotency/outbox/audit evidence and migration rollback/reapply proof for remaining records.
- [ ] Add exact `parties.party.update@1.0.0` invocation with stale-version rejection and deterministic target idempotency.
- [ ] Add deterministic activation-gated reconciliation, review, expiry, application and outcome-recovery workers.
- [ ] Prove provider replay, conflicting response, quota, circuit/failure and provider-disabled behavior across concrete adapters; exact registry and worker replay process proof is complete.
- [ ] Prove response-materialization and target-success/outcome-missing crash recovery.
- [ ] Add remaining permission-aware list surfaces, signed pagination and field redaction.
- [ ] Replace `tests/acceptance.rs` with real production-path evidence.
- [ ] Complete `production/CONTRIBUTION.md` through separately owned adapter/composition crates with exact route parity.
- [ ] Add remaining fresh-PostgreSQL real `crm-api` success, denial, stale, failure, disable/uninstall and cross-tenant process scenarios.
- [ ] Synchronize `MODULE_CATALOG.md`, roadmap/status, issue #125 and PR evidence.
- [ ] Pass all applicable exact-head Contract, Governance, Rust, Database, Application Runtime, Product Plane and enrichment process workflows.
