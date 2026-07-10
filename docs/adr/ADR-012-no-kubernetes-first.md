# ADR-012: No Kubernetes-first

- Status: Accepted
- Date: 2026-07-10

## Context

This decision closes an architecture boundary identified in Ultimate CRM v2.0–v2.2.

## Decision

Start with containers and simple orchestration. Adopt Kubernetes only after operational or tenant-isolation evidence justifies the complexity.

## Consequences

- CI, contracts and runtime checks must enforce this decision.
- A change requires a superseding ADR and architecture review.
- Tests and operational evidence must be added as implementation progresses.
