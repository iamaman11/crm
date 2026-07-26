# Contact Points Privacy Scope Owner Packet

Status: **Complete through PR #181**  
Parent program: #126  
Prerequisites: Parties PR #156, Consents PR #175, shared support PR #176, Customer Accounts PR #179  
Accepted source: `00c5b940326b14f5e4aab7d8c8b467ee688f6c9c`  
Merge: `96cd0cf548310592a0718c97242a724a29717a72`  
Permanent workflows: **24/24 successful on the unchanged accepted source**  
Coordinate: `contact_points.privacy.scope.contribute@1.0.0`

## 1. Objective

Implement `crm.contact-points` as the next authoritative privacy-scope owner contribution while preserving the accepted contract-only owner protocol and strict module ownership.

The packet is contract-only and non-runtime. It must prove that Contact Points can consume `crm-customer-privacy-owner-scope-support` as a fourth independently validated owner without moving Contact Point semantics, SQL, pagination, evidence classification, retention or errors into shared code.

## 2. Why Contact Points is next

Contact Points is selected because:

- the owner module already governs Party-associated Contact Point lifecycle and endpoint verification state;
- the exact privacy owner coordinate was published in PR #155 and remains non-runtime;
- one canonical Party may own zero or multiple Contact Points, providing another bounded multi-record owner shape;
- Contact Point state contains sensitive endpoint values and verification evidence that make reference-only output and byte-level non-disclosure especially important;
- the owner boundary is distinct from Consent and communication authorization, provider delivery state and Customer Privacy orchestration;
- the accepted shared request, lineage and canonical-Party proof can be reused without changing public contracts or promoting discovery runtime.

Selection does not authorize Contact Points behavior to enter the shared support package.

## 3. Frozen ownership boundary

`crm.contact-points` owns:

- Contact Point identity and authoritative Party reference;
- endpoint kind: Email, Phone, Postal, Web or Messaging;
- normalized and display values;
- Active/Inactive lifecycle state;
- preferred flag and validity interval;
- verification state and verification-evidence reference;
- canonical Contact Point version and timestamps.

It does not own:

- canonical Party identity or Identity Resolution topology;
- Consent or communication-authorization decisions;
- provider delivery state, inbox/outbox transport or omnichannel execution;
- Customer Privacy cases, discovery, planning, restrictions, legal holds or orchestration;
- derived search, projection or cache state.

The adapter must not read or mutate another module’s storage directly. Canonical Party proof is obtained only through the accepted transaction-scoped shared support boundary.

## 4. Exact implementation boundary

The implementation PR must:

1. validate the exact capability definition, `QueryRequest`, Protobuf wrapper, input contract and semantic input hash;
2. open exactly one tenant-bound PostgreSQL `REPEATABLE READ, READ ONLY` transaction in the Contact Points adapter;
3. pass the already-open transaction to shared support for common lineage, registry, purpose, effective-time, page-size and canonical Party proof;
4. scan only authoritative Contact Point records owned by `crm.contact-points`;
5. match only records whose rehydrated authoritative `party_ref` equals the canonical Party in the accepted lineage;
6. strictly validate persistence-envelope metadata, record type, tenant, record identity, version and full Contact Point domain state;
7. fail closed on malformed kinds, values, validity intervals, verification evidence, timestamps, version or Party references;
8. emit deterministic reference-only resources containing stable Contact Point identity/version and owner-approved evidence metadata only;
9. use bounded owner-specific keyset pagination, continuation cursor and page/cursor digest domains when more records remain;
10. write no records, relationships, business transactions, idempotency, events, outbox or audit data;
11. remain absent from public HTTP/gRPC ingress, worker registration and production runtime inventories.

## 5. Owner-specific decisions required before coding

The implementation must derive and freeze from current Contact Points contracts and persistence semantics:

- the exact authoritative record type and metadata contract;
- stable record ordering and maximum scanned candidates per page;
- default and maximum page sizes;
- cursor fields, binding inputs, encoding and stable error behavior;
- treatment of Active and Inactive records;
- treatment of records outside their validity interval at `effective_request_at_unix_ms`;
- treatment of verified and unverified records;
- Contact Point data class;
- evidence class and canonical retention-policy identity;
- whether inactive, expired or superseded endpoint references remain required evidence;
- exact owner-prefixed public error codes, categories, retryability and safe messages.

These decisions remain Contact Points-owned. Customer Privacy and shared support must not infer or redefine them.

## 6. Reference-only and non-disclosure rule

The response must never contain or permit reconstruction of:

- normalized endpoint values;
- display endpoint values;
- email addresses, phone numbers, postal addresses, URLs or messaging identifiers;
- preferred state;
- validity timestamps unless an existing owner evidence contract explicitly requires them;
- verification evidence references or verification timestamps;
- raw persisted JSON or association contents.

