# Ultimate CRM — Ultimate Architecture 10/10 Review and Closure Plan

Status: **Expert review companion and closure design**  
Review baseline: `main` at `eac6707e6799f74e761ede39d852bf8de7ac6a77` (Repository Step 22D merge)  
Primary normative program: `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` / issue #194  
Companion CI plan: `ARCHITECTURE_CI_SCALABILITY_AND_DEVICE_LAB_PLAN.md`

This document records an independent architecture/developer-experience review and a concrete path to an **actual mechanically demonstrated 10/10**, not a declared score. It is deliberately a companion to the existing normative architecture plan, not a competing roadmap. The repository execution order remains the one defined in `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md`.

The foundational architecture should **not** be replaced. The remaining work is convergence: remove unnecessary central knowledge, finish dependency/gate decisions, lower change and CI cost, eliminate documentation drift, prove the model through contrasting expert domains, and make the final quality state reproducible by a new developer and by automation.

---

## 1. Executive assessment

The repository already has unusually strong architecture governance:

- explicit authoritative owner domains;
- modular-monolith boundaries rather than network-distribution theater;
- exact versioned capabilities/queries/events/workers;
- governed execution, live authorization and durable activation;
- PostgreSQL transaction/idempotency/outbox/audit semantics;
- FORCE RLS and cross-tenant negative evidence;
- rebuildable projections/search;
- contract lifecycle governance;
- privacy owner lifecycle;
- affected-scope analysis;
- deterministic local lifecycle tooling;
- exact-head acceptance discipline;
- architecture policies that mechanically reject forbidden dependency shortcuts.

The project is therefore much closer to 10/10 than to a redesign.

### Current expert scorecard

These are review signals, not normative completion claims:

| Dimension | Current review | 10/10 condition |
|---|---:|---|
| Business ownership/modularity | 9.7 | ordinary owner growth remains isolated across Steps 23–24 |
| Layering | 9.6 | zero accidental owner logic in generic runtime/process layers |
| Architecture purity | 9.3 | all Step 22 fan-in rows resolved and removable owner edges removed |
| Change isolation/safety | 9.7 | measured low fan-out for representative leaf changes as product grows |
| Extensibility cost | 8.8 | Steps 23–24 prove near-constant ordinary capability cost |
| Developer comprehension | 8.7 | one mechanically explainable intent-to-code/test path with no stale live status |
| Build/CI scalability | 7.8 | value/cost-led gate topology, isolation, caching and critical-path evidence |
| Local reproducibility | 9.5 | clean-machine lifecycle remains deterministic and fast enough for daily use |
| Contract/lifecycle maturity | 9.8 | permanent compatibility/retirement evidence remains intact under new waves |
| Security/privacy | 9.8 | same guarantees preserved for every new expert domain and protected payload |
| Frontend/product architecture | 8.2 | representative expert domains have production-grade record/work surfaces |
| Operations/recoverability | 9.3 | restore/SLO/security/supply-chain evidence becomes standard, not bespoke |
| Overall architecture maturity | ~9.2 | Step 22 + Steps 23–24 + independent Step 25 evidence all pass |

The main structural gap is no longer “is the architecture conceptually correct?” It is “has the architecture demonstrated bounded cost and clean central boundaries as the product scales?”

---

## 2. Current verified state and drift

At the reviewed `main`:

- Repository Steps 1–21 are complete in the normative plan;
- Step 22 is in progress;
- Step 22A froze the runtime-fan-in and permanent-gate inventory;
- Step 22B finalized the first conclusive classifications;
- Step 22C removed the direct Customer Privacy query-adapter runtime edge;
- Step 22D removed the direct Customer 360 query-adapter runtime edge;
- the current Step 22 decision ledger reports 61 current internal direct dependencies of `crm-application-runtime` after the two removals, 60 production + 1 test-only;
- 19 accepted-inventory rows have final classifications, including 16 platform-generic, 2 removed and 1 test-only;
- 44 accepted inventory dependencies remain unresolved;
- permanent workflow/job disposition decisions remain unresolved;
- architecture 10/10 is correctly not declared.

There is also observable **current-state documentation drift**:

- `PROJECT_STATUS.md` still uses a 2026-08-05 checkpoint and describes Step 22 generically;
- issue #194 body is much older and still describes Step 14 as next;
- generated `ACTIVE_PACKET.md` still describes Step 22D as active even though PR #301 is merged.

