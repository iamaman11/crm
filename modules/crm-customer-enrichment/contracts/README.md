# Published contract boundary for `crm.customer-enrichment`

This directory records the module contract boundary; it does not contain an ad-hoc wire schema.

Before production behavior crosses the module boundary:

- publish compatible versioned Protobuf under canonical package `crm.customer_enrichment.v1`;
- bind every provided capability and event in `module.yaml` to its exact RPC/request/response or message;
- distinguish public mutation/query coordinates from internal worker-only coordinates;
- keep credential values and protected raw provider payloads out of public contracts and events;
- keep public wire schemas independent from private persisted-state schemas;
- regenerate `contracts/module-contract-bindings.json` from manifests and the canonical descriptor set rather than editing it manually;
- run all applicable Contract and generated-sync gates.

The planned first-slice coordinates are frozen in `docs/PHASE8A10_CUSTOMER_ENRICHMENT_ARCHITECTURE.md`.
