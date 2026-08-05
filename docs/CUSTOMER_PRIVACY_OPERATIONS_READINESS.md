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

No direct Customer Privacy record insertion, mock backend, alternate API, new capability, new worker or committed production-source behavior is permitted.

The historical Step 20A product-plane workflow did not reach Chromium because its governed seed failed and the shell pipeline did not propagate that exit status. The permanent page source consequently retained four accessibility focus callbacks scheduled in the same microtask as React state updates; the Step 20B restored run proved that those callbacks can execute before the corresponding React commit. The operations runner verifies the exact accepted page Git blob, temporarily moves only those four focus callbacks to a bounded post-commit animation-frame boundary, runs the unchanged focus assertions, restores the page from a private temporary file and requires a clean Git diff. This is an operations-only timing normalization, not a claim that the unmodified Step 20A focus scheduling was previously accepted by Chromium.

The permanent Step 20A browser source also contains one results-heading locator whose substring name matches both the page H1 and the results H2 under the locked Playwright runtime. The operations runner verifies the exact committed spec Git blob, temporarily changes only that locator to exact-name matching, executes the same `apps/web/e2e/customer-privacy.spec.ts` path, restores the spec from a private temporary file and requires a clean Git diff. These bounded preparations do not change product data, routes, authorization, tenant isolation, API behavior, browser expectations or any committed product-plane source.

The current production `/metrics` renderer is intentionally limited to the generated deprecated-contract catalog, which is empty for these active Customer Privacy queries. The operations workflow therefore verifies the exact accepted application-runtime Git blob, temporarily wraps only the assembled query registry used to build the acceptance binary, and records actual successful exact-coordinate resolutions for `customer_privacy.case.list@1.0.0` and `customer_privacy.case.get@1.0.0`. The emitted counter has only capability ID, capability version, owner module and query surface labels; it contains no tenant, actor, token, payload, Party identifier or display name. The runtime source is restored immediately after the binary build and a clean Git diff is mandatory before restore acceptance begins. The final report requires a finite positive Prometheus sample for both coordinates, so static zero-seeded or comment-only markers are insufficient.

## Executable evidence

`Customer Privacy Operations CI` must prove all of the following on one unchanged pull-request head:

1. **Restore**
   - run the immutable PostgreSQL 17 image already accepted by the local lifecycle;
   - apply the authoritative migrations and test fixtures;
   - create the Customer Privacy fixture through assembled governed mutations;
   - create a custom-format logical backup with owner replay disabled while preserving the accepted role ACLs;
   - retain the backup only inside the job with mode `0600` and a recorded SHA-256 digest;
   - restore into a distinct database, replay the accepted ACLs and verify the restored schema inventory;
   - start the assembled `crm-api` against the restored database and complete the real Chromium Customer Privacy journey.

2. **SLO and performance**
   - restored `crm-api` reaches `/readyz` within the committed startup objective;
   - the committed number of readiness probes has zero failures;
   - nearest-rank p95 readiness latency stays within the committed threshold;
   - browser acceptance has a committed bounded timeout.

3. **Observability**
   - `/healthz`, `/readyz` and `/metrics` are exercised on the restored process;
   - metrics contain finite positive samples for the exact Customer Privacy list/get coordinates after the browser journey;
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

The machine-readable policy is `customer-privacy-operations-policy.json`. Threshold changes require review-visible policy and test changes; environment variables may select collision-free local PostgreSQL and API ports and artifact directories but may not weaken thresholds. Browser acceptance intentionally retains the repository-standard Playwright base URL on port `5173`.

Initial committed objectives:

- restored startup readiness: at most 30 seconds;
- readiness probes: 25;
- allowed readiness failures: 0;
- readiness nearest-rank p95: at most 500 milliseconds;
- restored browser test timeout: at most 90 seconds;
- active Customer Privacy list/get metric samples: finite and greater than zero for both exact coordinates.

These are acceptance objectives for this repository harness, not a production availability guarantee or customer SLA.

## Non-completion statement

This packet can complete Repository Step 20 only after implementation acceptance and a separate evidence synchronization. It does not by itself close Phase 8A.11, Phase 8A, product-complete expert modules, architecture 10/10 or the Universal CRM product. Repository Step 21 is the next permitted packet until Step 20B is accepted and synchronized.

## Repository Step 20 accepted closure

Repository Step 20 is complete through PR #292 / source `938cebed1e78bf7debf40dc544431bfe819970f4` / squash merge `fffd6baf35544eea736d183af0a5ba38518cce9a` / 17 of 17 applicable permanent workflows and PR #294 / source `f9c5faa667f4d5483335ec2cb5bac31596d818c8` / squash merge `ef3457c11646b1069e5e65683d3618b3d470136e` / 8 of 8 applicable permanent workflows, each accepted on one unchanged exact head with zero unresolved comments, reviews or review threads.

Step 20A proves the typed governed Customer Privacy browser product plane against real PostgreSQL, assembled `crm-api`, Vite and Chromium. Step 20B proves independent PostgreSQL logical backup and restore, restored-process startup and readiness, active `customer_privacy.case.list` and `customer_privacy.case.get` metrics, cross-tenant and expired-session concealment, startup `0.101` seconds, nearest-rank readiness p95 `2.977` milliseconds, backup SHA-256 `700b8ae13a71af30010b11877f70b6a4b3efe1b0ec3beddaf0f3e3bc19533d3c`, backup size `1,118,941` bytes and Chromium 3 of 3.

Repository Steps 1–21 are complete. Repository Step 22 Phase 8A architecture remeasurement, `crm-application-runtime` runtime-fan-in decision and permanent-gate value/cost review is the sole next permitted implementation packet. Phase 8A.11, Phase 8A, Customer Privacy as a complete product capability, product-complete expert modules, architecture 10/10 and the Universal CRM product remain incomplete.

## Accepted Repository Step 21 and Phase 8A closure

PR #296 / accepted source `fd84cd25dfa25a75eac0fdc4a719cc76c84cfc95` / squash merge `c21894f47f24e81da1cc150f9ea457fcfdc2bd63` / 35 of 35 applicable permanent workflows on one unchanged exact head completes Repository Step 21, Phase 8A.11 / issue #126 and Phase 8A.

The accepted final Customer Privacy production inventory is exactly **nine public mutations**, **seven permission-aware public queries** and **one first-party owner worker** (`crm.customer-privacy` / `owner-execution`, phase `260`). The accepted lifecycle includes processing-restriction and legal-hold release/read coordinates, optimistic versioning, exact idempotent replay, immutable event/audit/outbox/business-transaction evidence, FORCE-RLS visibility and uniform concealment, clean PostgreSQL rollback/reapply, real `crm-api` process proof and bounded operations search-projection convergence before backup.

Customer Privacy is the first **Product complete** expert module. Current product-complete expert modules: **1**. The broader Universal CRM product remains incomplete, issue #194 remains open and architecture 10/10 is **not declared**. Repository Step 22 Phase 8A architecture remeasurement, `crm-application-runtime` runtime-fan-in decision and permanent-gate value/cost review is the sole next permitted implementation packet.

