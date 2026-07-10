# ADR-029: Authenticated Capability Ingress

- Status: Accepted
- Date: 2026-07-11

## Context

The capability gateway accepts a complete typed `CapabilityRequest`; it intentionally does not trust or interpret transport credentials and headers. Before public mutation endpoints exist, HTTP and gRPC must share one authenticated boundary that resolves tenant and actor identity, constructs every `ExecutionContext` field, computes the semantic input hash, applies a bounded timeout and maps failures without exposing internal policy or dependency details.

## Decision

The platform introduces `crm-capability-ingress`. Both HTTP and gRPC middleware delegate to one `CapabilityIngress`, which owns only three collaborators: a request authenticator, an execution-context resolver and the capability gateway. Transport code has no executor, database or business-module mutation handle.

### Authentication and tenant resolution

Bearer tokens are stored only as SHA-256 digests and must meet a minimum high-entropy length. A token grant binds one actor to an explicit non-empty set of tenants and an expiry. Revocation is checked on every authentication call. The tenant supplied by request metadata is parsed as a typed identifier and must belong to the authenticated principal.

### Execution context

The resolver constructs all required fields:

- tenant and actor from authenticated state;
- request, correlation, causation and trace IDs from validated metadata or controlled randomness;
- exact capability ID and version from the route binding;
- caller-supplied idempotency key;
- business transaction ID from validated metadata or controlled randomness;
- schema version from the route binding;
- request start time from the controlled clock.

Correlation and causation default to the resolved request ID. Generated values use the injected `RandomSource`; process-global randomness and wall-clock reads are forbidden.

The semantic input hash uses a versioned length-prefixed SHA-256 profile over payload owner, schema identity, descriptor hash, data class, encoding, retention policy, declared maximum size and payload bytes. The resulting hash is the value later bound to approval and PostgreSQL idempotency evidence.

### Timeout budget

The server defines a positive default and maximum timeout. A caller may request a smaller positive duration. The timeout wraps the complete gateway future from outside; it does not insert any awaited work between live authorization and the transactional executor.

### Transport mapping

HTTP and gRPC expose the same stable safe error code, category, retryability and optional retry interval. HTTP maps categories to status codes and standard retry metadata. gRPC maps categories to canonical codes and places the stable error code in response metadata. Internal references, authorization reason codes, SQL details and credentials are never returned.

## Consequences

- authentication and tenant isolation are uniform across HTTP and gRPC;
- rejected credentials, cross-tenant requests and invalid metadata cannot reach the gateway executor;
- trace, correlation and causation identity is complete before capability resolution;
- transport handlers cannot bypass the gateway by construction;
- timeout cancellation surrounds the transaction boundary rather than weakening authorization ordering;
- PostgreSQL end-to-end tests and static no-bypass enforcement remain required before issue #8 can close.
