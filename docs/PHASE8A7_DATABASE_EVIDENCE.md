# Phase 8A.7 Database Evidence Notes

## Atomic artifact capability acceptance

The PostgreSQL foundation acceptance must prove that an immutable source-artifact mutation and its capability evidence share one transaction.

The acceptance verifies:

- artifact business state is committed exactly once;
- capability idempotency is completed;
- one durable lifecycle outbox event is present for a real state change;
- one durable audit record is present;
- the business-transaction completion marker records the expected evidence counts;
- a synthetic evidence-building failure rolls back both the artifact mutation and the idempotency claim.

The file-artifact acceptance uses a seeded tenant/actor pair distinct from the original record-foundation acceptance. This keeps the test focused on file transaction semantics instead of creating an artificial race between two independent tests advancing the same tenant audit chain in parallel.

Production correctness does not rely on test isolation: the audit chain remains concurrency-safe and serialized by its PostgreSQL evidence path. The separate test tenants make failures attributable to the capability under test.
