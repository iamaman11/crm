# Phase 8 Contract Map

Status: **Draft planning map for expert-domain delivery**

Phase 8 is designed as one coherent domain program but delivered through reviewable packets. This map defines where Protobuf is mandatory and where native implementation types remain preferable.

## Contract rule

Use versioned Protobuf for stable boundaries that cross a module, process, network, language or generated-client boundary. Keep private domain models, internal algorithms, repository implementations and in-process policy types native to their implementation language unless they are intentionally promoted to a stable contract.

### Protobuf is mandatory for

- public capability request/response contracts;
- public query request/response contracts;
- durable cross-module events;
- module-to-module integration messages;
- browser/server generated transport contracts;
- externally published integration contracts;
- long-lived payloads whose compatibility must be enforced independently of one implementation language.

### Protobuf is not the default for

- private aggregate/domain structs and value objects;
- internal planner state;
- repository/adapter implementation details;
- local policy types that never cross a stable boundary;
- UI component props and host-owned UI-extension contexts;
- database row shapes and migration internals.

The governing test is not “can this be expressed in Protobuf?” but “is this a stable boundary that must survive independent evolution?”

## Phase 8 domain ownership

### 8A — Customer Master, Identity and Consent

Canonical owner boundaries:

- Party: person and organization identity;
- Account: customer/commercial relationship referencing one or more parties;
- Contact Point: email, phone, postal address and messaging identity;
- Party Relationship: employment, household, hierarchy and configurable typed relationships;
- Consent and Communication Preference;
- Identity Resolution, Match Candidate, Merge/Unmerge and Survivorship lineage.

First stable Protobuf contract families should cover:

1. immutable resource identifiers and references;
2. Party/Account/ContactPoint public reads and governed commands;
3. durable lifecycle/domain events;
4. consent authorization decisions and evidence references;
5. identity-resolution candidate/review/merge commands and events;
6. import/export job control and result contracts where they cross the application boundary.

Private matching algorithms, survivorship policy implementations and persistence row shapes remain native implementation details.

### 8B — Product Catalog and Quote-to-Revenue

Canonical owner boundaries:

- Product and Product Version;
- Catalog and Assortment;
- Price Book and Pricing Rule;
- Configuration/CPQ;
- Quote and Quote Revision;
- Order;
- Contract;
- Subscription and entitlement references.

Stable Protobuf boundaries must reference customer identities from 8A rather than defining local account/contact models.

### Later Phase 8 domain waves

The same rule applies to communications, service, marketing, billing, projects, documents/e-signature and analytics: each domain owns its invariants internally and publishes only the stable versioned boundaries required by other modules and product clients.

## Delivery sequence

1. Freeze this ownership and contract-boundary map.
2. Deliver 8A.1 identity/reference contracts and owner-module skeletons.
3. Deliver 8A vertical packets for Party, Account/ContactPoint/Relationship, Consent, Identity Resolution, Merge/Unmerge and Import/Privacy workflows.
4. Begin 8B only against stable merged 8A reference contracts.
5. Continue independent domain waves without a single giant Phase 8 merge.

Every packet must keep exact-version compatibility, generated-code synchronization, tenant isolation, authorization, idempotency and audit evidence explicit.
