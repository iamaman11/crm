# Acceptance gates for `crm.customer-privacy`

Current state: **Gate review for the first production vertical slice**. The architecture, pure domain, canonical private persistence, immutable public contracts and FORCE RLS proof are merged. Draft PR #146 promotes only `customer_privacy.case.create@1.0.0`; every other public Customer Privacy coordinate remains non-runtime.

- [x] Freeze immutable module identity and first ownership boundary.
- [x] Keep the pure module core infrastructure-neutral and deny direct cross-owner storage access.
- [x] Accept the Phase 8A.11 architecture and guardrail freeze on unchanged source SHA `62aaaeeb6dec68d520b3b50bb8a6c83fa44e62f8`, merged through PR #140 as `b54ecf42eab4cb465db79ea0103e40edd3ba9d79`.
- [x] Implement deterministic in-memory privacy case, restriction and legal-hold aggregates with optimistic versions, monotonic transition time, bounded errors, exact retry/resume and canonical-lineage rescope semantics.
- [x] Define immutable private state identities, descriptor hashes, byte ceilings, `crm.cjson/v1` encoding and strict rehydration for privacy case, restriction and legal-hold aggregates.
- [x] Publish compatible versioned privacy case, restriction and legal-hold Protobuf contracts, exact manifest bindings and contract-only route classification.
- [x] Implement the governed `crm.records` persistence adapter and prove ENABLE + FORCE RLS, rollback/reapply, missing-tenant denial and `NOBYPASSRLS` isolation on clean and reapplied PostgreSQL schemas through PR #145; accepted source `f37d9a5e025745abaaf0aeb351ff9bb534455aab`, merge `721a1cf185ffbdea309bd1199c6c4568cf82d7a1`.
- [ ] Accept draft PR #146 on one unchanged exact source SHA after all applicable workflows pass and review threads are resolved.
- [x] Implement the bounded `customer_privacy.case.create@1.0.0` production candidate with deterministic tenant/idempotency identity, Draft/version-1 confidential state, one immutable event, one audit intent, one capability-idempotency claim and one atomic batch.
- [x] Keep root creation on `AggregatePresence::MustBeAbsent` and enforce optional predecessor lineage through a transaction-scoped `FOR SHARE` reference guard, strict snapshot rehydration, tenant concealment and terminal-only validation.
- [x] Compose only `case.create` through the generic application mutation ingress, shared live authorizer and activation gate; add no capability-specific HTTP/gRPC route.
- [x] Freeze production-route parity at exactly one runtime Customer Privacy mutation and fifteen non-runtime public Customer Privacy coordinates; worker-only and crypto-shred classifications remain unchanged.
- [x] Add permanent unit, fresh-PostgreSQL, rollback/reapply and real-`crm-api` acceptance for deterministic identity, replay/conflict, metadata, atomic evidence, tenant isolation, authorization, activation and bounded safe errors.
- [ ] Add the remaining permission-aware public case, restriction and legal-hold mutations/queries through separately bounded module-owned production contributions.
- [ ] Add the shared `tenant_id + canonical_party_id` final subject-lock enforcement port to protected owner paths.
- [ ] Prove privacy restriction is deny-only, live, race-free and cannot be bypassed by module disable/uninstall.
- [ ] Add bounded owner scope/action contribution contracts without direct storage coupling.
- [ ] Reuse Customer Data Operations jobs, manifests, artifacts and audited disclosure for privacy export.
- [ ] Implement legal-hold and retention precedence with immutable reasoned evidence.
- [ ] Implement deterministic resumable owner attempts/outcomes and crash-window recovery without duplicate effects.
- [ ] Preserve erased Party tombstones and required immutable evidence without orphaned references.
- [ ] Prove projection, search and cache tombstone/rebuild convergence.
- [ ] Promote every remaining public, worker-only and reasoned non-runtime coordinate exactly once after its production proof.
- [ ] Complete fresh-PostgreSQL worker-process, restriction/legal-hold, deletion/convergence and full-lifecycle acceptance.
- [ ] Synchronize module catalog, roadmap/status, issue #126 and PR evidence on the final unchanged accepted source SHA.

Phase 8A.11 remains **In progress** after `case.create`; the next bounded slice is selected and accepted separately.
