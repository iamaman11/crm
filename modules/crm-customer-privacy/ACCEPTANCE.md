# Acceptance gates for `crm.customer-privacy`

Current state: **Foundation, pure domain, canonical private persistence, public contracts and FORCE RLS persistence proof**. These gates block any claim of a production vertical slice.

- [x] Freeze immutable module identity and first ownership boundary.
- [x] Keep the pure module core infrastructure-neutral and deny direct cross-owner storage access.
- [x] Accept the Phase 8A.11 architecture and guardrail freeze on unchanged source SHA `62aaaeeb6dec68d520b3b50bb8a6c83fa44e62f8`, merged through PR #140 as `b54ecf42eab4cb465db79ea0103e40edd3ba9d79`.
- [x] Implement deterministic in-memory privacy case, restriction and legal-hold aggregates with optimistic versions, monotonic transition time, bounded errors, exact retry/resume and canonical-lineage rescope semantics.
- [x] Define immutable private state identities, descriptor hashes, byte ceilings, `crm.cjson/v1` encoding and strict rehydration for privacy case, restriction and legal-hold aggregates.
- [x] Publish compatible versioned privacy case, restriction and legal-hold Protobuf contracts, exact manifest bindings and contract-only route classification.
- [x] Implement the governed `crm.records` persistence adapter and prove ENABLE + FORCE RLS, rollback/reapply, missing-tenant denial and `NOBYPASSRLS` isolation on clean and reapplied PostgreSQL schemas.
- [ ] Add permission-aware public case, restriction and legal-hold mutations/queries through module-owned production contributions.
- [ ] Add the shared `tenant_id + canonical_party_id` final subject-lock enforcement port to protected owner paths.
- [ ] Prove privacy restriction is deny-only, live, race-free and cannot be bypassed by module disable/uninstall.
- [ ] Add bounded owner scope/action contribution contracts without direct storage coupling.
- [ ] Reuse Customer Data Operations jobs, manifests, artifacts and audited disclosure for privacy export.
- [ ] Implement legal-hold and retention precedence with immutable reasoned evidence.
- [ ] Implement deterministic resumable owner attempts/outcomes and crash-window recovery without duplicate effects.
- [ ] Preserve erased Party tombstones and required immutable evidence without orphaned references.
- [ ] Prove projection, search and cache tombstone/rebuild convergence.
- [ ] Replace `tests/acceptance.rs` with production-path acceptance evidence.
- [ ] Promote every public, worker-only and reasoned non-runtime coordinate exactly once after its production proof.
- [ ] Complete fresh-PostgreSQL real-`crm-api`, worker-process, cross-tenant, authorization and safe-error acceptance.
- [ ] Synchronize module catalog, roadmap/status, issue #126 and PR evidence on one unchanged exact SHA.
