# ADR-002: Protobuf as internal contract source of truth

- Status: Accepted
- Date: 2026-07-10

## Context

This decision closes an architecture boundary identified in Ultimate CRM v2.0–v2.2.

## Decision

Use versioned Protobuf descriptors for internal service, event, AI-tool and marketplace contracts. JSON Schema is generated only at external boundaries.

## Consequences

- CI, contracts and runtime checks must enforce this decision.
- A change requires a superseding ADR and architecture review.
- Tests and operational evidence must be added as implementation progresses.
