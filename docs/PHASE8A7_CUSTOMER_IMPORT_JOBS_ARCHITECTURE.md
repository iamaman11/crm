# Phase 8A.7 — Customer Import Jobs Architecture

Status: **Normative implementation boundary**

Issue: #120  
Parent program: #28

## 1. Purpose

Phase 8A.7 introduces governed bulk customer import without creating a generic record or SQL bypass. The first production target is Party import, but the job, parser-profile, mapping and row-outcome model must remain extensible to later customer-master owner modules.

The import subsystem coordinates work. It does not become an alternate owner of Party, Account, Contact Point, Party Relationship, Consent or Identity Resolution state.

## 2. Ownership boundary

The customer-data-operations coordination boundary may own only:

- import job identity and lifecycle;
- immutable source-content identity metadata;
- immutable source parser/import profile identity;
- immutable mapping definitions and mapping version identity;
- source-system identifier evidence that is explicitly separate from canonical CRM resource identity;
- deterministic row identity;
- durable validation and execution outcomes;
- resumable checkpoints and aggregate counters;
- bounded safe error-report metadata.

It must not own mutable customer-master fields or copy authoritative owner records as competing masters.

Successful target mutations must enter through exact versioned governed owner capabilities. The first production adapter invokes the existing Party create capability.

## 3. Deterministic identities and immutable interpretation

### Import job

`import_job_id` is a stable tenant-scoped record identity supplied or generated through the governed command boundary.

### Source content

Each job captures immutable source metadata including a canonical SHA-256 content digest. The digest identifies the exact input bytes used for validation and execution. Replacing bytes requires a different source identity or job revision; source bytes may not silently change under an existing job.

### Parser/import profile

The exact same source bytes must not be allowed to change meaning after parser implementation changes. Every job therefore binds to one immutable parser/import profile covering at least:

- parser profile identifier and semantic version;
- source format;
- text encoding;
- exact delimiter/dialect semantics where applicable;
- quote semantics where applicable;
- header semantics;
- canonicalization profile identity.

Changing any interpretation field requires a new job/source interpretation. Validation and execution must use the same source digest, parser profile and mapping version.

The first production parser profile is bounded delimited text. Arbitrary parser plug-ins or executable user code are out of scope.

### Mapping version

A job binds to one immutable mapping version at creation time. Later edits create another mapping version and cannot reinterpret rows already validated or executed by an existing job.

### Source external identifier versus canonical Party identity

A source-system external identifier and a canonical CRM `PartyId` are different concepts and must never be conflated by naming, mapping or persistence.

The Party import mapping may contain:

- an optional `source_external_id_column` retained as import-owned source evidence;
- an optional `target_party_id_column` only when the import explicitly supplies an already governed canonical CRM Party identity;
- otherwise, application composition derives a deterministic canonical target Party identity from the import job and row identity.

A source external identifier must never silently become a `PartyId`. When retained in row evidence, the first packet stores only bounded derived evidence such as its SHA-256 digest unless a later explicit source-identity owner model justifies stronger storage semantics.

### Row identity

Every row has a deterministic identity inside the job. The preferred identity is derived from a canonical explicit external row key when supplied; otherwise it is derived from the stable source row position. The external row key is an import-row identity input and is distinct from both source-system business identifiers and canonical CRM Party identity.

The same source digest, parser profile, mapping and row identity inputs must produce the same row identity.

Target mutation idempotency keys are derived deterministically from job identity, row identity, target owner and target capability version. Retrying or resuming the same row therefore cannot duplicate target side effects.

## 4. Mapping model

The first mapping type is a strict typed Party-import mapping. It may map bounded source columns to:

- optional canonical target Party ID input;
- Party kind;
- Party display name;
- optional source-system external identifier evidence;
- optional external row key used only for deterministic row identity.

Mappings are data, not executable code. Unknown fields, duplicate target assignments, unsupported conversions and unbounded expressions are rejected.

No arbitrary JavaScript, SQL, templates or user-provided executable expressions are part of this packet.

## 5. Job lifecycle

The initial lifecycle is:

```text
created
  -> validated
  -> executing
  -> completed

created|validated|executing
  -> cancelled
```

