# ADR-006: NATS JetStream event transport

- Status: Accepted
- Date: 2026-07-10

## Context

This decision closes an architecture boundary identified in Ultimate CRM v2.0–v2.2.

## Decision

Use NATS JetStream as transport, while PostgreSQL state and outbox remain authoritative. Consumers are idempotent and replayable.

## Consequences

- CI, contracts and runtime checks must enforce this decision.
- A change requires a superseding ADR and architecture review.
- Tests and operational evidence must be added as implementation progresses.
