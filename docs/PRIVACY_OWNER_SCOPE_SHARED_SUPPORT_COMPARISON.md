# Privacy Owner Scope Shared Support — Accepted Boundary and Consumer Comparison

Status: **Shared boundary accepted through PR #176; fourth consumer accepted through PR #181**  
Accepted source: `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`  
Merge: `80411d54a3ca45a783d982152c5cd8317f1fd9bd`  
Permanent workflows: **21/21 successful on the unchanged accepted source**  
Accepted comparison baseline: **Parties PR #156 + Consents PR #175**  
Current proven consumers: **Parties + Consents + Customer Accounts PR #179 + Contact Points PR #181**  
Base `main`: `039d6461803208f6cb70ce0fbcfcaffaf59d7125`

## 1. Purpose

This document is the review contract for extracting privacy owner-scope support only after two independently accepted and materially contrasting owner implementations exist.

The extraction is behavior-neutral. It reduces duplicated protocol mechanics without changing any published contract, route classification, owner authority, public error code, SQL result, pagination rule, evidence classification, digest domain or response byte sequence.

The packet does not promote any privacy-scope coordinate to runtime and does not make `customer_privacy.scope.discover@1.0.0` production-reachable.

## 2. Accepted implementations compared

| Dimension | Parties | Consents | Extraction decision |
|---|---|---|---|
| Authoritative resource shape | One canonical Party record | Zero or more Consent records | Owner-specific |
| Discovery path | Direct record lookup | Party-to-Consent relationship traversal | Owner-specific |
| Pagination | Exactly one terminal page; non-empty cursor rejected | Owner keyset pagination with owner cursor | Owner-specific |
| Persisted-state validation | Strict Party metadata and domain rehydration | Strict Consent metadata, relationship consistency and domain rehydration | Owner-specific |
| Evidence class | `RETAIN_MINIMIZED_EVIDENCE` | `IMMUTABLE_REQUIRED_EVIDENCE` | Owner-specific |
| Retention policy | Canonical Party retention | Canonical Consent authorization retention | Owner-specific |
| Query request context/input validation | Identical protocol | Identical protocol | Shared now |
| Exact owner/capability/version/input-contract binding | Identical protocol, owner parameters differ | Identical protocol, owner parameters differ | Shared now |
| Input SHA-256 validation | Identical protocol | Identical protocol | Shared now |
| Tenant/case/canonical Party lineage validation | Identical protocol | Identical protocol | Shared now |
| Identity Resolution generation claim validation | Identical protocol | Identical protocol | Shared now |
| Canonical owner registry version/digest validation | Identical protocol | Identical protocol | Shared now |
| Purpose normalization and effective-time validation | Identical protocol | Identical protocol | Shared now |
| Page-size default and maximum validation | Identical protocol, constants supplied by owner | Identical protocol, constants supplied by owner | Shared now |
| Canonical Party topology proof in a supplied read-only transaction | Identical SQL and semantics | Identical SQL and semantics | Shared now |
| Length-framed SHA-256 primitive | Identical primitive | Identical primitive | Shared now |
| Stable error codes and safe messages | Parties namespace | Consents namespace | Owner-specific mapping |
| Response construction and digest field selection | Single-resource terminal evidence | Multi-resource paginated evidence | Owner-specific |

## 3. Shared support boundary

`crm-customer-privacy-owner-scope-support` may own only:

1. validation of `QueryExecutionContext` and typed input payload;
2. exact comparison with an owner-supplied `CapabilityDefinition`;
3. semantic input SHA-256 verification;
4. common privacy lineage validation;
5. canonical registry version and digest validation;
6. purpose-code and effective-request-time validation;
7. owner-supplied page-size default/maximum handling;
8. canonical Party generation, visibility and active-redirect proof inside a caller-supplied `BoundReadTransaction`;
9. deterministic length-framed SHA-256 primitives;
10. internal support error kinds that owners map to their existing stable `SdkError` contracts.

The support crate must not begin, commit or otherwise own the database transaction. The exact allowlist for `begin_bound_read_transaction` is restricted to the Parties, Consents, Customer Accounts and Contact Points PostgreSQL adapters.

The dependency boundary is also mechanical. `architecture-policy.json` names the Parties, Consents, Customer Accounts and Contact Points adapter manifests as the only required consumers, and `scripts/check_architecture.py` rejects missing allowed manifests, missing required consumers and every unexpected consumer. A future owner may use this crate only through an explicit policy change reviewed together with that independently proven owner implementation.

