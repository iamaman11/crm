# ADR-021: Provider-independent WORM interface

- Status: Accepted
- Date: 2026-07-10

## Context

This decision closes an architecture boundary identified in Ultimate CRM v2.0–v2.2.

## Decision

Write audit checkpoints through a WORM port supporting retention lock/object lock; provider is deployment configuration, not domain code.

## Consequences

- CI, contracts and runtime checks must enforce this decision.
- A change requires a superseding ADR and architecture review.
- Tests and operational evidence must be added as implementation progresses.
