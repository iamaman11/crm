# ADR-003: Record storage is not JSONB-primary

- Status: Accepted
- Date: 2026-07-10

## Context

This decision closes an architecture boundary identified in Ultimate CRM v2.0–v2.2.

## Decision

Store immutable Protobuf payloads plus typed shadow/index tables. JSONB may be used for non-authoritative auxiliary configuration only.

## Consequences

- CI, contracts and runtime checks must enforce this decision.
- A change requires a superseding ADR and architecture review.
- Tests and operational evidence must be added as implementation progresses.
