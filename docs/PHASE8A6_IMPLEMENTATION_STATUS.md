# Phase 8A.6 Implementation Status

Issue: #117  
Draft PR: #118

## Implemented foundation

- Normative reversible Party merge/unmerge architecture and owner boundaries.
- Pure `crm.identity-resolution` merge-lineage, survivorship and unmerge domain.
- Strict canonical merge-lineage persistence.
- `crm.parties` active/merged lifecycle domain with preserved Party identity and exact redirect lineage.
- Immutable Party state v1 writer plus additive v2 merge-lifecycle state and dual-read compatibility.
- Party-owned merge/unmerge mutation fragments with Party-owned lifecycle events.
- Identity Resolution-owned merge-lineage mutation fragments and Party-to-lineage access relationships.
- Additive merge/unmerge, lineage and canonical-resolution Protobuf contracts.
- Validated multi-owner mutation composition model.
- Atomic PostgreSQL multi-owner batch executor that preserves owner-bound payload/event checks while sharing one governed business transaction, idempotency claim and audit chain.

## Current integration gate

The next production layer is the composed merge/unmerge capability executor:

1. authoritative candidate-case and Party reads;
2. exact confirmed-case, pair, tenant, kind, lifecycle and version checks;
3. Party-owned mutation fragment planning;
4. Identity Resolution-owned lineage fragment planning;
5. one atomic `ComposedBatchMutationPlan` execution;
6. high-risk capability/runtime/registry wiring;
7. permission-aware lineage and canonical-resolution queries;
8. fresh-PostgreSQL real `crm-api` acceptance.

No merge or unmerge operation is considered production-complete until all applicable workflows pass together on one unchanged final SHA.
