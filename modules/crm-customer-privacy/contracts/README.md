# Published contract boundary for `crm.customer-privacy`

The first compatible Customer Privacy public contract set is published under
`proto/crm/customer_privacy/v1`.

## Published surface

The contract packet contains exactly the frozen public case/control inventory:

- five case mutations and four case/plan/outcome queries;
- four restriction coordinates;
- three customer-data legal-hold coordinates;
- immutable case, restriction and legal-hold events.

Every capability is bound through `module.yaml` to an exact Protobuf RPC,
request and response type. Every event is bound to an exact Protobuf message.
Generated bindings and client hashes remain repository-generated artifacts.

## Current runtime state

These contracts are **contract-only non-runtime**. They are intentionally not
registered in public HTTP/gRPC ingress until all of the following exist:

- FORCE RLS persistence and rollback/reapply proof;
- live authorization and permission-aware query visibility;
- shared `tenant_id + canonical_party_id` final-lock enforcement;
- idempotent audit/outbox/business-transaction composition;
- fresh-PostgreSQL real-process acceptance.

The temporary non-runtime classification is explicit and must be removed
coordinate-by-coordinate as production promotion is proven.

## Boundaries

Public wire schemas remain independent from private `crm.cjson/v1` state.
They expose governed references and safe lifecycle evidence, never verification
documents, legal-authority material, raw owner payloads or internal diagnostics.
Worker orchestration and owner-contribution contracts are not part of this
public-contract slice.
