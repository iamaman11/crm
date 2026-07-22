# Acceptance gates for `crm.customer-privacy`

Current state: **Gate review for the third production vertical slice**. The architecture, pure domain, canonical private persistence, immutable public contracts, FORCE RLS proof, `customer_privacy.case.create@1.0.0` and `customer_privacy.case.submit@1.0.0` are merged. Draft PR #148 promotes only `customer_privacy.case.subject.verify@1.0.0`; the remaining thirteen public Customer Privacy coordinates stay non-runtime.

- [x] Freeze immutable module identity and first ownership boundary.
- [x] Keep the pure module core infrastructure-neutral and deny direct cross-owner storage access.
- [x] Accept the Phase 8A.11 architecture and guardrail freeze on unchanged source SHA `62aaaeeb6dec68d520b3b50bb8a6c83fa44e62f8`, merged through PR #140 as `b54ecf42eab4cb465db79ea0103e40edd3ba9d79`.
- [x] Implement deterministic in-memory privacy case, restriction and legal-hold aggregates with optimistic versions, monotonic transition time, bounded errors, exact retry/resume and canonical-lineage rescope semantics.
- [x] Define immutable private state identities, descriptor hashes, byte ceilings, `crm.cjson/v1` encoding and strict rehydration for privacy case, restriction and legal-hold aggregates.
- [x] Publish compatible versioned privacy case, restriction and legal-hold Protobuf contracts, exact manifest bindings and contract-only route classification.
- [x] Implement the governed `crm.records` persistence adapter and prove ENABLE + FORCE RLS, rollback/reapply, missing-tenant denial and `NOBYPASSRLS` isolation on clean and reapplied PostgreSQL schemas through PR #145; accepted source `f37d9a5e025745abaaf0aeb351ff9bb534455aab`, merge `721a1cf185ffbdea309bd1199c6c4568cf82d7a1`.
- [x] Accept `customer_privacy.case.create@1.0.0` on unchanged source SHA `9b53c3ebd81b58518dc445b02b33b35403ffa7c3` after 18/18 workflows passed and no review threads remained; merge PR #146 as `2d28937a123e4ba31ab0d835c4c30e3dfed0f187`.
- [x] Implement deterministic tenant/idempotency case creation with Draft/version-1 confidential state, one immutable event, one audit intent, one capability-idempotency claim and one atomic batch.
- [x] Keep root creation on `AggregatePresence::MustBeAbsent` and enforce optional predecessor lineage through a transaction-scoped `FOR SHARE` reference guard, strict snapshot rehydration, tenant concealment and terminal-only validation.
- [x] Compose `case.create` through the generic application mutation ingress, shared live authorizer and activation gate; add no capability-specific HTTP/gRPC route.
- [x] Add permanent unit, fresh-PostgreSQL, rollback/reapply and real-`crm-api` acceptance for create identity, replay/conflict, metadata, atomic evidence, tenant isolation, authorization, activation and bounded safe errors.
- [x] Accept `customer_privacy.case.submit@1.0.0` on unchanged source SHA `8b41e8420b1a897777596c68cb615e2b8bf80c34` after 18/18 workflows passed and no review threads remained; merge PR #147 as `0eba56084405301eb667f2173b3aef6565b95f87`.
- [x] Implement submit as a strict `MustExist` aggregate update with canonical confidential rehydration, optimistic `Draft -> Submitted`, one immutable event, one audit intent and one idempotency claim in one transaction.
- [x] Prove submit replay/conflict, stale version, wrong lifecycle, cross-tenant concealment, malformed rollback, authorization, activation, FORCE RLS and clean/reapplied real ingress.
- [x] Implement verified subject binding with exact submitted/canonical Party identities, authoritative Identity Resolution topology generation and active merge-lineage validation.
- [x] Reuse owner-side Party reference and Identity Resolution composition APIs; add no Customer Privacy topology store or direct owner SQL mutation path.
- [x] Apply shared fail-fast topology and `tenant_id + canonical_party_id` subject locks inside the same PostgreSQL transaction as case update, event, audit and idempotency.
- [x] Prove `Submitted N -> SubjectVerified N + 1`, exact replay, incompatible replay, stale version/generation, invalid canonical redirect, missing/cross-tenant Party concealment, malformed state rollback and subject-lock contention.
- [x] Prove candidate-only and accepted production composition through the existing generic HTTP/gRPC ingress on clean migrations and after full rollback/schema removal/reapply, under a non-privileged FORCE-RLS runtime role, with bounded safe transport errors.
- [x] Freeze candidate production-route parity at exactly three runtime Customer Privacy mutations and thirteen non-runtime public Customer Privacy coordinates; worker-only and crypto-shred classifications remain unchanged.
- [ ] Accept draft PR #148 on one unchanged post-Generated-Sync source SHA after all 18 applicable workflows pass and review threads are resolved.
- [ ] Add the remaining permission-aware public case, restriction and legal-hold mutations/queries through separately bounded module-owned production contributions.
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

Phase 8A.11 remains **In progress** after `case.create` and `case.submit`; `case.subject.verify` is the active separately bounded gate-review slice.
