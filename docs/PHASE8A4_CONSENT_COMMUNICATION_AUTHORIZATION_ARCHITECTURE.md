# Phase 8A.4 — Consent and Communication Authorization Architecture

Status: **Normative delivery boundary for #112**  
Parent program: #28  
Depends on: merged Customer 360 #110 / PR #111 and stable Party, Account, Contact Point and Party Relationship owner contracts

## 1. Purpose

Phase 8A.4 establishes an authoritative, independently governed Consent and Communication Authorization owner domain.

The domain records purpose-specific communication assertions and answers whether a communication is currently authorized for a canonical Party, purpose, channel and optional Contact Point. It must keep legal-basis evidence and withdrawal history explainable without hiding authorization inside Contact Point verification, preference or Customer 360 state.

A preferred or verified Contact Point is **availability/endpoint state only**. It never authorizes communication by itself.

## 2. Ownership boundary

The Consent owner owns:

- immutable Consent Authorization identity;
- canonical subject `PartyRef`;
- optional canonical `ContactPointRef` scope;
- purpose code;
- communication channel;
- assertion effect: grant or deny;
- legal-basis code;
- jurisdiction code;
- source code;
- immutable proof/evidence reference;
- effective time and optional expiry time;
- active/withdrawn lifecycle for grant assertions;
- withdrawal timestamp and optimistic resource version;
- owner events required to explain authoritative lifecycle history.

The Consent owner does **not** own:

- Party identity attributes;
- Account membership or Party Relationship state;
- Contact Point endpoint value, verification, preference, availability or provider delivery state;
- campaign, journey, service-notification or provider send execution;
- generic suppression/provider bounce state;
- identity resolution, duplicate matching, merge/unmerge or survivorship;
- Customer 360 mutation;
- generic enterprise policy-engine state unrelated to customer communication authorization.

Pure owner code contains no SQL, transport/provider types or direct cross-owner storage access.

## 3. Authoritative assertion model

### 3.1 Immutable identity and scope

One `ConsentAuthorization` aggregate is one immutable assertion. The following fields never change after creation:

- authorization id;
- subject Party reference;
- optional Contact Point reference;
- purpose;
- channel;
- effect (`grant` or `deny`);
- legal basis;
- jurisdiction;
- source;
- evidence reference;
- effective time;
- optional expiry time.

Corrections, renewed consent, changed purpose, changed legal basis or changed evidence create a **new assertion with a new id**. Historical evidence is never rewritten in place.

### 3.2 Communication channels

The initial canonical communication channels are:

- email;
- phone;
- sms;
- postal;
- messaging;
- push.

This channel model is intentionally separate from `ContactPointKind`. For example, one Phone Contact Point may be technically usable for voice and SMS, but each communication authorization remains channel-specific.

No v1 `all` or wildcard channel exists. Broad authorization requires explicit channel assertions so evidence remains unambiguous.

### 3.3 Optional Contact Point scope

`contact_point_ref = None` means the assertion applies to the Party for the exact purpose and channel regardless of which eligible Contact Point is later selected.

`contact_point_ref = Some(...)` means the assertion is additionally scoped to that exact Contact Point.

Application composition validates that an optional Contact Point:

- exists in the same tenant;
- belongs to the referenced Party;
- is compatible with the requested communication channel where a deterministic mapping exists.

The pure owner aggregate does not perform those cross-owner reads.

### 3.4 Semantic codes

Purpose, legal basis, jurisdiction and source are bounded canonical semantic identifiers, not free-form display labels.

Initial rules:

- trim surrounding whitespace;
- lowercase ASCII canonicalization;
- allow ASCII alphanumeric characters plus `.`, `_` and `-`;
- reject separators at either edge;
- reject control characters and overlong values.

The owner validates shape, not jurisdiction-specific legal correctness. Tenant or regulatory policy may further restrict which codes are usable through governed application composition.

## 4. Lifecycle

### 4.1 Creation

Creation establishes exactly one immutable assertion:

- `effect = grant`; or
- `effect = deny`.

The aggregate starts at version 1 with:

- status `active`;
- `created_at = updated_at = occurred_at`;
- no withdrawal timestamp.

`effective_from` is required and may be equal to or later than creation time. `expires_at`, when present, must be strictly later than `effective_from`.

### 4.2 Withdrawal

The only v1 owner-state mutation is withdrawal of an active grant assertion.

Withdrawal:

- requires exact expected version;
- requires strictly increasing governed mutation time;
- records an immutable `withdrawn_at` timestamp;
- changes status to `withdrawn`;
- increments the optimistic version exactly once;
- rejects replay through the capability idempotency boundary rather than silently treating a second domain transition as success;
- is irreversible for that aggregate.

A withdrawn grant is never reactivated. A later renewed authorization is a new assertion with new evidence and a new id.

### 4.3 Deny assertions

A deny assertion is immutable in v1. It is active only during its effective/expiry window.

A later assertion may supersede it for a matching request according to the deterministic decision ordering below. The deny record itself is never mutated or deleted to make that happen.

### 4.4 No generic update mutation

V1 deliberately has no generic `update authorization` capability. Mutable scope/evidence would weaken auditability and make authorization history ambiguous.

Later packets may add a narrowly governed correction workflow only if legal/audit requirements prove that append-new-assertion semantics are insufficient. Published v1 contracts remain immutable.

## 5. Current-state evaluation

For an evaluation time `T`:

An active grant/deny assertion is current when:

- `effective_from <= T`; and
- `expires_at` is absent or `T < expires_at`.

A withdrawn grant is never an active grant. Its withdrawal remains an authoritative decision barrier for the same immutable scope until a later applicable assertion supersedes it.

The owner aggregate exposes deterministic current-state facts but does not scan other records.

## 6. Communication authorization decision

Communication authorization is a governed read over authoritative Consent records, not a rebuildable projection and not Contact Point state.

### 6.1 Request key

The decision request contains:

- Party reference;
- exact purpose;
- exact communication channel;
- optional exact Contact Point reference;
- governed evaluation time from the query execution context.

The caller cannot supply tenant identity.

### 6.2 Applicable assertions

An assertion is applicable when:

- Party matches exactly;
- purpose matches exactly;
- channel matches exactly;
- the assertion has no Contact Point scope, or the request contains that same Contact Point;
- the assertion has become effective by the evaluation time.

Expired active assertions do not authorize or deny current communication.

A withdrawn grant contributes a withdrawal decision point at `withdrawn_at` for its immutable scope. This prevents an older grant from becoming effective again merely because a newer grant was withdrawn.

### 6.3 Deterministic precedence

For each applicable assertion, the decision engine derives one decision point:

- active grant/deny → `effective_from`;
- withdrawn grant → `withdrawn_at`, with deny-like effect.

The decision engine:

1. finds the greatest decision-point timestamp not later than evaluation time;
2. considers every applicable assertion at that exact latest timestamp;
3. returns denied if any latest point is deny or withdrawal;
4. otherwise returns allowed when at least one latest point is an active grant;
5. otherwise returns denied by default.

This permits a later explicit grant with new evidence to supersede an older denial or withdrawal while making ties fail closed.

The v1 decision is exact-scope and time ordered. It does not implement probabilistic policy inference, campaign rules or provider suppression logic.

### 6.4 Explainability

Every decision returns:

- `allowed`;
- stable reason code;
- evaluated Party/purpose/channel/optional Contact Point scope;
- evaluation time;
- zero or more authoritative Consent Authorization references that determined the result.

Initial stable reason families:

- `active_grant`;
- `active_deny`;
- `withdrawn`;
- `no_applicable_grant`;
- `authorization_data_unavailable` for safe fail-closed dependency failure.

Internal policy or storage details are not exposed.

## 7. Cross-owner integrity

Application composition validates references before owner mutation:

- Party must exist in the same tenant;
- optional Contact Point must exist in the same tenant;
- optional Contact Point must belong to the referenced Party;
- deterministic channel compatibility is checked where applicable.

Missing and cross-tenant references return the same safe unavailable result without Consent side effects.

