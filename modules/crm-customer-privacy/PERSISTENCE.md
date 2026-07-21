# Customer Privacy canonical private persistence

This layer defines the private persisted representation for the pure Customer Privacy aggregates. It is not a public API contract, database adapter or runtime route declaration.

## Canonical profile

Every state payload uses `crm.cjson/v1` with:

- lexicographically sorted ASCII identifier keys;
- no insignificant whitespace;
- no floating-point values;
- explicit `canonicalization_profile` evidence;
- decimal strings for every aggregate version, Identity Resolution generation and Unix-nanosecond timestamp;
- decode → strict rehydration → re-encode byte equality.

Unknown fields, unsupported profiles, noncanonical decimal forms, contradictory lifecycle evidence and oversized payloads fail closed as `CUSTOMER_PRIVACY_PERSISTED_STATE_INVALID`.

## Versioned state identities

The first immutable private schemas are:

- `crm.customer-privacy.case.state@1.0.0`;
- `crm.customer-privacy.processing_restriction.state@1.0.0`;
- `crm.customer-privacy.legal_hold.state@1.0.0`.

Each schema has an explicit descriptor hash, byte ceiling and retention-policy identity. Public Protobuf contracts remain separate and may evolve independently from these private schemas.

## Strict rehydration

Rehydration rejects state that could not have been produced by the domain lifecycle, including:

- zero aggregate versions or non-monotonic transition times;
- scope without verified subject binding;
- action plan without immutable scope evidence;
- approval without an action plan;
- rescope evidence that does not advance the authoritative Identity Resolution generation;
- active restrictions or legal holds containing release evidence;
- released controls without actor/time evidence;
- expired restrictions without an exact expiry boundary;
- invalid legal-hold reason codes.

## Boundary

This code performs no SQL, authorization, locking, routing or cross-owner work. PostgreSQL record envelopes, FORCE RLS, idempotency, audit, outbox and business-transaction atomicity belong to separately governed adapter and composition crates.
