# CRM Customer Enrichment

Governed **provider-neutral owner/coordinator module** for `crm.customer-enrichment`.

The module owns enrichment requests, immutable provider/mapping definitions, response receipts, provider-response conflicts, suggestions/provenance, review decisions, provider usage and owner-capability application evidence. It does not own authoritative Party, Account, Contact Point, Consent, Identity Resolution or Data Quality values.

Phase 8A.10 is **Complete** through PR #137. Accepted source checkpoint `f92d101206886e3ceaf94d0e56e52580cec21093` passed all 17 permanent workflows unchanged and was squash-merged as `150e44b95d9dbdc08c1792563de03ec73f34aed1`.

The frozen production inventory is exactly **6 public mutations + 6 permission-aware queries + 5 activation-gated worker-only coordinates**. Provider dispatch, response recording, suggestion materialization, Party display-name application and application-outcome recording have no public HTTP/gRPC ingress. The machine-readable authority is `contracts/customer-enrichment-production-promotion.json`, and every manifest-bound coordinate is accounted for as public runtime or worker runtime.

The first production slice is limited to reviewed Party display-name suggestions applied only through exact capability `parties.party.update@1.0.0` after exact-version revalidation, policy/approval and final live authorization. Provider dispatch uses exact host-owned transports and tenant-bound secret resolution; raw provider payload and credential material are never exposed to the pure module core or public transport surface.

Production evidence combines a permanent real-`crm-api` HTTP/gRPC process E2E for public ingress, guarded persistence and bounded denials with fresh-PostgreSQL provider/materialization/review/application process workflows for worker-only coordinates. Exact background phase ordering 240 → 245 → 250 and disable/uninstall shutdown are mechanically tested.

Direct PostgreSQL, broker, arbitrary HTTP, provider SDK, secret-store and cross-module internal dependencies remain forbidden in the pure module core. Host-owned PostgreSQL reference guards and concrete transports live outside this crate.
