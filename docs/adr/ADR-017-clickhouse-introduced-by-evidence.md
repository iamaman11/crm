# ADR-017: ClickHouse introduced by evidence

- Status: Accepted
- Date: 2026-07-10

## Context

This decision closes an architecture boundary identified in Ultimate CRM v2.0–v2.2.

## Decision

Use PostgreSQL projections initially; introduce ClickHouse when benchmarked analytical workloads exceed agreed thresholds.

## Consequences

- CI, contracts and runtime checks must enforce this decision.
- A change requires a superseding ADR and architecture review.
- Tests and operational evidence must be added as implementation progresses.
