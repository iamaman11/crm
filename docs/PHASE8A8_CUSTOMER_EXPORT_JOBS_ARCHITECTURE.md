# Phase 8A.8 — Customer Export Jobs, Artifacts and Reconciliation Architecture

Status: **Normative architecture for the active #123 production packet**

## 1. Objective

Deliver governed customer export without creating a generic database dump path or a second customer master.

The first v1 production target is **Party export**. The model must remain extensible to Account, Contact Point and other customer-master owners without weakening owner boundaries.

## 2. Ownership boundary

`crm.customer-data-operations` may own only:

- export-job identity and lifecycle;
- immutable export specification/profile identity;
- exact selected-resource manifest evidence;
- resumable execution/checkpoint state;
- derived artifact identity and lifecycle evidence;
- selected/emitted/excluded/redacted reconciliation counts;
- bounded safe diagnostics.

It does not own mutable Party data. Party remains authoritative.

No export code may read Party tables directly. Selection and serialization must use governed application query/capability composition with tenant isolation and live authorization.

## 3. V1 scope

V1 exports Party resources only.

The bounded export profile is:

- format: UTF-8 CSV;
- canonicalization version: `PARTY_EXPORT_CANONICALIZATION_V1`;
- header: fixed and versioned;
- supported fields: exact allowlisted Party contract fields only;
- maximum selected resources: bounded by the packet contract;
- maximum final artifact size: bounded by `crm-core-files` (`MAXIMUM_FILE_ARTIFACT_BYTES`).

No arbitrary SQL, user expressions, custom scripts or caller-supplied serialization code are allowed.

## 4. Immutable export specification

A Party export job binds an immutable specification containing:

- export profile/version;
- requested Party scope/filter;
- requested allowlisted fields;
- retention policy identity;
- canonicalization version.

A deterministic `export_specification_version_id` is derived from canonical encoding of every semantic field. Changing any semantic input requires a new job/specification identity.

Unknown fields or unsupported profile values are rejected before job creation.

## 5. Stable selection strategy

Long-running database snapshots are not used.

At selection time the worker:

1. invokes the governed Party list/query surface under the job tenant and execution actor;
2. repeats live resource and field authorization through that governed surface;
3. records an immutable export-owned **selection manifest** containing only exact `PartyRef + Party resource_version` evidence in deterministic order;
4. records the total selected count and a deterministic manifest digest.

The manifest is coordination evidence, not a copy of mutable Party state.

The selection manifest is the immutable source-selection boundary for retries and restarts. A retry never silently rebuilds a different population for the same job.

## 6. Serialization and live authorization

For each selected manifest entry, execution:

1. invokes governed Party get/query composition for the exact Party reference;
2. repeats current tenant/resource/field authorization immediately before serialization;
3. requires the authoritative Party resource version to match the manifest version;
4. serializes only fields still visible under the immutable export profile.

If the Party is no longer visible, unavailable or has changed version, v1 records a deterministic bounded exclusion reason rather than exporting a different resource state silently.

This preserves a stable export intent without copying authoritative mutable records into the data-operations module.

## 7. Deterministic canonical bytes

CSV bytes are deterministic for the same manifest, profile and visible authoritative values:

- fixed UTF-8 encoding;
- fixed header and column order;
- fixed newline convention;
- exact escaping rules owned by the canonicalization version;
- deterministic manifest order;
- no locale-dependent formatting;
- no wall-clock values inside exported rows.

The worker computes final bytes, SHA-256 and byte size before artifact creation. V1 is intentionally bounded so the complete canonical artifact fits within the governed file-artifact size limit.

## 8. Artifact publication

Export uses `crm-core-files::ImmutableFileArtifactStore` rather than raw filesystem, object storage or database access.

The final logical artifact identity is deterministic from the export job and immutable specification identity.

Publication sequence:

1. build canonical bounded bytes;
2. compute exact SHA-256 and byte size;
3. create the immutable file artifact with owner module `crm.customer-data-operations`, exact media type, data class and retention policy;
4. append chunks in exact sequential order with per-chunk SHA-256;
5. finalize and verify exact final metadata;
6. only after successful file finalization, atomically persist the export-owned completed job/artifact reference/reconciliation evidence.

No partially uploaded artifact is returned as a completed export result.

A restart repeats the same deterministic artifact identity and exact bytes. Replay must either recover the same finalized artifact or reject a semantic conflict; it must not publish a second logical artifact for the same job.

## 9. Export job lifecycle

V1 lifecycle:

- `CREATED` — immutable specification accepted;
- `SELECTING` — governed Party selection manifest is being built;
- `READY` — immutable selection manifest is complete;
- `EXECUTING` — selected entries are being authorized, version-checked and serialized;
- `COMPLETED` — exactly one finalized artifact and reconciliation record are bound to the job;
- `FAILED_RETRYABLE` — bounded retryable execution evidence, resumable without changing job intent;
- `CANCELLED` — terminal cancellation before completion.

Terminal completion and cancellation are irreversible.

## 10. Reconciliation evidence

Completion records exact bounded counters:

- selected resources;
- emitted rows;
- excluded not visible;
- excluded version changed;
- excluded unavailable;
- redacted field count where field-level visibility removes optional output;
- final artifact byte size;
- final artifact SHA-256;
- final artifact reference.

The invariant is:

`selected = emitted + excluded_not_visible + excluded_version_changed + excluded_unavailable`

Field redactions do not change resource reconciliation counts.

## 11. Retry and crash semantics

Required uncertainty scenarios:

### Selection crash

A restart resumes or deterministically rebuilds only the incomplete manifest under the same job intent; once manifest finalization is committed, it is immutable.

### Artifact upload crash

A restart regenerates the same canonical bytes and uses the same deterministic artifact identity and sequential chunk hashes.

### Artifact finalized / job outcome missing

If file finalization succeeds and the process terminates before the export-owned completion transaction, restart must recover the same finalized artifact metadata and commit the missing job/reconciliation outcome without creating a duplicate logical artifact.

## 12. Authorization and privacy boundary

Export is not an authorization bypass.

The worker must repeat live authorization during selection and again before serialization. The export path must honor current field visibility, tenant isolation and any privacy/consent/restriction policy exposed through governed owner/query boundaries.

Phase 8A.8 does not implement the full privacy-request lifecycle (#126), but it must not create a path that can bypass restrictions already enforced by current authoritative/query policy.

## 13. Public contract surface

The additive v1 contract should expose bounded operations for:

- create Party export job;
- start/finalize selection where required by the implementation boundary;
- start/resume export execution;
- cancel export job;
- get export job;
- list export jobs;
- get completed artifact/reconciliation metadata through the job representation.

Internal worker-only capabilities may persist selection/checkpoint/completion outcomes but must not leak into public mutation catalogs.

## 14. Acceptance gates

Before #123 may leave draft state, the merged candidate must prove:

- immutable export specification/profile validation and deterministic version identity;
- no direct Party storage reads;
- governed Party selection and get/query composition;
- immutable manifest of exact Party refs and resource versions;
- live authorization and field visibility before serialization;
- deterministic canonical bytes and artifact digest;
- no partial artifact publication as completed output;
- deterministic retry/resume without duplicate logical artifacts;
- crash after artifact finalization but before job completion, followed by restart recovery;
- exact reconciliation invariant and artifact metadata;
- cross-tenant non-disclosure;
- migration clean apply, rollback and reapply;
- fresh-PostgreSQL real `crm-api` process acceptance;
- all applicable workflows green on one unchanged exact final SHA.

## 15. Explicit non-goals

- arbitrary SQL exports;
- generic ETL/warehouse orchestration;
- arbitrary user code or executable export expressions;
- direct cross-module table reads;
- ownership of mutable customer-master records;
- generalized data-quality/stewardship (#124);
- external enrichment (#125);
- full privacy request orchestration (#126).
