# Privacy Owner Scope Shared Support

Status: implementation packet for Phase 8A.11 after the independently accepted Parties and Consents owner contributions.

## Purpose

This document records the comparison that justifies `crm-customer-privacy-owner-scope-support` and defines its maximum allowed boundary.

The shared package exists only because two contrasting production-shaped implementations now prove the same protocol behavior:

- Parties contributes one authoritative Party record and always returns a terminal page;
- Consents traverses authoritative Party-to-Consent relationships, validates multiple Consent records and uses bounded keyset pagination.

The difference between those owners is deliberate. Shared support is extracted only where both implementations had identical semantics before extraction.

## Proven shared seams

### Exact query request integrity

Both owners must:

- validate the query execution context and typed input payload;
- match owner module, capability ID and capability version to the supplied definition;
- match the exact input payload contract;
- recompute SHA-256 over the encoded input and reject a mismatched input hash.

The support package returns typed failures. Each owner maps them to its existing owner-specific public error codes and safe messages.

### Common lineage validation

Both owners apply the same validation to `PrivacyScopeContributionLineage`:

- lineage tenant must equal the execution tenant;
- privacy case ID and canonical Party ID must be valid record identifiers;
- Identity Resolution generation must be positive;
- registry version and digest must match the canonical owner-scope registry;
- purpose code must be normalized uppercase ASCII with digits and underscores and remain bounded;
- effective request time must be positive and not later than request start;
- zero page size selects the owner contract default and the resolved value must not exceed the owner maximum.

The support package returns the validated lineage, canonical Party ID, claimed generation and resolved page size. It does not interpret an owner cursor.

### Canonical Party topology proof

Both owners prove the same claim inside an already opened tenant-bound repeatable READ ONLY transaction:

- acquire the Identity Resolution topology lock for the tenant snapshot;
- read the current topology generation;
- reject a stale claimed generation;
- require the canonical Party record to be visible through tenant RLS;
- reject an active outgoing canonical redirect.

The support package receives `BoundReadTransaction`; it cannot construct one. Architecture policy continues to allow `begin_bound_read_transaction` only in the Parties and Consents PostgreSQL adapters.

### Deterministic digest framing

Both owners use domain-separated SHA-256 with unsigned 64-bit big-endian length framing for every field. The shared package exposes:

- complete digest construction for fixed field sequences;
- incremental frame appending for owner-specific variable-length evidence streams.

Digest domains and field selection remain owner-owned.

## Explicitly owner-specific

The following must not move into the shared support package without a new comparison against additional independently accepted owners:

- PostgreSQL record and relationship queries;
- relationship type and record type coordinates;
- strict persisted metadata validation and domain rehydration;
- Parties terminal-only cursor rejection;
- Consents keyset cursor encoding, decoding and binding;
- page construction and resource ordering;
- evidence class and retention policy selection;
- response Protobuf type and typed output contract;
- owner-specific error codes, safe messages and retry policy;
- runtime registration, ingress, workers or privacy-case orchestration.

## Dependency and authority rules

- Owner adapters depend on the shared support package.
- The support package may depend on stable platform, Identity Resolution, Party contract and Customer Privacy registry boundaries required to prove the common protocol.
- The support package must not depend on either privacy owner adapter.
- It owns no module ID, capability coordinate, cursor format, record inventory or evidence policy.
- It must not open a database transaction or expose unrestricted datastore access.
- Parties and Consents remain the source of truth for their contracts and authoritative reads.

## New-crate justification

An internal module in one owner would make the other owner depend on the wrong authority. Duplicating the code would preserve drift in security-sensitive lineage and topology checks. A dedicated package provides a mechanically enforceable dependency direction while remaining smaller than either owner adapter.

The crate is justified only while it remains a protocol-support boundary shared by at least the two independently proven owners. It must not become a generic privacy framework or a catalog of remaining owner behavior.

## Acceptance requirements

The extraction is behavior-neutral only when one unchanged candidate SHA passes:

- shared support unit tests;
- Parties privacy-scope clean database and rollback/reapply acceptance;
- Consents privacy-scope clean database and rollback/reapply acceptance;
- architecture and dependency checks;
- lockfile, formatting, Clippy and full workspace tests;
- every other applicable permanent workflow selected by the final diff.

A temporary construction or validation workflow is staging assistance only and must be absent from the final accepted diff.

## Next boundary

After this packet, additional owner privacy contributions may reuse the shared protocol support only when their semantics match the existing API. New owner-specific behavior must first remain local and be proven independently. Runtime privacy orchestration, approvals, restrictions, legal holds, execution plans, outcomes and worker recovery remain separate delivery packets.
