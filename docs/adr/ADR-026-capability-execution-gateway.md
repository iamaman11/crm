# ADR-026: Capability Execution Gateway

- Status: Accepted
- Date: 2026-07-10

## Context

Every state-changing action in Ultimate CRM must pass through one uniform boundary regardless of whether the caller is a user, workflow, integration, AI actor or another module. Direct invocation of module internals would bypass version resolution, contract validation, policy evaluation, approval binding, idempotency and audit guarantees.

## Decision

The platform provides one infrastructure-neutral Capability Execution Gateway with the following ordered stages:

1. validate the complete execution context and typed payload envelope;
2. resolve the exact capability ID and version;
3. verify definition ownership and exact input contract metadata;
4. run semantic input validation;
5. enforce the declared rate-limit policy;
6. bind and cryptographically verify approval evidence when required;
7. perform live authorization;
8. immediately invoke one transactional executor.

Live authorization is the final awaited decision before the transactional side-effect boundary. No network access, validation or other awaited work may be inserted between authorization and execution.

The transactional executor owns idempotency, mutation, output validation before commit, outbox evidence and audit evidence. A retry of an identical semantic request returns the persisted original result without repeating side effects. Reusing an idempotency key with a different semantic request hash is rejected.

The gateway depends only on object-safe ports:

- capability registry;
- semantic validator;
- rate limiter;
- approval verifier;
- live authorizer;
- transactional executor;
- controlled clock.

Transport authentication, HTTP/gRPC middleware and PostgreSQL adapters bind to these ports in later change sets. Business modules cannot import the gateway or infrastructure implementations directly; they continue to use the governed Module SDK.

## Error contract

Gateway errors are typed and mapped to stable SDK error codes. Public errors contain safe messages. Policy decision IDs and dependency diagnostics may appear only as internal references and must never require clients to parse human-readable text.

## Consequences

- all action paths share one deterministic control flow;
- independent teams can implement policy, rate limiting and execution adapters behind stable ports;
- authorization ordering is mechanically testable;
- no rejected request can call the transactional executor;
- transport and persistence can evolve without changing business modules;
- the executor must prove output-contract validation and atomic rollback before the gateway is exposed through public APIs.
