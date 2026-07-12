# Ultimate CRM — Exact-SHA Multi-Agent Development Protocol

Status: **Normative contributor and coding-agent protocol**  
Tracked by: issue #70  
Applies to: delivery packets developed by more than one human or coding agent, and to any packet that uses an independent local verifier.

This document defines the repository-governed two-agent development system for Ultimate CRM. Its purpose is to shorten the implementation-feedback loop without weakening architecture, correctness, security, tenant isolation, rollback, audit or final CI requirements.

The protocol complements `SYSTEM_INVARIANTS.md`, accepted ADRs, `DEVELOPMENT_WORKFLOW.md`, `APPLICATION_ARCHITECTURE.md`, `MODULE_DEVELOPMENT.md` and `AGENTS.md`. It never overrides a system invariant or accepted architecture decision.

## 1. Core model

The default two-agent model has three independent responsibilities:

1. **Architect / Implementer** — owns the delivery packet design and primary implementation.
2. **Local Integrator / Verifier** — checks an exact immutable commit SHA in a complete local toolchain and returns reproducible evidence.
3. **GitHub CI** — remains the final independent merge authority for all applicable repository gates.

The model is:

```text
Architect / Implementer
  → publishes an exact checkpoint SHA
    → Local Integrator / Verifier checks that exact SHA
      → returns structured evidence
        → Architect / Implementer fixes the correct cause
          → publishes a new exact SHA
            → local verification repeats as needed
              → GitHub CI validates one exact final review head
                → merge
```

Local verification accelerates diagnosis. It does not replace required GitHub CI.

## 2. Why the protocol exists

Remote CI is valuable because it provides a clean independent environment, but it is an expensive place to discover ordinary compile, lint, local integration or process-startup defects. A local verifier with a persistent checkout and the full toolchain can shorten the loop substantially.

The protocol therefore separates:

- **design authority and implementation ownership** from
- **independent local reproduction and integration evidence**.

The separation is deliberate. The verifier is not a second hidden architect editing the same code concurrently.

## 3. Definitions

### 3.1 Delivery packet

A coherent architecture result as defined by `DEVELOPMENT_WORKFLOW.md`, such as one governed runtime, one complete module slice or one complete frontend/backend workflow.

### 3.2 Primary writer

The actor currently authorized to make non-mechanical changes to the delivery packet branch. There must be exactly one primary writer for overlapping code scope at a time.

### 3.3 Exact checkpoint SHA

An immutable Git commit identifier published for verification. Branch names are navigation aids; the SHA is the verification identity.

### 3.4 Verification record

A structured report that names the exact SHA, environment, commands, results and failures observed by the verifier.

### 3.5 Stale evidence

Any verification result produced for a SHA other than the current review head. Stale evidence may help diagnosis but cannot prove the new head is green.

### 3.6 Writer handoff

An explicit transfer of write authority from one actor to another. A writer handoff is required before the verifier may make any repository change beyond a separately authorized mechanical fix.

## 4. Non-negotiable rules

1. **Exact-SHA rule.** Every verification claim must identify the exact commit SHA that was tested.
2. **Stale-result rule.** Any new commit invalidates green status for checks that have not run on the new SHA.
3. **Single-writer rule.** Two actors must not concurrently make hidden edits to overlapping code on the same delivery branch.
4. **Verify-first default.** The Local Integrator / Verifier operates in `VERIFY_ONLY` mode unless a writer handoff explicitly authorizes changes.
5. **No architecture by accident.** The verifier may diagnose and suggest a class of fix but does not silently change ownership, contracts, domain semantics, authorization, tenant isolation, persistence semantics or public behavior.
6. **No CI weakening.** A failing gate is fixed or explicitly recorded according to repository policy; the gate is not disabled, narrowed or suppressed merely to make a packet pass.
7. **Final exact-head rule.** Merge requires all applicable GitHub checks green on one exact review head, regardless of local verification status.
8. **Repository state over chat memory.** The active issue, branch, PR, committed documentation and exact SHA carry the durable handoff state. Chat is coordination, not the sole source of truth.

## 5. Roles and authority

## 5.1 Architect / Implementer

The Architect / Implementer owns:

- delivery-packet scope and non-goals;
- authoritative ownership decisions;
- invariants and dependency boundaries;
- public and internal contract design;
- domain/application semantics;
- infrastructure adapter design;
- production composition;
- test strategy and required negative paths;
- documentation and roadmap/status synchronization;
- deciding the correct fix for architecture or behavior defects;
- publishing verifier-ready checkpoints;
- deciding when the packet is ready for final CI and merge.

