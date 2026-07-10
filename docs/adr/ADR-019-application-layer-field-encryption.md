# ADR-019: Application-layer field encryption

- Status: Accepted
- Date: 2026-07-10

## Context

This decision closes an architecture boundary identified in Ultimate CRM v2.0–v2.2.

## Decision

Encrypt sensitive fields with envelope encryption before persistence. Metadata identifies encryption and masking policy; ciphertext is not searchable by default.

## Consequences

- CI, contracts and runtime checks must enforce this decision.
- A change requires a superseding ADR and architecture review.
- Tests and operational evidence must be added as implementation progresses.