## 4. Owner responsibilities retained

Each owner adapter remains solely responsible for:

- decoding its exact public Protobuf wrapper;
- owner capability definition construction and definition validation;
- owner-specific public error codes, categories, retryability and safe messages;
- cursor acceptance, rejection, encoding and decoding;
- SQL and relationship traversal;
- persisted record metadata validation;
- aggregate/domain rehydration;
- resource identity and version validation;
- data-class, evidence-class and retention-policy decisions;
- response wrapper construction;
- page/cursor digest domain and field selection;
- output typed-payload contract;
- owner PostgreSQL and no-write acceptance.

No trait-based universal owner framework is introduced in this packet. A future interface may be selected only after the remaining owner implementations demonstrate that another stable abstraction is necessary.

## 5. Behavior-neutral parity requirements

For the same accepted fixture inputs, migration to shared support must preserve:

- successful response Protobuf bytes;
- resource order and versions;
- page number, scan count, emitted count and terminal flag;
- next cursor bytes;
- cursor and page digest bytes;
- output owner/schema/version/data class/encoding/size/retention metadata;
- exact error code;
- exact `ErrorCategory`;
- exact retryable flag;
- exact safe message;
- absence or presence of internal references where already observable to tests;
- zero writes to records, relationships, transactions, idempotency, outbox and audit surfaces.

The Parties, Consents, Customer Accounts and Contact Points permanent workflows remain authoritative. The shared crate is present in all four path-filter graphs so a change to common behavior re-runs every proven owner acceptance suite.

The compatibility proof is explicit rather than merely self-consistent. Shared conformance tests exercise every common request and lineage error kind; both owner suites assert exact owner-prefixed codes, categories and retryability after mapping; and `tests/digest_compatibility.rs` freezes the accepted Parties and Consents cursor/page digest bytes from the pre-extraction implementations. A future framing or field-order change therefore requires an intentional versioned protocol decision rather than silently changing evidence under the existing domains.

## 6. New crate justification

- **Protected boundary:** common privacy owner-scope protocol mechanics and canonical Party proof.
- **Isolated dependencies:** read-only PostgreSQL proof support, privacy contracts and Identity Resolution/Party identifiers.
- **Expected consumers:** Parties, Consents, Customer Accounts and Contact Points are proven; any additional consumer requires an explicit architecture-policy change with its independently accepted owner packet.
- **Why an internal module is insufficient:** the behavior is consumed by independent owner adapter crates and must not make either owner package authoritative for shared protocol semantics.
- **Lifecycle/extraction seam:** stable first-party privacy owner contribution support; not a runtime service or business owner.
- **Expected fan-out effect:** one additional shared dependency for owner adapters, offset by removal of duplicated validation, topology SQL and digest implementation.

## 7. Explicit non-goals

PR #176 does not:

- add or implement a third privacy owner;
- change Protobuf or manifest contracts;
- add migrations or fixtures;
- alter route classifications;
- register public HTTP/gRPC ingress;
- register a worker;
- promote `customer_privacy.scope.discover@1.0.0`;
- introduce Customer Privacy orchestration;
- unify Parties and Consents cursor behavior;
- unify evidence or retention decisions;
- resolve runtime cursor authentication or cross-page snapshot semantics.

## 8. Deferred pre-runtime decisions

The current Consents cursor uses deterministic SHA-256 binding, not a secret-backed MAC. This is sufficient for contract-only deterministic evidence but must not be treated as an authenticated untrusted-client cursor. Before any public or worker runtime promotion, the architecture must select a governed HMAC/signed continuation mechanism or prove that the cursor never crosses an untrusted boundary.

Each page currently uses a separate repeatable-read transaction. Before runtime promotion, the program must define whether enumeration is snapshot-consistent across pages, revision/generation-bound, bounded by an as-of marker, or intentionally reconciled under documented weak consistency.

`effective_request_at_unix_ms` is currently validated and bound into lineage/digests but does not independently create an owner-storage historical snapshot. Its normative meaning must be fixed before orchestration relies on it as an as-of query.

These decisions are deliberately excluded from the behavior-neutral extraction packet.

## 9. Acceptance gate for PR #176

The packet may enter Gate review only when:

