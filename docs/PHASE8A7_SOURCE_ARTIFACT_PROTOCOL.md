# Phase 8A.7 Immutable Source Artifact Protocol

## Status

Normative for the Phase 8A.7 customer Party import production path.

## Purpose

An import job must be derived from the exact immutable bytes that were uploaded, finalized and bound to that job. A client-supplied collection of pre-parsed row maps is not sufficient production evidence for the identity or interpretation of an import source.

## Ownership boundaries

- `crm-core-files` owns the platform contract for tenant-aware immutable file artifacts.
- PostgreSQL is an adapter for that platform contract and is not exposed directly to business modules.
- `crm.customer-data-operations` owns import-source semantics, parser profiles, mappings, import jobs, row outcomes and checkpoints.
- `crm.parties` remains the only owner of canonical Party records.

## Governed source capability sequence

The production sequence is:

1. `customer_data.import.party.source.create@1.0.0`
2. one or more `customer_data.import.party.source.chunk.append@1.0.0`
3. `customer_data.import.party.source.finalize@1.0.0`
4. `customer_data.import.party.source.job.create@1.0.0`
5. one or more `customer_data.import.party.source.rows.validate@1.0.0`
6. `customer_data.import.party.validation.finalize@1.0.0`
7. `customer_data.import.party.execution.start@1.0.0`
8. background execution through the ordinary governed Party capability gateway

The former client-preparsed coordinates `customer_data.import.party.create` and `customer_data.import.party.rows.validate` are not part of the public production application catalog.

## Artifact creation

Creation freezes:

- tenant;
- artifact ID;
- owner module;
- media type;
- data class;
- retention policy;
- expected byte length;
- expected SHA-256.

Artifact creation is idempotent only when the same artifact ID is bound to the same immutable metadata.

## Chunk append

Each chunk must be:

- non-empty;
- no larger than the platform chunk limit;
- supplied with an exact SHA-256 of its bytes;
- appended at the exact next chunk index.

A previously accepted chunk index may be replayed only with identical bytes and digest. A conflicting replay is rejected. Out-of-order append is rejected.

## Finalization

Finalization succeeds only when:

- the received byte count equals the declared immutable size;
- chunk indexes are contiguous from zero;
- every stored chunk still matches its stored digest and size;
- the ordered concatenated bytes match the declared artifact SHA-256.

After finalization the artifact is immutable. Finalized reads re-verify chunk integrity and full-artifact SHA-256 before returning bytes.

## Transactional evidence

Public artifact create, chunk append and finalize are capability mutations. A real state change must commit in the same PostgreSQL transaction as:

- capability idempotency claim and completion;
- durable outbox event evidence;
- durable audit evidence;
- business-transaction completion evidence.

If evidence construction or persistence fails, the artifact mutation must roll back. An identical no-op state retry may complete idempotently without emitting a second business lifecycle event.

## Job binding

The parser profile is not generic file metadata. The same finalized bytes may be interpreted by separate jobs under different explicitly versioned parser profiles.

Artifact-backed job creation:

1. reads only a finalized artifact through the governed file port;
2. verifies the finalized bytes;
3. parses the exact bytes with the requested immutable parser profile;
4. derives the authoritative row count server-side;
5. creates a `SourceDescriptor` bound to the exact artifact ID and SHA-256;
6. freezes source-system identity, parser profile, mapping and partial-execution policy in the import job.

## Server-side validation

Production validation never accepts authoritative pre-parsed row maps from the client.

For each validation batch the system:

1. loads the authoritative import job;
2. requires the job to be in `Created` state;
3. reads the exact finalized artifact bound to the job;
4. verifies artifact SHA-256 against the job binding;
5. parses the exact bytes with the job's immutable parser profile;
6. verifies the parsed row count against the job binding;
7. selects the requested bounded one-based source-position range;
8. applies the frozen mapping and domain validation rules;
9. builds the full mutation plan;
10. repeats live authorization after read/parse work;
11. without any intervening awaited work, commits job progress, row records, job-to-row relationships, outbox events, idempotency and audit evidence atomically.

## Dry-run invariant

Source upload, job creation and validation may mutate only import-owned source/job/row evidence. They must create zero target Party records, zero target Party idempotency records, zero target Party outbox events and zero target Party mutation audit records.

## Execution invariant

Execution invokes exact `parties.party.create@1.0.0` through `GatewayCapabilityClient` and the ordinary `CapabilityGateway`. Import code never writes Party storage directly.

The deterministic per-row target idempotency key and durable import-owned outcome/checkpoint state are the basis for crash/restart correctness.