Real PostgreSQL/internal failures remain distinguishable internally and are never collapsed into a missing-reference response.

No Party or Contact Point owner module receives Consent dependencies or direct cross-owner storage access.

## 8. Persistence boundary

The owner persistence envelope is versioned, deterministic and private.

V1 persisted state contains only canonical owner state:

- authorization id;
- Party id;
- optional Contact Point id;
- purpose;
- channel;
- effect;
- legal basis;
- jurisdiction;
- source;
- evidence reference;
- effective/expiry times;
- active/withdrawn status;
- optional withdrawn-at time;
- created/updated times;
- optimistic version.

Strict decode must reject:

- unknown fields;
- noncanonical semantic identifiers;
- invalid enum values;
- impossible lifecycle/timestamp combinations;
- impossible version-1 shapes;
- non-positive versions;
- oversized state.

Persistence decode always rehydrates through domain invariants.

## 9. Public contract boundary

Expected initial owner capabilities:

- `consents.authorization.create@1.0.0`;
- `consents.authorization.withdraw@1.0.0`;
- `consents.authorization.get@1.0.0`;
- `consents.authorization.list@1.0.0`;
- `consents.communication.authorize@1.0.0`.

Expected initial owner events:

- `consents.authorization.created@1.0.0`;
- `consents.authorization.withdrawn@1.0.0`.

Exact Protobuf package/service/message names are finalized additively before publication.

The decision capability is read-only. It does not create provider sends, mutate Contact Points or write Customer 360 state.

## 10. Query and disclosure model

Get/list queries are permission-aware and repeat live resource/field visibility before disclosure.

List requires:

- deterministic signed cursor pagination;
- typed filters for Party, optional Contact Point, purpose, channel, effect and lifecycle status;
- deterministic updated-time ordering with stable id tie-break;
- tenant-bound cursor context.

Evidence references and legal-basis fields are independently redactable from basic lifecycle/scope fields.

The authorization decision capability may use authoritative storage reads after the query gateway's live capability authorization. It returns only the bounded explainable decision contract, not raw owner persistence rows.

## 11. Production composition

```text
governed create/withdraw request
→ application-level Party/Contact Point integrity checks
→ pure Consent aggregate planner
→ transactional authoritative record + audit + outbox + idempotency

communication authorization query
→ governed query gateway
→ authoritative tenant-scoped Consent candidate read
→ deterministic decision precedence
→ explainable allow/deny response
```

No Customer 360 or search projection is an authorization oracle.

## 12. Acceptance gate

Fresh PostgreSQL plus a real `crm-api` process must prove:

1. governed Party and Contact Point prerequisites;
2. same safe missing/cross-tenant Party and Contact Point failures without Consent side effects;
3. Contact Point-to-Party ownership validation;
4. create grant and deny assertions through governed capabilities;
5. exact idempotent replay and idempotency conflict behavior;
6. permission-aware get/list with typed filters and signed cursor integrity;
7. matching current grant allows communication;
8. unrelated purpose or channel does not authorize;
9. optional Contact Point scope is enforced;
10. effective/expiry windows are deterministic;
11. preferred/verified Contact Point state alone never authorizes;
12. withdrawal immediately changes authoritative decision to denied;
13. a later new grant can supersede an older withdrawal/deny according to exact timestamp precedence;
14. ties fail closed;
15. stale expected version and repeated withdrawal fail atomically;
16. unauthenticated rejection and cross-tenant non-disclosure;
17. durable record/audit/outbox/idempotency evidence;
18. deterministic persistence round-trip and corruption rejection;
19. migration clean install and rollback proof;
20. exact Rust/browser descriptor parity;
21. all applicable CI workflows green together on one unchanged final head SHA.

## 13. Non-goals

This packet does not deliver:

- provider send/delivery execution or webhook ownership;
- campaign or journey orchestration;
- generic provider suppression/bounce state;
- identity resolution or duplicate candidates;
- merge/unmerge, survivorship or field provenance;
- Customer 360 mutation;
- full privacy export/deletion/legal-hold orchestration.

Those remain later explicit owner/composition packets.
