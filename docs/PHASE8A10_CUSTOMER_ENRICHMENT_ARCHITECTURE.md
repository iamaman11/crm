# Phase 8A.10 — Governed Customer Enrichment and Provenance Architecture

Status: **Accepted implementation boundary for active packet #125**

Parent program: Phase 8A / #28  
Delivery packet: #125  
Required baseline: issue #134 / PR #135 / merge `023fa5ef1d510d5bcc32222c739e6d58e5696fb8`

## 1. Objective

Deliver optional external customer-data enrichment without creating an alternate customer master, an unreviewed mutation path, or provider-specific business logic inside the application core.

The packet introduces one provider-neutral owner/coordinator module, `crm.customer-enrichment`. It owns enrichment requests, immutable provider and mapping definitions, suggestion provenance, review decisions, provider-exchange evidence and owner-capability application attempts. Existing customer-master modules remain the only owners of authoritative customer values.

The first production slice is Party-focused. Accepted Party display-name changes enter authoritative state only through the exact governed capability `parties.party.update@1.0.0`.

## 2. Ownership decision

### 2.1 `crm.customer-enrichment` owns

- immutable provider-profile versions;
- immutable request/response mapping versions;
- enrichment-request lifecycle and exact target snapshot;
- replay-safe provider exchange receipts and bounded reconciliation evidence;
- immutable suggestions and their source provenance;
- review and approval decisions;
- suggestion expiry and supersession evidence;
- immutable application-attempt evidence;
- deterministic retry, quota-accounting and crash-recovery evidence owned by the enrichment process.

The module may retain references to authoritative owner resources, exact resource versions, exact owner-capability coordinates, secret-handle aliases and governed file/evidence references. It does not own the referenced customer values, secret material or provider payload transport.

### 2.2 Existing owner modules retain authority

- `crm.parties` owns Party identity and mutable Party fields;
- `crm.customer-accounts` owns Account values and Party associations;
- `crm.contact-points` owns contact endpoint values and verification/preference state;
- `crm.consents` owns consent assertions and communication authorization;
- `crm.identity-resolution` owns duplicate/merge lineage and canonical resolution;
- `crm.data-quality` owns quality-governance evidence, not enriched customer values.

No enrichment handler may write another module's storage, invoke its adapter directly or reinterpret a projection as authoritative state.

### 2.3 First production slice

The first accepted application path is:

```text
authorized enrichment request
  -> exact Party snapshot read through a governed pre-authorization port
  -> provider adapter dispatch outside the business core
  -> immutable response receipt and suggestion provenance
  -> governed review/approval
  -> exact Party version revalidation
  -> parties.party.update@1.0.0
  -> immutable enrichment application outcome
```

The initial authoritative mutation surface is limited to Party display name. Additional Party, Account or Contact Point fields require later explicit contract and owner-capability expansion; they are not implied by this architecture document.

## 3. Layer and dependency boundary

### 3.1 Pure module core

`modules/crm-customer-enrichment` must contain provider-neutral domain/application semantics only. It may depend on stable CRM contracts and Module SDK boundaries. It must not depend on:

- SQLx, PostgreSQL clients or migration tooling;
- arbitrary HTTP clients or provider SDKs;
- cloud secret managers or raw credentials;
- filesystem or object-storage clients;
- another business module's internal crate;
- transport-generated request/response types as persisted state;
- generic scripting, SQL expressions or user-provided executable mapping code.

### 3.2 Application ports

The pure core operates through narrow typed ports for:

- authoritative Party snapshot and exact-version validation;
- current consent/purpose evidence when a provider policy requires it;
- provider dispatch and sanitized response retrieval;
- secret-handle resolution owned by infrastructure;
- governed file/evidence storage for permitted protected payload retention;
- exact owner-capability invocation;
- time, deterministic identity and policy/approval verification.

Cross-owner reads occur during pre-authorization semantic validation. Final live authorization is repeated immediately before protected disclosure and immediately before an authoritative owner mutation.

### 3.3 Infrastructure adapters