The generated packet may legitimately remain until a dedicated synchronization packet is accepted, but from a developer-comprehension perspective this drift is now a first-class defect to remove from the final 10/10 state.

---

## 3. What “ultimate 10/10” must mean

A 10/10 claim should require all of these properties simultaneously:

1. **One owner per mutable aggregate** — no ambiguous authority.
2. **Pure business boundaries** — business modules have no raw database/network/provider clients.
3. **Exact governed runtime** — every mutation/query/worker uses exact typed registration and live authorization.
4. **No owner knowledge in generic algorithms** — routers/workers/process host do not branch on owner identities.
5. **Near-constant ordinary change cost** — adding a normal capability to an existing owner does not become more expensive as module count grows.
6. **Bounded new-owner cost** — a new domain introduces only justified packages/boundaries.
7. **Mechanical dependency governance** — no hidden or unresolved process-host edges.
8. **Mechanical exception governance** — no expired, ownerless or invisible exceptions/suppressions.
9. **Contract lifecycle completeness** — compatibility, deprecation, migration and retirement are real production disciplines.
10. **Storage correctness** — RLS, rollback/reapply, retention and recovery are owner-verified.
11. **Replay correctness** — derived state is rebuildable and historical evidence remains interpretable.
12. **Explainable CI selection** — every selected and skipped gate has a machine-readable reason.
13. **Efficient CI topology** — no material duplicate equivalent work; expensive suites are isolated and parallelizable when safe.
14. **Low flake rate** — retry/rerun is not the normal route to green.
15. **Fast developer feedback** — focused leaf changes receive a short trustworthy local/CI path.
16. **Deterministic clean-machine development** — bootstrap/dev/reset/seed/smoke are reliable.
17. **Product-plane parity** — critical expert-domain journeys are accessible, typed and production-backed.
18. **Operational parity** — restore, observability, SLO, performance, security and supply-chain proof accompany product completion.
19. **Documentation truthfulness** — a new contributor never has to infer current state from conflicting documents.
20. **Reproducible final proof** — Step 25 can rerun the measurements and reach the same pass/fail outcome without interpretation by the original author.

A score is therefore an output of evidence, not a substitute for it.

---

## 4. Repository Step 22 — required closure strategy

Step 22 is the most important architecture checkpoint before adding new expert domains. It should finish in small bounded packets, but the cumulative outcome must be comprehensive.

### 4.1 Dependency objective

The long-term shape of `crm-application-runtime` should be:

```text
crm-application-runtime
  -> platform-generic runtime/composition contracts
  -> one bounded first-party contribution aggregation boundary
  -> generic infrastructure/process provider registries where a real process boundary exists

NOT

crm-application-runtime
  -> dozens of owner capability/query/composition adapters
```

The process host should know that “first-party contributions exist”, not how each owner implements them.

### 4.2 Finish all 44 unresolved rows

For every remaining accepted Step 22A stable ID, record exactly one final classification:

```text
removed
platform-generic
owner-specific-unavoidable
test-only
```

No nulls, “temporary”, “transitional” or invented fifth category should survive Step 22.

Each retained production edge needs:

- stable ID;
- final classification;
- boundary ID;
- owning team/domain;
- exact source/manifest evidence;
- why the runtime must know this boundary;
- why aggregation or inversion is not safer/cleaner;
- review/reopen condition.

Every `owner-specific-unavoidable` edge should carry a much higher proof burden than `platform-generic`.

### 4.3 Preferred outcome: zero owner-specific runtime fan-in

The architecture should actively attempt to reach **zero direct owner-specific dependencies in `crm-application-runtime`**.

This is not a dogmatic package-count target. It is a dependency-direction target: owner identity, capabilities, queries, visibility and workers should enter through owner-owned production contributions and bounded aggregation.

Retain an owner-specific direct edge only when there is a concrete boundary that cannot safely be represented by the contribution model.

### 4.4 Extend first-party aggregation beyond mutation/query inventory

`crm-first-party-modules` is currently a mechanically narrow aggregation boundary. To eliminate remaining runtime owner knowledge, the contribution model can be expanded carefully to aggregate additional owner-owned production metadata without becoming a central business router.

