# ADR-001: Modular monolith first

- Status: Accepted
- Date: 2026-07-10

## Context

This decision closes an architecture boundary identified in Ultimate CRM v2.0–v2.2.

## Decision

Use a modular monolith for Phase 0–4 with enforceable crate, contract, data and ownership boundaries. Split services only for proven scaling, isolation or compliance needs.

## Consequences

- CI, contracts and runtime checks must enforce this decision.
- A change requires a superseding ADR and architecture review.
- Tests and operational evidence must be added as implementation progresses.