1. this comparison and all current status/roadmap documents are synchronized;
2. the support crate contains only the accepted shared boundary;
3. Parties and Consents retain exact owner-specific error mapping;
4. the shared conformance suite, digest compatibility proof and both adapter unit suites pass;
5. both clean/reapplied PostgreSQL acceptance suites pass;
6. architecture policy still permits bound-read transaction creation only in the two owner adapters;
7. both permanent workflow path filters include the shared crate;
8. full workspace formatting, Clippy, tests, dependency checks and applicable governance checks pass on one unchanged SHA;
9. every temporary validator or normalizer workflow is absent from the authoritative candidate;
10. no runtime inventory, migration, public contract or owner implementation beyond Parties/Consents is added;
11. the PR is based on the accepted PR #175 merge and has no unresolved review thread.

## 10. Development sequence after this packet

After PR #176 is accepted and merged:

1. implement remaining owner privacy-scope contributions one owner at a time using the proven support boundary;
2. retain owner-specific authoritative reads, rehydration, classification and pagination;
3. compare new owners against the support contract and extend shared support only when another repeated behavior is proven, never speculatively;
4. complete the sufficient owner set required by `customer_privacy.scope.discover@1.0.0`;
5. separately implement scope discovery/planning orchestration and worker production proof;
6. promote approval and permission-aware plan/outcome reads only after a genuine reachable lifecycle exists;
7. implement immediate deny-only restrictions and legal-hold/retention precedence as separate trust-boundary packets;
8. add resumable owner execution, crash-window recovery, export/deletion convergence and erased-Party/no-orphan proof;
9. close Phase 8A only after complete privacy/customer-master interaction and real-process acceptance;
10. begin Phase 8B Catalog/Pricing/CPQ only from the completed Phase 8A baseline.


## 12. Fourth proven consumer — Contact Points PR #181

PR #181 accepted unchanged user-authored source `00c5b940326b14f5e4aab7d8c8b467ee688f6c9c`, passed all 24 applicable permanent workflows and was squash-merged as `96cd0cf548310592a0718c97242a724a29717a72`.

Contact Points independently proves the accepted shared boundary against a fourth authoritative resource shape:

- authoritative Contact Point records with a direct persisted Party reference;
- strict endpoint kind, normalized/display value, lifecycle, validity, verification, timestamp and version rehydration before evidence emission;
- inclusion of owner-approved Active/Inactive and verified/unverified shapes without exposing endpoint values or verification references;
- owner-specific bounded keyset scan, continuation cursor, page/cursor digest domains, evidence classification, retention and stable error prefixes;
- deterministic reference-only response bytes with email, phone, postal, URL, messaging and verification fixture values excluded;
- one caller-opened tenant-bound `REPEATABLE READ, READ ONLY` PostgreSQL transaction;
- clean migration, full rollback/schema removal, reapply and repeated acceptance with zero writes across record, relationship, transaction, idempotency, outbox and audit surfaces.

PR #181 changed no shared-support behavior. It only extended the mechanical consumer and bound-read allowlists to the independently proven Contact Points adapter. Party Relationships is the next candidate consumer and must earn the same bounded policy change without moving two-endpoint temporal relationship semantics into shared code.

## 11. Third proven consumer — Customer Accounts PR #179

PR #179 accepted unchanged user-authored source `7d3e44e6dede36f76dfe92145dea6129a2b4639e`, passed all 23 applicable permanent workflows and was squash-merged as `5b5252a437c6bebbd7afdead0162063af4c0b7e4`.

Customer Accounts independently proves the accepted shared boundary against a third authoritative resource shape:

- authoritative Account records rather than Party records or relationship-owned Consent records;
- embedded Account-owned Party associations, including both `Primary` and indirect `Member` matches;
- strict Account metadata and full domain-state rehydration before evidence emission;
- owner-specific bounded keyset scan, continuation cursor, page/cursor digest domains, evidence classification, retention and stable error prefixes;
- deterministic reference-only response bytes with Account names, status and association contents excluded;
- one caller-opened tenant-bound `REPEATABLE READ, READ ONLY` PostgreSQL transaction;
- clean migration, full rollback/schema removal, reapply and repeated acceptance with zero writes across record, relationship, transaction, idempotency, outbox and audit surfaces.

PR #179 changed no shared-support behavior. It only extended the mechanical consumer and bound-read allowlists to the independently proven Customer Accounts adapter. PR #181 later earned the same explicit policy change for Contact Points without extending shared behavior. The next candidate consumer is Party Relationships, but it must be proven independently in its own bounded packet; shared support must not be expanded speculatively.