The Architect / Implementer should run whatever checks are available in its own environment, but lack of a complete local toolchain is exactly the case the independent verifier is designed to cover.

## 5.2 Local Integrator / Verifier

The Local Integrator / Verifier owns:

- fetching repository state without relying on a stale working tree;
- checking out the exact requested SHA;
- verifying toolchain and dependency availability;
- running the requested local commands faithfully;
- starting required local dependencies such as PostgreSQL or containerized services;
- reproducing failures with the smallest reliable command;
- classifying failures;
- returning enough evidence for the Architect / Implementer to act without guessing;
- refusing to claim a new SHA was verified when only an older SHA was tested.

The verifier does not own product scope or architecture merely because it discovered a failure.

## 5.3 GitHub CI

GitHub CI provides the final independent repository evidence:

- contract compatibility;
- governance and architecture policy;
- formatting and linting;
- workspace tests;
- database migration and runtime acceptance;
- specialized event, projection, search, application or future frontend gates;
- generated-source synchronization where applicable.

Local green status is a pre-merge accelerator, not a merge exception.

## 6. Delivery packet state machine

Every multi-agent packet moves through these conceptual states:

```text
PLANNING
  → IMPLEMENTING
    → READY_FOR_LOCAL_VERIFICATION
      → LOCAL_VERIFICATION_IN_PROGRESS
        → FIX_REQUIRED ───────────────┐
        │                             │
        └────────────→ LOCAL_VERIFIED │
                         → READY_FOR_EXACT_HEAD_CI
                           → GATE_REVIEW
                             → COMPLETE
```

A fix after `LOCAL_VERIFIED` creates a new SHA and returns the packet to the appropriate verification state.

### State meanings

- `PLANNING` — scope, ownership, contracts, gates and non-goals are being defined.
- `IMPLEMENTING` — the primary writer is changing the packet; the verifier must not modify overlapping scope.
- `READY_FOR_LOCAL_VERIFICATION` — an exact SHA and verification contract have been published.
- `LOCAL_VERIFICATION_IN_PROGRESS` — the verifier is testing that exact SHA.
- `FIX_REQUIRED` — one or more reproducible defects require implementation work.
- `LOCAL_VERIFIED` — the requested local verification set is green on one exact SHA.
- `READY_FOR_EXACT_HEAD_CI` — implementation and local evidence are sufficient to request the final repository gate set.
- `GATE_REVIEW` — GitHub CI/review is evaluating the exact PR head.
- `COMPLETE` — merged with required gates satisfied and documentation synchronized.

These are coordination states, not substitutes for the roadmap work states.

## 7. Required handoff manifest

The Architect / Implementer must publish a verification handoff containing at least:

```text
STATUS: READY_FOR_LOCAL_VERIFICATION
REPOSITORY: owner/repo
DELIVERY_PACKET: <issue/title>
BRANCH: <branch>
VERIFY_SHA: <40-character commit SHA>
MODE: VERIFY_ONLY | MECHANICAL_FIX_ALLOWED | WRITER_HANDOFF
SCOPE: <affected paths/components>
EXPECTED_ENVIRONMENT: <toolchain/services>
CHECKPOINT: A | B | C | custom
REQUIRED_COMMANDS:
  - <command 1>
  - <command 2>
OPTIONAL_DIAGNOSTICS:
  - <command>
KNOWN_LIMITATIONS:
  - <none or explicit limitation>
REPORT_FORMAT: docs/MULTI_AGENT_DEVELOPMENT.md section 10
```

A branch name without `VERIFY_SHA` is not a valid verification handoff.

## 8. Verification checkpoints

The local verification system mirrors the repository development checkpoints.

## 8.1 Checkpoint A — architecture and compile feedback

Purpose: catch boundary and compile failures before broad integration work.

Typical checks:

- architecture/source-boundary enforcement;
- manifest/schema consistency;
- contract generation or compatibility checks when affected;
- affected package/crate compilation;
- generated-client or generated-source consistency when affected;
- focused static analysis.

Checkpoint A should be fast enough to run at major structural milestones.

## 8.2 Checkpoint B — behavior and integration

Purpose: prove the changed behavior and important failure paths.

Typical checks:

- focused unit/domain tests;
- affected integration tests;
- PostgreSQL acceptance where applicable;
- tenant and cross-tenant negative tests;
- authorization and visibility behavior;
- idempotency, retry, conflict and replay behavior;
- rollback/failure behavior;
- process or browser integration tests for the changed path.

Checkpoint B is required before claiming the implementation behavior is ready.

