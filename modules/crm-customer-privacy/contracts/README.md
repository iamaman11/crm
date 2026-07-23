# Published contract boundary for `crm.customer-privacy`

Customer Privacy wire contracts are published under
`proto/crm/customer_privacy/v1`.

## Public case and control surface

The public contract packet contains exactly the frozen case/control inventory:

- five case mutations and four case/plan/outcome queries;
- four restriction coordinates;
- three customer-data legal-hold coordinates;
- immutable case, restriction and legal-hold events.

Every public capability is bound through `module.yaml` to an exact Protobuf RPC,
request and response type. Generated bindings and client hashes remain
repository-generated artifacts.

Current production promotion remains exact and incremental: four public
mutations plus permission-aware `case.get` and subject-scoped `case.list` are
runtime. The remaining ten public Customer Privacy coordinates are explicitly
non-runtime.

## Owner scope contribution plane

`contributions.proto` defines one shared reference-only immutable envelope and
nine unique service/RPC identities. Each authoritative owner module publishes
exactly one capability:

- `parties.privacy.scope.contribute@1.0.0`;
- `customer_accounts.privacy.scope.contribute@1.0.0`;
- `contact_points.privacy.scope.contribute@1.0.0`;
- `party_relationships.privacy.scope.contribute@1.0.0`;
- `consents.privacy.scope.contribute@1.0.0`;
- `identity_resolution.privacy.scope.contribute@1.0.0`;
- `customer_data.privacy.scope.contribute@1.0.0`;
- `data_quality.privacy.scope.contribute@1.0.0`;
- `customer_enrichment.privacy.scope.contribute@1.0.0`.

The shared request binds privacy case, tenant, canonical Party, exact Identity
Resolution generation, registry identity, purpose and effective request time.
Responses echo that lineage, exact owner/capability/version identity, immutable
resource references and bounded page-completeness evidence. Raw owner values,
private state, credentials and internal diagnostics are forbidden.

The machine-readable parity contract is
`contracts/customer-privacy-owner-scope-contracts.json`.

## Current owner contribution state

All nine owner scope capabilities are **contract-only non-runtime**. No owner
adapter, production factory, public ingress or worker registration is claimed by
this packet. `crm.customer-privacy` intentionally does not consume them yet.

Promotion requires, owner by owner:

- authoritative owner-side lookup through governed ports or owner storage;
- tenant and canonical-subject binding with exact topology generation;
- bounded signed cursor semantics and deterministic page digests;
- live authorization and durable module activation;
- strict data/evidence/retention classification;
- no raw value disclosure;
- real PostgreSQL/process acceptance.

Only after a sufficient complete owner set exists may
`customer_privacy.scope.discover@1.0.0` become a worker candidate.

## Boundaries

Public and worker wire schemas remain independent from private `crm.cjson/v1`
state. They expose governed references and safe lifecycle evidence, never
verification documents, legal-authority material, raw owner payloads or internal
diagnostics. A contract declaration is not evidence of a production route.
