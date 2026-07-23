# Production contribution boundary for `crm.customer-privacy`

This file is a mandatory architecture boundary, not an implementation placeholder inside the pure module core.

Before production readiness, separately owned adapter/composition crates must:

- contribute every exact versioned privacy mutation and query coordinate owned by `crm.customer-privacy`;
- perform Party, Consent, Identity Resolution, export and owner-contribution reads through governed ports in pre-authorization semantic validation;
- invoke scope discovery only through the exact deterministic registry of nine architecture-frozen owner contribution coordinates; direct reads of another owner's storage or private adapter are forbidden;
- bind every owner contribution to tenant, canonical Party, exact Identity Resolution generation and the registered owner/capability/version tuple;
- finalize an immutable scope snapshot only after every registered owner proves terminal completeness across its bounded pages; missing, duplicate, partial, stale-lineage or contract-mismatched evidence must fail closed;
- repeat final live authorization and the authoritative privacy restriction decision immediately before protected persistence, disclosure or external I/O;
- acquire the shared `tenant_id + canonical_party_id` lock for restriction placement/release and every protected owner action;
- gate routes and deterministic phases 260, 270, 280 and 290 through durable module activation without treating inactivity as allow;
- fail startup on duplicate coordinates, owner mismatches, route-kind mismatches or incomplete public/worker/non-runtime classification;
- persist case transitions, immutable snapshots/receipts, idempotency, outbox, audit and business-transaction evidence atomically where the production slice requires one transaction;
- dispatch owner actions only through exact module-owned capabilities with deterministic attempt and append-once outcome identities;
- reuse Customer Data Operations privacy export jobs/artifacts rather than introducing a second disclosure path;
- prove cross-tenant concealment, FORCE RLS, legal-hold blocking, recovery, tombstone integrity and convergence through real production acceptance;
- require no edits to generic capability, query or worker routers.

The module core must remain infrastructure-neutral and must not depend on the production host. The immutable scope foundation does not by itself register `customer_privacy.scope.discover@1.0.0` or change production inventory.