Potential generic contribution categories:

```text
ModuleContributionSet
MutationContributionSet
QueryContributionSet
VisibilityContributionSet
WorkerContributionSet
ProjectionContributionSet
BootstrapVisibilityContributionSet
ProcessProviderContributionSet
ModuleIdentitySet
```

The aggregate package should merge opaque typed contributions. It should not switch on business IDs or own business policy.

### 4.5 Remove module-ID imports from runtime

A practical smell is a runtime source file importing:

```text
OWNER_X_MODULE_ID
OWNER_Y_RECORD_TYPE
OWNER_Z_QUERY_DEFINITIONS
```

When the generic process host needs such constants only to assemble registries, those values should normally travel inside typed contributions rather than be individually imported.

A permanent guard should fail if a new owner-specific `MODULE_ID`/capability inventory import is added to generic runtime source without a registered architecture exception.

### 4.6 Worker contribution closure

Workers are a common source of composition fan-in.

Target:

```text
owner production package
  -> contributes WorkerDefinition/WorkerFactory
first-party aggregation
  -> merges workers
application runtime
  -> validates/sorts/executes generic worker contracts
```

The runtime algorithm should not instantiate owner worker concrete types directly.

Provider/process workers may require a real host-owned transport or secret boundary. In that case, separate:

```text
owner semantics
provider-neutral port
host-owned provider adapter/process contribution
```

rather than making the process host own domain dispatch.

### 4.7 Bootstrap visibility closure

Current bootstrap visibility registration should become fully declarative/module-contributed so `crm-application-runtime` does not need one source edit per owner.

The generic runtime can validate:

- duplicates;
- resource ownership;
- permission identifiers;
- route parity;
- deterministic ordering.

It should not contain the owner-specific inventory itself.

### 4.8 Test-only dependency cleanup

The current direct Consents dev dependency is classified test-only. Step 22 should still ask whether test fixture construction can move to a reusable production-like fixture contribution so generic runtime tests do not need one owner package directly.

If retained, document the exact removal condition.

---

## 5. Step 22 permanent-gate value/cost review

The repository has a large verification surface. Quality is strong, but workflow count is not itself quality.

Every permanent workflow/job needs a ledger row containing:

```text
stable_id
owner
failure_mode_prevented
unique_evidence
input/path ownership
p50_duration
p95_duration
queue_time
setup_time
test_time
runner/environment cost
flake/rerun rate
historical defects caught
known overlap
disposition
retirement condition
```

Final dispositions:

```text
retain
simplify
merge
remove
```

### 5.1 Merge orchestration, not domain evidence

Several privacy-scope workflows share highly similar PostgreSQL setup/orchestration. The preferred optimization is:

- retain owner-specific test commands and failure attribution;
- generate a matrix from governed owner metadata;
- centralize repeated environment setup through reusable workflows/composite actions;
- preserve independent owner test binaries and assertions.

Do **not** centralize owner business semantics into one generic test implementation merely to reduce YAML.

### 5.2 Avoid duplicate push + pull-request work

Feature branches should not run equivalent full checks both as `push` and `pull_request` unless the two runs prove different things.

Target trigger model:

```text
pull_request -> iterative branch verification
main/release -> authoritative post-merge/release verification
```

Use PR-scoped cancellation for superseded iterative runs, but never cancel selected exact-head acceptance evidence as ordinary obsolete work.

### 5.3 CI critical path as a first-class architecture metric

For each PR, measure:

- total workflows selected;
- total jobs;
- queue time;
- wall-clock to first trustworthy failure;
- wall-clock to exact-head green;
- total compute;
- duplicate work;
- cancelled superseded work;
- setup vs compile vs migration vs test time;
- retries/reruns/flakes.

Optimization decisions should cite these measurements.

### 5.4 Rust caching

Pilot trusted Cargo artifact caching and `sccache` under the companion CI policy.

Correctness must remain cache-independent.

Cache identity should include at least:

- OS;
- Rust version;
- target;
- profile;
- lockfile digest;
- material feature grouping.

Untrusted PRs must not publish to a trusted shared cache namespace.

### 5.5 PostgreSQL process isolation

Separate:

```text
migration correctness lane
process behavior lane
```

Migration correctness remains sequential where order is itself the subject under test.

