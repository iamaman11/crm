# Party Relationships Privacy Scope Owner Packet

Status: **Ready after Contact Points PR #181 and post-merge synchronization**  
Parent program: #126  
Prerequisites: Parties PR #156, Consents PR #175, shared support PR #176, Customer Accounts PR #179, Contact Points PR #181  
Coordinate: `party_relationships.privacy.scope.contribute@1.0.0`

## 1. Objective

Implement `crm.party-relationships` as the next authoritative privacy-scope owner contribution while preserving strict module ownership and the accepted contract-only owner protocol.

The packet is contract-only and non-runtime. It must prove that Party Relationships can consume `crm-customer-privacy-owner-scope-support` as a fifth independently validated owner without moving two-endpoint relationship semantics, SQL, matching, pagination, evidence classification, retention or errors into shared code.

## 2. Why Party Relationships is next

Party Relationships is selected because:

- the owner module already governs typed temporal Party-to-Party relationship state and hierarchy foundations;
- the exact privacy owner coordinate was published in PR #155 and remains non-runtime;
- every authoritative resource has two Party endpoints rather than one direct owner reference or embedded association collection;
- directional and reciprocal relationships require different canonical endpoint semantics while remaining one owner resource shape;
- relationship state may reveal another person or organization, role, hierarchy, household, employment, advisor, guarantor or partner context, making reference-only output and byte-level non-disclosure mandatory;
- the accepted shared request, lineage and canonical-Party proof can be reused without changing public contracts or promoting discovery runtime.

Selection does not authorize Party Relationship behavior to enter shared support.

## 3. Frozen ownership boundary

`crm.party-relationships` owns:

- Party Relationship identity;
- authoritative `from_party_ref` and `to_party_ref` endpoints;
- relationship type code;
- directional or reciprocal semantics;
- canonical from-role and to-role codes;
- Active/Inactive lifecycle state;
- optional validity interval;
- canonical timestamps and version;
- reciprocal endpoint ordering invariants and reserved built-in relationship semantics.

It does not own:

- canonical Party identity or Identity Resolution topology;
- Account, Contact Point, Consent or other owner state;
- Customer Privacy cases, discovery, planning, restrictions, legal holds or orchestration;
- hierarchy projections, search indexes, graph caches or Customer 360 composition.

The adapter must not read or mutate another module’s storage directly. Canonical subject Party proof is obtained only through accepted transaction-scoped shared support.

## 4. Exact implementation boundary

The implementation PR must:

1. validate the exact capability definition, `QueryRequest`, Protobuf wrapper, input contract and semantic input hash;
2. open exactly one tenant-bound PostgreSQL `REPEATABLE READ, READ ONLY` transaction in the Party Relationships adapter;
3. pass the already-open transaction to shared support for common lineage, registry, purpose, effective-time, page-size and canonical Party proof;
4. scan only authoritative `party-relationships.party_relationship` records owned by `crm.party-relationships`;
5. strictly validate persistence-envelope metadata, record type, tenant, record identity, version and the full Party Relationship domain state;
6. include a resource only when the rehydrated canonical Party equals either authoritative endpoint;
7. treat `from` and `to` endpoint matching symmetrically for privacy enumeration while preserving directional/reciprocal owner invariants during rehydration;
8. fail closed on malformed endpoint references, relationship type semantics, directionality, role codes, status, validity, timestamps, reciprocal ordering or version;
9. emit deterministic reference-only resources containing stable Party Relationship identity/version and owner-approved evidence metadata only;
10. use bounded owner-specific keyset pagination, continuation cursor and page/cursor digest domains when more records remain;
11. write no records, relationships, business transactions, idempotency, events, outbox or audit data;
12. remain absent from public HTTP/gRPC ingress, worker registration and production runtime inventories.

## 5. Owner-specific decisions required before coding

The implementation must derive and freeze from current Party Relationships contracts and persistence semantics:

- the exact authoritative record type and metadata contract;
- stable record ordering and maximum scanned candidates per page;
- default and maximum page sizes;
- cursor fields, binding inputs, encoding and stable error behavior;
- whether matching both endpoints is always sufficient or whether any accepted owner state has additional subject-association semantics;
- treatment of Active and Inactive relationships;
- treatment of relationships outside their validity interval at `effective_request_at_unix_ms`;
- treatment of directional versus reciprocal resources for evidence classification;
- Party Relationship data class;
- evidence class and canonical retention-policy identity;
- whether inactive, expired or superseded relationship references remain required evidence;
- exact owner-prefixed public error codes, categories, retryability and safe messages.

These decisions remain Party Relationships-owned. Customer Privacy and shared support must not infer or redefine them.

## 6. Reference-only and non-disclosure rule

The response must never contain or permit reconstruction of:

- the other endpoint Party identifier;
- whether the canonical subject appeared as the `from` or `to` endpoint;
- relationship type codes such as employment, household, parent/subsidiary, partner, advisor or guarantor;
- from-role or to-role codes;
- directionality;
- Active/Inactive state;
- validity timestamps;
- raw persisted JSON, hierarchy data or relationship contents.

