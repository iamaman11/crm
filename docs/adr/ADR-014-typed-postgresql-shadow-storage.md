# ADR-014: Typed PostgreSQL shadow storage

- Status: Accepted
- Date: 2026-07-10

## Context

This decision closes an architecture boundary identified in Ultimate CRM v2.0–v2.2.

## Decision

Keep payload bytes authoritative and maintain typed `record_index_values` for hot/custom indexed fields; do not query arbitrary protobuf payloads in SQL.

## Consequences

- CI, contracts and runtime checks must enforce this decision.
- A change requires a superseding ADR and architecture review.
- Tests and operational evidence must be added as implementation progresses.
