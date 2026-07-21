# Phase 8A.10 — Customer Enrichment Delivery Guardrails

Status: **Accepted completed guardrails for #125**

Architecture: `PHASE8A10_CUSTOMER_ENRICHMENT_ARCHITECTURE.md`  
Accepted source checkpoint: `f92d101206886e3ceaf94d0e56e52580cec21093`  
Merge: PR #137 / `150e44b95d9dbdc08c1792563de03ec73f34aed1`

## 1. Packet invariant

`crm.customer-enrichment` coordinates governed external enrichment evidence. It never becomes a second Party, Account, Contact Point, Consent, Identity Resolution or Data Quality owner.

No suggestion changes authoritative state until the exact current owner version is revalidated, policy/approval requirements are satisfied, live authorization is repeated and an exact owner capability succeeds.

## 2. Frozen first-slice boundary

- target owner: `crm.parties`;
- target resource: Party;
- target field: display name;
- authoritative mutation: `parties.party.update@1.0.0`;
- review: explicit reviewer acceptance;
- approval: required when versioned policy marks the change high risk;
- provider transport: exact host-owned registry, initially `registry_http:registry_http_v1@1.0.0`;
- provider payload: never placed in public errors, audit, events or logs.

Additional fields, owners or transports require a separately governed scope change and complete acceptance evidence.

## 3. Frozen production inventory

The exact accepted inventory is:

- 6 public mutations;
- 6 permission-aware public queries;
- 5 activation-gated worker-only coordinates;
- zero completed Customer Enrichment non-runtime coordinates.

Worker phases are deterministic:

1. phase 240 — dispatch and response reconciliation;
2. phase 245 — suggestion materialization;
3. phase 250 — Party application and outcome recovery.

No worker-only coordinate may gain public HTTP/gRPC ingress without a separately reviewed inventory expansion.

## 4. Required owned records

Stable immutable or governed identities exist for:

- `customer_enrichment.provider_profile_version`;
- `customer_enrichment.mapping_version`;
- `customer_enrichment.request`;
- `customer_enrichment.provider_response_receipt`;
- `customer_enrichment.provider_response_conflict`;
- `customer_enrichment.suggestion`;
- `customer_enrichment.review_decision`;
- `customer_enrichment.application_attempt`;
- `customer_enrichment.provider_usage_entry`.

Provider connection state, circuit internals and secret material remain infrastructure state. Business-relevant quota/reconciliation outcomes are retained as bounded evidence.

## 5. Pure-core and infrastructure restrictions

The pure module core must not contain SQLx/PostgreSQL, arbitrary HTTP, provider SDKs, secret-store clients, process scheduling, filesystem/object-storage clients, another module's internals or executable user mapping code.

Provider infrastructure may resolve approved secret handles, perform bounded network I/O, enforce timeouts/quotas/circuits and translate responses into sanitized canonical evidence. It must not write owner storage, decide review, bypass policy/approval, expose credential values or reinterpret immutable mappings.

Mappings are immutable bounded data. Arbitrary SQL, JavaScript, shell, untrusted WASM, filesystem/network access, recursive expressions and direct target mutations are forbidden.

## 6. Authorization and policy order

Public mutation execution remains:

```text
authentication
  -> durable module activation
  -> typed validation
  -> governed Party/Consent semantic validation
  -> transaction-scoped exact reference guards
  -> versioned policy/approval validation
  -> final live authorization
  -> atomic persistence or exact owner capability
  -> immutable outcome evidence
```

Public query execution remains:

```text
authentication
  -> durable module activation
  -> typed validation
  -> live resource/field visibility
  -> authoritative enrichment read
```

Worker execution repeats live authorization at each protected dispatch, response, materialization or application boundary. A projection, search index, cache or identifier is never authority.

## 7. Replay and crash-window rules

### Provider dispatch

A pending dispatch is committed before external I/O. Recovery reuses the exact provider idempotency key. When a provider cannot support idempotency, uncertainty is explicit and exactly-once charging is never claimed.

