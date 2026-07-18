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
- [ ] Add governed public and worker-only capability/query adapters and production composition for every remaining published coordinate.
- [ ] Implement the remaining Party/Consent semantic port adapters plus final live authorization and declarative field visibility.
- [ ] Add provider infrastructure adapters outside the pure module core with sanitized errors and no credential/raw-payload leakage.
- [ ] Add tenant-scoped PostgreSQL persistence with FORCE RLS, deterministic uniqueness, atomic idempotency/outbox/audit evidence and migration rollback/reapply proof.
- [ ] Add exact `parties.party.update@1.0.0` invocation with stale-version rejection and deterministic target idempotency.
- [ ] Add deterministic activation-gated dispatch, reconciliation, materialization, expiry, application and outcome-recovery workers.
- [ ] Prove provider replay, conflicting response, quota, circuit/failure and provider-disabled behavior across adapters and process acceptance; pure-domain replay, quota-shape and conflicting-evidence proof is complete.
- [ ] Prove provider-dispatch, response-materialization and target-success/outcome-missing crash recovery.
- [ ] Add remaining permission-aware list surfaces, signed pagination and field redaction.
- [ ] Replace `tests/acceptance.rs` with real production-path evidence.
- [ ] Complete `production/CONTRIBUTION.md` through separately owned adapter/composition crates with exact route parity.
- [ ] Add fresh-PostgreSQL real `crm-api` success, denial, stale, replay, failure, disable/uninstall and cross-tenant process scenarios.
- [ ] Synchronize `MODULE_CATALOG.md`, roadmap/status, issue #125 and PR evidence.
- [ ] Pass all applicable exact-head Contract, Governance, Rust, Database, Application Runtime, Product Plane and enrichment process workflows.