Independent process suites should use isolated database/service namespaces and run concurrently where proven safe.

### 5.6 Flake budget

A 10/10 CI system should target effectively zero accepted flakiness.

Suggested gate:

- any test that requires routine rerun to pass is a defect;
- track 30/60/90-day flake rate per job;
- repeated flake crosses a threshold -> owner required, quarantine only with expiry and compensating evidence;
- phase closure requires no expired flake quarantine.

---

## 6. Package and dependency model

### 6.1 Do not chase a smaller crate count blindly

112 workspace packages is not inherently wrong. The real questions are:

- does the package protect a real boundary?
- does it reduce or increase change fan-out?
- does it permit independent testing/compilation/security?
- does it expose a justified public API?
- does ordinary work require touching it?

Consolidate only when a package has no independent ownership, persistence, process, trust, publication or compiler-enforced visibility reason.

### 6.2 Public Rust surface

Continue reduction-only governance of the conservative public surface.

Additional target:

- default `pub(crate)` or private;
- every new cross-crate public item must have a concrete current consumer;
- unused re-exports are rejected;
- owner implementation types are not re-exported merely for tests;
- test fixtures use explicit testing modules rather than widening production visibility.

### 6.3 Dependency depth and reverse impact

Current maximum dependency depth is 18. Do not optimize the number aesthetically, but Step 25 should show:

- no regression;
- no hidden high-fan-out central package introduced by Steps 23–24;
- the largest reverse-impact package has an explicit platform role;
- owner-specific leaf changes do not pull the whole workspace into affected scope.

An aspirational reduction below 18 is valuable only if it follows real boundary simplification.

### 6.4 External dependency governance

10/10 requires:

- one governed version policy per dependency family;
- feature divergence is intentional and documented;
- no ad-hoc dependency added inside an owner when a stable platform port exists;
- provider SDKs remain in infrastructure boundaries;
- heavy features are measured and non-growing unless justified;
- lockfile changes are reviewed as supply-chain changes.

---

## 7. Developer comprehension and navigation

A sophisticated repository is only 10/10 if a capable new developer can navigate it without archaeology.

### 7.1 One-minute ownership answer

Given `parties.identity_document.review.accept@1.0.0`, tooling should be able to answer:

- owning module;
- Protobuf contract;
- application planner/handler;
- persistence adapter;
- production contribution;
- authorization policy;
- event(s);
- worker(s);
- frontend client/use;
- privacy scope;
- tests;
- workflows;
- current lifecycle/publication state.

### 7.2 Extend `repo.py explain`

Proposed enhancements:

```bash
python scripts/repo.py explain <coordinate> --journey
python scripts/repo.py explain <module> --dependencies
python scripts/repo.py explain <module> --reverse-impact
python scripts/repo.py explain <path> --why
```

`--journey` should render the end-to-end path from product plane to authoritative state and derived outputs.

### 7.3 Add architecture graph output

Generate machine-readable and human-readable graphs for:

- workspace dependency DAG;
- owner/module dependency DAG;
- process-host direct fan-in;
- route owner map;
- worker phase map;
- projection/source relationships;
- privacy scope ownership.

Generated graphs should be deterministic and diffable.

### 7.4 Documentation freshness

A permanent check should enforce that current-state sources cannot remain stale after an accepted packet synchronization.

Possible mechanism:

```text
PROJECT_STATUS accepted_main_sha
repository-packet baseline/status
latest accepted evidence ledger
```

The validator should detect impossible states such as:

- merged packet still described as “not started” in current status;
- issue continuation pointing to a completed historical step;
- generated active packet baseline behind the authoritative declared state.

Historical packet documents remain immutable.

### 7.5 Contribution templates

Scaffolding should produce not just files but a developer checklist:

```text
owner decision
contract decision
persistence decision
privacy/data-class impact
route kind
worker impact
frontend impact
affected checks
operations proof
```

The ideal result is fewer architectural decisions per ordinary feature, not more prose.

---

## 8. Architecture policy strengthening

### 8.1 Generic-runtime owner-import guard

Add a mechanical rule that generic runtime/process packages cannot import owner-specific implementation crates or owner-specific constants unless an exact registered Step-22-compatible boundary exception exists.

### 8.2 Contribution completeness guard

Every first-party owner should expose one discoverable production contribution entry point. A new route/worker/query that is not reachable through that contribution should fail conformance.

