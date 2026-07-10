# ADR-016: Constrained Postgres durable workflow engine

- Status: Accepted
- Date: 2026-07-10

## Context

This decision closes an architecture boundary identified in Ultimate CRM v2.0–v2.2.

## Decision

Implement CRM-native durable workflows in PostgreSQL behind WorkflowRuntimePort; do not build a general-purpose Temporal clone.

## Consequences

- CI, contracts and runtime checks must enforce this decision.
- A change requires a superseding ADR and architecture review.
- Tests and operational evidence must be added as implementation progresses.
