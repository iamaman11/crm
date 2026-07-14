# Ultimate CRM — Project Status

Status date: 2026-07-14

This document is the concise human-readable status page. It is not the normative roadmap.

Authoritative references:

1. `SYSTEM_INVARIANTS.md` — absolute architecture rules.
2. `DELIVERY_GOVERNANCE.md` — source-of-truth, packet-state and synchronization policy.
3. `IMPLEMENTATION_ROADMAP.md` — normative phase sequence.
4. `PHASE8_DELIVERY_PLAN.md` — detailed active Phase 8 packet sequence.
5. `CRM_CAPABILITY_COVERAGE.md` — functional completeness guardrail.
6. `MODULE_CATALOG.md` — module ownership and readiness accounting.

## Current position

**Phases 0.1–7 are complete. Phase 8A is the active expert owner-domain program. Phase 8A.6 is merged and complete. Phase 8A.7 is the single active customer-master production packet.**

Current execution baseline:

- **8A.1 — Complete:** canonical Party, Account and Contact Point references plus owner foundations (#92 / merged PR #93).
- **8A.2a — Complete:** authoritative Party create/get (#94 / merged PR #95).
- **8A.2b — Complete:** optimistic Party update and permission-aware cursor listing (#96 / merged PR #97).
- **8A.2c — Complete:** permission-aware rebuildable Party search/customer discovery (#98 / merged PR #99).
- **8A.3a — Complete:** authoritative Account lifecycle and Party associations (#101 / merged PR #102).
- **8A.3b — Complete:** authoritative Contact Point lifecycle, verification and preference (#103 / merged PR #104).
- **8A.3c — Complete:** authoritative Party Relationship lifecycle and hierarchy foundations (#108 / merged PR #109).
- **8A.3d — Complete:** permission-aware rebuildable Customer 360 composition (#110 / merged PR #111).
- **8A.4 — Complete:** authoritative Consent and Communication Authorization (#112 / merged PR #113).
- **8A.5 — Complete:** Identity Resolution duplicate-candidate cases and reviewer decisions (#114 / merged PR #115).
- **8A.6 — Complete:** approval-required reversible merge/unmerge, immutable lineage, survivorship provenance and canonical Party resolution (#116 / merged PR #119; merge commit `d5cb4502ad0c49158e0789d8749dc09160da7895`).
- **8A.7 — In progress:** Customer Import Jobs, Versioned Mappings and Resumable Execution (#120 / draft PR #121).
- **8A.8 — Planned:** Customer Export Jobs, Artifacts and Reconciliation Evidence (#123).
- **8A.9 — Planned:** Customer Data Quality Rules, Completeness and Stewardship (#124).
- **8A.10 — Planned:** Governed Customer Enrichment and Provenance (#125).
- **8A.11 — Planned:** Customer Privacy Lifecycle, Restriction, Deletion and Legal Hold (#126).

The active dependency lane is therefore:

`8A.7 -> 8A.8 -> 8A.9 -> 8A.10 -> 8A.11 -> 8B`

A later packet may have architecture preparation, but it is not the active production merge target until its prerequisite is merged and it is verified against the accepted baseline.

## Active packet — Phase 8A.7

Issue #120 and draft PR #121 implement the first governed customer-data operations vertical slice.

Already present on the active branch:

- normative customer-import architecture;
- `crm.customer-data-operations` module foundation;
- import-job and import-row domain models;
- immutable source-content and mapping identity;
- deterministic row identity and target idempotency derivation;
- explicit partial-execution policy;
- resumable checkpoint/lifecycle semantics;
- strict deterministic versioned private persistence.

Still required before 8A.7 can claim a production vertical slice:

- public versioned Protobuf mutation/query/event contracts;
- governed capability and query adapters;
- application-runtime composition and exact owner-capability invocation;
- PostgreSQL persistence, tenant isolation and migration implementation;
- dry-run proof with zero target Party mutation side effects;
- retry/restart resume without duplicate Party creation;
- permission-aware get/list/row-outcome queries with signed cursors;
- fresh-PostgreSQL real `crm-api` process acceptance;
- one unchanged exact candidate SHA with all applicable gates green.

The active PR remains draft until those layers are complete.

## Implemented platform and product foundations

The repository contains a production-composed modular CRM platform foundation with:

- executable architecture governance and strict system invariants;
- typed Module Manifest IR and immutable module identity;
- governed Module SDK and deterministic test harness;
- module publication, installation and lifecycle runtime;
- PostgreSQL tenant/RLS, record, relationship, idempotency, outbox and append-only audit foundations;
- authenticated mutation and permission-bound query gateways;
- independent Sales Deal and Activities Task production vertical slices;
- governed event delivery and an optional Sales–Activities link module;
- generalized rebuildable projections and permission-aware search;
- a neutral cross-domain global-search composition;
- a real application composition boundary and deployable `services/crm-api` process host;
- a typed web product shell with governed generated browser clients and browser E2E;
- immutable tenant-authorized metadata publication and rollback;
- Admin Studio publish/impact/activate/rollback workflow;
- typed trusted-code UI-extension runtime with failure isolation.

## Implemented customer-master foundations

Merged `main` now includes production paths for:

- canonical Party identity create/update/get/list/search;
- Customer Account lifecycle and typed Party associations;
- Contact Point lifecycle, normalization, verification and preference;
- typed Party Relationships and rebuildable hierarchy projection;
- permission-aware rebuildable Customer 360 composition;
- Consent and Communication Authorization with immediate authoritative decision semantics;
- explainable duplicate-candidate cases and reviewer decisions;
- approval-required non-destructive Party merge/unmerge;
- immutable merge lineage and field-level survivorship provenance;
- cycle-safe canonical Party resolution without destructive Party deletion or mandatory historical-reference rewrites.

## Product completeness reality

The project is **not yet a complete universal CRM**.

`CRM_CAPABILITY_COVERAGE.md` remains the product-scope guardrail. Major required capability families still include:

- full Sales and Activities expert expansion;
- Product Catalog, Pricing, CPQ, Quotes, Orders, Contracts and Subscriptions;
- communications and omnichannel;
- Service/Support, Knowledge and optional Field Service;
- Marketing automation and attribution;
- Customer Success and optional PRM;
- projects/configurable work, documents and e-signature;
- analytics, reporting and performance management;
- workflow, approvals, human tasks and collaboration;
- broader data/integration platform capabilities;
- responsive/mobile/offline/accessibility/localization completeness;
- AI-native governed actor/tool layer;
- signed marketplace and sandboxed untrusted extensions;
- enterprise identity, encryption, privacy lifecycle, backup/restore, residency, security and operational proof.

No broad “ultimate CRM complete” claim is valid while required capability families remain only planned or platform-ready.

## Engineering quality baseline

Current architecture and delivery standards require:

- one authoritative owner for every mutable aggregate;
- no alternate mutation path around governed capabilities;
- live authorization immediately before side effects;
- atomic state, idempotency, outbox and audit evidence;
- FORCE RLS and cross-tenant negative testing;
- immutable published versions and compatibility gates;
- rebuildable non-authoritative search/projections;
- exact-SHA final evidence for every merge candidate;
- synchronization of roadmap, status, module accounting, issues and PR descriptions when state changes.

See `DELIVERY_GOVERNANCE.md` for the normative control rules.

## Immediate next actions

1. Merge/adopt the documentation-governance synchronization packet (#122).
2. Close superseded PR #118 so it is no longer an active Phase 8A.6 path.
3. Keep PR #121 as the single active 8A customer-master implementation packet and keep its description synchronized with actual scope.
4. Complete the missing production layers and exact-head gates for 8A.7.
5. Continue sequentially through #123, #124, #125 and #126.
6. Begin Phase 8B / #29 only from a stable customer-master baseline, while enterprise/security/operational hardening continues continuously.