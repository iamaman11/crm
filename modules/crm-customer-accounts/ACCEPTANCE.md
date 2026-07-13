# Acceptance gates for `crm.customer-accounts`

Current readiness: **Foundation**.

- [x] Immutable module identity and canonical Account ownership are explicit.
- [x] Cross-owner `AccountRef` is published as a stable Protobuf contract.
- [x] Lifecycle dependency on the canonical Parties owner is explicit.
- [ ] Define Account aggregate/value-object invariants and Party association semantics.
- [ ] Publish versioned Account capabilities, queries and lifecycle events.
- [ ] Add governed adapters, tenant isolation, authorization, idempotency and cross-tenant negative coverage.
- [ ] Add PostgreSQL persistence only through platform adapters outside the module core.
- [ ] Add production composition and process-level acceptance.
- [ ] Raise readiness beyond Foundation only after exact-head production acceptance is green.
