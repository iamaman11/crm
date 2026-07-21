# Phase 8A.10 — Governed Customer Enrichment and Provenance Architecture

Status: **Accepted completed implementation boundary for packet #125**

Parent program: Phase 8A / #28  
Delivery packet: #125  
Implementation PR: #137  
Accepted source checkpoint: `f92d101206886e3ceaf94d0e56e52580cec21093`  
Squash merge: `150e44b95d9dbdc08c1792563de03ec73f34aed1`

## 1. Objective

Deliver optional external customer-data enrichment without creating an alternate customer master, an unreviewed mutation path, or provider-specific business logic inside the application core.

`crm.customer-enrichment` is a provider-neutral owner/coordinator. It owns enrichment requests and immutable provider, mapping, response, conflict, suggestion, review, usage and owner-application evidence. Existing customer-master modules remain the only owners of authoritative customer values.

The accepted first production slice is Party display-name enrichment. Accepted values enter authoritative Party state only through exact governed capability `parties.party.update@1.0.0`.

## 2. Ownership boundary

### `crm.customer-enrichment` owns

- immutable provider-profile versions;
- immutable request/response mapping versions;
- enrichment-request lifecycle and exact target snapshot;
- replay-safe provider exchange receipts and reconciliation conflicts;
- immutable suggestions and source provenance;
- review and approval decisions;
- suggestion expiry and supersession evidence;
- immutable application attempts and outcomes;
- bounded provider-usage, retry, quota and recovery evidence.

### Existing owners retain authority

- `crm.parties` owns Party identity and mutable Party fields;
- `crm.customer-accounts` owns Account values and Party associations;
- `crm.contact-points` owns endpoint values, verification and preferences;
- `crm.consents` owns consent assertions and communication authorization;
- `crm.identity-resolution` owns duplicate/merge lineage and canonical resolution;
- `crm.data-quality` owns quality-governance evidence, not enriched customer values.

Customer Enrichment never writes another module's storage, invokes another module's internal adapter or treats a projection/search index as authoritative state.

## 3. Accepted production flow

```text
authorized enrichment request
  -> governed exact Party snapshot and policy/Consent validation
  -> transaction-scoped exact Party/profile reference guards
  -> provider-process pending dispatch commit
  -> exact host-owned provider transport and tenant secret resolution
  -> sanitized immutable response/usage/conflict evidence
  -> deterministic suggestion materialization
  -> governed review/approval
  -> exact Party version revalidation and final live authorization
  -> parties.party.update@1.0.0
  -> append-once enrichment application outcome
```

A stale Party version never silently rebases an accepted suggestion. Re-enrichment and a new explicit review are required.

## 4. Layer and dependency boundary

### Pure module core

`modules/crm-customer-enrichment` contains provider-neutral domain/application semantics only. It has no dependency on SQLx, PostgreSQL, arbitrary HTTP, provider SDKs, secret managers, filesystem/object-storage clients, another business module's internals or executable user mapping code.

### Application ports

The core uses narrow typed ports for:

- governed Party snapshot and exact-version validation;
- current Consent/purpose evidence;
- sanitized provider dispatch results;
- exact owner-capability invocation;
- time, deterministic identity, authorization, policy and approval verification.

Cross-owner semantic reads happen before final authorization. Live authorization is repeated immediately before provider disclosure/response persistence, materialization and authoritative owner mutation as applicable.

### Host-owned infrastructure

The process host owns:

- exact adapter kind/version and transport-key resolution;
- concrete HTTP transport construction;
- tenant-bound secret material resolution;
- endpoint allowlists, bounded request/response bodies, deadlines and redirect rejection;
- quotas and circuit isolation;
- PostgreSQL transaction-scoped reference guards;
- activation-gated phase-ordered worker registration.

No generic router or scheduler contains a provider-ID or Customer Enrichment capability switch.

## 5. Provider and secret boundary

Each immutable provider-profile version records stable identity, exact adapter kind/version, supported target fields, purpose/permitted-use/licensing/residency/retention policy, bounded timeout/retry/quota identifiers, credential handle aliases, effective window and content digest.

