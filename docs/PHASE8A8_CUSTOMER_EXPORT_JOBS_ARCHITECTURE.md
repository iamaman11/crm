# Phase 8A.8 — Customer Export Jobs, Artifacts and Reconciliation Architecture

Status: **Normative architecture for the active #123 production packet**

## 1. Objective

Deliver governed customer export without creating a generic database dump path or a second customer master.

The first v1 production target is **Party export**. The model must remain extensible to Account, Contact Point and other customer-master owners without weakening owner boundaries.

## 2. Ownership boundary

`crm.customer-data-operations` may own only:

- export-job identity and lifecycle;
- immutable export specification/profile identity;
- immutable selection-boundary and exact selected-resource manifest evidence;
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

## 5. Stable selection strategy and immutable boundary

Long-running database snapshots are not used during export execution.

V1 uses an explicit **Party creation-time selection boundary**:

1. the first successful transition from `CREATED` to `SELECTING` persists one immutable `selection_cutoff_unix_nanos` under the export job;
2. the cutoff is the governed execution time of that transition and never changes for the job;
3. only Parties whose immutable authoritative `created_at_unix_nanos <= selection_cutoff_unix_nanos` are eligible;
4. governed Party selection uses deterministic `(created_at_unix_nanos ASC, party_id ASC)` ordering and the immutable job scope/kind filter;
5. continuation state is bound to the same tenant, actor/execution identity, export job, specification version, cutoff, filter and sort;
6. every selected entry records only exact `PartyRef + Party resource_version` evidence in deterministic manifest order;
7. the finalized manifest digest binds the selection-boundary version, exact cutoff and every ordered `PartyRef + resource_version` entry.

The Party creation timestamp is immutable owner state. A Party created after the cutoff can never enter the job after a retry. A Party that existed by the cutoff remains part of the eligible population even if it is updated later; its exact version is captured when selected and is revalidated before serialization.

The worker-private Party selection port may expose authoritative creation time and deterministic continuation only for governed selection composition. This is not a public bulk-discovery API and does not transfer Party ownership to `crm.customer-data-operations`.

Selection retry rules are exact:

- a restart reuses the same immutable cutoff and continuation ordering;
- it never chooses a new cutoff for the same job;
- committed staged manifest entries are verified against deterministic position, Party identity and version before continuation;
- once manifest finalization is committed, the manifest is immutable;
- no export bytes may be produced before manifest finalization.

The selection manifest is coordination evidence, not a copy of mutable Party state.

## 6. Serialization and live authorization

For each finalized manifest entry, execution:

1. invokes governed Party get/query composition for the exact Party reference;
2. repeats current tenant/resource/field authorization immediately before serialization;
3. requires the authoritative Party resource version to match the manifest version;
4. serializes only fields still visible under the immutable export profile.

If the Party is no longer visible, unavailable or has changed version, v1 records a deterministic bounded exclusion reason rather than exporting a different resource state silently.

This preserves stable export intent without copying authoritative mutable records into the data-operations module.

## 7. Deterministic and spreadsheet-safe canonical bytes

CSV bytes are deterministic for the same finalized manifest, profile and visible authoritative values:

- fixed UTF-8 encoding without BOM;
- fixed header and column order;
- LF (`\n`) newline convention;
- RFC 4180-style quote escaping for canonical cells;
- deterministic manifest order;
- no locale-dependent formatting;
- no wall-clock values inside exported rows.

`PARTY_EXPORT_CANONICALIZATION_V1` is also spreadsheet-formula safe. Before ordinary CSV quote escaping, a textual cell whose first non-whitespace scalar begins with `=`, `+`, `-` or `@` is deterministically prefixed with a single apostrophe (`'`). This neutralization rule is part of the canonicalization version and therefore part of artifact identity and digest semantics.

The v1 profile intentionally favors safe human spreadsheet consumption over byte-for-byte round-trip of arbitrary text. A future machine-lossless format or canonicalization profile must use a new explicit version rather than changing v1 semantics.

## 8. Artifact publication and checkpoint invariant

Export uses `crm-core-files::ImmutableFileArtifactStore` rather than raw filesystem, object storage or database access.

The final logical artifact identity is deterministic from the export job and immutable specification identity.

V1 publication is deterministic and resumable:

1. create or recover the same deterministic staged logical artifact identity;
2. append the fixed canonical header as deterministic chunk `0`;
3. process finalized manifest entries strictly in manifest order;
4. for an emitted row, derive exact canonical row bytes and a deterministic chunk identity/hash before append;
5. append that chunk through `ImmutableFileArtifactStore`;
6. only after the store confirms the expected chunk identity and hash may the export checkpoint advance past that manifest position;
7. for an exclusion with no artifact bytes, persist the exact exclusion outcome and checkpoint advancement atomically in the export-owned transaction;
8. after every selected manifest position has a durable emitted/excluded outcome, finalize the immutable artifact and verify exact final metadata;
9. only after successful file finalization, atomically persist the export-owned completed job/artifact reference/reconciliation evidence.