## 8.3 Checkpoint C — local delivery preflight

Purpose: reduce avoidable final CI failures.

Typical checks:

- repository architecture checks;
- formatting check;
- lint with warnings denied;
- full affected workspace tests or the complete workspace suite when required;
- migration lifecycle checks;
- process-level acceptance;
- generated artifact synchronization;
- future frontend typecheck/lint/unit/browser E2E as applicable.

Checkpoint C does not replace GitHub CI.

## 9. Test selection policy

The Architect / Implementer chooses the required local verification set from the affected scope. The verifier may run additional diagnostics but must distinguish them from required gates.

The goal is not to run every expensive check after every edit. The default sequence is:

```text
structural change
  → Checkpoint A
behavior becomes coherent
  → Checkpoint B
packet is nearly ready
  → Checkpoint C
final immutable review head
  → all applicable GitHub CI
```

After a narrow fix, rerun the smallest checks that can invalidate the failure first, then rerun the broader checkpoint before promoting the packet state.

## 10. Required verification report

The verifier must return a report in this shape:

```text
VERIFICATION_STATUS: GREEN | RED | BLOCKED
VERIFIED_SHA: <exact SHA>
BRANCH_OBSERVED: <branch or detached HEAD>
MODE: VERIFY_ONLY | MECHANICAL_FIX_ALLOWED | WRITER_HANDOFF
ENVIRONMENT:
  OS: <value>
  toolchain: <relevant versions>
  services: <relevant versions/status>

REQUIRED_CHECKS:
  - command: <exact command>
    result: PASS | FAIL | BLOCKED
    duration: <optional>

FAILURES:
  - id: F1
    classification: COMPILE | LINT | TEST | DATABASE | PROCESS | FRONTEND | ENVIRONMENT | FLAKY_SUSPECTED | ARCHITECTURE | CONTRACT | OTHER
    command: <smallest reliable reproduction command>
    location: <file:line or component when available>
    symptom: <concise observed failure>
    minimal_cause: <technical cause supported by evidence>
    suggested_fix_class: <optional, non-authoritative>
    logs: <relevant excerpt or artifact location>

UNVERIFIED:
  - <anything requested but not actually run>

NOTES:
  - <environment-specific facts, flakiness evidence, or none>
```

A report must not say `GREEN` if any required check was not run. Use `BLOCKED` or list the unverified scope explicitly.

## 11. Failure classification and ownership

Default fix ownership:

| Failure class | Default owner |
|---|---|
| Architecture/ownership/dependency boundary | Architect / Implementer |
| Public contract/schema/versioning | Architect / Implementer |
| Domain/application behavior | Architect / Implementer |
| Authorization/tenant/privacy semantics | Architect / Implementer |
| Persistence/migration/transaction semantics | Architect / Implementer |
| Process composition/runtime behavior | Architect / Implementer |
| Mechanical formatting/import/lockfile/generated refresh | Verifier only when explicitly authorized |
| Local environment/tool installation | Local Integrator / Verifier |
| Suspected CI-only infrastructure problem | investigated jointly; GitHub evidence remains authoritative |

The verifier should distinguish **root cause** from **first visible symptom** whenever possible.

## 12. Mechanical-fix exception

`MECHANICAL_FIX_ALLOWED` is optional and narrow. It may authorize only explicitly listed classes such as:

- formatter output;
- import ordering;
- generated artifact refresh using the canonical generator;
- lockfile refresh;
- a clearly identified typographical correction.

It does not authorize:

- changing a public contract;
- suppressing a lint instead of fixing the design;
- weakening a test or gate;
- altering domain semantics;
- changing authorization or tenant behavior;
- changing migration semantics;
- broad refactoring.

If a mechanical change creates a new commit, that new SHA must be handed back to the Architect / Implementer and independently verified as required.

## 13. Writer handoff protocol

A verifier may become the active writer only through an explicit `WRITER_HANDOFF`.

Before handoff:

1. the current primary writer stops modifying the overlapping scope;
2. the exact base SHA is recorded;
3. the allowed write scope is named;
4. the expected commit/result is named.

After handoff:

1. the new writer publishes the resulting SHA;
2. the previous writer fetches the new SHA before resuming;
3. no local uncommitted assumptions are carried across the handoff;
4. the single-writer rule resumes with the new current owner.

## 14. Exact-SHA evidence rules

### 14.1 Green evidence is SHA-specific

If `abc123` passed Checkpoint B and a fix creates `def456`, the statement is:

```text
abc123: Checkpoint B GREEN

def456: Checkpoint B UNKNOWN until rerun
```

