# ADR-013: Metadata lifecycle and publish gates

- Status: Accepted
- Date: 2026-07-10

## Context

This decision closes an architecture boundary identified in Ultimate CRM v2.0–v2.2.

## Decision

Metadata changes are immutable versions passing compatibility, dependency impact, migration and rollback gates.

## Consequences

- CI, contracts and runtime checks must enforce this decision.
- A change requires a superseding ADR and architecture review.
- Tests and operational evidence must be added as implementation progresses.
