# Phase 8A.9 — Customer Data Quality Rules, Completeness and Stewardship Architecture

## Status

Normative implementation architecture for issue #124.

**Delivery state: In progress.** Phase 8A.8 is merged through PR #130 and delivery governance is synchronized through PR #131 / merge `565fb922f2fdaf3763bebb88228a0c9392d6d17a`. This document freezes the first production boundary before public contract publication.

## 1. Ownership decision

Phase 8A.9 introduces a distinct business module:

- module id: `crm.data-quality`;
- role: authoritative owner/coordinator of long-lived quality-governance definitions, findings, completeness evidence and stewardship lifecycle;
- first production target: canonical Party quality.

The module exists because quality-governance state has a durable lifecycle, ownership and audit boundary that does not belong to customer import/export jobs and is not authoritative Party state.

`crm.data-quality` must **not** become:

- a competing customer master;
- a generic record store;
- a generic ETL or import/export subsystem;
- a generic workflow/BPM engine;
- an arbitrary SQL or user-code execution engine;
- an enrichment-provider integration owner.

`crm.customer-data-operations` remains the owner of governed import/export jobs and their evidence. `crm.parties` remains the only authoritative Party owner.

## 2. Owned state

`crm.data-quality` may own only:

1. immutable published Party quality rule-set versions;
2. immutable published Party completeness-profile versions;
3. Party evaluation jobs and bounded restart-safe execution evidence;
4. immutable Party evaluation input snapshots containing only the exact bounded fields required by the selected v1 evaluators;
5. deterministic rule-evaluation outcomes tied to exact Party resource versions;
6. logical quality findings and immutable finding observations;
7. deterministic completeness results and exact component lineage;
8. stewardship assignment, acknowledgement and waiver evidence;
9. deterministic remediation-attempt evidence for owner-capability mutations;
10. bounded safe diagnostics and reconciliation counters.

It does not own mutable Party values. A stored evaluation input snapshot is derived, version-bound evidence and never authoritative customer state.

## 3. Data classification and minimization

The first Party evaluation input snapshot contains only:

- `PartyRef`;
- exact Party `resource_version`;
- Party kind;
- normalized `display_name` required by the v1 evaluators;
- governed observation timestamp.

The snapshot is classified as personal data because `display_name` may contain personal information. It is private evaluation evidence, is not exposed through generic public queries and is retained only under the data-quality retention policy.

Public findings expose bounded reason codes and rule/source-version lineage, not the raw staged display name.

## 4. First v1 evaluator vocabulary

The first public rule vocabulary is deliberately small and closed. No expression language is introduced.

### 4.1 `PARTY_DISPLAY_NAME_MIN_UTF8_BYTES`

Parameters:

- `minimum_utf8_bytes`: integer in the inclusive range `2..=64`.

Evaluation:

- pass when the normalized Party `display_name` UTF-8 byte length is greater than or equal to the configured minimum;
- fail otherwise.

This is a quality threshold, not a Party validity rule. A Party value accepted by `crm.parties` may still produce a quality finding.

### 4.2 `PARTY_DISPLAY_NAME_PLACEHOLDER_EXACT_ASCII_CASEFOLD`

Parameters:

- `placeholder_tokens`: `1..=32` unique canonical tokens;
- every token is ASCII, trimmed, non-empty and at most 64 bytes;
- tokens are canonicalized with ASCII lowercase and sorted lexicographically before identity hashing.

Evaluation:

- ASCII-lowercase the normalized Party `display_name` only when every character is ASCII;
- fail when the result exactly equals one configured canonical placeholder token;
- non-ASCII values do not match this evaluator and therefore pass it.

The evaluator performs exact equality only. It does not use regex, fuzzy matching, locale-dependent case conversion or substring matching.

### 4.3 Explicitly forbidden execution surfaces

The v1 evaluator runtime has no support for:

- arbitrary SQL;
- JavaScript, Python, WASM or other user-supplied code;
- regular expressions supplied by tenants;
- filesystem access;
- network access;
- shell/process execution;
- reflection over arbitrary record fields;
- unbounded recursion or iteration.

Adding a new evaluator kind requires an additive reviewed contract and a new immutable evaluator semantic version.

## 5. Rule-set version model

A Party rule-set version is published as immutable content.

Each rule contains:

- tenant-scoped stable `rule_key`;
- severity: `INFO`, `WARNING`, `ERROR` or `CRITICAL`;
- one exact v1 evaluator kind;
- evaluator parameters validated and canonicalized by kind;
- bounded non-empty human-facing title and remediation guidance;
- evaluator semantic version.

Canonicalization rules:

1. rule keys are unique;
2. rules are sorted by canonical `rule_key` before identity calculation;
3. evaluator parameters are encoded in one canonical representation;
4. all bounded strings reject control characters;
5. unknown evaluator kinds and unknown fields fail closed.

The immutable `rule_set_version_id` is deterministic from a domain-separated SHA-256 digest over the canonical target type, evaluator semantic versions and ordered canonical rule definitions.

Publishing identical canonical content is an idempotent replay. The same caller-provided logical publication key with different canonical content conflicts.

Published content is never edited in place.

## 6. Completeness profile model

A completeness profile version is immutable and references one exact Party rule-set version.

Each component contains:

- a unique component key;
- one exact referenced `rule_key` from the bound rule-set version;
- a positive integer `weight_basis_points`.

Invariants:

- components are sorted by component key for identity hashing;
- every referenced rule exists in the bound rule-set version;
- component keys are unique;
- weights are positive;
- the exact sum of all component weights is `10_000` basis points.

A Party completeness score is integer-only:

`score_basis_points = sum(weight_basis_points for components whose referenced rule evaluation passed)`.

No floating-point arithmetic is used.

The result stores exact component lineage containing component key, referenced rule key, rule outcome identity and awarded basis points. The awarded component sum must equal the stored score exactly.

The immutable `completeness_profile_version_id` is a domain-separated SHA-256 digest over the bound rule-set version and canonical ordered components.

## 7. Governed Party source boundary

The pure `crm.data-quality` owner module contains no SQL, transport code or Party storage access.

Application composition provides a narrow Party quality source port that returns only the fields required by the frozen v1 evaluator vocabulary:

- Party reference;
- Party kind;
- normalized display name;
- exact authoritative Party resource version.

Every source read:

1. is tenant-bound;
2. performs live top-level query authorization;
3. performs live resource/field visibility checks;
4. reads through a Party-owned governed query boundary;
5. never reads Party tables directly from the data-quality module or its adapters.

Missing, invisible and cross-tenant Party references use safe non-disclosing public behavior.

## 8. Evaluation job and crash boundary

Party evaluation is an asynchronous durable job so the real process can prove restart/retry behavior.

Job lifecycle:

`CREATED -> STAGED -> COMPLETED`

Terminal failure states may be added only for bounded non-retryable configuration/state errors. Retryable infrastructure failures leave the job recoverable.

### 8.1 Stage before deterministic evaluation

The first successful worker pass:

1. performs the governed Party read;
2. creates one deterministic job-bound immutable evaluation input snapshot;
3. records the exact Party source version and bounded v1 source fields;
4. advances the job to `STAGED` atomically with the snapshot evidence.

After staging, evaluation never re-reads live Party values for that job. This makes evaluation deterministic across process restart even when Party changes later.

The immutable snapshot is evidence, not a competing Party master.

### 8.2 Deterministic outcomes

For one staged job:

- one rule outcome exists per rule in the exact bound rule-set version;
- `rule_outcome_id = H(job_id, rule_key, rule_set_version_id)`;
- one completeness result exists for the exact bound completeness-profile version;
- all outcome writes are replay-safe.

A job becomes `COMPLETED` only after:

- every expected rule outcome is durable;
- finding observation/current-state effects for failed rules are durable;
- pass-driven remediation state changes for previously open logical findings are durable where applicable;
- the completeness result and exact component reconciliation are durable.

## 9. Finding identity and historical evidence

A logical finding is stable across reevaluation:

`finding_id = H(tenant, target_owner_module, target_resource_type, target_resource_id, rule_set_version_id, rule_key)`.

The source resource version is deliberately not part of the logical finding identity.

Each failed exact source version creates at most one immutable observation:

`finding_observation_id = H(finding_id, source_resource_version)`.

Therefore:

- retrying the same job cannot duplicate an observation;
- evaluating the same exact Party version in another job cannot duplicate the logical observation;
- a newer failing Party version produces a new immutable observation under the same logical finding.

A finding current state stores:

- latest observation id;
- latest evaluated source version;
- current lifecycle status;
- optional assigned actor;
- optimistic finding version;
- governed mutation timestamps.

Historical observations are never deleted or rewritten when current state changes.

