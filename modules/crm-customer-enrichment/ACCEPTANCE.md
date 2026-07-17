# Acceptance gates for `crm.customer-enrichment`

Foundation state: **In progress — not a production vertical slice**. These gates block any completion or readiness claim.

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
- [x] Add prepared `mapping.publish`/`mapping.get` adapter foundations: strict content-derived provider-profile IDs, immutable mapping planner/persistence/event evidence, tenant-scoped provider-profile semantic validation, strict mapping snapshot decode and permission-aware lookup/redaction. These coordinates intentionally remain non-runtime until their validator/bootstrap visibility contributions are composed.
- [ ] Add governed public and worker-only capability/query adapters and production composition for every remaining published coordinate.
- [ ] Implement the Party/Consent semantic port adapters plus final live authorization and declarative field visibility.
- [ ] Add provider infrastructure adapters outside the pure module core with sanitized errors and no credential/raw-payload leakage.
- [ ] Add tenant-scoped PostgreSQL persistence with FORCE RLS, deterministic uniqueness, atomic idempotency/outbox/audit evidence and migration rollback/reapply proof.
- [ ] Add exact `parties.party.update@1.0.0` invocation with stale-version rejection and deterministic target idempotency.
- [ ] Add deterministic activation-gated dispatch, reconciliation, materialization, expiry, application and outcome-recovery workers.
- [ ] Prove provider replay, conflicting response, quota, circuit/failure and provider-disabled behavior across adapters and process acceptance; pure-domain replay, quota-shape and conflicting-evidence proof is complete.
- [ ] Prove provider-dispatch, response-materialization and target-success/outcome-missing crash recovery.
- [ ] Add remaining permission-aware get/list surfaces, signed pagination and field redaction.
- [ ] Replace `tests/acceptance.rs` with real production-path evidence.
- [ ] Complete `production/CONTRIBUTION.md` through separately owned adapter/composition crates with exact route parity.
- [ ] Add fresh-PostgreSQL real `crm-api` success, denial, stale, replay, failure, disable/uninstall and cross-tenant process scenarios.
- [ ] Synchronize `MODULE_CATALOG.md`, roadmap/status, issue #125 and PR evidence.
- [ ] Pass all applicable exact-head Contract, Governance, Rust, Database, Application Runtime, Product Plane and enrichment process workflows.