### Provider response

The same replay identity and same canonical evidence is an exact duplicate. Distinct replay identity with semantically identical evidence is a semantic duplicate. Changed canonical class, digest, metering or protected-evidence reference under the same lineage is a fail-closed conflict.

### Materialization

Suggestion identity includes tenant, request, receipt, mapping, target coordinate and canonical proposed-value digest. Only finalized immutable evidence advances the checkpoint; replay cannot create a second logical suggestion.

### Owner application

Target idempotency derives from tenant, suggestion, application generation and exact owner capability. Target-success/outcome-missing recovery must produce one Party update and one logical outcome.

## 8. Disable and uninstall rules

When a provider profile is disabled, no new dispatch occurs; existing provenance remains available to authorized actors; no Party rollback occurs; retryable requests pause or receive a typed provider-disabled outcome.

When the module is disabled or uninstalled:

- public routes and all phase 240/245/250 workers stop through durable `crm.module_installations` state;
- core customer-master owner paths continue;
- provenance is retained;
- no bootstrap bypass can reactivate work;
- reinstall revalidates policy/provider state before retry.

## 9. Safe error surface

Public and worker errors are typed and bounded. They distinguish invalid request, inactive module/provider, unavailable adapter/secret, quota/circuit, retryable/terminal provider failure, mapping conflict, policy/Consent/license/residency/retention denial, expired/superseded suggestion, approval failure, missing/stale target, authorization denial, conflicting replay and persistence corruption.

Provider body fragments, credential-bearing URLs, headers, tokens, protected documents and arbitrary upstream text never leave infrastructure boundaries.

## 10. Persistence and isolation

All protected Customer Enrichment records use tenant-scoped ENABLE + FORCE RLS. Application roles are `NOBYPASSRLS`; no-context/cross-tenant reads are concealed and cross-tenant writes fail.

Mapping publication atomically locks/revalidates the exact immutable provider profile. Request creation atomically locks/revalidates the exact Party row/version. Reference guards cannot commit independently, perform external I/O or mutate referenced resources.

State, idempotency, outbox, audit and business-transaction evidence are atomic. Persisted state rejects corrupt identities, unknown versions and non-canonical values. Rollback/reapply restores identical RLS enforcement.

## 11. Mandatory acceptance topology

Production acceptance is intentionally split by exposure boundary:

1. the real `crm-api` binary on fresh PostgreSQL exercises actual public HTTP/gRPC ingress, successful Party/profile/mapping/request persistence, exact reference guards, redaction, concealment, Consent, activation and authorization denials;
2. fresh-PostgreSQL provider process tests exercise exact transport/secret selection, quota, circuit, timeout, sanitized failures, dispatch/response replay and conflict recovery;
3. fresh-PostgreSQL materialization/review/application tests exercise finalized evidence, suggestion lifecycle, approval, stale Party rejection and target-success/outcome-missing recovery;
4. background registry tests prove exact 240 → 245 → 250 order and disable/uninstall shutdown;
5. manifest/binding/public-route/worker-route parity proves every coordinate has exactly one production classification.

Worker-only coordinates are not exposed publicly merely to satisfy process tests.

## 12. Completion evidence

Phase 8A.10 is Complete because:

- pure ownership and layer boundaries are implemented;
- all 17 contract coordinates are classified as public runtime or worker runtime;
- exact public/worker composition and durable activation are implemented;
- concrete exact-coordinate provider transport and secret isolation are implemented;
- FORCE RLS, visibility, policy, Consent and owner-capability guards are implemented;
- provider/materialization/application crash recovery is proven;
- real process and fresh-PostgreSQL acceptance is permanent;
- accepted source SHA `f92d101206886e3ceaf94d0e56e52580cec21093` passed all 17 workflows unchanged;
- PR #137 merged as `150e44b95d9dbdc08c1792563de03ec73f34aed1` with no unresolved review blockers.

A completed production integration slice is not product completeness. Additional provider breadth, target fields, user workflows and privacy lifecycle remain separate governed work.