Secret values never enter module state, public contracts, logs, audit envelopes, events or provider evidence. The host resolves configured secret environment values only for an enabled exact adapter/transport coordinate and supplies minimum material to the transport.

The first concrete transport is exact coordinate:

- transport key: `registry_http`;
- adapter kind: `registry_http_v1`;
- adapter contract version: `1.0.0`.

It accepts only exact allowlisted HTTPS endpoints, except loopback HTTP for tests, disables redirects and proxies, bounds request/response sizes and exposes only sanitized typed failures.

## 6. Mapping boundary

Mapping versions are immutable bounded data, not executable programs. They bind an exact provider-profile version, canonical response field path, target owner/resource/field, normalization version, confidence/evidence requirements, maximum suggestion count and content digest.

Arbitrary SQL, JavaScript, shell, untrusted WASM, filesystem access, arbitrary network access, recursive expressions and direct owner-mutation instructions are forbidden.

Mapping publication uses the mapping as its primary aggregate and locks/revalidates the exact persisted provider-profile row inside the same PostgreSQL transaction. The guard verifies canonical persisted identity and target-field support without external I/O or referenced-record mutation.

## 7. Request, response and suggestion semantics

An enrichment request fixes tenant/actor context, exact Party ID/version, provider-profile and mapping versions, requested fields, purpose/legal basis/optional Consent evidence, policy version, deterministic identity/idempotency, deadlines/expiry and bounded lifecycle diagnostics.

Request creation uses the request as primary aggregate and locks the exact Party row/version in the same transaction before persistence.

Provider dispatch commits a deterministic pending attempt before external I/O. Recovery reuses the same provider idempotency lineage.

Every accepted provider response yields immutable sanitized evidence containing request/profile/mapping lineage, replay identity, canonical digest, response class, bounded correlation, timing and metering. Raw provider payload is never interpreted by the materialization process.

Reconciliation is deterministic:

- same replay identity and same canonical evidence: exact duplicate/no-op;
- distinct replay identity with semantically identical canonical evidence: semantic duplicate/no-op with evidence;
- same lineage with changed canonical class, digest, metering or protected-evidence reference: fail-closed conflict.

Only finalized response evidence may advance materialization. Missing, malformed, future or unresolved-conflict evidence stops execution without advancing the checkpoint.

Suggestions bind exact request/receipt/profile/mapping/Party version, proposed value digest, timestamps, confidence and policy/licensing/Consent/residency/retention evidence. Historical accepted, rejected, expired, superseded and applied provenance is immutable.

## 8. Review and owner application

The first Party display-name slice requires explicit reviewer acceptance. Approval evidence is additionally required when the versioned policy marks the change high risk. Decisions bind exact suggestion, proposed value digest, actor, Party version, owner capability, policy version and expiry.

Application commits a deterministic pending attempt before owner I/O, revalidates exact Party version and live authorization, invokes only `parties.party.update@1.0.0`, then appends one outcome.

When the Party mutation succeeds but outcome persistence is interrupted, restart uses the same target idempotency lineage and records one logical outcome without a second Party update.

## 9. Frozen production inventory

The machine-readable authority is `contracts/customer-enrichment-production-promotion.json`.

### Public mutations — exactly 6

- `customer_enrichment.provider_profile.publish@1.0.0`;
- `customer_enrichment.mapping.publish@1.0.0`;
- `customer_enrichment.request.create@1.0.0`;
- `customer_enrichment.request.cancel@1.0.0`;
- `customer_enrichment.suggestion.accept@1.0.0`;
- `customer_enrichment.suggestion.reject@1.0.0`.

### Permission-aware public queries — exactly 6

- `customer_enrichment.provider_profile.get@1.0.0`;
- `customer_enrichment.mapping.get@1.0.0`;
- `customer_enrichment.request.get@1.0.0`;
- `customer_enrichment.request.list@1.0.0`;
- `customer_enrichment.suggestion.get@1.0.0`;
- `customer_enrichment.suggestion.list_by_party@1.0.0`.

