# Second Module-Owned Production Contribution — Consents

Status: bounded comparison candidate

Consents is the second stable owner moved from concrete construction in the generic application runtime to an owner-built `ModuleContributionSet`.

## Why this owner contrasts with Customer Accounts

Customer Accounts validates Party references through one shared reader. Consents validates a richer scope across Party and optional Contact Point records, checks ownership and communication-channel compatibility, wraps the aggregate executor with owner-specific capability validation, and exposes permission-aware queries.

## Boundary

The existing `crm-consents-capability-composition` package owns mutation planner/executor construction, PostgreSQL reference reading, semantic validation, query adapter construction and activation gates. The generic application runtime supplies only production context and merges the contribution.

Customer Enrichment may still construct a `ConsentQueryAdapter` as an explicit cross-owner query dependency. The mechanical guard therefore forbids only central Consents route registration and owner mutation/reference construction, not every use of the query-adapter type.

## Decision boundary

This packet introduces no new crate and does not yet add a first-party aggregate package. After exact-head acceptance, Customer Accounts and Consents provide two contrasting examples from which the common production-contribution shape can be stabilized without inventing a framework from one owner.
