# Ultimate CRM — Project Status

Status date: 2026-07-22

This is the concise human-readable status page. Normative delivery order remains in `IMPLEMENTATION_ROADMAP.md` and `PHASE8_DELIVERY_PLAN.md`.

Authoritative references:

1. `SYSTEM_INVARIANTS.md` — absolute architecture rules.
2. `ARCHITECTURE_READINESS.md` — accepted native-composition baseline.
3. `DELIVERY_GOVERNANCE.md` — packet-state and synchronization policy.
4. `IMPLEMENTATION_ROADMAP.md` — normative phase sequence.
5. `PHASE8_DELIVERY_PLAN.md` — detailed Phase 8 packet sequence.
6. `CRM_CAPABILITY_COVERAGE.md` — functional completeness guardrail.
7. `MODULE_CATALOG.md` — merged business-module readiness accounting.

## Current position

**Phases 0.1–7 are complete. Phase 8A is the active expert owner-domain program. Phase 8A.10 is merged and Complete. Phase 8A.11 is In progress through the ownership/enforcement architecture freeze in draft PR #140.**

Current Phase 8A baseline:

- **8A.1–8A.6 — Complete:** customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution.
- **8A.7 — Complete:** governed customer import and resumable execution (#120 / PR #121).
- **8A.8 — Complete:** governed customer export, artifacts and reconciliation (#123 / PR #130).
- **8A.9 — Complete:** Customer Data Quality Rules, Completeness and Stewardship (#124 / PR #132).
- **8A.10 — Complete:** Governed Customer Enrichment and Provenance (#125 / PR #137).
- **8A.11 — In progress:** Customer Privacy Lifecycle architecture freeze (#126 / draft PR #140); runtime behavior is not implemented yet.

The active dependency lane is:

`8A.11 architecture freeze -> 8A.11 implementation -> Phase 8A closure -> 8B`

## Phase 8A.10 accepted result

Phase 8A.10 was accepted on unchanged user-authored SHA `f92d101206886e3ceaf94d0e56e52580cec21093`, which passed all 17 permanent workflows, and squash-merged through PR #137 as `150e44b95d9dbdc08c1792563de03ec73f34aed1`.

The frozen production inventory is exactly:

- **6 public mutations**;
- **6 permission-aware queries**;
- **5 activation-gated worker-only coordinates** with no public HTTP/gRPC ingress.

All 17 manifest-bound Customer Enrichment coordinates are now classified as public runtime or worker runtime. Provider dispatch/response run in phase 240, materialization in phase 245 and owner application/outcome recovery in phase 250.

Accepted behavior includes:

- immutable provider-profile, mapping, request, response, conflict, suggestion, review, usage and application evidence;
- exact registry HTTP transport outside the pure core, tenant-bound secret resolution, endpoint allowlisting, bounded network behavior, quota and circuit control;
- independent live authorization for dispatch, response, materialization and owner application;
- commit-before-I/O, exact/semantic duplicate reconciliation, fail-closed response conflicts and deterministic crash recovery;
- reviewed Party display-name application only through `parties.party.update@1.0.0`;
- permission-aware reads, declarative field redaction, tenant concealment and durable activation shutdown;
- transaction-scoped provider-profile and exact Party-version reference guards;
- FORCE RLS and migration rollback/reapply proof;
- permanent real-`crm-api` and fresh-PostgreSQL provider/materialization/review/application process evidence;
- bounded safe HTTP/gRPC errors with no credential, raw provider payload or internal diagnostic leakage.

## Merged platform and customer-master baseline

Merged `main` contains:

- executable architecture governance and strict system invariants;
- typed Module Manifest IR, Module SDK, registry and durable installation lifecycle;
- PostgreSQL tenant/RLS, records, relationships, idempotency, outbox, audit and governed artifact foundations;
- authenticated mutation and permission-bound query gateways;
- native module-owned exact-coordinate composition and deployable `crm-api` process acceptance;
- durable activation-gated workers, event delivery, projections and permission-aware search;
- Party, Account, Contact Point, Party Relationship, Customer 360, Consent, reversible Identity Resolution, import, export, Data Quality and Customer Enrichment production slices.

## Active packet: Phase 8A.11

Issue #126 is In progress because draft PR #140 now contains the mandatory pre-contract ownership and guardrail freeze.

The frozen coordinator is `crm.customer-privacy`. It owns privacy cases, verified subject binding, scope snapshots, restrictions, customer-data legal holds, retention decisions, deterministic owner plans, attempts/outcomes, checkpoints, export references and convergence evidence. Existing modules retain all authoritative customer values.

The initial architecture inventory is exactly:

- **9 public mutations**;
- **7 permission-aware public queries**;
- **9 trusted worker/internal coordinates** in phases 260 → 270 → 280 → 290;
- **1 reasoned non-runtime crypto-shredding coordinate**.

Critical frozen invariants include a shared tenant + canonical Party subject lock, fail-closed live restriction, Consent as an independent authoritative deny source, Customer Data Operations artifact reuse, owner-specific deterministic actions, legal-hold/retention precedence, non-reusable erased Party tombstones and preservation of required immutable evidence.

No Protobuf, manifest, migration, production route or runtime behavior is claimed by the architecture-freeze PR.

## Product completeness reality

The project is **not yet a complete universal CRM**. Major required families still include the implemented Phase 8A.11 runtime, Product Catalog/Pricing/CPQ/Quotes/Orders/Contracts/Subscriptions, broader Sales and Activities, omnichannel, Service, Marketing, Customer Success, projects, documents/e-signature, analytics, workflow/collaboration, AI governance, marketplace and enterprise operational proof.

No broad “ultimate CRM complete” claim is valid while those domains remain planned or partial.

## Immediate next actions

1. Validate and accept the Phase 8A.11 architecture freeze in draft PR #140 on one unchanged exact SHA.
2. Scaffold `crm.customer-privacy` and publish immutable case/restriction/legal-hold contracts only after that freeze is accepted.
3. Implement FORCE RLS persistence, shared-lock live enforcement, owner contributions, governed privacy export, retention/legal-hold planning and crash recovery.
4. Close Phase 8A only after the full customer-master privacy interaction baseline is merged and reconciled.
5. Begin Phase 8B / #29 only after Phase 8A closure.
