# Phase 8A.11 — Customer Privacy Lifecycle Architecture

Status: **In progress architecture freeze for packet #126**

Parent program: Phase 8A / #28  
Delivery packet: #126  
Baseline: merged Phase 8A.10 plus post-merge integrity synchronization  
Implementation branch: `agent/phase8a11-privacy-architecture`

## 1. Objective

Deliver a production-proven customer privacy lifecycle covering access, portability export, processing restriction, erasure/anonymization, retention and legal hold without creating a second customer master, bypassing authoritative owner modules, weakening Consent, or deleting immutable evidence that the system is legally or operationally required to preserve.

The packet introduces `crm.customer-privacy` as the authoritative privacy case and orchestration owner. It coordinates exact owner capabilities and stores immutable privacy decision evidence. It never mutates another module's storage directly and never treats search, projections, caches or exported files as authoritative state.

## 2. Authoritative ownership boundary

### `crm.customer-privacy` owns

- privacy case identity, request type and lifecycle;
- verified subject-binding evidence and exact canonical Party reference;
- immutable scope snapshots and owner-contribution receipts;
- current processing/communication restriction directives and decisions;
- customer-data legal holds and release evidence;
- retention-policy snapshots and precedence decisions;
- deterministic owner/data-class action plans;
- per-owner dispatch attempts and append-once outcomes;
- orchestration checkpoints, retry state and compensation evidence;
- references to governed access/export jobs and artifacts;
- convergence evidence for projections, search and caches.

### Existing owners retain authority

- `crm.parties` owns Party identity, lifecycle and mutable Party fields;
- `crm.customer-accounts` owns Account values and Party associations;
- `crm.contact-points` owns endpoint values, verification and preferences;
- `crm.party-relationships` owns typed temporal Party relationships;
- `crm.consents` owns Consent assertions, withdrawals and Communication Authorization;
- `crm.identity-resolution` owns candidate, merge/unmerge, canonical redirect and survivorship lineage;
- `crm.customer-data-operations` owns import/export jobs, manifests, artifacts and reconciliation;
- `crm.data-quality` owns quality definitions, findings, observations and stewardship evidence;
- `crm.customer-enrichment` owns enrichment requests, provenance, review and application evidence;
- `crm.customer360`, projections, search and caches remain rebuildable non-authoritative read models.

No privacy operation writes another module's records, invokes another module's private adapter, or bypasses an exact versioned owner capability.

## 3. Stable subject identity

A privacy case binds:

- tenant identity;
- stable case identity and idempotency lineage;
- request type and jurisdiction/policy version;
- exact submitted Party reference;
- canonical Party reference and identity-resolution generation at verification time;
- subject-verification method, verifier, timestamp and bounded evidence reference;
- effective request timestamp, deadline and priority;
- purpose/legal basis for processing the privacy case itself.

Canonical redirects are resolved through the authoritative Identity Resolution path. A later merge, unmerge or canonical change never silently rebases an in-flight case. The case enters a typed rescope-required state, records the old and new lineage, and repeats bounded discovery before execution.

Possession of a Party ID, case ID, export job ID or artifact ID is never authority.

## 4. Case and control lifecycles

### Privacy case types

- `Access`;
- `PortabilityExport`;
- `RestrictProcessing`;
- `Erasure`.

Additional rights require a separately governed contract expansion.

### Privacy case lifecycle

```text
Draft
  -> Submitted
  -> SubjectVerified
  -> Scoping
  -> Scoped
  -> Planned
  -> AwaitingApproval (when policy requires)
  -> Executing
  -> Converging
  -> Completed | PartiallyCompleted | Denied

Draft|Submitted|SubjectVerified|Scoped|Planned|AwaitingApproval
  -> Cancelled

Any non-terminal state
  -> FailedRetryable | FailedTerminal
```

Transitions are optimistic-versioned, idempotent and append immutable events/audit evidence. A terminal case is never reopened in place; a new case references the prior one.

### Restriction lifecycle

A restriction is a separate subject-scoped aggregate with `Active`, `Released` and optional `Expired` states. Placement and release are explicit, versioned and audited. Case completion does not implicitly release an active restriction.

### Legal-hold lifecycle

A legal hold is a separate subject/data-class/scope aggregate with `Active` and `Released` states, authority reference, reason code, effective window and policy version. Release does not erase historical hold evidence.

## 5. Enforcement order and deny semantics

Privacy restriction is an additional deny decision, not a replacement for Consent or Communication Authorization.

For communication and purpose-bound processing, the effective decision is the most restrictive result of:

1. authentication and tenant binding;
2. durable module activation;
3. typed command/query validation;
4. authoritative owner visibility and semantic validation;
5. current Consent/Communication Authorization decision when applicable;
6. current privacy restriction/legal-hold decision;
7. versioned policy/approval checks;
8. final live authorization;
9. atomic owner persistence or protected external I/O.

A privacy restriction can deny processing that Consent would otherwise permit. It can never grant communication or processing that Consent denies.

If the privacy decision source is unavailable, corrupt, cross-tenant or cannot prove freshness, protected personal-data mutation, disclosure and worker execution fail closed with a bounded typed error. Module inactivity is never interpreted as allow.

## 6. Immediate restriction and race freedom

Restriction placement and every protected owner mutation, disclosure or worker boundary use one deterministic subject lock derived from tenant ID plus canonical Party ID.

The final guard executes while that lock is held and reloads the authoritative restriction decision from PostgreSQL before persistence or protected I/O. Restriction placement acquires the same lock before committing. This produces a total order:

- an operation ordered before placement may complete and records that ordering;
- an operation ordered after placement observes the active restriction and is denied;
- no stale allow cache or projection creates a TOCTOU bypass.

Allow decisions are not cached across protected transactions. Deny notifications may be propagated for performance, but the authoritative final guard remains mandatory.

## 7. Restriction matrix

An active processing restriction denies by default:

- non-essential Party, Account, Contact Point and Relationship mutation;
- import mutation targeting the subject;
- Data Quality remediation that changes owner state;
- Customer Enrichment dispatch, materialization and owner application;
- ordinary customer-data export or disclosure;
- marketing or non-essential communication activation;
- future module workers not explicitly classified as permitted.

The restriction may permit, under an exact purpose and policy decision:

- privacy access/export fulfillment;
- legal-hold, retention, security, fraud and audit processing;
- Consent withdrawal and restriction release;
- identity correction required to bind the right request safely;
- minimal owner action necessary to execute lawful erasure/anonymization.

Every permit override records case, purpose, policy version, actor/worker and exact owner coordinate.

## 8. Owner discovery and action protocol

The coordinator uses a deterministic registry of module-owned privacy contributions. Each authoritative customer-master owner provides two worker-only exact capabilities:

- `<owner>.privacy.scope.contribute@1.0.0`;
- `<owner>.privacy.action.apply@1.0.0`.

The initial owner set is:

- `parties`;
- `customer_accounts`;
- `contact_points`;
- `party_relationships`;
- `consents`;
- `identity_resolution`;
- `customer_data`;
- `data_quality`;
- `customer_enrichment`.

Scope contribution is bounded by tenant, exact canonical subject, identity generation, purpose, request timestamp and cursor limits. It returns immutable resource references, exact versions, data classes, retention classifications and safe counts; it never provides unauthorized tenant-wide bulk discovery.

Action application receives one immutable plan item and deterministic target idempotency key. The owner revalidates exact version, legal hold, retention, restriction purpose and live authorization before applying one of:

- `Retain`;
- `RestrictOnly`;
- `Anonymize`;
- `Delete`;
- `CryptoShred`;
- `NoOpAlreadyCompliant`.

Owner outcomes are append-once and distinguish applied, already applied, blocked by hold, blocked by retention, version changed, not found, not visible, retryable failure and terminal failure.

## 9. Owner-specific preservation rules

### Parties

Erasure preserves the stable Party identity as an `Erased` tombstone when other immutable evidence references it. Direct physical deletion is forbidden while references remain. Personal fields are removed or irreversibly anonymized, lifecycle/version evidence remains, and later reuse of the erased identifier is forbidden.

### Customer Accounts

Commercial and statutory records are retained or minimized according to policy. Non-required personal labels and associations are anonymized or detached through Account-owned semantics.

### Contact Points

Endpoint values, verification secrets and channel metadata are deleted or anonymized where lawful. Minimal tombstone evidence prevents accidental reactivation and preserves restriction/withdrawal semantics.

### Party Relationships

Non-required relationship metadata is deleted or anonymized. Legally required relationship facts retain minimized stable references.

### Consents

Consent assertions, withdrawals and decision lineage are immutable legal evidence. They are not deleted merely because the subject requests erasure. Subject references may be pseudonymized when policy permits, while purpose, channel, timestamps and withdrawal evidence remain verifiable.

### Identity Resolution

Merge/unmerge and canonical redirect lineage remains intact to prevent orphan references and identity resurrection. Survivorship payloads are minimized, but stable lineage and erased tombstone references remain.

### Customer Data Operations