A job may also remain non-terminal with invalid rows after validation when partial execution policy permits valid rows to proceed.

Lifecycle changes use exact optimistic versions and monotonic governed time. Terminal completion and cancellation are irreversible.

The domain must reject impossible counter shapes, lifecycle regression and checkpoint regression during strict rehydration.

## 6. Row lifecycle and outcome evidence

Rows progress through bounded states such as:

```text
pending
  -> valid
  -> succeeded

pending
  -> invalid

valid
  -> failed
  -> succeeded   # retryable execution failure only
```

Validation failure is not execution failure. Safe row error evidence stores stable error codes and bounded field-level diagnostics, not raw infrastructure errors or secrets.

Outcome history must be sufficient to prove what happened without copying the target owner's authoritative record state.

## 7. Dry-run semantics

Dry run performs the same deterministic source interpretation, row identity, mapping, domain validation and target-command semantic preparation required for execution.

Dry run must not execute target owner mutations and therefore must create no target Party record, target idempotency record, target outbox event or target audit record.

The import subsystem may persist its own governed job/row validation evidence.

Execution after dry run must use the same immutable source digest, parser profile and mapping version. A changed source, parser profile or mapping invalidates prior validation rather than being silently accepted.

## 8. Resumable execution

Execution is row-addressable and checkpointed durably. A process restart may re-read durable pending/valid/failed-retryable rows and continue.

Correctness does not depend on an in-memory queue or process-local cursor.

For each executable row:

1. reconstruct the exact immutable source, parser-profile and mapping context;
2. reconstruct the typed target command;
3. derive the deterministic target idempotency key;
4. invoke the exact governed Party capability through application composition;
5. atomically persist the import row outcome/checkpoint under import-job ownership after the target result is known;
6. on retry after an uncertain boundary, rely on target capability idempotency to prevent duplicate Party creation.

No direct Party storage access is allowed from the pure import domain.

## 9. Partial execution policy

The first packet supports an explicit policy chosen at job creation:

- `all_valid_rows`: invalid rows are retained as errors while valid rows may execute;
- `require_all_valid`: any invalid row prevents execution until a new corrected job/source version is created.

The policy is immutable for the job.

## 10. Query and authorization model

Queries expose:

- get import job;
- list import jobs;
- list row outcomes with status filtering and signed cursors.

Visibility is tenant-, actor-, capability-, resource- and field-aware. Raw source values are not automatically disclosed by job queries. Later source-artifact download requires a separate governed file/export boundary.

## 11. PostgreSQL boundary

Durable storage must preserve:

- tenant isolation and FORCE RLS;
- exact optimistic job versions;
- immutable source digest, parser profile and mapping identity binding;
- explicit separation of source external identifier evidence from canonical Party identity;
- deterministic row uniqueness inside a job;
- monotonic checkpoint progression;
- atomic import-owned state/audit/outbox evidence for import commands;
- migration clean apply, rollback and reapply.

The owner/coordinator domain crate remains free of SQL and infrastructure clients.

## 12. Production acceptance

The production vertical slice is not complete until a fresh-PostgreSQL real `crm-api` process proves:

- parser-profile and mapping validation with unknown-field rejection;
- identical source bytes cannot be silently reinterpreted under a changed parser profile;
- source external identifiers are never treated as canonical Party IDs;
- deterministic row identity across replay;
- dry run with zero target Party mutation side effects;
- execute-after-validation through the governed Party create capability;
- deterministic target idempotency and no duplicate Party creation on retry;
- partial invalid-row handling according to explicit policy;
- interruption/restart resume from durable state;
- exact job/row counters and terminal lifecycle;
- tenant isolation and safe non-disclosure;
- signed cursor tamper rejection for row queries;
- migration clean apply, rollback and reapply.

## 13. Explicit later packets

Not part of Phase 8A.7:

- export jobs and downloadable artifact lifecycle — #123;
- generalized data-quality rule engines and stewardship queues — #124;
- external enrichment provider orchestration and licensing/freshness provenance — #125;
- privacy access/export/deletion/legal-hold orchestration — #126;
- automatic duplicate merging;
- generic bulk mutation access to arbitrary modules.
