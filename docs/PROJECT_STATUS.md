# Ultimate CRM — Project Status

Status date: 2026-07-15

This document is the concise human-readable status page. It is not the normative roadmap.

Authoritative references:

1. `SYSTEM_INVARIANTS.md` — absolute architecture rules.
2. `DELIVERY_GOVERNANCE.md` — source-of-truth, packet-state and synchronization policy.
3. `IMPLEMENTATION_ROADMAP.md` — normative phase sequence.
4. `PHASE8_DELIVERY_PLAN.md` — detailed active Phase 8 packet sequence.
5. `CRM_CAPABILITY_COVERAGE.md` — functional completeness guardrail.
6. `MODULE_CATALOG.md` — business-module ownership and readiness accounting.

## Current position

**Phases 0.1–7 are complete. Phase 8A is the active expert owner-domain program. Phase 8A.7 is merged and complete. Phase 8A.8 is the single active customer-master production packet.**

Current Phase 8A execution baseline:

- **8A.1 — Complete:** canonical Party, Account and Contact Point references plus owner foundations (#92 / PR #93).
- **8A.2a — Complete:** authoritative Party create/get (#94 / PR #95).
- **8A.2b — Complete:** optimistic Party update and permission-aware cursor listing (#96 / PR #97).
- **8A.2c — Complete:** permission-aware rebuildable Party search/customer discovery (#98 / PR #99).
- **8A.3a — Complete:** authoritative Account lifecycle and Party associations (#101 / PR #102).
- **8A.3b — Complete:** authoritative Contact Point lifecycle, verification and preference (#103 / PR #104).
- **8A.3c — Complete:** authoritative Party Relationship lifecycle and hierarchy foundations (#108 / PR #109).
- **8A.3d — Complete:** permission-aware rebuildable Customer 360 composition (#110 / PR #111).
- **8A.4 — Complete:** authoritative Consent and Communication Authorization (#112 / PR #113).
- **8A.5 — Complete:** explainable Identity Resolution duplicate candidates and reviewer decisions (#114 / PR #115).
- **8A.6 — Complete:** approval-required reversible merge/unmerge, immutable lineage, survivorship provenance and canonical Party resolution (#116 / PR #119; merge `d5cb4502ad0c49158e0789d8749dc09160da7895`).
- **8A.7 — Complete:** governed immutable source artifacts, server-side import parsing/validation, resumable Party import execution, retry recovery and crash/restart process proof (#120 / PR #121; merge `5f60f24d6d3a3bb46720658f4e98d4a7ebb15637`).
- **8A.8 — In progress:** Customer Export Jobs, Artifacts and Reconciliation Evidence (#123).
- **8A.9 — Planned:** Customer Data Quality Rules, Completeness and Stewardship (#124).
- **8A.10 — Planned:** Governed Customer Enrichment and Provenance (#125).
- **8A.11 — Planned:** Customer Privacy Lifecycle, Restriction, Deletion and Legal Hold (#126).

The active dependency lane is:

`8A.8 -> 8A.9 -> 8A.10 -> 8A.11 -> Phase 8A closure -> 8B`

A later packet may have architecture preparation, but it is not the active production merge target until its prerequisite is merged and it is verified against the accepted baseline.

## Active packet — Phase 8A.8

Issue #123 is the single active customer-master production packet.

The export packet must deliver a governed derived-data path rather than a generic database dump surface. Its required production boundary includes:

- immutable versioned export specification/profile identity;
- explicit bounded customer-master resource scope;
- deterministic snapshot or equivalent immutable selection evidence;
- live authorization, resource visibility and field/data-class filtering during execution;
- owner-domain reads through governed interfaces rather than cross-module table bypass;
- staged derived artifacts with exactly-once logical finalization;
- artifact digest, byte-size, retention and expiry metadata;
- deterministic retry/resume and crash/restart behavior;
- exact selected/emitted/excluded/redacted counts and reconciliation evidence;
- tenant isolation and safe non-disclosure;
- fresh-PostgreSQL real `crm-api` process acceptance;
- one unchanged exact candidate SHA with every applicable gate green.

Exported bytes are derived artifacts. They do not become authoritative customer-master state and they must not bypass consent, privacy restriction, legal hold, masking or authorization policy.

## Implemented platform and product foundations

The repository contains a production-composed modular CRM platform foundation with:

- executable architecture governance and strict system invariants;
- typed Module Manifest IR and immutable module identity;
- governed Module SDK and deterministic test harness;
- module publication, installation and lifecycle runtime;
- PostgreSQL tenant/RLS, record, relationship, idempotency, outbox, append-only audit and governed file/artifact foundations;
- authenticated mutation and permission-bound query gateways;
- independent Sales Deal and Activities Task production vertical slices;
- governed event delivery and an optional Sales–Activities link module;
- generalized rebuildable projections and permission-aware search;
- neutral cross-domain global-search composition;
- real application composition and deployable `services/crm-api` process acceptance;
- typed web product shell with governed generated browser clients and browser E2E;
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
- cycle-safe canonical Party resolution;
- governed immutable import source artifacts and server-side parser profiles;
- true import dry-run proof with zero Party-side effects;
- resumable Party import through the exact owner capability;
- target-success/checkpoint crash recovery without duplicate Party creation;
- durable retryable target-failure recovery on fresh PostgreSQL.

## Product completeness reality

The project is **not yet a complete universal CRM**.

`CRM_CAPABILITY_COVERAGE.md` remains the product-scope guardrail. Major required capability families still include:

- Phase 8A export, data quality, enrichment and privacy lifecycle;
- Product Catalog, Pricing, CPQ, Quotes, Orders, Contracts and Subscriptions;
- broader Sales and Activities expert expansion;
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

No broad “ultimate CRM complete” claim is valid while required capability families remain planned or only partially delivered.

## Engineering quality baseline

Current architecture and delivery standards require:

- one authoritative owner for every mutable aggregate;
- no alternate mutation path around governed capabilities;
- live authorization immediately before reads that expose protected data and before side effects;
- atomic state, idempotency, outbox and audit evidence;
- FORCE RLS and cross-tenant negative testing;
- immutable published versions and compatibility gates;
- rebuildable non-authoritative search/projections;
- exact-SHA final evidence for every merge candidate;
- synchronization of roadmap, status, module accounting, issues and PR descriptions when state changes.

See `DELIVERY_GOVERNANCE.md` for the normative control rules.

## Immediate next actions

1. Synchronize merged 8A.7 status across roadmap, Phase 8 plan, module catalog and issues.
2. Keep #123 as the single active Phase 8A production packet.
3. Freeze the 8A.8 ownership, snapshot, artifact and reconciliation contract before broad implementation.
4. Deliver 8A.8 in coherent vertical slices through contracts, domain, persistence, runtime composition and fresh-process acceptance.
5. Continue sequentially through #124, #125 and #126.
6. Close Phase 8A only after its full acceptance baseline is merged.
7. Begin Phase 8B / #29 from the stable completed customer-master baseline while enterprise/security/operational hardening continues continuously.