Import sources, staged values and ordinary export artifacts follow retention and erasure policy. Required hashes, manifests, reconciliation and privacy-disclosure evidence are retained in minimized form. Privacy access/export uses the existing governed artifact boundary rather than a second download path.

### Data Quality

Subject-bearing snapshots and observations are removed or pseudonymized where lawful. Rule definitions, aggregate quality evidence and stewardship history remain when they no longer disclose erased values.

### Customer Enrichment

Provider-derived values and candidate evidence are removed or anonymized according to provider licensing and retention policy. Minimum request/review/application/provenance and usage evidence survives when required for legal, billing, security or audit purposes. No provider call occurs during erasure without an exact governed policy.

## 10. Access and portability export

`crm.customer-privacy` owns the right-request case and exact scope, but `crm.customer-data-operations` remains the artifact/job owner.

The coordinator requests a privacy export through an exact worker-only Customer Data Operations capability. Owner contributions are assembled into an immutable manifest with exact resource/version lineage, exclusions and retention decisions. Artifact creation, chunking, hashing, completion, retention and disclosure reuse the existing governed export controls.

The privacy case stores only stable export job/artifact references and completion evidence. Artifact download remains separately authenticated, live-authorized, resource-visible, audited, `private, no-store` and integrity-verified. The privacy module exposes no alternate file endpoint.

## 11. Retention and legal-hold precedence

The precedence order is:

1. active legal hold;
2. mandatory statutory/contractual/security retention;
3. approved erasure/anonymization policy;
4. ordinary product retention/expiry policy.

A hold or mandatory retention blocks destructive action but does not silently complete the case. The plan records the exact blocked resource/data class, authority, policy version, reason, review date and permitted minimization/restriction action.

Where full deletion is prohibited, the system applies the strongest lawful alternative: restriction, field minimization, pseudonymization or separation of access.

Hold placement and release are exact public mutations. Owner execution repeats the hold/retention decision immediately before destructive persistence.

## 12. Evidence classes

Every discovered item is classified as one of:

- `DestroyableSubjectData` — may be deleted/anonymized;
- `RetainMinimizedEvidence` — subject-bearing fields are minimized or pseudonymized;
- `ImmutableRequiredEvidence` — audit, Consent withdrawal, privacy decisions, legal hold, identity lineage, security and integrity evidence retained under policy;
- `DerivedRebuildableState` — projection/search/cache state removed or rebuilt;
- `CryptoShreddableData` — only when a tenant/subject key hierarchy and hold-aware key policy exist.

Evidence classification and policy version are immutable plan inputs. Unknown data classes fail closed and cannot be silently treated as destroyable.

## 13. Crypto-shredding boundary

Subject/tenant crypto-shredding is not claimed until the platform has a versioned subject-scoped data-encryption-key hierarchy, key-usage inventory, hold-aware destruction approval and restore/backup semantics.

`customer_privacy.crypto_shred.execute@1.0.0` remains explicitly non-runtime in the initial packet. The implemented planner may emit `CryptoShred` only when an installed key provider proves those prerequisites; otherwise it emits a typed unsupported/blocking outcome and chooses no destructive fallback.

## 14. Production inventory freeze

The initial `crm.customer-privacy` inventory is frozen before Protobuf or manifest promotion.

### Public mutations — exactly 9

- `customer_privacy.case.create@1.0.0`;
- `customer_privacy.case.submit@1.0.0`;
- `customer_privacy.case.subject.verify@1.0.0`;
- `customer_privacy.case.approve@1.0.0`;
- `customer_privacy.case.cancel@1.0.0`;
- `customer_privacy.restriction.place@1.0.0`;
- `customer_privacy.restriction.release@1.0.0`;
- `customer_privacy.legal_hold.place@1.0.0`;
- `customer_privacy.legal_hold.release@1.0.0`.

### Permission-aware public queries — exactly 7

- `customer_privacy.case.get@1.0.0`;
- `customer_privacy.case.list@1.0.0`;
- `customer_privacy.case.plan.get@1.0.0`;
- `customer_privacy.case.owner_outcomes.list@1.0.0`;
- `customer_privacy.restriction.get@1.0.0`;
- `customer_privacy.legal_hold.get@1.0.0`;
- `customer_privacy.legal_hold.list_by_subject@1.0.0`.

### Activation-gated worker/internal coordinates — exactly 9

Phase 260 — scope and decision:

- `customer_privacy.enforcement.decide@1.0.0`;
- `customer_privacy.scope.discover@1.0.0`;
- `customer_privacy.retention.evaluate@1.0.0`.

Phase 270 — planning and disclosure:

