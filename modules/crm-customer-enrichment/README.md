# CRM Customer Enrichment

Governed **provider-neutral owner/coordinator module** for `crm.customer-enrichment`.

The module owns enrichment requests, immutable provider/mapping definitions, response receipts, provider-response conflicts, suggestions/provenance, review decisions, provider usage and owner-capability application evidence. It does not own authoritative Party, Account, Contact Point, Consent, Identity Resolution or Data Quality values.

The accepted Phase 8A.10 inventory is exactly **6 public mutations + 6 permission-aware queries + 2 activation-gated worker coordinates**. Three provider/materialization coordinates remain worker-only and have no public HTTP/gRPC ingress. The public inventory is frozen by `contracts/customer-enrichment-production-promotion.json`.

The first production slice is limited to reviewed Party display-name suggestions applied only through exact capability `parties.party.update@1.0.0` after exact-version revalidation, policy/approval and final live authorization. Provider dispatch uses exact host-owned transports and tenant-bound secret resolution; raw provider payload and credential material are never exposed to the pure module core or public transport surface.

Phase 8A.10 is **Complete**. Accepted source checkpoint `f92d101206886e3ceaf94d0e56e52580cec21093` passed all 17 permanent workflows on one unchanged user-authored SHA and was squash-merged through PR #137 as `150e44b95d9dbdc08c1792563de03ec73f34aed1`. Production-path evidence includes fresh-PostgreSQL worker/review/materialization processes and a permanent real-`crm-api` HTTP/gRPC process E2E covering successful governed persistence plus bounded authentication, tenant, activation, authorization, visibility and Consent denials.

Direct PostgreSQL, broker, arbitrary HTTP, provider SDK, secret-store and cross-module internal dependencies remain forbidden in the pure module core. Host-owned PostgreSQL reference guards and concrete transports live outside this crate.
