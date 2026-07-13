# Phase 8 Delivery Plan

Status: **Active planning baseline after Phase 7 closure**

Parent program: #11
First owner-domain program: #28
Commercial follow-on: #29

## Goal

Build the broad expert CRM domain layer on top of the completed governed platform foundations without collapsing ownership into Sales, without a giant long-lived Phase 8 branch and without weakening compatibility, tenant, authorization, audit or rollback guarantees.

## Delivery model

Phase 8 is planned as one coherent architecture program and delivered as multiple mergeable packets. Each packet must establish a stable boundary that downstream work can safely consume.

Do not defer all Phase 8 merging until the end. A giant branch would make exact-SHA evidence, rollback, bisectability, code review and ownership discipline materially weaker.

## Wave 8A — canonical customer master, identity and consent

### 8A.1 — identity/reference contracts and owner skeletons

Deliver:

- canonical typed resource identifiers/references for Party, Account and Contact Point;
- exact versioned Protobuf capability/query/event contract families for the first public boundaries;
- owner-module manifests and dependency boundaries;
- explicit prohibition on Sales/Service/Marketing/Billing defining competing customer identity owners;
- generated Rust/browser contract synchronization and compatibility gates.

### 8A.2 — Party vertical slice

Deliver Person and Organization owner aggregates with governed create/update/get/list/search paths, tenant isolation, optimistic concurrency, audit evidence and process-level acceptance.

### 8A.3 — Account, Contact Point and Party Relationship

Deliver customer/commercial relationship ownership, verified/preferred contact points, time-bounded typed relationships and hierarchy foundations.

### 8A.4 — Consent and communication authorization

Deliver purpose/channel/jurisdiction/legal-basis/source/proof/effective/expiry/withdrawal semantics and an exact authorization decision boundary that downstream communication modules must use.

### 8A.5 — identity resolution and duplicate candidates

Deliver deterministic candidate generation first, explainable evidence, review state and governed approval boundaries. Probabilistic/AI suggestions may enrich candidates later but cannot bypass governed merge approval.

### 8A.6 — merge, unmerge, provenance and survivorship

Deliver immutable lineage, source evidence preservation, reference redirection, field-level provenance and reversible merge history.

### 8A.7 — import/export and privacy lifecycle proof

Deliver versioned mapping, dry-run validation, resumable idempotent imports, export, deletion/legal-hold interaction evidence and end-to-end acceptance.

## Wave 8B — product catalog and quote-to-revenue

Begin only against stable merged 8A customer reference contracts.

Planned packets:

1. Product/Catalog ownership and versioning;
2. Price Book and governed pricing semantics;
3. CPQ/configuration and quote revision lifecycle;
4. Order and commercial commitment handoff;
5. Contract and Subscription lifecycle;
6. billing/ERP/payment/tax integration boundaries.

## Parallel later domain waves

After stable shared references exist, non-overlapping work may proceed in parallel for:

- communications and omnichannel history;
- support/service management;
- marketing segmentation, journeys and attribution;
- projects/cases/configurable work management;
- documents and e-signature;
- analytics, forecasting and performance management.

## Merge rule

A packet is merged when its natural architecture boundary is complete and all applicable exact-head gates are green. Later packets build from merged stable contracts rather than from a single accumulating Phase 8 mega-branch.
