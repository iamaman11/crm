# CRM Customer Privacy

`crm.customer-privacy` is the authoritative owner for customer privacy cases, current processing restrictions, customer-data legal holds, immutable retention decisions, owner-aware action plans, deterministic owner attempts/outcomes and convergence evidence.

This foundation is intentionally not a production feature. It publishes no capability, event, public route or worker coordinate. Runtime promotion is blocked until the Phase 8A.11 architecture freeze, compatible contracts, FORCE RLS persistence, live subject-lock enforcement and real-process acceptance are complete.

The module never owns or directly mutates Party, Account, Contact Point, Relationship, Consent, Identity Resolution, import/export, Data Quality or Customer Enrichment values. Cross-owner work must use exact governed capabilities through separately owned application adapters and composition.

Direct PostgreSQL, broker, arbitrary HTTP, secret-store, scheduler and cross-module internal dependencies are forbidden in this pure core.