### Activation-gated worker-only coordinates — exactly 5

Provider process, phase 240:

- `customer_enrichment.request.dispatch@1.0.0`;
- `customer_enrichment.response.record@1.0.0`.

Materialization process, phase 245:

- `customer_enrichment.suggestions.materialize@1.0.0`.

Application process, phase 250:

- `customer_enrichment.party.display_name.apply@1.0.0`;
- `customer_enrichment.application.outcome.record@1.0.0`.

All 17 manifest-bound coordinates are public runtime or worker runtime. Worker-only coordinates have no public HTTP/gRPC ingress. No completed Customer Enrichment coordinate remains classified non-runtime.

## 10. Visibility, persistence and lifecycle

All six queries use module-owned resource/field visibility. Hidden resources are concealed with not-found semantics; confidential definition fields may be redacted by deployment ceilings. Possession of an enrichment identifier is not authority.

Protected Customer Enrichment records use tenant-scoped ENABLE + FORCE RLS. Application roles are `NOBYPASSRLS`; cross-tenant/no-context reads are concealed and cross-tenant writes are rejected. Rollback/reapply restores the same policies.

Mutations preserve atomic record, optimistic version, idempotency, outbox, audit and business-transaction evidence. Persisted state is strictly canonical and rejects corrupt identities or future/unknown state.

Disable/uninstall stops public routes and all three worker processes through durable `crm.module_installations`, retains provenance and leaves core customer-master owners operational. Reinstall revalidates policy/provider state before retryable work resumes.

## 11. Failure and recovery semantics

The implementation distinguishes safe typed outcomes for module/provider inactive, unavailable transport/secret, quota, circuit open, retryable/terminal upstream failure, mapping conflict, policy/Consent/license/residency/retention denial, expired/superseded suggestion, missing/invalid approval, stale Party, authorization denial, conflicting replay and corrupted persistence.

Provider bodies, credentials, headers, protected documents, URLs containing secrets and internal diagnostics never cross public transport boundaries.

Proven crash windows include:

1. dispatch committed before provider I/O and replay with the same provider idempotency key;
2. response evidence recorded before materialization and restart without duplicate receipts/suggestions;
3. Party update succeeded before application outcome and restart without duplicate Party mutation.

## 12. Acceptance topology

Permanent production evidence is split according to the production boundary:

- Application Runtime starts the real `crm-api` binary on a fresh PostgreSQL database and exercises actual HTTP/gRPC public ingress, successful Party/profile/mapping/request persistence, transaction guards, visibility, Consent, activation, authorization and bounded safe errors;
- Customer Enrichment Worker Process Runtime exercises exact provider transport, dispatch/response reconciliation, quota/circuit/failure isolation and recovery on fresh PostgreSQL;
- Customer Enrichment Review Process Runtime exercises deterministic materialization, review, approval and owner-application recovery on fresh PostgreSQL;
- background registry tests prove exact phase order 240 → 245 → 250 and durable disable/uninstall shutdown;
- Contract, Governance, Rust, Generated Sync, Database, Product Plane and remaining runtime workflows prove full repository compatibility on one unchanged SHA.

Worker-only coordinates are intentionally not exposed publicly for testing.

The accepted source SHA `f92d101206886e3ceaf94d0e56e52580cec21093` passed all 17 permanent workflows unchanged, had no unresolved review threads or change requests, and was merged through PR #137 as `150e44b95d9dbdc08c1792563de03ec73f34aed1`.

## 13. Explicit non-goals

Phase 8A.10 does not introduce autonomous unreviewed mutation, arbitrary-field patching, Account/Contact Point mutation, direct provider infrastructure in the pure core, executable mapping code, provider-owned authoritative customer records, projection/search authority, automatic identity merge or privacy deletion orchestration.

Privacy access/export/restriction/deletion/legal-hold orchestration remains Phase 8A.11 / #126.
