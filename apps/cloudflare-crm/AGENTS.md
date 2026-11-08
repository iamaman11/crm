# Cloudflare CRM — operating contract for ChatGPT development

This file applies to **apps/cloudflare-crm/**. The sole canonical source of truth is
**iamaman11/crm**: issue #310 (product/acceptance), #321 (Cloudflare architecture),
#322 (chat delivery), and the active PR/commit. Consult root AGENTS.md for
repository-wide governance; this guide describes the native Cloudflare pilot.
When instructions conflict, do not quietly weaken the repository gate: resolve
the conflict transparently in the canonical issue and exact-source tests.

## End state — complex product, simple engineering

The CRM must become **expert, functionally powerful and industrial**. Do not
constrain business capability to keep code small. Instead isolate complexity:
each business feature has one owner, defined inputs/outputs, local invariants,
explicit data ownership, correlated errors, and targeted tests. Keep cross-feature
flows explicit. Share mechanisms only after demonstrated reuse; no speculative
framework, service or parallel data/auth stack.

**Developer workflow is exclusively this ChatGPT conversation → GitHub code,
issues and PR → GitHub Actions → Cloudflare deployed readback and real Chromium
E2E.** ChatGPT owns diagnosis, implementation and acceptance; never delegate
ordinary work to a local Windows agent or require users to run developer commands.
GitHub Actions may run those commands. No new app or connector is a prerequisite.

## First five actions in every new chat or development slice

1. Read the latest #310, current PR/branch HEAD, this scoped AGENTS.md, and the
   affected code in iamaman11/crm (not another similarly named CRM repository).
2. Use the existing registry, `npm run locate -- <module-or-error-code>`,
   and `npm run doctor` through GitHub Actions. Do not scan unrelated Rust code.
3. Locate the owning feature; trace real API error code + x-request-id to Worker
   log, then verify reproducibility. Never label guesses as diagnosed causes.
4. Change only owned files, necessary published contracts, migration and tests;
   do not weaken security, tenant isolation, or existing CI to obtain green.
5. Check fast targeted tests → complete CRM CI/build → protected Cloudflare
   deployment readback → real Chromium E2E → monorepo required checks.
   Record exact SHA and honest PASS/FAIL/NOT_TESTED evidence in #310/PR.

## Feature ownership and growth

- The single list of business features and fields is `shared/catalog.mjs:modules`.
- `implementationOwners` maps each feature to its *source-code location*;
  `ownerFiles` maps that location to UI, API, validation, migration, unit and E2E.
  `npm run locate` must use these maps, not a hardcoded universal file list.
- The initial four modules intentionally share the tiny `pilot-core` files. **A
  new expert module needs a distinct owner**, typically code under
  `features/<feature>/`, plus registry and ownerFiles entries. Do not create
  new microservices; these are source-code modules in the same Worker/React app.
- As existing owners gain substantial independent business rules, extract their
  real implementation into an owned folder in one behavior-preserving slice,
  update ownerFiles, and prove parity. No speculative mass refactor.
- The Worker/React entrypoints coordinate/auth/transport; domain-specific
  decisions belong to their feature. Feature A must not mutate B's D1 rows
  without an explicit reviewed cross-feature contract.
- New/changed D1 tables or fields require additive ordered migrations and
  appropriate tenant/isolation negative tests. No implicit prod migrations
  or destructive assumptions. Existing accepted tests remain applicable.

## Developer observability and honest speed

- Browser errors show a stable error code and x-request-id; the Worker logs
  code, request_id, HTTP status, and measured duration. Never log Access tokens,
  secrets, personal data or raw request bodies.
- Protected GET /api/doctor is read-only and reports deployed SHA/schema state;
  internal traces, SQL, credentials and user data stay out of responses.
- `npm run doctor` verifies registry ↔ migration and source ownership;
  `npm run locate -- deals` and `-- INVALID_INPUT` print exact owner paths.
  These local in-process lookups can take milliseconds. GitHub CI, network calls,
  D1 queries, browser workflows and actual root-cause diagnosis cannot be
  promised in milliseconds.
- At every feature slice, demonstrate a locator hit, targeted test and one
  real useful behavior, including relevant failure/rollback and tenant cases.
  No documentation-only acceptance.

## Preserve simplicity as scope grows

Keep one Cloudflare Worker + Static Assets + D1 + existing Cloudflare Access
until a **measured business requirement** proves another component necessary.
Industrial quality means real correctness, usable behavior, sound tenancy,
backups/recovery when needed, observability and verified acceptance; it does
not mean adding infrastructure by default. Per owner decision, optional extra
hardening is deferred until **after real production**, not merely until launch.

Do not assert full industrial readiness or human interactive login without
physical proof. PR stays Draft until its stated acceptance criteria are met.

### Visual evidence for every deployed release

The **existing** protected `deploy-crm-dev` job runs real Chromium against the
published URL. Its `crm-dev-release-<git-sha>` artifact contains
`e2e-evidence/manifest.json` and screenshots of desktop home, Rust Party,
customers, contacts, deals, tasks, Customer 360, pipeline, updated linked card,
and mobile UI. The manifest records the exact live Worker SHA, actions, API
readbacks and PASS/FAIL without tokens, cookies, tenant IDs or raw customer data.
Failure screenshots and relevant Actions job logs remain available. This is
**headless reproducible UI interaction**, not an always-open interactive browser
that ChatGPT can directly control from this conversation. Keep this in the
existing E2E and release artifact; do not create a second dashboard or service.
