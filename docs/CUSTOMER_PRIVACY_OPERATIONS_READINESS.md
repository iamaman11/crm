# Customer Privacy Operations Readiness — Repository Step 20B

Status: **implementation candidate; not accepted until an unchanged exact head passes every applicable permanent workflow and is merged**

Tracking: #194, #126

Baseline: `main` / `d3d066d0446a4936bd61574506e729c9fd9104dc`

## Objective

Prove that the already accepted Customer Privacy product and worker lifecycle can be restored and operated through executable production-like evidence without changing its business behavior, public coordinates, schemas, migrations, persistence ownership or authorization model.

## Accepted input boundary

Repository Step 20A remains accepted through PR #292. This packet reuses:

- the governed `customer_privacy.case.list@1.0.0` and `customer_privacy.case.get@1.0.0` product journey;
- the production-composition seed fixture, which creates Party and PrivacyCase state only through governed mutations;
- the assembled `crm-api` process, PostgreSQL persistence, Vite product plane and Chromium acceptance;
- the accepted one-worker Customer Privacy inventory, seven public mutations and four permission-aware public queries.

No direct Customer Privacy record insertion, mock backend, alternate API, new capability, new worker or production-source behavior is permitted.

## Executable evidence

`Customer Privacy Operations CI` must prove all of the following on one unchanged pull-request head:

1. **Restore**
   - run the immutable PostgreSQL 17 image already accepted by the local lifecycle;
   - apply the authoritative migrations and test fixtures;
   - create the Customer Privacy fixture through assembled governed mutations;
   - create a custom-format logical backup with owner and privilege replay disabled;
   - retain the backup only inside the job with mode `0600` and a recorded SHA-256 digest;
   - restore into a distinct database and verify the restored schema inventory;
   - start the assembled `crm-api` against the restored database and complete the real Chromium Customer Privacy journey.

2. **SLO and performance**
   - restored `crm-api` reaches `/readyz` within the committed startup objective;
   - the committed number of readiness probes has zero failures;
   - nearest-rank p95 readiness latency stays within the committed threshold;
   - browser acceptance has a committed bounded timeout.

3. **Observability**
   - `/healthz`, `/readyz` and `/metrics` are exercised on the restored process;
   - metrics contain the exact Customer Privacy list/get coordinates after the browser journey;
   - metrics do not contain the bearer token, Party fixture identifier or display-name fixture marker;
   - bounded report and metric digests are retained as CI evidence.

4. **Security**
   - the existing browser suite re-proves unauthenticated/session-expiry and cross-tenant concealment against restored state;
   - backup permissions and cleanup fail closed;
   - raw backup bytes are never uploaded as a workflow artifact;
   - no secrets are printed through shell tracing or included in the generated report.

5. **Supply chain**
   - every third-party GitHub Action remains immutable-SHA pinned;
   - Rust metadata resolves only through `Cargo.lock` with `--locked`;
   - product dependencies install only through `pnpm-lock.yaml` with `--frozen-lockfile`;
   - the PostgreSQL image is immutable-digest pinned and matches the accepted local lifecycle image;
   - a SHA-256 manifest covers the lockfiles, toolchain, workflow, policy checker and operations runner.

## Quantitative policy

The machine-readable policy is `customer-privacy-operations-policy.json`. Threshold changes require review-visible policy and test changes; environment variables may select collision-free local ports and artifact directories but may not weaken thresholds.

Initial committed objectives:

- restored startup readiness: at most 30 seconds;
- readiness probes: 25;
- allowed readiness failures: 0;
- readiness nearest-rank p95: at most 500 milliseconds;
- restored browser test timeout: at most 90 seconds.

These are acceptance objectives for this repository harness, not a production availability guarantee or customer SLA.

## Non-completion statement

This packet can complete Repository Step 20 only after implementation acceptance and a separate evidence synchronization. It does not by itself close Phase 8A.11, Phase 8A, product-complete expert modules, architecture 10/10 or the Universal CRM product. Repository Step 21 remains blocked until Step 20B is accepted and synchronized.