- `customer_privacy.plan.build@1.0.0`;
- `customer_privacy.access_export.request@1.0.0`.

Phase 280 — owner execution:

- `customer_privacy.owner_action.dispatch@1.0.0`;
- `customer_privacy.owner_outcome.record@1.0.0`.

Phase 290 — convergence and completion:

- `customer_privacy.convergence.verify@1.0.0`;
- `customer_privacy.case.finalize@1.0.0`.

Worker/internal coordinates have no public HTTP/gRPC ingress. `enforcement.decide` is callable only through trusted in-process owner guards and remains fail-closed when orchestration workers are disabled.

### Non-runtime coordinate — exactly 1

- `customer_privacy.crypto_shred.execute@1.0.0` — blocked until subject-scoped key architecture is implemented and accepted.

Any inventory change requires a separate reviewed architecture change and exact route-classification parity.

## 15. Persistence and isolation

Protected privacy records use tenant-scoped ENABLE + FORCE RLS. Application roles are `NOBYPASSRLS`; no-context/cross-tenant reads are concealed and cross-tenant writes are rejected.

State, optimistic version, idempotency, outbox, audit and business transaction evidence are atomic. Persisted identities, lifecycle values, policy versions, subject references and plan digests are strictly canonical and reject unknown/future state.

Destructive owner actions use transaction-scoped exact-version and subject-lock guards. A guard cannot commit independently, perform external I/O or mutate referenced resources.

## 16. Retry and crash-window semantics

The process commits deterministic attempts before owner or artifact I/O. Recovery reuses the same target idempotency lineage.

Mandatory crash windows:

1. restriction committed before notification/convergence — protected paths already deny through the authoritative live guard;
2. scope contribution persisted before checkpoint — replay records no duplicate resource contribution;
3. owner action succeeded before privacy outcome — replay uses the same owner idempotency key and records one logical outcome;
4. export artifact finalized before case reference/completion — replay links the existing artifact without creating a second logical export;
5. all owner outcomes recorded before projection/search convergence — case does not complete until deterministic convergence evidence exists.

A checkpoint advances only across a contiguous durable outcome prefix. Missing, malformed, future-version or conflicting evidence stops progress without skipping work.

## 17. Projection, search and cache convergence

Owner actions emit typed lifecycle/change events. Derived systems remove subject-bearing documents or rebuild from authoritative tombstones.

Convergence evidence binds case, owner outcome set, event positions, projection/search generation and deadline. A case may be `PartiallyCompleted` when authoritative actions are final but a bounded derived-system failure remains explicit; it may not falsely claim complete convergence.

Search, projection and cache state is never evidence that authoritative deletion succeeded.

## 18. Disable and uninstall behavior

Disabling the module stops new cases and phases 260/270/280/290 orchestration, but does not turn active restrictions or legal holds into allow decisions.

Owner guards treat unavailable enforcement as deny. Existing active restrictions/holds remain enforceable and queryable by trusted guards.

Uninstall is rejected while any active restriction, active legal hold or non-terminal case exists. After controls are resolved, uninstall retains privacy cases, decisions, plans, outcomes and audit evidence under `retain_business_records`. Reinstall revalidates policy and resumes retryable work from durable checkpoints.

## 19. Acceptance topology

The packet is complete only when permanent evidence proves:

1. real `crm-api` public HTTP/gRPC case, restriction and legal-hold paths on fresh PostgreSQL;
2. immediate restriction under the shared subject lock, including concurrent mutation/worker races;
3. owner-scope contribution and exact owner-action execution without cross-storage access;
4. privacy access/export through Customer Data Operations with no alternate disclosure route;
5. legal-hold and retention blocking with immutable reason evidence;
6. Party tombstone/anonymization and no orphaned canonical references;
7. target-success/outcome-missing and artifact-success/case-link-missing recovery without duplicates;
8. projection/search/cache convergence and rebuild proof;
9. cross-tenant concealment, live authorization, field visibility, FORCE RLS and migration rollback/reapply;
10. disable/uninstall fail-closed enforcement;
11. exact manifest/binding/public/worker/non-runtime classification parity;
12. all applicable workflows successful on one unchanged source-authored SHA.

## 20. Explicit non-goals

The initial packet does not implement a generic records-management product, enterprise e-discovery, arbitrary jurisdiction scripting, autonomous legal advice, deletion of audit/Consent/identity lineage required by policy, physical deletion of stable Party identity while referenced, or crypto-shredding without the required key architecture.

Future product/catalog, Sales, Service, Marketing and other owner domains integrate through the same contribution protocol and cannot bypass the privacy coordinator or authoritative owner boundaries.