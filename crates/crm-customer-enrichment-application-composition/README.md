# CRM Customer Enrichment Application Composition

This non-runtime PostgreSQL composition loads strict immutable suggestion and review evidence, then delegates the locked application-attempt or outcome mutation to the transactional aggregate executor.

The composition preserves the owner boundary:

- it creates pending Customer Enrichment application evidence before external I/O;
- it appends one exact outcome after external I/O;
- it never writes Party-owned records directly;
- it invokes only exact `parties.party.update@1.0.0` through an injected `CapabilityClient`;
- target authorization, rate limiting, semantic validation, optimistic locking, idempotency, audit and outbox remain owned by the ordinary Party capability gateway;
- stale-version resolution uses an injected governed `PartySnapshotPort`, not error-text parsing;
- exact success requires matching typed Party response and affected-resource evidence;
- it does not register either application coordinate in production.

The remaining orchestration layer must perform final owner-application policy evaluation, recover target-success/outcome-missing crashes by replaying the same deterministic target idempotency key, and append the resulting application outcome.
