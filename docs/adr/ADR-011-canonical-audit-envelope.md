# ADR-011: Canonical audit envelope

- Status: Accepted
- Date: 2026-07-10

## Context

This decision closes an architecture boundary identified in Ultimate CRM v2.0–v2.2.

## Decision

Hash canonical persisted audit bytes in a per-tenant append-only chain. Never recompute from dynamically re-encoded business events.

## Consequences

- CI, contracts and runtime checks must enforce this decision.
- A change requires a superseding ADR and architecture review.
- Tests and operational evidence must be added as implementation progresses.
