# Customer Privacy pure-domain invariants

This document describes the implemented in-memory domain semantics only. It is not a public API contract or production-route declaration. The scope-snapshot section also freezes its canonical private persisted envelope.

## Privacy case

- A case starts at version 1 in `Draft`.
- Every accepted transition increments the optimistic aggregate version exactly once.
- A stale expected version changes no state and returns `CUSTOMER_PRIVACY_VERSION_CONFLICT`.
- Transition timestamps are non-negative and monotonic within the aggregate.
- Subject verification binds the submitted Party, canonical Party and exact Identity Resolution generation.
- A later canonical-generation advance enters `RescopeRequired`; scope, plan and approval evidence cannot be silently reused.
- Re-verification is required before scoping resumes.
- Retryable failure records the deterministic lifecycle stage from which work resumes.
- Terminal states cannot be reopened; a later request must create a new case referencing the prior case.

## Immutable scope discovery foundation

- The coordinator owns only immutable scope snapshots and contribution receipts; authoritative owners retain their records and values.
- The canonical registry contains one exact versioned scope-contribution coordinate for each of the nine architecture-frozen owner modules.
- Every contribution is bound to tenant, canonical Party, exact Identity Resolution generation and the registered owner/capability/version tuple.
- Finalization requires exactly one terminal-complete contribution from every registered owner. Missing, duplicate, partial, extra, stale-lineage or contract-mismatched evidence fails closed.
- Owner resources are normalized by owner, resource type, resource identity, exact version, data class, evidence class and retention policy.
- Exact duplicate resources collapse deterministically. Conflicting versions or classifications for one owner resource are rejected rather than guessed.
- Registry, contribution, completeness and snapshot identities use domain-separated length-framed SHA-256.
- The snapshot ID is derived from case, tenant, canonical Party, Identity Resolution generation, registry digest, completeness digest and capture time.
- Canonical `crm.cjson/v1` persistence uses decimal-string integers, hexadecimal digests, deny-unknown-fields decoding and decode → rehydrate → re-encode byte equality.
- The pure foundation performs no cross-owner storage reads, no owner mutation, no transport, no scheduler work and no runtime registration.

## Processing restriction

- Restriction is a separate subject-scoped aggregate and is never inferred from case status.
- Scope is processing, communication or both.
- Effective and optional expiry timestamps are exact half-open boundaries.
- Release and explicit expiry are optimistic-versioned transitions.
- Privacy-case completion never implicitly releases a restriction.

## Customer-data legal hold

- Legal hold is a separate subject aggregate scoped to all customer data, one data class or one authoritative owner module.
- Authority is referenced by governed identity; protected authority material is not stored in domain errors.
- Reason codes are bounded canonical uppercase identifiers.
- Release appends state and actor/time evidence; it never removes historical hold identity.

## Boundary

The domain code performs no database access, authorization, scheduling, transport, external I/O or cross-owner mutation. Those responsibilities remain in separately governed contract, adapter, composition and infrastructure layers.
