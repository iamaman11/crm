# Ultimate CRM — Plan Consistency and Context Recovery

Status: **Orientation and recovery contract; not runtime truth**  
Verified repository baseline for this review: `main` at `eac6707e6799f74e761ede39d852bf8de7ac6a77` (merged PR #301 / Repository Step 22D)  
Architecture execution issue: #194  
Passport/identity-document product issue: #302

This document exists so that development can be resumed correctly even if all conversational context is lost. It does not create a competing roadmap. It records how to reconstruct the authoritative state, which plans own which decisions, and which expert design decisions must not disappear between sessions.

## 1. Recovery rule

A new developer or coding agent must be able to recover the project without chat history by reading, in order:

1. `../AGENTS.md` — repository operating discipline;
2. `SYSTEM_INVARIANTS.md` — non-negotiable architecture/security rules;
3. published Protobuf, schemas, manifests and accepted ADRs — machine/runtime contracts;
4. `APPLICATION_ARCHITECTURE.md` — stable layering/composition model;
5. `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` section 2.4 — the single repository implementation order;
6. `IMPLEMENTATION_ROADMAP.md` and `PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md` — product dependency order and complete product target;
7. `PHASE8_DELIVERY_PLAN.md` — current Phase 8 sequencing;
8. `PROJECT_STATUS.md` — current merged-state snapshot;
9. `repository-packet.json` and generated `ACTIVE_PACKET.md` — exact active bounded change;
10. the active GitHub issue/PR — work-in-progress scope and exact-head evidence;
11. `ULTIMATE_ARCHITECTURE_10_OF_10_REVIEW_AND_CLOSURE_PLAN.md` — independent closure criteria for a real architecture 10/10;
12. `PASSPORT_RECOGNITION_AND_IDENTITY_DOCUMENTS_PLAN.md` and issue #302 — complete identity-document/document-intelligence product design.

If two documents disagree, use the repository precedence defined in `AGENTS.md` and `docs/README.md`; never resolve disagreement from memory or chat.

## 2. Verified current position at the time of this review

The verified default branch is `eac6707e6799f74e761ede39d852bf8de7ac6a77`.

Accepted facts:

- Repository Steps 1–21 are complete;
- Phase 8A and Customer Privacy product completion are accepted through PR #296;
- Repository Step 22 is in progress under ADR-032 and is not architecture closure;
- Step 22A froze 63 internal direct `crm-application-runtime` dependencies: 62 production and 1 test-only, plus 41 permanent workflows / 42 permanent jobs;
- Step 22B finalized 16 platform-generic dependencies and 1 test-only dependency;
- Step 22C removed the direct Customer Privacy query-adapter runtime edge;
- Step 22D / PR #301 removed the direct Customer 360 query-adapter runtime edge;
- current runtime fan-in after Step 22D is 61 total / 60 production / 1 test-only;
- current decision ledger contains 19 final classifications: 16 platform-generic, 2 removed, 1 test-only;
- 44 accepted-inventory dependency rows remain unresolved;
- permanent gate dispositions remain unresolved;
- architecture 10/10 is intentionally not declared.

Issue #194 has been synchronized to these facts. Generated/current-state repository documents may temporarily lag a just-merged bounded packet until the next governed packet regenerates/synchronizes them; such lag is a defect to be removed before final 10/10, never a reason to reinterpret the execution order.

## 3. Single execution order that must survive context loss

The active architecture program remains:

```text
Repository Step 22
  finish all runtime-fan-in classifications/remediation
  finish permanent workflow/job value-cost dispositions
  remeasure architecture/change-cost/CI evidence
        ↓
Repository Step 23 / Phase 8B.1
  Catalog + effective-dated Pricing
  reference-heavy extension-cost proof
        ↓
Repository Step 24 / Phase 8B.2
  CPQ + approvals/orchestration
  process-heavy extension-cost proof
        ↓
Repository Step 25
  independent, mechanically reproducible architecture 10/10 closure review
        ↓
remaining product waves from PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md
```

Passport/identity-document development is a complete committed product direction, tracked by issue #302, but its implementation must enter the same governed packet system. Functional scope is not reduced by delivery sequencing. If product priority requires changing the execution order, the normative roadmap must be deliberately amended and mechanically synchronized rather than bypassed informally.

## 4. Architecture 10/10 decisions that must not be lost

The foundation remains a modular monolith, not a microservice rewrite. Final 10/10 requires evidence that the architecture stays clean as the product grows.

Non-loss decisions:

- one authoritative owner per mutable aggregate;
- business modules never receive raw database, broker, object-storage, secret-store, arbitrary HTTP or model-provider clients;
- all state changes enter through exact versioned capabilities, live authorization and one governed transaction with idempotency/outbox/audit;
- generic runtime algorithms contain no business-owner switches;
- ordinary capability growth in an existing owner creates zero crates by default and does not edit generic runtime;
- owner-specific `crm-application-runtime` fan-in is removed when a valid owner/production boundary already exists;
- any owner-specific edge retained after Step 22 must have explicit unavoidable-boundary evidence;
- every permanent CI gate must have a unique failure mode, owner, value/cost evidence and review/retirement condition;
- exact-head acceptance, FORCE RLS, cross-tenant negative proof, rollback/reapply, privacy and recoverability are never weakened for speed;
- final 10/10 is a machine-reproducible evidence state, not a subjective score.

See `ULTIMATE_ARCHITECTURE_10_OF_10_REVIEW_AND_CLOSURE_PLAN.md` for the full closure matrix.

## 5. Passport and identity-document decisions that must not be lost

The full end-state is defined in `PASSPORT_RECOGNITION_AND_IDENTITY_DOCUMENTS_PLAN.md`. Core decisions are:

- `crm.parties` owns accepted Person/Profile/IdentityDocument state;
- recognition is evidence/candidate production, never an alternate mutation path;
- original protected document bytes enter through the governed immutable file boundary;
- image quality, crop/orientation/perspective/blur/glare/exposure/completeness checks are server-authoritative and run before paid recognition where possible;
- transient normalized images/MRZ crops should not become unnecessary permanent protected copies;
- extraction is provider-neutral and supports provider substitution, routing, fallback and future ensemble policies;
- VIZ OCR, MRZ parsing/check digits, document-profile validation and cross-source comparison are distinct stages;
- MRZ/format checks prove internal consistency, not physical document authenticity;
- field-level provenance records whether a value came from VIZ, MRZ, chip/NFC, manual correction or multiple agreeing sources;
- missing native-script values stay unknown/null rather than being invented by transliteration;
- user review and policy-controlled automatic acceptance are separate from model confidence;
- Customer 360 exposes only bounded masked projections;
- global search must not leak passport/personal-number values;
- exact protected lookup, duplicate-document detection and Identity Resolution signals are permission-aware;
- Customer Privacy covers source file, authoritative document state, extraction evidence, restriction, access/export, retention, legal hold, deletion/minimization and recovery;
- raw protected document bytes, MRZ, passport numbers and national identifiers do not enter ordinary logs/audit/event payloads;
- optional ePassport/NFC verification is a separate stronger evidence path with cryptographic chip/SOD/DG verification;
- provider/model promotion is benchmark-gated on protected representative datasets with exact-match, manual-correction, latency, cost and failure metrics;
- no real passport fixture is committed to the public repository.

Issue #302 is the durable product tracker for this capability.

## 6. Plan ownership matrix

| Concern | Authoritative / primary document | Companion / tracker |
|---|---|---|
| Absolute architecture/security invariants | `SYSTEM_INVARIANTS.md` | accepted ADRs |
| Stable layering/composition | `APPLICATION_ARCHITECTURE.md` | `ARCHITECTURE_READINESS.md` |
| Repository execution order / complexity closure | `ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md` | issue #194; `ULTIMATE_ARCHITECTURE_10_OF_10_REVIEW_AND_CLOSURE_PLAN.md` |
| CI scalability/device-lab | `ARCHITECTURE_CI_SCALABILITY_AND_DEVICE_LAB_PLAN.md` | issue #194 |
| Product dependency order | `IMPLEMENTATION_ROADMAP.md` | `PHASE8_DELIVERY_PLAN.md` |
| Complete Universal CRM product target | `PRODUCT_DEVELOPMENT_10_OF_10_PLAN.md` | `CRM_CAPABILITY_COVERAGE.md`, `MODULE_CATALOG.md` |
| Current merged state | `PROJECT_STATUS.md` | merged PR evidence |
| Current bounded work | `repository-packet.json` | generated `ACTIVE_PACKET.md`, active issue/PR |
| Passport/document intelligence | `PASSPORT_RECOGNITION_AND_IDENTITY_DOCUMENTS_PLAN.md` | issue #302 |
| Final architecture 10/10 evidence criteria | existing normative architecture plan | `ULTIMATE_ARCHITECTURE_10_OF_10_REVIEW_AND_CLOSURE_PLAN.md` |

No companion document may silently redefine a normative source.

## 7. Consistency checks required before claiming plans synchronized

A planning/synchronization review should verify all of the following:

- no document calls a completed repository step “next”;
- no document calls an unaccepted step complete;
- Phase 8A remains complete but architecture 10/10 remains unclaimed;
- Step 22 counts match the canonical inventory/decision ledger;
- current `crm-application-runtime` dependency count matches the accepted inventory minus recorded removals;
- workflow/job counts are not rewritten from the immutable Step 22A snapshot;
- Steps 23, 24 and 25 remain in that order unless a deliberate normative roadmap change is accepted;
- lifecycle commands implemented in Step 18 are not described as merely planned;
- generated navigation is regenerated whenever its authoritative inputs change;
- Passport/Identity Documents remains represented in durable docs and issue #302;
- every newly introduced product plan states its owner, dependencies, privacy/security implications, product UX evidence and Definition of Done;
- every current-status claim is tied to merged `main` evidence, never to an unmerged branch or chat statement.

## 8. Resume checklist after total context loss

Use this exact procedure:

```text
1. Read AGENTS.md + SYSTEM_INVARIANTS.md.
2. Resolve current main SHA.
3. Read PROJECT_STATUS.md and issue #194.
4. Read repository-packet.json / ACTIVE_PACKET.md.
5. Verify whether the packet implementation is merged or still open.
6. Run/review repo.py packet-check and affected-scope evidence for the current branch.
7. Continue only the first unfinished repository step.
8. For passport work, read issue #302 + PASSPORT_RECOGNITION_AND_IDENTITY_DOCUMENTS_PLAN.md before touching code.
9. After a merge, synchronize durable docs/issues before making completion claims.
10. Never reconstruct missing scope from chat memory.
```

The purpose of this file is not to duplicate the roadmap. It is to make the roadmap and expert design corpus recoverable, self-checking and resistant to loss of conversational context.
