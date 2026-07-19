# CRM Customer Enrichment Application Composition

This non-runtime PostgreSQL composition loads strict immutable suggestion and review evidence, persists deterministic application attempts and appends one exact outcome through the transactional aggregate executor.

The composition preserves the owner boundary:

- it creates pending Customer Enrichment application evidence before external I/O;
- it evaluates the final owner-application policy over exact governed lineage;
- it invokes only exact `parties.party.update@1.0.0` through an injected `CapabilityClient` boundary;
- it never writes Party-owned records directly;
- target authorization, rate limiting, semantic validation, optimistic locking, idempotency, audit and outbox remain owned by the ordinary Party capability gateway;
- stale-version resolution uses an injected governed `PartySnapshotPort`, not error-text parsing;
- exact success requires matching typed Party response and affected-resource evidence;
- it appends one exact outcome after policy and owner I/O;
- a pending attempt recovers the target-success/outcome-missing window by replaying the same deterministic target idempotency key;
- a completed attempt is loaded before policy or owner I/O and returns without repeating either boundary;
- it does not register either application coordinate in production.

The remaining work is activation-gated production composition plus the broader provider-failure, disable/uninstall, cross-tenant and real `crm-api` acceptance matrix.