## 10. Finding lifecycle

Public current lifecycle states:

- `OPEN`;
- `ACKNOWLEDGED`;
- `WAIVED`;
- `REMEDIATED`.

Rules:

1. first failed observation opens the finding;
2. acknowledgement and waiver are exact-version mutations against the current observation;
3. waiver requires a bounded non-empty reason;
4. a newer failing source version supersedes the previous observation and resets the current finding to `OPEN`; acknowledgement/waiver of older evidence does not silently apply to new evidence;
5. a newer passing evaluation may transition an existing non-remediated finding to `REMEDIATED` and records the exact passing rule outcome/source version that caused the transition;
6. a later newer failure may reopen the same logical finding with a new immutable observation;
7. lower or equal source versions cannot regress current evidence; exact equal-version replay is idempotent only when the evidence is identical.

Staleness is evidence-based rather than guessed. Public finding responses expose the exact evaluated source version. Operations that require current authority, especially remediation, re-read the live Party and reject a source-version mismatch.

## 11. Stewardship model

The first stewardship boundary is intentionally part of the finding aggregate rather than a generic workflow engine.

Supported mutations:

- assign or clear one actor assignment with optimistic finding version;
- acknowledge the current observation;
- waive the current observation with reason;
- request the exact Party display-name remediation path described below.

Permission-aware queries provide:

- get finding;
- list findings by Party;
- list assigned findings by actor/status/severity using bounded signed pagination.

Queue membership is derived from authoritative finding state and query filters. There is no separate mutable generic queue engine in v1.

## 12. Governed remediation boundary

The first exact remediation path is limited to Party display-name corrections because both v1 evaluators operate on `display_name`.

A remediation request binds:

- finding reference;
- exact expected finding version;
- exact current finding observation;
- exact expected Party resource version;
- proposed new display name;
- caller idempotency identity.

Application composition must:

1. authorize the data-quality remediation capability;
2. load the current finding and ensure it is applicable to the Party display-name evaluator family;
3. perform a fresh governed Party read;
4. reject if the live Party version differs from the finding/request source version;
5. invoke the exact `parties.party.update@1.0.0` owner capability through the governed capability gateway;
6. use a deterministic target idempotency identity derived from the remediation attempt;
7. record a separate deterministic remediation-attempt outcome in `crm.data-quality`.

No data-quality adapter writes Party storage directly.

### 12.1 Remediation crash recovery

The remediation attempt has a deterministic identity and target Party idempotency key.

If the Party update succeeds but the data-quality outcome commit is interrupted, retry invokes the same Party capability with the same target idempotency identity, obtains the exact replay result, then commits the missing remediation outcome exactly once.

The finding is not marked `REMEDIATED` merely because a mutation was requested. A subsequent deterministic evaluation of the new Party version is the authority for rule pass/fail and remediation state.

## 13. Public contract surface

Initial mutation capabilities:

- `data_quality.party.rule_set.publish@1.0.0`;
- `data_quality.party.completeness_profile.publish@1.0.0`;
- `data_quality.party.evaluation.request@1.0.0`;
- `data_quality.finding.assign@1.0.0`;
- `data_quality.finding.acknowledge@1.0.0`;
- `data_quality.finding.waive@1.0.0`;
- `data_quality.party.display_name.remediate@1.0.0`.

Initial query capabilities:

- `data_quality.party.rule_set.get@1.0.0`;
- `data_quality.party.completeness_profile.get@1.0.0`;
- `data_quality.party.evaluation.get@1.0.0`;
- `data_quality.finding.get@1.0.0`;
- `data_quality.finding.list_by_party@1.0.0`;
- `data_quality.finding.list_assigned@1.0.0`;
- `data_quality.party.completeness.get@1.0.0`.

Public contracts are additive `crm.data_quality.v1` Protobuf contracts. Private aggregate persistence is owner implementation state.

## 14. Event boundary

The first event family is typed and bounded:

- `data_quality.party.rule_set.published@1.0.0`;
- `data_quality.party.completeness_profile.published@1.0.0`;
- `data_quality.party.evaluation.requested@1.0.0`;
- `data_quality.party.evaluation.completed@1.0.0`;
- `data_quality.finding.opened@1.0.0`;
- `data_quality.finding.observed@1.0.0`;
- `data_quality.finding.status_changed@1.0.0`;
- `data_quality.finding.assignment_changed@1.0.0`;
- `data_quality.party.remediation.completed@1.0.0`.

