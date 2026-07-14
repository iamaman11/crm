# Phase 8A.7 Process Acceptance

This document defines the process-level acceptance evidence required before Phase 8A.7 can leave draft state. It complements the architecture and execution/resume protocol and does not replace exact-head CI evidence.

## Required CI execution

The fresh-PostgreSQL process scenarios in this document are executed by two dedicated merge gates: `Import Process Runtime CI` proves source bytes, dry-run, target-success crash/restart recovery, tenant isolation and cursor tamper rejection; `Import Retryable Process Runtime CI` proves durable retryable target failure without checkpoint advancement followed by restart recovery and exactly-once target creation. Each process scenario publishes its required durable capability/worker fixtures into its own fresh database before `crm-api` starts, so process evidence does not depend on state left by another test job. Adding these tests and workflows does not itself constitute passing evidence; both workflows must be green on the unchanged final candidate SHA.

## Source artifact proof

The production path must prove:

1. the caller creates an immutable source artifact;
2. chunks are accepted only in exact sequential order;
3. identical chunk replay is idempotent and conflicting replay is rejected;
4. finalize verifies declared byte length and SHA-256 over the exact stored bytes;
5. `ImportJob` binds the finalized artifact identity, immutable digest and parser profile;
6. validation reparses the finalized bytes server-side and never trusts caller-supplied pre-parsed rows as production evidence;
7. cross-tenant artifact access is non-disclosing.

## Dry-run proof

Validation-only execution may persist import-owned evidence, but it must produce zero target-side Party effects:

- zero Party records;
- zero Party capability idempotency rows;
- zero Party outbox events;
- zero Party mutation audit records.

Both partial-validation policies must be exercised, including the `RequireAllValid` refusal path.

## Execute and resume proof

A fresh PostgreSQL process scenario must prove:

1. a validated import starts execution through the governed application capability surface;
2. the worker reads the authoritative job and related rows under tenant isolation;
3. the next source position is derived from durable checkpoint state, not relationship pagination order;
4. valid rows invoke exact `parties.party.create@1.0.0` through `GatewayCapabilityClient`;
5. invalid rows never invoke the target Party capability;
6. successful target mutation is followed by import-owned row outcome and checkpoint evidence;
7. retryable target failure is durably recorded without advancing the checkpoint;
8. terminal completion is persisted only after all immutable source positions are accounted for.

Target replay evidence is checked using the platform canonical idempotency scope `capability:parties.party.create:1.0.0`, not a display-oriented capability coordinate.

Target capability mutations and import-owned outcome/checkpoint commits must also use distinct deterministic business transaction identities. A target row uses its restart-stable target idempotency identity; each private outcome uses its own restart-stable outcome identity. Reusing the import job ID as the transaction ID for both sides is invalid because a committed target mutation must never block recovery of the subsequent import-owned outcome on the `business_transactions` primary key.

## Crash-window proof

The required uncertain-success scenario is:

1. `parties.party.create@1.0.0` commits successfully;
2. the process terminates before the import-owned row outcome/checkpoint commit;
3. the application restarts against the same PostgreSQL database;
4. the worker reloads the same authoritative job and row;
5. the target call is repeated with the same deterministic target idempotency key and equivalent input;
6. no duplicate Party is created;
7. the import-owned success outcome and checkpoint are recovered;
8. processing continues to terminal completion.

## Query and isolation proof

The process acceptance suite must also prove:

- tenant non-disclosure for job, row and source-artifact reads;
- live field visibility on import queries;
- signed cursor tamper rejection;
- cursor binding to tenant, actor, capability, filter and page size;
- private internal outcome capabilities are unavailable through public HTTP/gRPC mutation catalogs.

## Final gate

After all process scenarios are green, no source, generated artifact, manifest or normative document may change before the final merge gate. Every applicable workflow must be green on one unchanged exact commit SHA. Only that SHA is a merge candidate.
