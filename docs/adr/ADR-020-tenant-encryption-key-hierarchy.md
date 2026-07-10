# ADR-020: Tenant encryption key hierarchy

- Status: Accepted
- Date: 2026-07-10

## Context

This decision closes an architecture boundary identified in Ultimate CRM v2.0–v2.2.

## Decision

Use per-tenant key-encryption keys in managed KMS and per-record/file data-encryption keys, with rotation, versioning and crypto-shredding.

## Consequences

- CI, contracts and runtime checks must enforce this decision.
- A change requires a superseding ADR and architecture review.
- Tests and operational evidence must be added as implementation progresses.