Provider SDKs, arbitrary HTTP, authentication protocols, credential retrieval, rate-limit clients, circuit breakers, vendor payload parsing and vendor-specific error mapping are infrastructure adapters outside the business core.

A provider adapter is selected through an infrastructure-owned exact adapter registry keyed by immutable provider-profile adapter kind/version. Generic application routers and worker schedulers must not branch on provider ID, module ID, capability ID or concrete adapter type.

## 4. Provider profile and secret boundary

Each immutable provider-profile version records:

- stable provider identity and display metadata;
- exact adapter kind and adapter contract version;
- supported target owner/resource/field coordinates;
- allowed purpose and permitted-use classifications;
- licensing identifier/version and usage restrictions;
- residency and region constraints;
- retention and raw-payload handling policy;
- request timeout, retry class, rate/quota policy identifiers;
- credential **handle aliases only**;
- activation state and effective window;
- immutable publication identity and content digest.

Secret values never enter module state, public contracts, logs, audit envelopes, outbox events or provider-exchange evidence. The infrastructure adapter resolves an approved secret handle at execution time and receives only the minimum credential material needed for that attempt.

Disabling a provider profile prevents new dispatch and retries that require the provider. It does not delete existing provenance or break Party, Account, Contact Point, Consent or other customer-master paths.

## 5. Mapping boundary

A mapping version is immutable and provider-neutral. It defines:

- exact provider-profile version;
- request field selection and canonical request representation;
- response field extraction into a bounded canonical suggestion vocabulary;
- target owner/resource/field coordinate;
- normalization identifier/version;
- confidence and evidence extraction rules;
- null/absence/error handling;
- maximum response and suggestion counts;
- content digest and publication evidence.

Mappings are declarative and bounded. Arbitrary SQL, JavaScript, WASM, templates with side effects, filesystem access and arbitrary network calls are forbidden in Phase 8A.10.

## 6. Enrichment request model

An enrichment request fixes:

- tenant and actor context;
- target owner/resource ID;
- exact authoritative target resource version;
- provider-profile and mapping versions;
- purpose, legal basis and optional consent evidence reference;
- requested fields;
- execution policy and approval policy version;
- deterministic request identity and idempotency key;
- creation, effective, deadline and expiry timestamps;
- lifecycle state and bounded diagnostics.

Request lifecycle:

```text
Draft/Created -> Queued -> Dispatched -> ResponseRecorded -> SuggestionsMaterialized
             -> Completed | FailedRetryable | FailedTerminal | Cancelled | Expired
```

Lifecycle transitions are monotonic except explicit retry transitions from `FailedRetryable`. Cancellation never erases provider or suggestion evidence already produced.

## 7. Provider response and raw payload handling

Every accepted provider response produces one immutable response receipt containing:

- request ID and provider-profile/mapping versions;
- provider request/response correlation identities after sanitization;
- retrieval timestamp and provider-observed timestamp when present;
- canonical response digest;
- safe status/error classification;
- metering/quota evidence;
- replay identity and reconciliation status;
- optional governed file/evidence reference when retention policy permits protected raw payload storage.

Raw provider payloads are never copied into audit details, logs, typed public errors or outbox event bodies. When protected raw payload retention is not permitted, only the canonical digest and bounded extracted provenance are retained.

Duplicate provider callbacks or retried responses with the same deterministic replay identity are no-ops when content matches and conflicts when content differs.

## 8. Suggestion and provenance model

Each immutable suggestion binds:

- enrichment request and response receipt;
- provider-profile and mapping versions;
- target owner/resource/field coordinate;
- exact source owner resource version observed for the request;
- proposed canonical value and value digest;
- observed, retrieved, effective, freshness and expiry timestamps with explicit semantics;
- confidence and bounded evidence references;
- purpose, legal-basis, consent, licensing, residency, retention and permitted-use evidence;
- deterministic logical suggestion identity;
- current review/application status derived from immutable decisions and attempts.

Suggestion lifecycle evidence supports:

- proposed;
- accepted;
- rejected;
- expired;
- superseded;
- applied;
- application failed/retryable.