Never silently transfer green status from one SHA to another.

### 14.2 Narrow fixes may use targeted reruns first

A targeted rerun may prove the immediate failure is fixed, but broader packet status is restored only after the required checkpoint set runs on the new SHA.

### 14.3 Final merge evidence is PR-head-specific

The exact final review head named in the PR must be the head on which all applicable required GitHub checks are simultaneously green.

## 15. Branch and commit policy

The existing delivery-packet rules remain unchanged:

- one coherent packet may use one long-lived implementation branch;
- incremental working commits are allowed;
- temporary commits are acceptable during active implementation;
- final PR history should be reduced to semantic commits where repository tooling permits;
- never create a new branch or PR merely for a mechanical fix inside the same packet.

The multi-agent protocol adds only this requirement: every verification handoff must identify a commit, not merely a moving branch.

## 16. Repository artifacts and source of truth

Durable coordination belongs in the repository:

- issue — scope, goal, dependencies and acceptance;
- branch — active implementation line;
- PR — delivery artifact and final evidence summary;
- committed docs — normative process and architecture;
- exact SHA — immutable verification identity;
- CI checks — final independent gate evidence.

Chat messages may announce `CONNECT_SECOND_AGENT`, but the second agent must be able to reconstruct its task from repository state plus the handoff manifest.

## 17. Standard coordination signals

The Architect / Implementer may use these concise signals when coordinating through a human operator:

### `SECOND_AGENT_NOT_NEEDED`

The packet is in planning or active implementation. The verifier should not begin an overlapping verification/write cycle yet.

### `CONNECT_SECOND_AGENT`

A complete exact-SHA handoff manifest is available. The verifier may fetch and test that SHA.

### `SECOND_AGENT_REPORT_NEEDED`

Implementation is paused pending the structured report for the named SHA.

### `READY_FOR_EXACT_HEAD_CI`

The local verification requirement for the packet is satisfied and the current review head is ready for the final applicable GitHub gate set.

These signals are convenience labels. The exact SHA and repository state remain authoritative.

## 18. Security and environment discipline

The verifier must not:

- commit secrets, credentials or local environment files;
- paste protected production data into reports;
- use production tenant data for local verification unless a separately approved process explicitly permits it;
- disable RLS, authentication or authorization merely to make a local test convenient;
- replace canonical migrations or generated contracts with local-only variants.

Environment-only workarounds must be clearly identified as environment setup, not product fixes.

## 19. Interrupted or abandoned verification

When verification stops before completion, the report must say `BLOCKED` or list `UNVERIFIED` checks. A partial run is still useful evidence, but it must not be promoted to green status.

If the branch moves while a verifier is testing an older SHA, the verifier may finish the old run for diagnostic value, but the result remains attached to that older SHA.

## 20. Pull request evidence

A delivery PR using this protocol should state:

- delivery packet and issue;
- architecture result;
- ownership/dependency boundaries;
- production path;
- failure and rollback behavior;
- local verification status and exact SHA;
- unverified local scope, if any;
- exact final review head;
- applicable GitHub CI evidence;
- remaining scope not claimed by the PR.

A PR may be opened before local verification is green, but it must not imply that unrun checks have passed.

## 21. Reference verifier instruction

The following is the default verifier instruction, specialized by each handoff manifest:

```text
Work as the Local Integrator / Verifier for this repository.

1. Fetch current remote refs.
2. Check out exactly VERIFY_SHA; do not substitute the current branch head.
3. Confirm the observed HEAD equals VERIFY_SHA before testing.
4. Operate in the requested MODE.
5. Run every REQUIRED_COMMAND exactly or report why it is blocked.
6. Start required local dependencies using repository-supported configuration.
7. For each failure, provide the smallest reliable reproduction command, classification, location, symptom and minimal evidence-backed cause.
8. Do not make architecture, product, contract, authorization, tenant or persistence changes in VERIFY_ONLY mode.
9. Do not claim a different SHA is verified.
10. Return the structured report from section 10.
```

## 22. Definition of success

The multi-agent system is working correctly when:

- the Architect / Implementer spends less time using remote CI as a compiler or basic local integration environment;
- the verifier can reproduce failures without relying on hidden chat context;
- two agents do not race on the same code;
- every green claim is traceable to an exact SHA;
- defects are fixed at the correct architectural layer rather than patched around;
- required final CI remains strict;
- delivery speed improves without reducing the repository's quality bar.

The system is optimized for faster evidence and clearer ownership, not for fewer correctness guarantees.