### 8.3 No duplicated owner inventory

Owner capability/query/worker inventories should have one machine source. The same list should not be hand-copied across:

- module manifest;
- runtime registration;
- frontend capability list;
- visibility bootstrap;
- tests.

Where multiple representations are required, generate or mechanically compare them.

### 8.4 Architecture exception half-life

Every exception needs:

- owner;
- introduced date;
- expiry;
- risk;
- compensating checks;
- removal condition.

Step 25 requires:

```text
expired exceptions = 0
ownerless exceptions = 0
undocumented bypasses = 0
```

---

## 9. Product-plane architecture closure

Backend excellence alone cannot justify an “Ultimate CRM 10/10” architecture claim.

### 9.1 Production record architecture

The product plane should evolve from isolated proof pages to coherent production record/work surfaces with:

- typed route registry;
- capability-aware navigation;
- governed clients only;
- shared loading/error/empty/retry primitives;
- permission-aware field rendering;
- predictable URL/deep-link semantics;
- record tabs/extensions;
- optimistic mutation only where rollback/conflict semantics are explicit;
- accessible focus and live-region behavior;
- responsive/mobile layouts.

### 9.2 No business invariants in React

Frontend may:

- validate obvious input shape for UX;
- hide unavailable actions;
- guide the user.

Frontend may not become authoritative for:

- permissions;
- lifecycle transitions;
- pricing/CPQ rules;
- privacy policy;
- identity-document acceptance;
- cross-owner invariants.

### 9.3 Frontend package boundaries

As product surface grows, avoid a single `App.tsx` or global client becoming the new central business router.

Recommended shape:

```text
apps/web shell
  -> route feature package/page
  -> typed domain client facade
  -> shared design-system primitives
```

Feature routes can be lazy-loaded and owned by product areas while backend domain ownership remains authoritative.

### 9.4 Browser evidence

Steps 23–24 must prove representative real journeys in Chromium against real backend/process state. The eventual product target should also include cross-browser strategy and mobile/device proof where the feature requires it.

---

## 10. Security and privacy architecture closure

The existing privacy model is a major strength. Step 25 should demonstrate that new domains do not weaken it.

Required final evidence:

- FORCE RLS everywhere applicable;
- cross-tenant negative tests for every new authoritative/query boundary;
- raw secrets/protected documents absent from logs/audit;
- data-class and retention declared for every typed payload;
- privacy scope contributions for customer-related expert domains;
- backup/restore preserving audit/event consistency;
- signed/immutable third-party Actions and supply-chain policy;
- dependency advisories/vulnerability scanning with governed disposition;
- provider/external-network boundaries explicitly owned;
- no broad default egress from pure business modules.

For especially sensitive domains, add threat-model documents that feed acceptance tests rather than exist as passive prose.

---

## 11. Observability and operations closure

Every product-complete expert domain should expose a standard observability envelope:

- request rate;
- success/failure class;
- authorization denial;
- conflict/idempotent replay;
- worker backlog;
- retry age;
- operation latency p50/p95/p99;
- database latency where useful;
- projection lag;
- external provider latency/failure/cost where applicable;
- SLO status;
- restore/runbook references.

Operational evidence should be produced from real assembled process behavior, not mocks.

### Standard product-completion operations gate

A mature reusable operations gate should prove, as applicable:

```text
backup
restore to independent target
process startup from restored state
readiness
critical query/mutation journey
observability presence
cross-tenant denial
security/supply-chain evidence
performance/SLO measurement
```

Avoid creating one bespoke 75-minute workflow per new module when a reusable operations harness with owner-specific scenarios can prove the same guarantees more efficiently.

---

## 12. Step 23 — first contrasting extension proof

The existing plan correctly selects Catalog/Pricing as the first post-Step-22 expert wave.

Why it is a good architecture test:

- reference-heavy;
- effective-dated state;
- versioned catalogs and price books;
- search/read-heavy;
- import/export;
- strong stable-reference semantics;
- meaningful frontend administration.

Step 23 should collect explicit change-cost evidence:

```text
new owner packages
files touched
runtime generic files touched
first-party aggregation files touched
new public Rust items
new dependency edges
reverse-impact delta
CI workflows selected
CI exact-head critical path
local test time
new suppressions/exceptions
```

