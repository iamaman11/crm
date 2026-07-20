# Customer Enrichment provider registry

Infrastructure-owned immutable registry and governed adapter boundary for Customer Enrichment.

The registry:

- keys entries by the complete adapter kind and contract version;
- rejects duplicate coordinates during construction;
- distinguishes unavailable and disabled coordinates;
- never falls back to another version, kind-only match or default adapter;
- implements the module-owned `ProviderAdapterRegistryPort`;
- is used by the production provider process, while startup enables no coordinate implicitly.

The adapter boundary also provides tenant-scoped handle resolution, exact-coordinate quota and circuit isolation, bounded safe failure classes, sanitized response validation and a deterministic process-test transport. Protected values and provider bodies remain outside module results, errors and debug output.

The provider process preserves commit-before-I/O and response replay semantics. `customer_enrichment.request.dispatch@1.0.0` and `customer_enrichment.response.record@1.0.0` remain internal non-runtime coordinates with no public HTTP/gRPC ingress.
