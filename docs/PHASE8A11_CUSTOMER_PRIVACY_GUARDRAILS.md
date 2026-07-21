# Phase 8A.11 — Customer Privacy Delivery Guardrails

Status: **In progress guardrail freeze for #126**

Architecture: `PHASE8A11_CUSTOMER_PRIVACY_ARCHITECTURE.md`  
Implementation branch: `agent/phase8a11-privacy-architecture`

## 1. Packet invariant

`crm.customer-privacy` owns privacy cases, restrictions, legal holds, plans and orchestration evidence. It never becomes the owner of Party, Account, Contact Point, Relationship, Consent, Identity Resolution, import/export, Data Quality or Enrichment values.

Every disclosure or destructive action must be attributable to one exact case, subject binding, policy version, plan item, owner capability, idempotency lineage and immutable outcome.

## 2. Non-negotiable enforcement rules

- privacy restriction is an additional deny decision and never grants processing;
- Consent and Communication Authorization remain authoritative within `crm.consents`;
- owner mutations, disclosures and protected workers repeat a live privacy decision at the final boundary;
- unavailable, corrupt or stale privacy decisions fail closed;
- restriction placement and protected actions share the same tenant-plus-canonical-Party lock;
- no allow decision is trusted from search, projection, cache, token claim or stale in-memory state;
- module disable/uninstall cannot convert active restrictions or legal holds into allow.

## 3. Ownership and storage restrictions

The privacy core must not depend on SQLx/PostgreSQL, another module's internal adapter, concrete object storage, search implementation, secret manager or executable policy code.

Owner discovery and actions occur only through exact module-owned capabilities registered in deterministic contributions. The privacy coordinator never scans another module's tables, writes foreign records, imports a private crate to mutate state or issues generic SQL against owner schemas.

## 4. Stable subject and canonicalization rules

A case binds an exact submitted Party reference, verified canonical Party reference and Identity Resolution generation. Merge/unmerge/canonical changes require explicit rescoping; they never silently move an executing case.

Erasure cannot remove a stable Party identity while immutable references remain. Parties uses an erased tombstone with non-reusable identity and destroyed/anonymized personal fields.

No owner may recreate erased personal data from historical projections, imports, enrichment evidence, caches or backups during ordinary replay/rebuild.

## 5. Frozen initial inventory

`crm.customer-privacy` starts with exactly:

- 9 public mutations;
- 7 permission-aware public queries;
- 9 trusted worker/internal coordinates in phases 260, 270, 280 and 290;
- 1 reasoned non-runtime crypto-shredding coordinate.

The exact IDs are normative in the architecture document and the machine-readable freeze contract. Worker/internal coordinates have no public HTTP/gRPC ingress.

No coordinate may be promoted, removed or reclassified without synchronized architecture, manifest/binding and production-route parity updates.

## 6. Owner contribution protocol

Every initial authoritative customer-master owner provides exactly two worker-only capability families:

- subject scope contribution;
- one-plan-item action application.

Each contribution is tenant-bound, bounded, cursor-safe, exact-versioned and permission/purpose-aware. It returns canonical typed evidence rather than raw SQL rows, private payloads or arbitrary maps.

Each action receives a deterministic target idempotency key and returns one typed owner outcome. Text matching is never used to classify success, retry, hold, retention or conflict.

## 7. Restriction matrix guardrails

Active restriction denies by default:

- ordinary customer-master mutation;
- non-privacy disclosure/export;
- import mutation targeting the subject;
- Data Quality remediation;
- Enrichment dispatch, materialization and application;
- non-essential communication activation;
- future workers not explicitly classified as permitted.

Only exact policy-approved privacy fulfillment, retention/legal/security/audit processing, Consent withdrawal, restriction release and minimal lawful owner action may bypass the deny. Every bypass is explicit immutable evidence.

## 8. Retention and legal-hold guardrails

Precedence is fixed:

1. active legal hold;
2. mandatory retention;
3. approved privacy action;
4. ordinary product retention.

