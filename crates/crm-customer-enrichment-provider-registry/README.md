# Customer Enrichment provider registry

Infrastructure-owned immutable registry for exact Customer Enrichment adapter coordinates.

The registry:

- keys entries by the complete adapter kind and contract version;
- rejects duplicate coordinates during construction;
- distinguishes unavailable and disabled coordinates;
- never falls back to another version, kind-only match or default adapter;
- implements the module-owned `ProviderAdapterRegistryPort`;
- is not yet connected to the production dispatch worker.

The worker foundation now also has a separate atomic pre-I/O dispatch planner for request state, RequestDispatched usage, idempotency, outbox and audit evidence.

`customer_enrichment.request.dispatch@1.0.0` remains non-runtime until durable worker composition and real provider process acceptance are complete.
