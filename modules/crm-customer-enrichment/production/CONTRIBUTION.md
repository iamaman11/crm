# Production contribution boundary for `crm.customer-enrichment`

This file is a mandatory architecture boundary. The pure module core does not wire itself into the process host and never contains provider SDK, HTTP, credentials, SQL, process scheduling or concrete owner-adapter logic.

The machine-readable authority is `contracts/customer-enrichment-production-promotion.json`. The accepted runtime inventory is exactly:

- 6 public mutations;
- 6 permission-aware queries;
- 5 activation-gated worker-only coordinates with no public HTTP/gRPC ingress:
  - provider process, phase 240: `customer_enrichment.request.dispatch@1.0.0` and `customer_enrichment.response.record@1.0.0`;
  - materialization process, phase 245: `customer_enrichment.suggestions.materialize@1.0.0`;
  - application process, phase 250: `customer_enrichment.party.display_name.apply@1.0.0` and `customer_enrichment.application.outcome.record@1.0.0`.

All 17 manifest-bound coordinates are classified as public runtime or worker runtime. No completed Customer Enrichment coordinate may remain in `non_runtime_contract_routes`.

Changing that inventory requires the promotion contract, route classifications, manifest/bindings, validators, acceptance evidence and PR inventory to change together on one green exact head.

## Module-owned contributions

Separately owned adapter/composition crates must:

- contribute every exact versioned public mutation, query and runtime worker coordinate;
- publish query visibility through `crm-customer-enrichment-visibility`, which is the single source of exact capability-to-resource and field declarations for all six production queries;
- keep the central process bootstrap limited to exact infrastructure registration and mechanical visibility/worker composition, with no Customer Enrichment business route switch or identity substitution;
- fail startup on duplicate coordinates, owner mismatches, route-kind mismatches or incomplete production/visibility parity;
- gate every route and worker through durable `ModuleActivationPort` / `crm.module_installations` state.

The central router, gateway and worker scheduler must not contain Customer Enrichment business routing, provider selection, query-field vocabulary or lifecycle decisions.

## Governed request policy boundary

Request creation must preserve this order:

1. typed request and immutable profile/mapping validation;
2. governed Party query authorization and live field/resource visibility;
3. governed Consent query authorization and live field/resource visibility when Consent evidence is supplied or required;
4. versioned purpose, legal-basis, effective-time and provider-policy validation;
5. final mutation authorization at the generic capability gateway;
6. transactional request persistence with exact Party-version and provider-profile guards.

For the `consent` legal basis, the referenced authorization must match the exact authorization identity, Party, purpose and legal basis; have `Grant` effect and `Active` status; be effective and unexpired at request evaluation time; and contain non-empty evidence provenance. Missing, mismatched, denied, withdrawn, not-yet-effective or expired Consent evidence fails before request persistence. Consent query-permission denial also fails before persistence.

Party and Consent remain authoritative owner modules. Customer Enrichment never persists substitute Party or Consent state and never reads their tables directly for production policy decisions.

## Provider and evidence boundary

Provider adapters are resolved only through the infrastructure-owned exact adapter-kind/version registry. Credential resolution, arbitrary HTTP, raw payload handling, timeouts, quotas and circuit isolation remain outside the pure module core.

Provider dispatch, response reconciliation and suggestion materialization are production worker-only coordinates. They preserve exact replay identities, immutable canonical receipts/outcomes, governed finalized evidence, sanitized error surfaces and restart-safe checkpoints. Raw provider payloads, credentials, headers, URLs containing secrets and upstream body fragments must not enter public errors, logs, audit records or events.

## Owner application boundary

Accepted suggestions change authoritative customer state only through exact owner capabilities, initially `parties.party.update@1.0.0`. Application revalidates the current Party version, policy/approval evidence and live authorization before owner invocation.

The target idempotency identity is deterministic. Recovery after Party success but before enrichment outcome persistence produces exactly one Party update and one logical immutable application outcome.

## Lifecycle and database invariants

Disable or uninstall stops all contributed routes and all three background processes through durable activation state, retains enrichment provenance and leaves existing customer-master owner paths operational.

All Customer Enrichment durable business state uses the shared authoritative `crm` tables and inherits dynamic ENABLE + FORCE ROW LEVEL SECURITY. Application roles are `NOBYPASSRLS`; cross-tenant and no-context direct reads are concealed, cross-tenant writes are rejected, and migration rollback/reapply restores the same policy enforcement.

## Acceptance topology

Production readiness is proven through complementary permanent gates:

- real fresh-database `crm-api` HTTP/gRPC acceptance for public mutations, queries, exact reference guards, visibility, Consent, activation, authorization and safe error surfaces;
- fresh-PostgreSQL provider, materialization, review and application process acceptance for worker-only coordinates and crash-window recovery;
- exact background registry phase-order and durable disable/uninstall tests;
- exact manifest/binding/public-route/worker-route classification parity.

Worker-only coordinates are not exposed publicly merely to satisfy tests. A scaffold, fake-only adapter or library-only test is insufficient.

Every accepted checkpoint retains:

- durable activation gating;
- module-owned contribution and declarative visibility;
- no central business route switch;
- provenance retention on uninstall;
- exact manifest/binding/compiled-route parity;
- one unchanged user-authored SHA green across all 17 applicable workflows.

Phase 8A.10 was accepted on source SHA `f92d101206886e3ceaf94d0e56e52580cec21093` and merged through PR #137 as `150e44b95d9dbdc08c1792563de03ec73f34aed1`.