Success condition: no owner-specific `crm-application-runtime` source/manifest edit for ordinary owner capabilities after the contribution boundary is established.

---

## 13. Step 24 — second contrasting extension proof

CPQ/approvals/orchestration is correctly chosen as a very different workload.

It stresses:

- durable workers;
- human tasks;
- waits/timers;
- serial/parallel approvals;
- retries/cancellation;
- rich cross-owner references;
- process-heavy UX;
- long-running execution evidence.

Step 24 must prove that the architecture remains clean when a domain is not a simple CRUD/reference owner.

Collect the same change-cost metrics as Step 23 and compare them directly.

The architecture should not “pass Step 23” by being good only for simple record modules and then regress for process-heavy domains.

---

## 14. Step 25 — mechanical final closure

Step 25 should produce a single reproducible evidence package, not a prose-only review.

Recommended generated artifact:

```text
architecture-10of10-evidence.json
```

Suggested shape:

```json
{
  "schema_version": "crm.architecture-10of10-evidence/v1",
  "source_sha": "...",
  "workspace": {},
  "dependencies": {},
  "runtime_fanin": {},
  "public_surface": {},
  "exceptions": {},
  "ci": {},
  "change_cost": {
    "step23": {},
    "step24": {}
  },
  "developer_journey": {},
  "local_lifecycle": {},
  "frontend": {},
  "operations": {},
  "security": {},
  "criteria": []
}
```

Each criterion should contain:

```text
id
pass/fail
measurement
threshold/decision rule
evidence paths
source SHA
```

### 14.1 Final pass criteria

In addition to the existing normative Section 13 criteria, this expert review recommends requiring:

1. unresolved Step 22 runtime classifications = 0;
2. owner-specific runtime direct dependencies = 0 unless each has individually accepted unavoidable-boundary evidence;
3. new owner-specific generic-runtime source edits in Steps 23–24 = 0;
4. expired architecture exceptions = 0;
5. unregistered suppressions = 0;
6. hidden direct lint bypasses = 0;
7. permanent gates without disposition/owner/retirement condition = 0;
8. duplicate equivalent feature-branch CI work without rationale = 0;
9. current-state documentation contradictions = 0;
10. stale generated navigation against authoritative inputs = 0;
11. Step 23 and Step 24 extension-cost budgets both pass;
12. max dependency depth does not regress;
13. public Rust surface stays within reduction/non-growth policy;
14. ordinary existing-owner capability still creates 0 crates by default;
15. affected-scope remains fail-closed;
16. clean-machine local lifecycle passes;
17. representative frontend/browser journeys pass;
18. representative restore/SLO/security/supply-chain gates pass;
19. cross-tenant negative proof passes for all representative new domains;
20. the full evidence artifact regenerates byte-for-byte deterministically from the accepted source where timestamps/external telemetry are excluded or canonically referenced.

### 14.2 Independent reproduction

A separate verifier should run the final review from a clean checkout without relying on chat context or undocumented local state.

A 10/10 claim is accepted only if the verifier reaches the same results.

---

## 15. Recommended Step 22 packet sequence after Step 22D

The exact stable IDs should determine final packet boundaries, but a practical continuation is:

### Step 22E — synchronize accepted Step 22D state

- record accepted PR #301 source/merge/workflow evidence;
- advance current status and active packet deliberately;
- do not mix new runtime remediation into evidence synchronization.

### Step 22F — classify/remediate one owner cluster

Choose one coherent cluster with the highest confidence and best fan-in reduction, e.g. a customer-domain set whose production contribution already has an owner/aggregation boundary.

### Step 22G — classify/remediate process-provider cluster

Separate true provider/process dependencies from owner semantics. Introduce or strengthen generic provider contribution boundaries only where justified.

### Step 22H — bootstrap visibility and worker fan-in closure

Move remaining owner inventories behind typed contributions and remove direct runtime imports.

### Step 22I — test-only fan-in review

Retain or remove the Consents dev edge with exact fixture-boundary evidence.

### Step 22J — permanent-gate ledger and low-risk consolidation

Complete every workflow/job disposition and implement safe merges/simplifications.

### Step 22K — Step 22 final remeasurement

Require:

