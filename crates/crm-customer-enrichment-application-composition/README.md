# CRM Customer Enrichment Application Composition

This non-runtime PostgreSQL composition loads strict immutable suggestion and review evidence, then delegates the locked application-attempt or outcome mutation to the transactional aggregate executor.

The composition preserves the owner boundary:

- it creates pending Customer Enrichment application evidence before external I/O;
- it appends one exact outcome after external I/O;
- it never writes Party-owned records;
- it never invokes Party storage or adapters directly;
- it does not register either application coordinate in production.

The next separately governed layer must invoke exact `parties.party.update@1.0.0` through the capability runtime using the deterministic target idempotency key and expected Party version recorded by the attempt.
