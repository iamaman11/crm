# Acceptance gates for `crm.contact-points`

Current readiness: **Foundation**.

- [x] Immutable module identity and canonical Contact Point ownership are explicit.
- [x] Cross-owner `ContactPointRef` is published as a stable Protobuf contract.
- [x] Lifecycle dependency on the canonical Parties owner is explicit.
- [ ] Define contact channel, normalized value, verification, validity and preference invariants.
- [ ] Publish versioned Contact Point capabilities, queries and lifecycle events.
- [ ] Add governed adapters, tenant isolation, authorization, idempotency and cross-tenant negative coverage.
- [ ] Add PostgreSQL persistence only through platform adapters outside the module core.
- [ ] Add production composition and process-level acceptance.
- [ ] Raise readiness beyond Foundation only after exact-head production acceptance is green.
