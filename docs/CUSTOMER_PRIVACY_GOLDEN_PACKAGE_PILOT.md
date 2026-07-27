# Customer Privacy Golden Package Pilot

Status: **Stage C behavior-neutral candidate**  
Product tracking: #126  
Architecture tracking: #194  
Base: `33186bab67932d5e878019fc7e59181e123bbf67`

## Objective

Establish the stable Customer Privacy owner package model before production scope discovery:

```text
modules/crm-customer-privacy/
crates/crm-customer-privacy-application/
crates/crm-customer-privacy-postgres/
crates/crm-customer-privacy-production/
```

The packet moves no feature behavior. Existing capability-specific command, query, persistence and PostgreSQL composition crates remain transitional implementation details behind the new stable boundaries.

## Stable ownership

- `crm-customer-privacy` remains the pure authoritative domain owner.
- `crm-customer-privacy-application` owns the exact application inventory: four mutations and two queries.
- `crm-customer-privacy-postgres` owns the stable persistence and transaction-guard boundary.
- `crm-customer-privacy-production` owns the single `build_contribution` process-composition entry point.
- `crm-application-runtime` imports the production package for all Customer Privacy execution/composition and retains one narrow query-adapter dependency only for existing bootstrap visibility metadata.

Production discovery remains unregistered. The accepted discovery coordinate is still non-runtime and no worker is added.

## Preserved behavior

The pilot must preserve:

- `customer_privacy.case.create@1.0.0`;
- `customer_privacy.case.submit@1.0.0`;
- `customer_privacy.case.subject.verify@1.0.0`;
- `customer_privacy.case.cancel@1.0.0`;
- `customer_privacy.case.get@1.0.0`;
- `customer_privacy.case.list@1.0.0`;
- four public mutations, two permission-aware queries and zero workers;
- activation, live authorization/visibility, RLS, audit, idempotency, rollback/reapply and real-process behavior;
- all published contracts, route classifications and migration ownership.

## Structural measurements

| Metric | Before | Candidate |
|---|---:|---:|
| Effective workspace packages | 110 | 113 |
| Customer Privacy stable target technical packages | 0 | 3 |
| Generic runtime direct Customer Privacy execution/composition dependencies | 5 | 0 |
| Generic runtime Customer Privacy visibility-metadata dependencies | 1 | 1 |
| Generic runtime Customer Privacy owner-production dependencies | 0 | 1 |
| Public Customer Privacy mutations | 4 | 4 |
| Public Customer Privacy queries | 2 | 2 |
| Customer Privacy workers | 0 | 0 |
| Cargo manifest dependency versions changed | 0 | 0 |
| Database migrations changed | 0 | 0 |

The temporary package-count increase is explicit and governed. It creates the permanent application/PostgreSQL/production boundaries while transitional crates remain until later behavior-neutral consolidation. Ordinary future Customer Privacy capabilities must add zero crates and must enter these packages.

## Dependency direction

```text
crm-customer-privacy
    <- crm-customer-privacy-application
    <- crm-customer-privacy-postgres
    <- crm-customer-privacy-production
    <- crm-application-runtime
```

The transitional command/query implementations are application-private dependencies. The transitional persistence and SQL transaction-guard implementations are PostgreSQL-private dependencies. The generic runtime no longer imports Customer Privacy planners, executors or PostgreSQL composition directly. Its remaining query-adapter edge supplies only the pre-existing bootstrap visibility metadata and is a later Stage D calibration target.

## Acceptance boundary

Required exact-head evidence:

- Cargo lockfile regeneration and equality;
- Rust formatting, Clippy and all workspace tests;
- architecture/new-crate/no-growth governance;
- unchanged application route and query inventories;
- unchanged Customer Privacy PostgreSQL, rollback/reapply and real-process acceptance;
- unchanged all-nine owner privacy-scope acceptance;
- package/dependency/fan-out/public-surface before/after report;
- no discovery worker, route, migration or generic-runtime business switch.

Only after this packet is accepted may production discovery and immutable snapshot persistence be implemented inside the stable target packages.
