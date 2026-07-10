# ADR-023: Marketplace review ownership

- Status: Accepted
- Date: 2026-07-10

## Context

This decision closes an architecture boundary identified in Ultimate CRM v2.0–v2.2.

## Decision

Platform Security owns security approval; Architecture owns contract compatibility; Product owns commercial approval. All three are required for publication.

## Consequences

- CI, contracts and runtime checks must enforce this decision.
- A change requires a superseding ADR and architecture review.
- Tests and operational evidence must be added as implementation progresses.
