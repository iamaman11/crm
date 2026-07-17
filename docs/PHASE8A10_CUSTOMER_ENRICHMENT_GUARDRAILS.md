# Phase 8A.10 — Customer Enrichment Delivery Guardrails

Status: **Normative packet guardrails for #125**

Architecture: `PHASE8A10_CUSTOMER_ENRICHMENT_ARCHITECTURE.md`

## 1. Packet invariant

`crm.customer-enrichment` coordinates governed external enrichment evidence. It never becomes a second Party, Account, Contact Point, Consent, Identity Resolution or Data Quality owner.

No accepted suggestion changes authoritative state until the exact current owner version is revalidated, policy/approval requirements are satisfied, live authorization is repeated and an exact owner capability succeeds.

## 2. First-slice limit

The first production slice is deliberately narrow:

- target owner: `crm.parties`;
- target resource: Party;
- target field: display name;
- authoritative mutation: `parties.party.update@1.0.0`;
- review: explicit reviewer acceptance;
- approval: required when the versioned policy marks the change high risk;
- provider: deterministic sanitized fake provider in process acceptance plus one provider-neutral infrastructure interface;
- provider payload: never stored in audit/events/logs; optional governed protected evidence only when policy permits.

Additional fields or owners require an explicit later scope change with contracts, owner-capability mapping and acceptance evidence.

## 3. Required owned records

The initial module foundation must use stable record identities for:

- `customer_enrichment.provider_profile_version`;
- `customer_enrichment.mapping_version`;
- `customer_enrichment.request`;
- `customer_enrichment.provider_response_receipt`;
- `customer_enrichment.suggestion`;
- `customer_enrichment.review_decision`;
- `customer_enrichment.application_attempt`;
- `customer_enrichment.provider_usage_entry`.

Provider circuit-breaker internals, HTTP connection state and secret material are infrastructure state, not business records. Durable quota/reconciliation facts that affect customer-enrichment decisions are recorded as bounded provider-usage evidence.

## 4. Contract freeze checklist

Before public Protobuf is merged, the implementation must freeze:

- exact aggregate and value-object identities;
- request lifecycle and retry transitions;
- suggestion lifecycle and immutable-history semantics;
- observed/retrieved/effective/freshness/expiry timestamp definitions;
- purpose, consent, licensing, residency, retention and permitted-use representations;
- exact provider replay identity;
- exact target idempotency identity;
- safe typed error vocabulary;
- public versus worker-only route classification;
- signed cursor filter/sort/page binding;
- approval evidence binding and expiry.

## 5. Provider adapter restrictions

Provider adapters may:

- resolve approved secret handles;
- perform provider authentication and network I/O;
- enforce provider timeouts, rate limits and circuit isolation;
- translate provider responses into a bounded canonical adapter result;
- produce sanitized correlation and metering evidence;
- store a protected raw payload only through an approved governed evidence/file port.

Provider adapters must not:

- call Party or other owner storage;
- decide reviewer acceptance;
- bypass policy or approval;
- emit raw provider payloads to logs, errors, audit or events;
- create arbitrary dynamic routes;
- expose credential values to the module core;
- silently reinterpret an immutable mapping version.

## 6. Mapping restrictions

Mappings are immutable data, not executable programs. The first version may support only a bounded canonical vocabulary sufficient for Party display-name suggestions.

Forbidden mapping features:

- arbitrary SQL;
- arbitrary JavaScript or shell execution;
- untrusted WASM;
- filesystem access;
- arbitrary network access;
- recursive/unbounded expressions;
- direct target-owner mutation instructions;
- hidden provider-specific defaults outside the immutable mapping version.

## 7. Authorization and policy order

Mutation execution order must remain:

```text
authentication
  -> durable module activation
  -> typed validation
  -> pre-authorization Party/Consent semantic validation
  -> versioned policy and approval validation
  -> final live authorization
  -> exact owner capability invocation
  -> immutable enrichment outcome evidence
```

Query execution order must remain:

```text
authentication
  -> durable module activation
  -> typed validation
  -> pre-authorization semantic validation
  -> live resource/field visibility
  -> authoritative enrichment read
```

A projection, search index, provider cache or possession of an enrichment ID is never authority.

## 8. Replay and crash-window rules

### Provider dispatch

A deterministic provider idempotency key is required when the provider supports one. When it does not, the adapter must expose the uncertainty explicitly and the worker must not claim exactly-once provider charging.

### Provider response

The same replay identity and same canonical digest is idempotent. The same replay identity and a different digest is a conflict requiring operator-visible reconciliation.

### Suggestion materialization

The deterministic suggestion identity includes tenant, request, mapping version, target coordinate and canonical proposed-value digest. Replay cannot create a second logical suggestion.

### Owner application

The target idempotency key is deterministic from tenant, suggestion, application generation and owner capability version. A target-success/outcome-missing restart must recover without a second Party update.

## 9. Disable and uninstall rules

When the provider profile is disabled:

- no new dispatch occurs;
- existing evidence remains queryable when the module is active and the actor is authorized;
- no automatic Party rollback occurs;
- retryable requests remain paused or fail with a typed provider-disabled outcome according to their policy.

When the module is disabled or uninstalled:

- exact routes and workers are inactive through durable `crm.module_installations` state;
- core customer-master paths remain operational;
- business provenance is retained;
- no bootstrap bypass may reactivate routes or workers;
- reinstall requires policy/provider revalidation before retryable work resumes.

## 10. Safe error surface

Public errors must be typed and bounded. At minimum distinguish:

- invalid request;
- module inactive;
- provider profile unavailable/disabled;
- provider adapter unavailable;
- secret handle unavailable;
- provider quota exceeded;
- provider circuit open;
- provider retryable failure;
- provider terminal failure;
- mapping conflict/unsupported response;
- purpose/consent/license/residency/retention policy denial;
- suggestion expired/superseded/not accepted;
- approval missing/invalid/expired;
- target reference unavailable;
- stale target version;
- authorization denied;
- conflicting replay;
- internal persistence/corruption failure.

Provider body fragments, URLs with credentials, headers, tokens, protected documents and arbitrary upstream error text are never returned.

## 11. Mandatory process scenarios

Fresh-PostgreSQL real `crm-api` acceptance must cover:

1. publish provider profile and mapping;
2. create and replay an enrichment request;
3. reject a missing/cross-tenant Party through one safe surface;
4. provider dispatch and duplicate-response replay;
5. materialize one deterministic Party display-name suggestion;
6. list/get with signed cursor and live field visibility;
7. reject unauthorized review/application;
8. reject purpose/consent/license policy failure;
9. reject expired or superseded suggestion;
10. reject stale Party version;
11. apply an accepted suggestion through `parties.party.update@1.0.0`;
12. restart after Party target success and before enrichment outcome commit;
13. prove exactly one Party update and one logical application outcome;
14. disable provider and prove Party/customer-master paths remain healthy;
15. disable/uninstall module and prove routes/workers stop without deleting provenance;
16. prove no credential or raw provider payload appears in logs, audit or events;
17. prove FORCE RLS and cross-tenant concealment.

## 12. Completion blockers

The packet remains `In progress` while any of these are missing:

- pure module ownership/invariants;
- compatible public contracts and generated bindings;
- exact production contributions and activation gating;
- provider infrastructure boundary;
- PostgreSQL persistence and FORCE RLS;
- live authorization and field visibility;
- exact Party capability application;
- provider and target crash recovery;
- fresh-process acceptance;
- exact-head green applicable workflows;
- synchronized roadmap/status/catalog/issue/PR evidence.

A valid manifest, scaffold, fake provider or library-only test is not a production vertical slice.