PostgreSQL acceptance must inspect encoded response bytes for fixture values from every exercised endpoint kind, not only decoded Protobuf fields.

## 7. Shared-support non-regression rule

The Contact Points adapter may become an approved consumer of `crm-customer-privacy-owner-scope-support` only together with its independently proven implementation and architecture-policy update.

No shared-support change is allowed merely to reduce Contact Points code. A shared extension is permitted only when the implementation proves behavior that is:

- semantically identical across Parties, Consents and Customer Accounts;
- independent of Contact Point SQL, lifecycle, verification, classification and pagination;
- covered by compatibility and regression tests for every accepted consumer;
- behavior-neutral for existing response bytes, digests and owner-prefixed errors.

Otherwise the behavior remains owner-specific.

## 8. Explicit exclusions

This packet does not:

- promote `contact_points.privacy.scope.contribute@1.0.0` to runtime;
- register or implement `customer_privacy.scope.discover@1.0.0`;
- add a Customer Privacy or Contact Points worker;
- implement `action.apply`, deletion, anonymization or restriction execution;
- change accepted Protobuf contracts;
- add a production schema migration unless an independently demonstrated persistence defect makes one unavoidable;
- change Contact Point ownership, normalization, verification or validity semantics;
- read Consent, communication-delivery or another owner’s storage;
- generalize cursor, pagination, evidence or retention policy across owners;
- implement another privacy owner in the same PR.

## 9. Deferred runtime blockers

This behavior-neutral owner packet records but must not fix:

- deterministic cursor digests are not secret-backed HMACs and must not be treated as authenticated untrusted-client cursors;
- separate page requests do not yet share a dataset generation or cross-page snapshot boundary under concurrent owner mutation;
- `effective_request_at_unix_ms` remains a validated lineage field and is not yet a general SQL as-of condition;
- exact Rust toolchain and PostgreSQL image pinning belong to a separate maintenance packet.

Any runtime promotion requires these decisions to be resolved or explicitly bounded in a separate architecture packet.

## 10. Required tests

### Unit and contract tests

- exact coordinate, version, owner module and contract-only classification;
- exact input wrapper and semantic SHA-256 integrity;
- common lineage error mapping to stable Contact Points prefixes;
- strict cursor acceptance, rebinding and corruption rejection;
- deterministic page and cursor digest vectors;
- deterministic reference-only response construction;
- evidence classification and retention identity derived from Contact Points policy;
- response-byte exclusion for all fixture endpoint values and verification references;
- sparse-page behavior when scanned records belong to another Party.

### PostgreSQL acceptance

On a clean database and again after full rollback/schema removal/reapply, prove:

- authoritative Party and Contact Point fixtures through accepted owner paths;
- zero, one and multiple Contact Points for the canonical Party;
- first, subsequent and terminal bounded pages with no duplicates;
- inclusion of all owner-approved lifecycle/verification shapes;
- exclusion of unrelated same-tenant Contact Points;
- cross-tenant concealment under tenant binding and RLS;
- current Identity Resolution generation acceptance and stale-generation rejection;
- canonical redirect rejection;
- strict metadata and full-domain corruption fail-closed behavior;
- cursor rebinding and corruption rejection;
- no values or verification references in encoded response bytes;
- zero writes to records, relationships, transactions, idempotency, events, outbox and audit surfaces for success and rejection paths.

### Repository gates

- architecture policy permits the shared-support dependency only for the exact Contact Points adapter manifest;
- `begin_bound_read_transaction` remains limited to proven owner adapter paths;
- Parties, Consents, Customer Accounts and Contact Points privacy workflows all trigger when shared support changes;
- permanent Contact Points workflow performs architecture, formatting, focused Clippy/tests, clean acceptance, rollback/schema removal, reapply, repeated acceptance and workspace dependency checks;
- Rust CI, Affected Scope CI, Governance CI, Database CI, Complexity Baseline and every other applicable permanent workflow pass on one unchanged user-authored SHA;
- every temporary validator, normalizer, dispatcher, diagnostic file and script is absent from the accepted diff.

## 11. Completion rule

The packet is complete through PR #181 on accepted source `00c5b940326b14f5e4aab7d8c8b467ee688f6c9c` and merge `96cd0cf548310592a0718c97242a724a29717a72`. `Contact Points Privacy Scope CI` is authoritative, all 24 applicable permanent workflows passed on the unchanged source, and the coordinate remains contract-only/non-runtime.

Completion does not authorize Customer Privacy discovery, planning, approval, restriction, legal-hold, action execution or worker runtime. Party Relationships is the next bounded contract-only owner packet, and those lifecycle capabilities remain separate until a sufficient owner set is explicitly reviewed and accepted.
