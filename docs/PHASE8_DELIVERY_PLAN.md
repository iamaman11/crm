# Ultimate CRM — Phase 8 Delivery Plan

Status: **Active execution — Phase 8A customer master**

Parent program: #11  
Customer-master program: #28  
Customer Privacy packet: #126  
Commercial follow-on: #29  
Architecture/developer-experience program: #194  
Delivery governance: `DELIVERY_GOVERNANCE.md`

## 1. Packet contract

Every Phase 8 packet defines authoritative ownership, stable identity, exact coordinates, persistence, tenant/authorization/audit boundaries, recovery, architecture impact and focused/process/rollback acceptance. A packet is complete only after merge to `main` with unchanged exact-head evidence.

Ordinary capabilities add zero crates, generic router/worker algorithms do not grow owner-specific switches, feature implementation and physical consolidation remain separate, and frozen historical evidence is not rewritten by later runtime acceptance.

## 2. Phase 8A completed work

- **8A.1–8A.6 — Complete:** customer references, Party, Account, Contact Point, Party Relationship, Customer 360, Consent and reversible Identity Resolution.
- **8A.7 — Complete:** governed import and recovery.
- **8A.8 — Complete:** governed export and recovery.
- **8A.9 — Complete:** Customer Data Quality Rules, Completeness and Stewardship.
- **8A.10 — Complete:** Governed Customer Enrichment and Provenance.

## 3. Phase 8A.11 Customer Privacy — current boundary

Issue #126 is **In progress**.

Merged public runtime inventory remains four mutations, two permission-aware public queries and zero Customer Privacy workers. Trusted-internal `customer_privacy.plan.build@1.0.0` has no public ingress. The two published plan/outcome read coordinates remain non-runtime.

All nine authoritative owner implementations are accepted:

1. Parties — PR #156;
2. Consents — PR #175;
3. Customer Accounts — PR #179;
4. Contact Points — PR #181;
5. Party Relationships — PR #183;
6. Identity Resolution — PR #186 / accepted source `24456b86379a1ef23ed5a60804cdcae5075d407c` / merge `509eb304a76055c9f49b0beed3b007963a91cb22` / 25 of 25 permanent workflows;
7. Customer Data Operations — PR #188 / accepted source `07f34786e82fdfa78d263790e9f50541529006f8` / merge `089be72fa3010b4aa15aff7f9ea55fd86290f8fc` / 26 of 26 permanent workflows;
8. Data Quality — PR #190 / accepted source `dcfe8faebc7462b888f8fc1721cb379a40fea88a` / merge `deac197c97cddc15bb9916092ca87f6e767ce1de` / 27 of 27 permanent workflows;
9. Customer Enrichment — PR #192 / accepted source `e90e36027de18a07be68e43327ea732810ff332a` / merge `e41cbab0cd30819fcbe2e3c5f2c7415fc6de3e8c` / 28 of 28 permanent workflows.

## 4. Nine-owner set complete

The owner implementation lane is complete. No accepted owner may be described as unstarted and no additional owner contribution is the next packet.

## 5. Accepted architecture prerequisites

Issue #194 runs alongside Phase 8A. PR #197, PR #199, PR #200 and PR #203 established reproducible architecture/dependency no-growth controls. PR #204 froze scope discovery and immutable snapshot semantics. PR #205 accepted the Customer Privacy domain/application/PostgreSQL/production package boundary. Workspace packages remain `113`.

## 6. Accepted scope discovery and immutable snapshot

State: **Accepted through PR #206 / source `086b17a95058eee285fcb67a903bd21d9263d357` / merge `95818fd3aeb54a9593a45642583f0b7224d5ecfe` / 31 of 31 permanent workflows**.

PR #206 implements exact-nine trusted-internal discovery, immutable tenant/case/Party/topology/registry/purpose/effective-time lineage, bounded owner pagination, durable pages/checkpoints, safe reference-only aggregation, strict snapshot rehydration, replay/crash recovery, permission-aware internal reads, safe audit, FORCE RLS, cross-tenant concealment, rollback/reapply and repeated acceptance.