- 0 unresolved dependency classifications;
- 0 unresolved gate dispositions;
- current complexity metrics;
- CI cost/critical-path metrics;
- no regression in invariants;
- accepted exact-head evidence;
- synchronized current-state docs.

This can still be delivered as smaller PRs if repository policy requires it; the sequence is a logical closure map, not authorization to create one giant change.

---

## 16. Metrics that should become permanent

### Architecture

- workspace package count by category;
- internal edges;
- maximum dependency depth;
- maximum reverse impact;
- `crm-application-runtime` direct internal fan-in;
- owner-specific runtime fan-in;
- public Rust surface;
- external dependency declarations;
- heavy-feature declarations;
- suppressions/exceptions;
- change fan-out by representative capability.

### Developer experience

- clean bootstrap time;
- incremental compile/test time for representative leaf owner change;
- `repo.py explain` completeness;
- affected package count;
- local smoke time;
- number of manually required commands per ordinary feature;
- documentation freshness violations.

### CI

- workflows/jobs per PR;
- selected vs skipped reasons;
- queue time;
- p50/p95 job duration;
- exact-head critical path;
- total compute;
- duplicate equivalent runs;
- cache hit/save/restore;
- migration/setup time;
- test time;
- flake/rerun rate.

### Product/operations

- representative browser journey duration/failure;
- process startup/readiness;
- restore time;
- worker backlog/recovery;
- key SLOs;
- supply-chain policy violations;
- cross-tenant/security negative failures.

Trends are more important than isolated scores.

---

## 17. Architecture patterns to preserve

Do not undo the strongest existing decisions while optimizing complexity:

- modular monolith remains the default deployment model;
- network extraction only for operational reasons;
- one authoritative owner per aggregate;
- exact versioned contracts;
- live authorization;
- transaction + idempotency + outbox + audit atomicity;
- FORCE RLS;
- immutable/rebuildable event/projection boundaries;
- module-owned production contributions;
- fail-closed affected scope;
- exact-head acceptance.

“Fewer crates” or “fewer workflows” is not a valid optimization if it weakens these boundaries.

---

## 18. Architecture smells that should block 10/10

Any of the following should prevent final closure:

- generic runtime switches on owner/capability/query/worker IDs;
- a normal existing-owner capability requires a new crate without an independent boundary;
- direct cross-owner SQL/storage access;
- business module imports provider SDK/network client;
- duplicated authoritative route/capability inventory;
- unresolved process-host dependency row;
- permanent gate with no known purpose/owner;
- stale current status forcing developers to reconstruct history;
- product-critical browser path backed by mock-only behavior;
- cross-tenant negative coverage missing for a new owner;
- new suppression/exception without governance;
- retry-to-green accepted as normal CI behavior;
- public protected payload written to logs/audit/search;
- Step 23/24 change cost grows materially with total module count;
- final evidence depends on one person’s local environment or manual interpretation.

---

## 19. Expected final architecture shape

The target conceptual structure is:

```text
Product plane
  feature routes / record workspaces
  typed governed clients
  shared accessible UI system

Delivery plane
  thin crm-api process root

Application runtime
  generic capability/query/worker/visibility machinery
  platform services
  first-party contribution aggregation

Owner modules
  pure domain + application contracts
  owner persistence/production boundaries
  no infrastructure clients in pure modules

Infrastructure
  PostgreSQL
  event delivery
  files/object storage
  search/projections
  provider integrations
  observability
```

Adding a new expert owner should primarily affect:

```text
new owner package set
its contracts
its migrations
its production contribution
its product feature
its focused tests
```

and should **not** require adding new business knowledge to generic runtime algorithms.

---

## 20. Final decision

The repository does not need a rewrite to reach 10/10. It needs disciplined completion of the architecture program it already started.

The highest-value remaining work is:

1. finish Step 22 dependency decisions and remove avoidable owner fan-in;
2. complete permanent-gate value/cost consolidation;
3. eliminate current-state documentation drift;
4. make the contribution model complete enough that new owners do not modify generic runtime business wiring;
5. prove bounded cost with Catalog/Pricing and then CPQ/orchestration;
6. generate a deterministic Step 25 evidence artifact and have an independent verifier reproduce it.

If those conditions are met while the existing tenant, ownership, privacy, contract and exact-head guarantees remain intact, an architecture 10/10 claim would be evidence-based rather than aspirational.
