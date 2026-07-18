# Customer Enrichment materialization adapter

Worker-only atomic planner for deterministic suggestion materialization.

The crate accepts strictly rehydrated immutable response-receipt, provider-profile and mapping snapshots, locks the one mutable enrichment request, and plans the request transition plus immutable suggestion records, outbox events, idempotency and audits in one transaction.

It is intentionally absent from the public production capability inventory.