The checkpoint invariant is:

**a persisted checkpoint must never claim a manifest position whose emitted bytes or exclusion outcome are not already durable.**

The complementary crash rule is also exact: if an emitted chunk becomes durable but the process terminates before checkpoint persistence, restart replays the same deterministic chunk identity and hash. The file-artifact boundary must treat an exact replay as idempotent and a different payload for the same logical chunk as a conflict. The worker then commits the missing checkpoint/outcome without duplicating bytes.

No partially uploaded artifact is returned as a completed export result.

A restart uses the same deterministic artifact identity, manifest order, canonicalization version and chunk identities. It must recover or reproduce the same logical bytes; it must not publish a second logical artifact for the same job.

## 9. Export job lifecycle

V1 lifecycle:

- `CREATED` — immutable specification accepted;
- `SELECTING` — immutable selection cutoff is fixed and governed Party selection manifest is being built;
- `READY` — immutable selection manifest is complete;
- `EXECUTING` — selected entries are being authorized, version-checked, reconciled and serialized;
- `COMPLETED` — exactly one finalized artifact and reconciliation record are bound to the job;
- `FAILED_RETRYABLE` — bounded retryable execution evidence, resumable without changing job intent;
- `CANCELLED` — terminal cancellation before completion.

Terminal completion and cancellation are irreversible.

Cancellation must not expose a staged artifact. Any staged artifact remains inaccessible through public download paths and is reclaimed according to an explicit bounded cleanup/retention policy.

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

Reconciliation counts are derived from durable per-position outcomes and must agree with the finalized artifact metadata before completion can commit.

## 11. Retry and crash semantics

Required uncertainty scenarios:

### Selection crash

A restart reuses the same immutable `selection_cutoff_unix_nanos`, deterministic ordering and continuation semantics. Parties created after the original cutoff cannot appear after restart. Committed staged entries are verified before continuation, and no bytes are produced until one immutable finalized manifest exists.

### Artifact-chunk / checkpoint crash

If a deterministic artifact chunk is durably appended but checkpoint persistence is missing, restart replays the same chunk identity/hash, recovers idempotently and commits the missing checkpoint. A checkpoint may never advance before the corresponding bytes or exclusion outcome are durable.

### Artifact finalized / job outcome missing

If file finalization succeeds and the process terminates before the export-owned completion transaction, restart must recover the same finalized artifact metadata and commit the missing job/reconciliation outcome without creating a duplicate logical artifact.

## 12. Authorization, approval, download and privacy boundary

Export is not an authorization bypass.

The worker must repeat live authorization during selection and again before serialization. The export path must honor current field visibility, tenant isolation and any privacy/consent/restriction policy exposed through governed owner/query boundaries.

Starting a bulk export is a high-risk operation. Production composition must apply an explicit tenant-configurable export policy using resource count, requested fields/data classes and actor privileges. Until a tenant policy explicitly permits a lower-friction threshold, the safe default is approval-required execution for bulk export.

Possession of a `file_id` is never sufficient to download an export artifact. Every download must re-check the authenticated actor, tenant, completed export-job visibility, artifact relationship, current export/download authorization and artifact expiry/retention state immediately before disclosure. Download itself produces traceable audit evidence.

Staged, cancelled, expired or otherwise non-completed artifacts are not downloadable through the public export surface.

Phase 8A.8 does not implement the full privacy-request lifecycle (#126), but it must not create a path that can bypass restrictions already enforced by current authoritative/query policy.

## 13. Public contract surface

The additive v1 contract should expose bounded operations for:

- create Party export job;
- start/resume export execution;
- cancel export job;
- get export job;
- list export jobs;
- get completed artifact/reconciliation metadata through the job representation.

Internal worker-only capabilities may persist selection boundary, selection/checkpoint/outcome/completion evidence but must not leak into public mutation catalogs.

Artifact download is a separate governed file-disclosure operation. The job representation may expose a reference, but never grants download authority by itself.

## 14. Acceptance gates

Before #123 may leave draft state, the merged candidate must prove:

- immutable export specification/profile validation and deterministic version identity;
- no direct Party storage reads;
- governed Party selection and get/query composition;
- one immutable selection cutoff persisted on first selection start;
- selection crash/restart uses the exact same cutoff and cannot admit Parties created after it;
- deterministic manifest ordering and digest bound to cutoff plus exact Party refs/resource versions;
- live authorization and field visibility before serialization;
- deterministic spreadsheet-safe canonical bytes and artifact digest;
- explicit formula-injection regression tests for dangerous leading text values;
- no partial/staged artifact publication as completed output;
- checkpoint never advances before corresponding bytes/exclusion outcome are durable;
- chunk-written/checkpoint-missing crash recovery is idempotent;
- deterministic retry/resume without duplicate logical artifacts;
- crash after artifact finalization but before job completion, followed by restart recovery;
- exact reconciliation invariant and artifact metadata;
- bulk-export approval/policy enforcement;
- live authorization on artifact download and rejection of staged/cancelled/expired artifacts;
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
