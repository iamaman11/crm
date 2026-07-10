# ADR-024: Controlled data formats and canonicalization

Status: Accepted  
Date: 2026-07-10  
Owners: Architecture, Security Engineering, Platform Engineering

## Context

Ultimate CRM must support human-authored configuration, strongly typed machine contracts, external APIs, immutable packages, audit hashing and independent module development. Unrestricted JSON, YAML and opaque byte payloads would make the platform ambiguous and difficult to validate. Conversely, banning JSON and YAML would make configuration, integrations and administration unnecessarily difficult.

Protobuf serialization is deterministic only within limited implementation scopes and is not a permanent canonical byte representation. Long-lived signatures, audit-chain hashes, package identities and semantic idempotency keys therefore require a separate, versioned canonicalization profile.

## Decision

### Authoritative roles

- Protobuf is authoritative for RPC, command and event schemas.
- SQL migrations are authoritative for PostgreSQL schema evolution.
- YAML is a human authoring format only.
- JSON is an exchange, UI, webhook, logging and normalized-IR format only.
- Published typed IR is authoritative for installed modules and activated configuration.
- CEL is allowed only for deterministic side-effect-free expressions.

### Strict YAML profile

Governance manifests use a JSON-compatible YAML 1.2 subset. Parsers must reject anchors, aliases, merge keys, custom tags, duplicate keys, non-string keys, implicit date/time values, floats and non-finite numbers.

The publication pipeline is:

`YAML source → strict parse → JSON-compatible tree → JSON Schema → semantic validation → normalized typed IR → canonical digest → immutable publication`.

Runtime services never execute raw YAML.

### JSON policy

JSON payloads must be schema-bound and size-limited. Duplicate keys and non-finite values are rejected. Unknown properties are rejected by default. Extension maps require an explicit owner, schema and version.

### Canonicalization Profile crm.cjson/v1

The profile accepts objects with ASCII string keys, arrays, strings, booleans, null and bounded integers. Floats are forbidden. Objects are serialized as UTF-8 JSON with lexicographically sorted keys, no insignificant whitespace and deterministic escaping. The profile identifier is stored with every digest.

This profile is an intentionally restricted JCS-compatible subset. Restricting keys and number types avoids cross-language ambiguity while preserving the properties required by CRM manifests and audit material.

### Protobuf hashing

Raw protobuf wire bytes must not be used as permanent semantic identity. A typed object that requires a stable semantic hash is projected to a versioned canonical JSON shape and hashed using its declared canonicalization profile.

Protobuf bytes may still be hashed for artifact integrity when the exact artifact bytes themselves are the subject of the hash, such as a generated descriptor-set artifact.

## Consequences

### Positive

- Human-friendly YAML remains available without becoming runtime truth.
- Independent implementations can reproduce package and audit digests.
- Configuration ambiguity is caught before publication.
- Module manifests become machine-verifiable and suitable for dependency analysis.
- Protobuf can evolve without silently invalidating historical semantic hashes.

### Negative

- YAML features familiar to infrastructure engineers are intentionally unavailable in domain manifests.
- A compilation/publication step is mandatory.
- Canonical projections must be maintained and versioned.
- Existing opaque JSON or byte fields need schema ownership and migration plans.

## Enforcement

- `docs/SYSTEM_INVARIANTS.md` is normative.
- `schemas/module.schema.json` defines the first-party and marketplace authoring contract.
- `scripts/validate_module_manifests.py` enforces syntax, schema and dependency rules.
- Governance CI blocks invalid manifests.
- Contract CI blocks incompatible protobuf changes.

## Follow-up

1. Introduce a typed `ModuleManifest` compiler output.
2. Store canonicalization profile IDs beside package, audit and idempotency digests.
3. Add cross-language canonicalization test vectors before implementing production signing.
4. Migrate opaque payloads to schema-owned envelopes.
