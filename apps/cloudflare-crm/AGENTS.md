# Cloudflare CRM — operating contract for ChatGPT development

**Целевая схема (утверждённая норма для нового кода):** [ARCHITECTURE.md](ARCHITECTURE.md), Issue #350. Исторический код мигрирует в неё, а не наоборот. Документ — спецификация, не доказательство завершённой миграции.
**Маршрутизатор выбран окончательно:** один встроенный Axum 0.8 + штатный Connect-Rust/Buffa внутри единственного workers-rs Worker; никогда не запускать отдельный Axum/Hyper сервер или JS gateway. #350 — единственный детальный план миграции, #338 — верхний план.

This file applies to **apps/cloudflare-crm/**. The sole canonical source of truth is
**iamaman11/crm**: issue #338 (единственный актуальный план Rust/Cloudflare и диагностики), #310 (product/acceptance), #321 (Cloudflare architecture),
#322 (chat delivery), and the active PR/commit. Consult root AGENTS.md for
repository-wide governance; this guide describes the native Cloudflare application.
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


**User decision 10 November 2026:** Do NOT enable GitHub branch-protection rulesets, required CI checks, bypass restrictions, or direct-push prohibitions for `dev` and `main`. The existing CI/deploy/live acceptance remains part of the release workflow. This governance exception does not allow skipping or weakening tests for GO.

**CI execution topology (11 November 2026):** PR targeting `dev` runs only
`.github/workflows/cloudflare-crm-spike.yml`. Checks are sequential in cost:
dependency-free repo topology/architecture/source checks, exact Rust complexity
and formatting budgets, Buf/TS/generated contracts/unit checks, then expensive
Rust/WASM/workerd/D1/Chromium. `deploy-crm-dev` runs only after this gate
on a successful `push dev`; it uses the sealed SHA-matched artifact with
automatic rollback on failed physical acceptance. Legacy PostgreSQL and
historical monorepo workflows target PR to `main` only; their checks are not
deleted. `scripts/check_ci_event_policy.py` + negative tests reject a
second workflow PR trigger for `dev`. No branch protection / required
GitHub status checks are configured.

## Recovery after context loss

1. Open canonical GitHub `iamaman11/crm`: Issue #338 and this `AGENTS.md`.
2. Read `dev` HEAD and the latest `Cloudflare CRM Spike CI` run for that SHA.
3. Read active Cloudflare `crm-dev` Worker version, its `GIT_SHA`, and the one D1 binding. Published SHA, not branch name, proves what is live.
4. If CI or deploy failed, inspect the exact job and live Chromium/D1/Logs evidence. An unmatched SHA means NOT_DEPLOYED until the protected acceptance passes.
5. Ordinary accepted fixes use `dev`. The ONE full architecture cutover is the explicit exception: work in temporary `cutover/complete-connect-tsx-20261110` and one PR to `dev`; no interim deploy. Never create another Environment, Worker, D1 or shadow plan.

## First five actions in every new chat or development slice

1. Read the latest #338 (and related #310), current PR/branch HEAD, this scoped AGENTS.md, and the
   affected code in iamaman11/crm (not another similarly named CRM repository).
2. Use the existing registry, `npm run locate -- <module-or-error-code>`,
   and `npm run doctor` through GitHub Actions. Do not scan unrelated Rust code.
3. Locate the owning feature; trace real API error code + x-request-id to Worker
   log, then verify reproducibility. Never label guesses as diagnosed causes.
4. Change only owned files, necessary published contracts, migration and tests;
   do not weaken security, tenant isolation, or existing CI to obtain green.
5. Ordinary changes: `dev` → existing CRM CI/build → crm-dev deployment → real Chromium E2E → D1/Logs readback. Full architecture cutover: one temporary integration branch/PR to `dev`, same full tests, then one accepted merge/deployment. No partial deployment.
   Record exact SHA and PASS/FAIL/NOT_TESTED in Issue #338. Do not merge
   `main` merely to deploy; it receives accepted milestones.

**Growth is open-ended, never 13 forever.** The canonical authored module
inventory is `modules/*/module.yaml` (strict schema/typed manifest compiler).
`scripts/crm-module-coverage.py` derives *temporary*, SHA-bound CI evidence
from those manifests, actual Worker Cargo dependencies and active Connect
proofs. Never recreate `contract-coverage.json` or a competing hand-written
feature catalogue. A discovered manifest is not an enabled service; new
capabilities require explicit statically linked Rust implementation, generated
RPC contract, permissions, D1 migration where necessary and physical tests.
Read-only compositions may have a Rust crate outside `modules/`. Module
count, source-crate count, UI feature count and RPC count are independent.
`shared/catalog.mjs` remains only a temporary legacy developer/UI locator
until the same full TSX cutover; do not use it for API ownership acceptance.

## Feature ownership and growth

- The current visible owner-source list is `shared/catalog.mjs:modules`. Rust domain contracts, not this UI locator, determine business validity.
- `implementationOwners` maps each feature to its *source-code location*;
  `ownerFiles` maps that location to UI, API, validation, migration, unit and E2E.
  `npm run locate` must use these maps, not a hardcoded universal file list.
- Existing screens use the original `crm-customer-accounts`, `crm-contact-points`,
  `crm-sales` and `crm-activities` domain crates. The temporary `pilot.rs`
  backend and its duplicate business rules are removed in integration PR #345.
  New capabilities must reuse original crate owners, not invent a new pilot
  master, registry or microservice. The twelve original module workflows already
  accepted in Issue #338 are not full module implementations. Customer Enrichment
  has an original immutable provider/mapping *configuration-only* slice;
  provider dispatch, response processing and Party application remain NOT_CONNECTED.
  An accepted UI/D1 scenario must be proven by protected CI before claiming live.
- The one Worker composes original module commands and existing typed contracts.
  Rust owner code alone decides transitions; the UI may display forms and
  transport IDs/versions, but must not duplicate domain validation.
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
- Protected GET /api/doctor is read-only and reports the actual native runtime, exact deployed SHA/version, Access, D1 schema, tenant-scoped reads and D1 request-ID audit capability. It never claims a CI/browser probe it did not execute;
  internal traces, SQL, credentials and user data stay out of responses.
- `npm run doctor` verifies registry ↔ migration and source ownership;
  `npm run locate -- deals` and `-- INVALID_INPUT` print exact owner paths.
  These local in-process lookups can take milliseconds. GitHub CI, network calls,
  D1 queries, browser workflows and actual root-cause diagnosis cannot be
  promised in milliseconds.
- At every feature slice, demonstrate a locator hit, targeted test and one
  real useful behavior, including relevant failure/rollback and tenant cases.
  No documentation-only acceptance. When a write succeeds but the next UI read
  fails, report the write as saved and the displayed data as potentially stale;
  never present a readback failure as failure of the already committed mutation.

## Preserve simplicity as scope grows

Keep one Cloudflare Worker + Static Assets + D1 + existing Cloudflare Access
until a **measured business requirement** proves another component necessary.
Industrial quality means real correctness, usable behavior, sound tenancy,
backups/recovery when needed, observability and verified acceptance; it does
not mean adding infrastructure by default. Per owner decision, optional extra
hardening is deferred until **after real production**, not merely until launch.

Do not assert full industrial readiness or human interactive login without
physical proof. Ordinary CRM fixes in `dev` need no PR; milestone `dev → main`
PR must pass its stated acceptance criteria.

### Visual evidence for every deployed release

The **existing** protected `deploy-crm-dev` job runs real Chromium against the
published URL (only after protected `push dev` release). Its `crm-dev-release-<git-sha>` artifact contains
`e2e-evidence/manifest.json` and screenshots of desktop home, Rust Party,
original Accounts, ContactPoints, Sales, Activities, Account Overview, pipeline, linked records,
and mobile UI. The manifest records the exact live Worker SHA, actions, API
readbacks and PASS/FAIL without tokens, cookies, tenant IDs or raw customer data.
Failure screenshots and relevant Actions job logs remain available. This is
**headless reproducible UI interaction**, not an always-open interactive browser
that ChatGPT can directly control from this conversation. Keep this in the
existing E2E and release artifact; do not create a second dashboard or service.


**Принцип функциональной приёмки:** исходные модули не обязаны повторять
пилотные CRUD. У Party и Account нет фиктивного Delete; Task завершается
оригинальной командой Complete, Deal продвигается через Advance с явной
версией, а деньги представлены точным целым числом минимальных единиц.
Любая проверка должна вызывать именно оригинальный command/query,
не маскировать несовместимость и не вводить JS-подстановки.

Любая отметка REACT_CONNECTED_NOT_LIVE означает только wiring в ветке:
Cloudflare deploy, настоящий Access A/B, фактические D1 и Logs остаются
NOT_TESTED до защищённой публикации из `dev`. Не превращать успешную
локальную workerd/Chromium проверку в утверждение об опубликованном продукте.

## Нативная приёмка — единственная действующая модель

Публикуемый `wrangler.jsonc.main` всегда `./rust-party/build/index.js`. JS API-router и дублированные JS домены запрещены. Единственный штатный `deploy-crm-dev` после публикации выполняет реальный Chromium E2E; `native-functional-cycle.json` содержит все UI-мутации и идентификаторы HTTP-запросов, `verify-data` сверяет каждую с D1 audit и Cloudflare Logs. Без полного PASS активный Worker откатывается и релиз не считается принятым. `GET /api/doctor` не подменяет эту приёмку статическим PASS. Результат обязательно фиксировать в Issue #338.

- The user-facing new-customer path must reuse original Party and Account domain commands: Party creation then preselected Account creation. Never duplicate those rules in React, add a fake atomic browser transaction or claim Party was rolled back on a failed Account submission.

- Deal display may show major currency units and percent, but only the UI boundary converts them to exact integer Rust `amount_minor_units`/`probability_bps`. Avoid floating-point currency math. Keep native crm-sales validation and live edit/reload/negative tests; never introduce a shadow domain owner.

- Existing physical UI inventory: `e2e/ui-inventory.mjs` enumerates 10 JSX components, 60 button templates, and 93 mandatory user action cases. `test/native-runtime.test.mjs` verifies JSX button/form/input/select counts and per-action source markers; live Chromium `e2e/live.mjs` requires the corresponding actual `cover(id)` for every case. Update the inventory and browser actions together for every new control; passing the old count is NOT proof of 100% state/branch coverage. Do not make up an extra test service or source-of-truth plan.


## Accepted target architecture and automated gates — 2026-11-10

**Canonical decision:** [Issue #350](https://github.com/iamaman11/crm/issues/350)
is the implementation checklist under the sole master plan #338.
For this Cloudflare application the accepted deployment is:
`React 19 + strict TypeScript → generated Buf Connect-Web → Cloudflare Access → ONE workers-rs Rust/WASM Worker (Connect; Axum routing only) → original Rust module owner → ONE native D1`.
The web interface uses standard CSS custom properties, responsive CSS, and
light/dark/system themes; none of this changes the backend.

**Scoped precedence:** root `AGENTS.md` governs common domain invariants and
repository practices. Its historical PostgreSQL/gateway deployment instructions
do **not** prescribe the Cloudflare CRM runtime: this scoped guide and approved
#338/#350 govern `apps/cloudflare-crm/`. Never add PostgreSQL, a second Worker,
database, gateway, JS API backend, parallel workflow/Environment or per-change
branch to reconcile legacy guidance. Resolve actual domain-invariant conflicts
explicitly in the canonical Issue, not by weakening a gate.

**One feature, one owner, one contract:**
1. Identify the existing `modules/crm-*` owner and command/query. The domain
   decides business rules, permissible actions, and versions. Do not mirror the
   domain in React or in a transport handler.
2. For CRM RPCs use the corresponding versioned `proto/` contract, generated
   Buf client and the same Connect service in the deployed native Worker.
   Keep native `prost` contracts where already required by original modules;
   `connectrpc`/Buffa belongs at the transport edge. Typed conversion at a
   real boundary is legitimate; additional gateway layers, duplicate DTO/domain
   models and repeated JSON/Protobuf serialization are not.
3. Access authenticates actor, role and tenant **once** before RPC dispatch.
   Do not trust client-supplied tenant, request ID, or owner. Preserve server
   `request_id` through response, D1 audit and Cloudflare Logs.
4. One shared Rust operation and the same D1 for each transition. During
   migration legacy REST may coexist *temporarily*, but must call the same
   original command. Remove each replaced REST endpoint/client after the
   matching Connect path passes physical acceptance. Keep non-RPC exceptions
   (e.g. CSV streaming/download) explicitly listed in #350.
5. New feature path: `Rust owner → .proto → generated Connect-Web → React TSX
   → existing Cloudflare CRM Spike CI → real protected Chromium/D1/Logs`.
   No separate CI, schema source, runtime service or adapter framework.

**Fail-closed CI target (migration is currently IN PROGRESS):**
extend only `.github/workflows/cloudflare-crm-spike.yml` with Buf
lint/breaking/generation consistency, strict `tsc --noEmit`, pinned Rust
WASM and existing workerd/Chromium/Access A/B/D1/Logs/rollback checks.
Do not add `|| true` to an acceptance command. Temporary explicitly listed
legacy paths are tolerated during migration, but no new manual JSON API
clients, unknown second API router, additional D1/Worker or duplicate state.
Mark a gate enforced only after the check actually exists and passes.
Do not label issue #350 DONE from a compile-only/transport-only proof.

**Global UI component ownership (approved):** design tokens and common
components are the only authority for shared styling. Adopt native CSS
Modules for scoped feature layouts, semantic CSS custom properties for colors,
spacing, typography, radius, focus, and states, and a small set of shared React
TSX primitives in `web/src/ui/` (Button, Input/Select, Dialog, Table, Card,
Notice, etc.), extracted only as real repeated usage requires. One change to
`ui/Button` or its shared tokens must propagate to buttons on *every* screen,
not merely the current page. The same holds for other shared UI elements.
Create new screens by composing these primitives; never reimplement global
controls, inline hardcoded colors, copy component CSS per screen, or override
common styles from feature modules. Feature `*.module.css` controls page layout
and truly feature-specific presentation only; shared components own appearance,
variants, interaction states, disabled/focus/hover behavior, and accessibility.
Extend common primitives centrally with typed variants when justified, not a
second button or design system. During migration, replace legacy raw controls
incrementally, update source-based inventory tests, and reject **new** bypasses.
Once migrated, enforce adoption in the same Cloudflare CRM CI; do not switch
to snapshot-count-only validation.

**Native CSS and theme:** keep Vite and standard CSS (no Tailwind, Bootstrap,
Sass or large UI library without a measured need), but migrate the present
monolithic `web/src/styles.css` to `web/src/styles/tokens.css`,
`themes.css`, `global.css`, shared `ui/*.module.css`, and
feature-scoped `*.module.css`. Theme preference is `system` (default),
`light`, or `dark`, stored only in browser `localStorage`, never D1;
system follows `prefers-color-scheme`. All shared components consume tokens
so changing tokens or the shared component updates the entire application.
Existing inventory and protected Chromium acceptance must include switch,
persistence/reload, dialogs, tables, mobile, forms, hover/focus/disabled and
error states in both themes. A styling change is ACCEPTED only after physical
verification; no second CI or UI service.

**Engineering simplicity:** one permanent `dev`, one GitHub Environment
`crm-dev`, one existing CI, exact committed source and published SHA.
Optional extra hardening remains deferred until *after real production*.
