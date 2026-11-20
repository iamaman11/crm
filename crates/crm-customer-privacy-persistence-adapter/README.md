# Customer Privacy persistence adapter

This crate binds the pure `crm.customer-privacy` aggregates to the governed
`crm.records` payload envelope.

It owns only:

- exact record references for privacy cases, processing restrictions and
  customer-data legal holds;
- immutable persisted payload contracts;
- `TypedPayload` construction from canonical `crm.cjson/v1` state;
- strict `RecordSnapshot` rehydration with record identity and version parity.

It does not own SQL, connection pools, capability routing, authorization,
subject locks, idempotency, audit or outbox execution. PostgreSQL persistence is
performed by the shared transactional core-data adapter, whose tenant-bearing
tables use ENABLE + FORCE RLS.
