# Acceptance gates for `crm.parties`

Current readiness: **Foundation**.

- [x] Immutable module identity and authoritative person/organization ownership are explicit.
- [x] Initial versioned Protobuf Party create/get/event contracts are published.
- [x] Canonical cross-owner customer references are separate from private Party persistence.
- [ ] Implement typed Person and Organization aggregate/value-object invariants.
- [ ] Add governed Party mutation and query adapters.
- [ ] Add tenant isolation, live authorization, idempotency and cross-tenant negative coverage.
- [ ] Add PostgreSQL persistence only through platform adapters outside the module core.
- [ ] Add production composition and process-level acceptance through governed gateways.
- [ ] Prove disable/upgrade/rollback/uninstall behavior and retained-record semantics.
- [ ] Raise readiness to Vertical slice only after exact-head production acceptance is green.