Events contain stable references, versions, bounded reason codes and reconciliation data. They do not publish the private staged Party display name.

## 15. Persistence and tenant isolation

The module uses the governed PostgreSQL record/outbox/idempotency/audit foundation.

Planned record types:

- `data_quality.party_rule_set_version`;
- `data_quality.party_completeness_profile_version`;
- `data_quality.party_evaluation_job`;
- `data_quality.party_evaluation_input`;
- `data_quality.rule_outcome`;
- `data_quality.finding`;
- `data_quality.finding_observation`;
- `data_quality.party_completeness_result`;
- `data_quality.remediation_attempt`.

All records are tenant-scoped and protected by the platform FORCE RLS model. Deterministic identities are still tenant-bound by storage and execution context.

Persistence decoders must strictly revalidate canonical domain state. Malformed, non-canonical or internally inconsistent stored state is corruption, not user input.

Migrations must prove clean apply, reverse rollback and reapply.

## 16. Bounded execution limits

The first implementation must define and enforce explicit maxima for:

- rules per rule-set version;
- placeholder tokens per rule;
- completeness components per profile;
- active evaluation jobs scanned per tenant cycle;
- jobs processed per tenant cycle;
- bounded strings and reason text;
- query page size and scan multiplier through existing platform query controls.

Worker scheduling is per-tenant and bounded. One tenant with a large queue must not create an unbounded single cycle.

## 17. Authorization and approval

- publishing immutable rule-set/profile versions requires explicit capability authorization;
- evaluation requests require authorization to evaluate the target Party and visibility of the required source fields;
- finding/completeness/stewardship queries repeat live query authorization and resource/field visibility;
- assignment, acknowledgement and waiver require explicit mutation authorization;
- Party remediation is a separate higher-risk capability and must also pass the target Party owner capability authorization;
- no possession of a finding id, evaluation id or Party id is authority.

Approval requirements are capability-specific. The first Party display-name remediation path must preserve the approval policy of the underlying Party update capability and may impose a stricter data-quality policy, never a weaker alternate path.

## 18. Acceptance proof

Fresh PostgreSQL plus a real `crm-api` process must prove at minimum:

1. canonical rule-set/profile publication and conflicting replay rejection;
2. deterministic rule-set/profile identities independent of caller ordering;
3. invalid/unknown evaluator definitions fail without side effects;
4. no arbitrary execution/network/filesystem path exists;
5. tenant-isolated governed Party reads and safe non-disclosure;
6. evaluation staging binds one exact Party version and restarts from the same immutable staged input after Party changes;
7. deterministic rule outcomes and exact equal-version replay;
8. one logical finding and one observation per exact rule/source version without duplicates;
9. newer failing version produces a new observation and reopens current state;
10. newer passing version remediates the current finding while historical observations remain;
11. completeness score equals exact component awarded-basis-point reconciliation;
12. assignment/acknowledgement/waiver optimistic concurrency;
13. stale Party version blocks remediation before owner mutation;
14. successful remediation invokes only the exact Party owner capability;
15. Party-success/data-quality-outcome-missing crash recovery replays the same target idempotency identity without duplicate Party mutation;
16. restart/retry does not duplicate jobs, outcomes, findings, observations, assignments or remediation outcomes;
17. permission-aware finding/completeness/stewardship queries and signed pagination;
18. cross-tenant non-disclosure;
19. exact record, outbox, idempotency, audit and business-transaction evidence;
20. migration clean apply, reverse rollback and reapply;
21. one unchanged final source-authored SHA with every applicable workflow green.

## 19. Delivery sequence inside PR #124

Implementation proceeds in this order:

1. pure domain identities, immutable rule-set/profile canonicalization and evaluator semantics;
2. strict persistence model and module manifest/storage declarations;
3. additive public Protobuf contracts and generated contract publication;
4. mutation/query adapter foundations for immutable definitions;
5. governed Party source port and application-runtime composition;
6. durable evaluation job, staging and restart-safe worker;
7. rule outcomes, finding observations/current lifecycle and completeness results;
8. permission-aware queries and stewardship mutations;
9. governed Party display-name remediation and target-success/outcome-missing recovery;
10. PostgreSQL fixtures/migrations/RLS and real-process acceptance;
11. exact-head final gate and Gate review.

No later layer may weaken the ownership, deterministic identity, exact source-version or governed owner-capability boundaries frozen above.
