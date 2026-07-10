# ADR-009: AI as Actor

- Status: Accepted
- Date: 2026-07-10

## Context

This decision closes an architecture boundary identified in Ultimate CRM v2.0–v2.2.

## Decision

AI has an ActorIdentity and never receives direct DB/filesystem/provider credentials. It uses allowlisted capabilities with permission-filtered context.

## Consequences

- CI, contracts and runtime checks must enforce this decision.
- A change requires a superseding ADR and architecture review.
- Tests and operational evidence must be added as implementation progresses.
