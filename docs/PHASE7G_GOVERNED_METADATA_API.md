# Phase 7G — Governed Metadata API and Application Composition

Issue: [#85](https://github.com/iamaman11/crm/issues/85)  
Parent phase: [#10](https://github.com/iamaman11/crm/issues/10)

## 1. Purpose

Phase 7G exposes the durable metadata lifecycle through the same governed mutation and query boundaries used by production CRM behavior. It does not add a generic metadata transport, raw persistence API or hash-based authorization shortcut.

The target path is:

```text
authenticated application request
→ exact versioned metadata capability/query
→ typed Protobuf contract validation
→ typed Admin Studio schema validation
→ live authorization
→ CapabilityGateway or QueryGateway
→ PostgresMetadataStore
→ canonical typed response
```

State-changing operations must also produce canonical global audit evidence through the normal capability transaction contract while preserving the append-only metadata transition evidence introduced in Phase 7F.

## 2. Public coordinates

### Mutation capabilities

- `metadata.bundle.publish@1.0.0`
- `metadata.revision.activate@1.0.0`
- `metadata.revision.rollback@1.0.0`

### Read-only queries

- `metadata.bundle.impact@1.0.0`
- `metadata.revision.get@1.0.0`
- `metadata.activation.get@1.0.0`

The exact owner module identifier and schema identifiers must be defined once in the authoritative contract/catalog layer and reused mechanically by Rust and browser clients.

## 3. Contract rules

Public publish input must consist of typed metadata definitions supported by `crm-metadata-schema`. Public callers must not submit arbitrary pre-canonicalized metadata documents.

The adapter sequence is:

```text
versioned Protobuf authoring definitions
→ strict typed schema values
→ semantic validation
→ canonical metadata documents
→ complete dependency-checked bundle
→ immutable revision publication
```

Activation input must include:

- target revision identity;
- expected activation generation;
- explicit breaking-change confirmation;
- request and transaction context supplied by the governed ingress.

Rollback input must include the expected activation generation and must preserve the durable pop-only rollback semantics.

## 4. Mutation boundary

All public metadata mutations enter through `CapabilityGateway` and therefore require:

1. authentication;
2. tenant and actor resolution;
3. exact capability/version resolution;
4. typed and semantic input validation;
5. live authorization immediately before execution;
6. idempotency and business-transaction binding;
7. one atomic PostgreSQL transaction;
8. canonical audit evidence;
9. typed safe response mapping.

The implementation must not insert fake outbox events merely to satisfy the business-transaction evidence chain. Metadata mutations may emit no integration event unless a real published event contract and consumer need exist.

## 5. Query boundary

Read-only metadata operations enter through `QueryGateway` and must not require mutation-only idempotency or business-transaction metadata.

Queries must:

- enforce tenant scope before revision lookup;
- treat revision hashes as identities only;
- return non-disclosing not-found/denied behavior across tenants;
- create no audit, outbox, idempotency or metadata transition rows;
- preserve exact typed payload identity and size limits.

## 6. Audit model

For each successful public publish, activate or rollback mutation, the same transaction must contain:

- the authoritative metadata state transition where applicable;
- append-only metadata transition evidence;
- one canonical `crm.audit_records` entry bound to tenant, actor, capability, request, business transaction, correlation and trace context;
- idempotency completion evidence.

Replay of an already-completed semantic idempotency identity must return the stored typed result without duplicating any of the above evidence.

## 7. Application composition

`crm-application-runtime` owns production composition of:

- metadata capability definitions and query definitions;
- semantic validators and typed adapters;
- `PostgresMetadataStore`;
- capability/query executors;
- authenticated HTTP/gRPC ingress exposure;
- process-level acceptance fixtures.

`services/crm-api` remains a thin host and must not import metadata persistence or metadata schema implementation crates directly.

## 8. Browser client boundary

The generated browser contract and `packages/client` surface must expose typed operations only. Feature code must not receive:

- arbitrary capability owner/id/version inputs;
- generic raw query or mutation envelopes;
- raw descriptor hashes;
- direct metadata persistence coordinates.

The first client operations should be sufficient for the follow-on Admin Studio workflow while keeping UI implementation outside this packet.

## 9. Acceptance matrix

The delivery packet is complete only when automated acceptance proves:

- typed bundle publication through authenticated production ingress;
- deterministic revision retrieval and impact analysis;
- activation with expected-generation success;
- stale-generation rejection;
- breaking activation rejection without confirmation;
- rollback restoring the prior revision and popping history;
- idempotent replay without duplicate state or evidence;
- canonical global audit plus metadata transition evidence correlation;
- cross-tenant read/mutation non-disclosure;
- query paths leave mutation evidence counts unchanged;
- Rust/browser generated contract synchronization;
- real PostgreSQL process-level execution through `crm-api`;
- all applicable exact-head CI gates green simultaneously.

## 10. Implementation sequence

1. Define authoritative Protobuf contracts and generated identities.
2. Add exact capability/query catalog entries and typed semantic adapters.
3. Implement mutation/query executors over `PostgresMetadataStore`.
4. Integrate canonical audit/idempotency transaction behavior.
5. Compose the runtime in `crm-application-runtime` and production ingress.
6. Add typed browser client operations.
7. Add real PostgreSQL and process-level acceptance.
8. Synchronize roadmap/status and freeze one exact review head for final CI.