A hold or retention conflict produces a blocked outcome with authority, reason, data class, policy version and review date. It never silently becomes success and never causes an unplanned destructive fallback.

Owner execution rechecks hold and retention inside the same protected transaction immediately before delete/anonymize persistence.

## 9. Evidence preservation classes

Every plan item must be classified before execution:

- destroyable subject data;
- retain-minimized evidence;
- immutable required evidence;
- derived rebuildable state;
- crypto-shreddable data.

Unknown classification fails closed. Audit, privacy decisions, legal hold, Consent withdrawal, identity lineage, security and integrity evidence are never treated as ordinary deletable fields.

Pseudonymization is not claimed as anonymization unless re-identification is outside the governed system boundary and policy evidence proves the distinction.

## 10. Export and disclosure guardrails

Privacy access/export must reuse `crm.customer-data-operations` jobs, immutable manifests, artifact integrity and audited disclosure. `crm.customer-privacy` stores references only.

The privacy module must not add a second artifact store, unaudited download endpoint, bearer-by-ID access path or unbounded inline payload response.

Export scope is frozen to exact owner contribution receipts and versions. Changed or newly discovered resources require a new scope generation rather than silent inclusion.

## 11. Replay and crash-window rules

- deterministic attempts are committed before owner/artifact I/O;
- retries reuse the exact target idempotency lineage;
- owner-success/outcome-missing recovery produces one owner effect and one logical outcome;
- artifact-finalized/case-link-missing recovery links the same artifact;
- checkpoints advance only over contiguous durable outcomes;
- missing, conflicting, malformed or future-version evidence stops progress;
- a case cannot complete before required convergence evidence exists.

## 12. Projection, search, cache and backup rules

Derived state is removed, tombstoned or rebuilt only from authoritative owner events. Search/projection deletion is not proof of owner deletion.

Convergence evidence binds event positions and generation. Rebuild tests must prove erased fields do not reappear.

Backup/restore semantics must preserve active restrictions, holds and tombstones. Crypto-shredding remains non-runtime until backup and restore key semantics are accepted.

## 13. Isolation and safe errors

Privacy records use tenant-scoped ENABLE + FORCE RLS and `NOBYPASSRLS` application roles. Hidden resources use not-found concealment; confidential decision/authority fields may be redacted by field policy.

Public and worker errors are typed and bounded. They may distinguish inactive module, enforcement unavailable, subject not verified, canonical lineage changed, restriction active, legal hold, mandatory retention, approval missing, version conflict, retryable owner failure, terminal owner failure and convergence timeout.

Personal values, identity-verification documents, legal authority documents, artifact contents, secret material, raw SQL/provider text and internal diagnostics never cross the safe error boundary.

## 14. Lifecycle behavior

Disabling the module stops new public case commands and orchestration phases, but trusted final enforcement guards remain deny-safe for active controls.

Uninstall is rejected while an active restriction, active hold or non-terminal case exists. Historical evidence is retained under `retain_business_records`. Reinstall resumes from durable checkpoints only after policy and owner registry validation.

## 15. Mandatory acceptance topology

Permanent acceptance must include:

1. real `crm-api` HTTP/gRPC public process tests;
2. concurrent shared-lock restriction race proof;
3. fresh-PostgreSQL owner discovery/action orchestration;
4. legal-hold and retention blocking;
5. Party erased-tombstone and no-orphan proof;
6. governed privacy export with existing artifact disclosure;
7. owner-success/outcome-missing and artifact-success/link-missing recovery;
8. projection/search/cache rebuild convergence;
9. cross-tenant concealment, field visibility and live authorization;
10. FORCE RLS plus migration rollback/reapply;
11. disable/uninstall fail-closed enforcement;
12. exact production-route and generated-contract parity;
13. strict Rust, Clippy, rustfmt and all applicable permanent workflows on one unchanged source-authored SHA.

## 16. Scope-change rule

No implementation shortcut may weaken these guardrails to make a test pass. A scope change requires an explicit architecture amendment that identifies ownership, data classes, lifecycle, authorization order, failure/retry behavior, route classification and new permanent acceptance evidence.