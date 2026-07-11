# ADR-035: Canonical Rust Protobuf contract runtime

- Status: Accepted
- Date: 2026-07-11
- Phase: 6D

## Context

The repository publishes versioned Protobuf contracts and already enforces Buf formatting, lint, descriptor generation, breaking-change checks and manifest bindings. Rust capability adapters now need concrete message types for Sales and Activities. Re-declaring those request, response or event structures manually would create a second contract source and allow semantic or field-number drift. Accepting ad-hoc JSON would bypass the published contract universe entirely.

Generated source files should not be committed because they are compiler artifacts, but every supported build environment must be able to regenerate them reproducibly without relying on a host-installed `protoc` binary.

## Decision

Add the workspace crate `crm-proto-contracts` as the canonical Rust runtime for all published files under `proto/`.

The crate:

- recursively discovers the complete published Protobuf source set in deterministic path order;
- uses a vendored `protoc` binary and `prost-build` during the Cargo build;
- generates nested Rust package modules into `OUT_DIR` rather than committing generated source;
- exposes the compiled `FileDescriptorSet` as immutable bytes;
- requests deterministic `BTreeMap` generation for all map fields;
- verifies representative Sales and Activities message round trips;
- verifies that the descriptor set contains all required first-party domain packages.

Contract CI remains authoritative for Buf formatting, lint, FILE-level compatibility and manifest bindings. Rust CI proves that the same published source set compiles into usable Rust message types.

## Consequences

- Capability adapters import generated types instead of duplicating wire schemas.
- A Protobuf breaking change fails both Contract CI and dependent Rust compilation.
- Generated files do not create repository churn or stale checked-in artifacts.
- Builds remain independent of a system `protoc` installation.
- Internal persisted aggregate schemas remain separate from public wire contracts and require their own versioning and migration policy.
