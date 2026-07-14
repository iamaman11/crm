# Phase 8A.7 — Party Import Execution and Resume Protocol

Status: **Normative implementation protocol**

Parent packet: #120  
Active implementation: PR #121

## 1. Purpose

This protocol defines how a validated Party import job executes target mutations, survives process interruption and resumes without duplicate Party creation or skipped source rows.

The customer-data-operations boundary coordinates execution. `crm.parties` remains the only authoritative owner of Party state.

## 2. Non-negotiable boundaries

1. Import execution MUST NOT write Party storage directly.
2. Every target Party create MUST invoke exact `parties.party.create@1.0.0` through the normal governed capability gateway.
3. Every target call repeats live authorization and uses the production transactional capability executor.
4. Each executable row uses the deterministic target idempotency key derived by the import domain from job identity, row identity, target owner and target capability version.
5. Import-owned row outcome and job checkpoint state are authoritative only for import coordination. They do not become a second Party source of truth.
6. A process crash MUST NOT require manual SQL repair to resume safely.

## 3. Pre-execution integrity gate

Before the first target mutation of a run or resume attempt, the worker MUST:

1. load the authoritative import job under tenant scope;
2. require job status `executing`;
3. enumerate the authoritative `customer_data.import.job.row` relationships for the job;
4. load the minimum row identity metadata required to build a bounded `row_position -> import_row_ref` index;
5. reject duplicate row positions;
6. reject row positions outside `1..=total_rows`;
7. reject a missing position anywhere in that range;
8. reject an index whose row count differs from immutable `total_rows`;
9. begin from `checkpoint_row_position + 1`.

Relationship pagination order is not execution order. External row-key-derived row IDs are intentionally independent of source position, so the worker MUST use the validated position index.

## 4. Row decision

For the next source position:

### Invalid row

When row status is `invalid`:

- `all_valid_rows` policy: advance the import checkpoint with `skipped_invalid = true` and do not invoke Party mutation;
- `require_all_valid` policy: execution must never have started because validation finalization/start rejects a job containing invalid rows.

### Valid row

When row status is `valid`:

1. reconstruct the exact prepared Party command from authoritative import-row state;
2. derive the deterministic target idempotency key;
3. invoke `parties.party.create@1.0.0` through the governed gateway;
4. classify the target result;
5. persist the import-owned result according to Sections 5–7.

### Succeeded row

A row already marked `succeeded` at or before the checkpoint is historical completed work. A succeeded row after the checkpoint is inconsistent import state and MUST fail closed until reconciled.

### Failed-retryable row

A `failed_retryable` row is executable again using the same deterministic target idempotency key.

## 5. Successful target result

After the governed Party create returns success or an idempotent replay of the same successful result:

1. persist the row as `succeeded` with the canonical target Party reference;
2. emit durable `customer_data.import.party.row_succeeded@1.0.0` evidence;
3. advance the job checkpoint to exactly that row position;
4. increment `succeeded_rows` exactly once;
5. emit durable `customer_data.import.party.checkpoint_advanced@1.0.0` evidence.

The row outcome and checkpoint update SHOULD commit atomically when the platform batch executor can lock both import-owned aggregates in one governed internal capability. Until that path is available, the worker must use the uncertain-boundary recovery rule below and must never infer target success from process-local memory.

## 6. Uncertain crash boundary

The critical failure window is:

```text
Party create committed successfully
        ↓
worker/process crashes before import row outcome or checkpoint commits
```

Recovery is deterministic:

1. restart loads the same row because the checkpoint did not advance;
2. the worker derives the same target Party ID and the same target idempotency key;
3. the worker invokes the exact same Party create capability again;
4. Party capability idempotency returns the original successful result instead of creating a duplicate Party;
5. the worker persists the missing import-owned success/checkpoint evidence;
6. execution continues with the next source position.

Therefore target idempotency is the correctness boundary for an uncertain cross-capability commit window.

## 7. Target failure classification

### Retryable target failure

For a retryable governed target failure:

- increment the bounded execution-attempt count;
- store only the stable safe error code, never raw infrastructure errors or secrets;
- persist row status `failed_retryable`;
- emit `customer_data.import.party.row_failed@1.0.0`;
- do not advance the checkpoint;
- stop or yield the current worker run according to bounded retry/backoff policy.

A later resume retries the same row with the same target idempotency key.

### Non-retryable target failure

The initial v1 packet MUST fail closed rather than silently skip an unexpected non-retryable execution failure after validation. Such a failure requires an explicit terminal/reconciliation policy before the worker may continue past the row.

Validation errors are not execution errors and remain represented by `invalid` rows before execution begins.

## 8. Job completion

After checkpoint advances to immutable `total_rows`:

1. require that every valid row is `succeeded`;
2. require that every non-succeeded row is `invalid` and was allowed to be skipped by `all_valid_rows`;
3. require `succeeded_rows == valid_rows`;
4. transition the job irreversibly to `completed`;
5. emit `customer_data.import.party.completed@1.0.0`.

No process-local queue, cursor or counter may be required to prove completion.

## 9. Concurrency and lease rule

Only one active executor may advance a given import job at a time.

The production worker MUST use a tenant-scoped durable execution lease or equivalent database-backed serialization boundary with:

- job identity;
- lease owner identity;
- bounded expiry;
- safe renewal;
- takeover only after expiry;
- audit/trace correlation.

A process-local mutex is insufficient for production correctness.

Optimistic job/row versions remain mandatory even with a lease.

## 10. Process acceptance

A fresh-PostgreSQL real `crm-api` acceptance scenario MUST prove at least:

1. create and validate a job;
2. start execution;
3. successfully create at least one Party through the governed gateway;
4. terminate the process after target success but before import checkpoint persistence;
5. restart the process;
6. replay the same target capability with the same idempotency key;
7. prove exactly one Party exists and target mutation evidence was not duplicated;
8. persist the missing row/checkpoint evidence;
9. continue remaining rows in source-position order regardless of relationship query order;
10. complete the job with exact counters;
11. prove tenant non-disclosure and no direct Party storage mutation path.

## 11. Current implementation consequence

The next execution implementation packet inside PR #121 must introduce governed import-owned execution result/checkpoint mutations and a worker/composition boundary that uses the production `GatewayCapabilityClient` or an equivalent adapter over the normal `CapabilityGateway`.

The worker must not be marked production-complete until the source-byte/parser-profile proof and the process-level crash/restart scenario are both automated.