PostgreSQL acceptance must inspect encoded response bytes for fixture values from directional and reciprocal relationship shapes, both endpoint positions, role/type codes and counterpart Party identifiers.

## 7. Shared-support non-regression rule

The Party Relationships adapter may become an approved consumer of `crm-customer-privacy-owner-scope-support` only together with its independently proven implementation and architecture-policy update.

No shared-support change is allowed merely to reduce Party Relationships code. A shared extension is permitted only when the implementation proves behavior that is:

- semantically identical across Parties, Consents, Customer Accounts and Contact Points;
- independent of relationship SQL, endpoint matching, directionality, roles, validity, classification and pagination;
- covered by compatibility and regression tests for every accepted consumer;
- behavior-neutral for existing response bytes, digests and owner-prefixed errors.

Otherwise the behavior remains owner-specific.

## 8. Explicit exclusions

This packet does not:

- promote `party_relationships.privacy.scope.contribute@1.0.0` to runtime;
- register or implement `customer_privacy.scope.discover@1.0.0`;
- add a Customer Privacy or Party Relationships worker;
- implement `action.apply`, deletion, anonymization or restriction execution;
- change accepted Protobuf contracts;
- add a production schema migration unless an independently demonstrated persistence defect makes one unavoidable;
- change relationship ownership, endpoint, directionality, role, validity or hierarchy semantics;
- read Party, Account, Consent or another owner’s storage directly;
- generalize endpoint matching, cursor, pagination, evidence or retention policy across owners;
- implement another privacy owner in the same PR.

## 9. Deferred runtime blockers

This behavior-neutral owner packet records but must not fix:

- deterministic cursor digests are not secret-backed HMACs and must not be treated as authenticated untrusted-client cursors;
- separate page requests do not share a dataset generation or cross-page snapshot boundary under concurrent owner mutation;
- `effective_request_at_unix_ms` remains a validated lineage field and is not yet a general SQL as-of condition;
- exact Rust toolchain and PostgreSQL image pinning belong to a separate maintenance packet.

Any runtime promotion requires these decisions to be resolved or explicitly bounded in a separate architecture packet.

## 10. Required tests

### Unit and contract tests

- exact coordinate, version, owner module and contract-only classification;
- exact input wrapper and semantic SHA-256 integrity;
- common lineage error mapping to stable Party Relationships prefixes;
- strict cursor acceptance, rebinding and corruption rejection;
- deterministic page and cursor digest vectors;
- deterministic reference-only response construction;
- evidence classification and retention identity derived from Party Relationships policy;
- response-byte exclusion for counterpart Party identifiers, endpoint position, relationship types, roles, status and validity;
- sparse-page behavior when scanned relationships do not include the canonical Party.

### PostgreSQL acceptance

On a clean database and again after full rollback/schema removal/reapply, prove:

- authoritative Party and Party Relationship fixtures through accepted owner paths;
- zero, one and multiple relationships for the canonical Party;
- matching when the canonical Party is the `from` endpoint and when it is the `to` endpoint;
- directional and reciprocal resources with canonical reciprocal ordering;
- first, subsequent and terminal bounded pages with no duplicates;
- inclusion of all owner-approved lifecycle/validity shapes;
- exclusion of unrelated same-tenant relationships;
- cross-tenant concealment under tenant binding and RLS;
- current Identity Resolution generation acceptance and stale-generation rejection;
- canonical redirect rejection;
- strict metadata and full-domain corruption fail-closed behavior;
- cursor rebinding and corruption rejection;
- no counterpart Party IDs, endpoint position, type/role/status/validity values in encoded response bytes;
- zero writes to records, relationships, transactions, idempotency, events, outbox and audit surfaces for success and rejection paths.

### Repository gates

- architecture policy permits the shared-support dependency only for the exact Party Relationships adapter manifest;
- `begin_bound_read_transaction` remains limited to proven owner adapter paths;
- Parties, Consents, Customer Accounts, Contact Points and Party Relationships privacy workflows all trigger when shared support changes;
- permanent Party Relationships workflow performs architecture, formatting, focused Clippy/tests, clean acceptance, rollback/schema removal, reapply, repeated acceptance and workspace dependency checks;
- Rust CI, Affected Scope CI, Governance CI, Database CI, Complexity Baseline and every other applicable permanent workflow pass on one unchanged user-authored SHA;
- every temporary validator, normalizer, dispatcher, diagnostic file and script is absent from the accepted diff.

## 11. Completion rule

The packet is complete only after its implementation PR is merged to `main`, exact-head source and merge SHAs are recorded in issue #126, permanent owner CI is authoritative, and post-merge documentation selects the following bounded owner.

Completion does not authorize Customer Privacy discovery, planning, approval, restriction, legal-hold, action execution or worker runtime. Those remain separate packets after a sufficient owner set is explicitly reviewed and accepted.
