# ADR-008: WASM Component Model marketplace

- Status: Accepted
- Date: 2026-07-10

## Context

This decision closes an architecture boundary identified in Ultimate CRM v2.0–v2.2.

## Decision

Run third-party modules in a capability-scoped WASM sandbox with signed manifests, resource limits, egress allowlists and lifecycle governance.

## Consequences

- CI, contracts and runtime checks must enforce this decision.
- A change requires a superseding ADR and architecture review.
- Tests and operational evidence must be added as implementation progresses.