PR #207 synchronized accepted evidence. The historical PR #204 freeze remains unchanged.

## 7. Accepted deterministic planning freeze and runtime

PR #208 / source `d16a42551918ac6142d7a57cbeb7802f8f162fb9` / merge `bbdbc12ed139367efe75033c2a7e7ddb3eaec59d` / 16 of 16 permanent workflows froze immutable plan lineage, exact actions, deterministic ordering, contiguous sequence, lineage/item/plan digests, strict canonical rehydration, unsupported crypto-shred failure and permission-aware plan/outcome read boundaries.

PR #209 / source `b97fd9bb4537c14df4497ad7b737d0f0a64c4f3b` / merge `30621ffff5c1e07e1275cc80fee3f1297a91f49e` / 29 of 29 permanent workflows accepted trusted-internal activation-gated planning.

Implemented behavior:

- exact case, snapshot, Party/Identity Resolution, policy and jurisdiction lineage validation;
- deterministic immutable action plan using `Retain`, `RestrictOnly`, `Anonymize`, `Delete`, supported `CryptoShred` and reserved `NoOpAlreadyCompliant`;
- atomic `Scoped → Planned` or `Scoped → AwaitingApproval` transition;
- strict rehydration, replay/conflict detection and append-only case/snapshot/plan/audit evidence;
- FORCE RLS, canonical `tenant_isolation`, cross-tenant concealment, clean PostgreSQL, rollback/reapply and repeated acceptance;
- unchanged 113 packages, 4 mutations, 2 queries and 0 workers.

## 8. Next packet — permission-aware reads

Promote only:

1. `customer_privacy.case.plan.get@1.0.0` with module activation, live permission/visibility, tenant-bound reads, strict case↔plan↔snapshot lineage and replay evidence, payload-safe summary, audited read and concealed unauthorized/cross-tenant existence;
2. `customer_privacy.case.owner_outcomes.list@1.0.0` with bounded validation and a deterministic empty terminal page (`items = []`) until owner execution and outcome persistence exist.

No new crate, generic-runtime switch, mutation, worker, owner mutation, approval, restriction, hold/retention decision, synthetic outcome or destructive execution is allowed.

## 9. Ordered continuation

1. permission-aware plan/outcome reads;
2. approval runtime;
3. immediate deny-only processing restrictions using final subject locks;
4. legal-hold and mandatory-retention precedence;
5. replay-safe resumable owner execution;
6. crash-window recovery;
7. governed access/export assembly;
8. owner-specific deletion, anonymization and crypto-shred execution;
9. Party tombstone and no-orphan proof;
10. projection/search/cache convergence;
11. Customer Privacy worker;
12. disable/uninstall fail-closed semantics;
13. complete process/end-to-end acceptance;
14. Phase 8A closure and then Phase 8B / issue #29.

## 10. Frozen ownership

`crm.customer-privacy` owns privacy cases, immutable scope snapshots, deterministic plans, restrictions, customer-data legal holds, retention decisions, per-owner attempts/outcomes, checkpoints, governed export references and convergence evidence. It does not directly mutate Party, Account, Contact Point, Relationship, Consent, Identity Resolution, Customer Data Operations, Data Quality or Customer Enrichment storage.

```text
legal hold > mandatory retention > approved privacy action > ordinary retention
```

## 11. Phase 8A closure

Phase 8A remains **In progress**.

It closes only after reads, approval, restrictions, holds/retention, owner execution, access/export, deletion/anonymization/crypto-shred, tombstone/no-orphan behavior, convergence, worker lifecycle and full process acceptance are merged.

## 12. Phase 8B and completion rule

Product Catalog, Pricing, CPQ, Quotes, Orders, Contracts, Subscriptions/Entitlements/Usage and governed billing/ERP/payment/tax/fulfillment remain planned and blocked on completed Phase 8A.

Current product-complete expert modules: **0**.