Historical suggestions and provenance are never rewritten or deleted merely because a newer suggestion supersedes them.

## 9. Review and approval policy

A suggestion cannot mutate authoritative customer state merely because a provider returned it.

Review policy is deterministic and versioned. It evaluates at least:

- target field risk class;
- provider/mapping trust classification;
- confidence threshold;
- evidence completeness;
- purpose, license, consent and permitted-use constraints;
- authoritative owner version freshness;
- actor permission and resource visibility;
- approval requirement.

The first Party display-name slice requires an explicit reviewer acceptance. Approval evidence is additionally required when the configured policy classifies the change as high risk. Review and approval evidence is bound to the exact suggestion, proposed value digest, actor, target resource version, capability coordinate, policy version and expiry.

## 10. Exact owner-capability application

Accepted changes are applied only through an exact versioned owner capability.

For the first slice:

- target owner: `crm.parties`;
- capability: `parties.party.update@1.0.0`;
- precondition: exact current Party version equals the suggestion's accepted target version;
- target idempotency: deterministic key derived from tenant, suggestion ID, application generation and target capability version;
- mutation input: only the reviewed Party display-name change and exact optimistic version;
- outcome: immutable application-attempt evidence with target business transaction ID, resulting Party version and safe error classification.

A stale Party version fails safely and never silently rebases the suggestion. Re-enrichment or a new explicit review is required.

## 11. Planned public contract surface

The first published contract packet will use package `crm.customer_enrichment.v1` and exact version `1.0.0` coordinates.

### Public mutations

- `customer_enrichment.provider_profile.publish@1.0.0`;
- `customer_enrichment.mapping.publish@1.0.0`;
- `customer_enrichment.request.create@1.0.0`;
- `customer_enrichment.request.cancel@1.0.0`;
- `customer_enrichment.suggestion.accept@1.0.0`;
- `customer_enrichment.suggestion.reject@1.0.0`;
- `customer_enrichment.party.display_name.apply@1.0.0`.

### Public queries

- `customer_enrichment.provider_profile.get@1.0.0`;
- `customer_enrichment.mapping.get@1.0.0`;
- `customer_enrichment.request.get@1.0.0`;
- `customer_enrichment.request.list@1.0.0`;
- `customer_enrichment.suggestion.get@1.0.0`;
- `customer_enrichment.suggestion.list_by_party@1.0.0`.

### Internal worker-only mutations

- `customer_enrichment.request.dispatch@1.0.0`;
- `customer_enrichment.response.record@1.0.0`;
- `customer_enrichment.suggestions.materialize@1.0.0`;
- `customer_enrichment.application.outcome.record@1.0.0`.

Internal worker-only coordinates are compiled and parity-checked but excluded from public HTTP/gRPC mutation catalogs.

### Events

Planned immutable events include provider-profile/mapping publication, request lifecycle, response receipt, suggestion materialization/review/expiry and application completion/failure. Every event payload is bounded and excludes credentials and protected raw provider payloads.

Publishing these contracts requires the module manifest bindings and generated contract registry to be updated from the canonical Protobuf descriptor set; the registry is never hand-edited.

## 12. Native production contributions

A separately owned adapter/composition boundary will contribute:

- every exact public and internal mutation route;
- every exact public query route;
- pre-authorization Party/Consent semantic validators;
- declarative visibility fields for request, response receipt, suggestion, decision and application-attempt reads;
- activation-gated deterministic workers;
- exact route parity or an individually reasoned non-runtime classification.

Required worker phases are ordered as:

1. provider dispatch;
2. provider response reconciliation;
3. suggestion materialization/expiry;
4. accepted-suggestion application;
5. application outcome recovery.

Each worker is tenant-bounded, batch-bounded, deterministic, replay-safe and gated by durable `crm.module_installations` state plus the provider profile's effective activation state.

No generic capability router, query router or worker scheduler may gain a customer-enrichment or provider-specific switch.

## 13. Persistence and lifecycle

PostgreSQL adapter state must use tenant-scoped FORCE RLS for all protected enrichment records. Persistence must preserve:

- exact optimistic versions where state is mutable;
- immutable definitions, receipts, suggestions, decisions and attempts;
- deterministic uniqueness for requests, responses, suggestions and target attempts;
- atomic state/idempotency/outbox/audit evidence;
- safe persisted-state conversion independent from public wire schemas.

Module uninstall policy is `retain_business_records`. Disable/uninstall behavior:

- no new enrichment routes or workers execute while inactive;
- existing customer-master owner modules continue normally;
- retained enrichment provenance remains unchanged;
- no automatic owner rollback or mutation occurs;
- reinstall may resume retryable work only after exact policy and provider activation revalidation;
- secret handles may be revoked by infrastructure without deleting business provenance.

## 14. Failure and recovery semantics

The implementation must prove at least these crash windows:

1. provider accepted/dispatched the request but dispatch outcome was not durably recorded;
2. provider response was durably received but response receipt or suggestion materialization was incomplete;
3. owner capability succeeded but enrichment application outcome was not durably recorded.

Recovery uses deterministic replay identities and target idempotency. It must not duplicate provider charges where a provider supports an idempotency key, duplicate response receipts, duplicate suggestions, duplicate Party mutations or duplicate application evidence.

Provider unavailable, quota exceeded, circuit open, mapping invalid, secret unavailable, purpose/consent denied, license restricted, stale target version and authorization denied are distinct typed safe outcomes. Raw provider errors are sanitized at the adapter boundary.

## 15. Acceptance matrix

Completion requires one unchanged exact review SHA with all applicable gates green and evidence for:

- `python scripts/repo.py conformance`;
- manifest/binding/compiled route parity and immutable publication compatibility;
- pure-domain invariants and strict persisted-state rehydration;
- deterministic provider replay and duplicate-response handling;
- mapping immutability and bounded canonicalization;
- provenance retention across accepted, rejected, expired, superseded and applied suggestions;
- purpose, consent, licensing, residency, retention and permitted-use enforcement;
- stale target version rejection;
- reviewer and approval binding;
- exact Party owner-capability invocation with no direct storage write;
- target-success/outcome-missing recovery with no duplicate Party update;
- provider dispatch/response crash recovery and quota reconciliation;
- disabled provider and disabled/uninstalled module behavior;
- no secret or protected raw payload leakage in logs, errors, audit or events;
- live authorization, resource/field visibility and signed pagination;
- FORCE RLS and cross-tenant negative proof;
- migration clean apply, rollback/reapply or explicit compensation evidence;
- fresh-PostgreSQL real `crm-api` process acceptance including provider-failure scenarios;
- applicable Contract, Governance, Rust, Database, Application Runtime, Product Plane and specialized enrichment process workflows.

## 16. Explicit non-goals

This packet does not introduce:

- autonomous unreviewed customer-master mutation;
- generic arbitrary-field patching;
- direct Account or Contact Point mutation in the first slice;
- direct provider SDK/HTTP/secret access from the pure module core;
- arbitrary user code, scripts, SQL or generic expression execution;
- provider-owned authoritative customer records;
- search/projection state as an authorization or mutation oracle;
- automatic identity merge or duplicate resolution;
- deletion/privacy lifecycle orchestration, which remains Phase 8A.11 / #126;
- a central provider switch in generic routers or workers.

## 17. Delivery sequence

1. Add the governed module foundation and immutable ownership objects.
2. Publish compatible Protobuf contracts and manifest bindings.
3. Implement pure domain/application semantics and persisted-state conversion.
4. Add PostgreSQL capability/query adapters and pre-authorization owner/consent ports.
5. Add provider infrastructure adapter contracts, sanitized fake provider and deterministic workers.
6. Register exact module-owned routes, visibility and workers through native composition.
7. Add fresh-process PostgreSQL acceptance for success, denial, stale evidence, replay and crash recovery.
8. Synchronize roadmap, project status, Phase 8 plan, module catalog, issue and PR evidence.
9. Move to Gate review only after all applicable checks pass on one exact SHA.
