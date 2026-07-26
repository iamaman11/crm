# Customer Accounts Privacy Scope Owner Packet

Status: **Ready after post-PR #176 synchronization**  
Parent program: #126  
Prerequisites: Parties PR #156, Consents PR #175, shared support PR #176  
Coordinate: `customer_accounts.privacy.scope.contribute@1.0.0`

## 1. Objective

Implement Customer Accounts as the next authoritative privacy-scope owner contribution while preserving the accepted owner-scope protocol boundary.

The packet is contract-only and non-runtime. It proves that a third owner can consume `crm-customer-privacy-owner-scope-support` without changing the shared crate or weakening Account ownership, tenant isolation, strict persisted-state validation or deterministic reference-only evidence.

## 2. Why Customer Accounts is next

Customer Accounts is selected because:

- its authoritative owner module and production contribution boundary are already accepted;
- its PostgreSQL process-isolation baseline is already measured;
- Account records are independently owned and associated with Parties through governed owner semantics;
- it provides a useful third shape after one direct Party record and relationship-traversed Consent records;
- it can validate reuse of shared request, lineage and canonical-Party proof without requiring runtime orchestration.

Selection does not imply that Customer Accounts behavior belongs in the shared support package.

## 3. Exact implementation boundary

The owner adapter must:

1. validate its exact capability definition and Protobuf wrapper;
2. use the accepted shared support for query integrity, common lineage, registry, purpose, effective time, page-size handling and canonical Party proof;
3. open one tenant-bound `REPEATABLE READ, READ ONLY` transaction in the Customer Accounts adapter;
4. read only authoritative Account records and authoritative Account-to-Party association evidence owned by Customer Accounts;
5. strictly validate persisted Account metadata and rehydrate the canonical Account domain state;
6. reject missing, cross-tenant, stale-generation, noncanonical-Party, malformed relationship and corrupt Account state fail-closed;
7. emit only stable Account references, versions, data class, evidence class and retention-policy identity;
8. produce deterministic page and cursor evidence under an owner-specific digest domain;
9. write no records, relationships, business transactions, idempotency, events, outbox or audit data;
10. remain absent from public ingress, worker registry and production runtime inventories.

## 4. Owner-specific decisions required before coding

The implementation PR must determine from existing authoritative Customer Accounts contracts:

- the exact Account record type;
- the authoritative Party association representation and direction;
- whether one Party may own zero, one or multiple visible Accounts;
- stable ordering and whether pagination is required;
- the exact Account data class;
- the correct privacy evidence class;
- the canonical Account retention policy;
- whether deleted or inactive Accounts remain required privacy evidence;
- the exact owner-prefixed error codes and safe messages.

These decisions must be derived from current owner contracts and persisted semantics, not invented by Customer Privacy.

## 5. Shared-support non-regression rule

The Customer Accounts packet may add the Customer Accounts adapter as an approved consumer of `crm-customer-privacy-owner-scope-support` only together with its independently proven implementation.

It must not change shared support merely to reduce owner code. A shared-support extension is allowed only when Customer Accounts demonstrates behavior that is:

- semantically identical to both Parties and Consents;
- independent of Account SQL, state or classification;
- covered by compatibility tests for all existing consumers;
- behavior-neutral for accepted owner contracts and digest evidence.

Otherwise the behavior stays owner-specific.

## 6. Explicit exclusions

This packet does not:

- promote `customer_accounts.privacy.scope.contribute@1.0.0` to runtime;
- register `customer_privacy.scope.discover@1.0.0`;
- add a Customer Privacy worker;
- modify Account business state;
- add deletion, anonymization or restriction execution;
- change public Protobuf contracts;
- redesign Account ownership or Party association semantics;
- generalize cursor or pagination policy across owners;
- resolve Consents cursor authentication or cross-page snapshot semantics;
- implement another owner in the same PR.

## 7. Required tests

### Unit and contract tests

- exact contract-only coordinate and definition;
- exact input contract and SHA-256 integrity;
- common lineage error mapping to stable Customer Accounts codes;
- owner-specific cursor/page behavior;
- deterministic reference-only response;
- stable digest compatibility vectors;
- no raw Account values in the response.

### PostgreSQL acceptance

- clean migration application and owner fixtures;
- tenant/RLS isolation and cross-tenant concealment;
- current Identity Resolution generation acceptance;
- stale generation rejection;
- canonical redirect rejection;
- zero, one and multiple Account association cases as permitted by owner semantics;
- deterministic multi-page behavior when pagination is required;
- malformed association and corrupt persisted-state rejection;
- complete schema rollback and removal;
- migration reapply and repeated acceptance;
- zero writes for successful and rejected reads.

### Repository gates

- architecture policy permits bound-read transaction creation only in exact approved owner adapters;
- architecture policy adds Customer Accounts as a shared-support consumer only with this implementation;
- Customer Accounts owner workflow includes shared-support paths where applicable;
- Rust formatting, Clippy and workspace tests;
- Affected Scope, Governance, Complexity and generated-source checks;
- all applicable permanent workflows green on one unchanged candidate SHA.

## 8. Completion rule

The packet is complete only when the implementation is merged to `main`, exact-head evidence is recorded, documentation is synchronized and the coordinate remains correctly classified contract-only/non-runtime.

Completion of this owner does not authorize Customer Privacy discovery or planning runtime. The program continues with the remaining owners until a separately reviewed sufficient-owner-set decision is recorded.
