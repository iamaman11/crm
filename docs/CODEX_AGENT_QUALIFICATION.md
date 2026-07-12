# Ultimate CRM — Codex Local Agent Qualification

Status: **Normative qualification companion to the multi-agent protocol**  
Tracked by: issue #74  
Applies to: a ChatGPT Codex agent proposed for persistent local repository work.

This document defines how the repository evaluates a second coding agent before granting it responsibility beyond exact-SHA verification. The default role remains Local Integrator / Verifier until the agent demonstrates the required environment, Git, test, review and implementation capabilities.

## 1. Why qualification is required

A capable local coding agent may be more valuable than a passive verifier. It may be able to:

- maintain a persistent local checkout;
- run complete toolchains and local services;
- inspect and modify code across Rust, PostgreSQL and TypeScript workspaces;
- diagnose failures from source and logs;
- implement bounded fixes;
- review diffs and architecture boundaries;
- own a non-overlapping sub-packet;
- commit, push and open or update pull requests.

Those capabilities must be established explicitly rather than assumed. More authority is granted only when the agent can preserve exact-SHA evidence, the single-writer rule, repository invariants and final CI discipline.

## 2. Persistent local checkout requirement

A qualified local agent SHOULD maintain one persistent checkout of `iamaman11/crm` in a stable local directory rather than repeatedly downloading disposable copies.

The agent must report:

```text
LOCAL_REPOSITORY_PATH: <absolute path>
REMOTE_ORIGIN: <git remote get-url origin>
DEFAULT_BRANCH: main
CURRENT_BRANCH: <branch>
CURRENT_HEAD: <40-character SHA>
WORKTREE_STATUS: clean | dirty
```

The exact path is environment-specific and must not be invented by another agent. The local agent chooses or confirms a writable durable directory and reports it.

Before every handoff run, the local agent must:

1. inspect `git status --short`;
2. preserve or explicitly dispose of any local uncommitted work;
3. fetch remote refs;
4. verify the requested branch and exact SHA;
5. never silently overwrite unrelated local work.

A dirty worktree is not automatically forbidden, but the agent must not mix unrelated uncommitted state into verification or implementation evidence.

## 3. Responsibility levels

### Level 0 — Environment unavailable

The agent cannot reliably clone/fetch the repository, run the required toolchain or preserve a local checkout.

Allowed responsibility: none beyond advisory analysis.

### Level 1 — Exact-SHA Verifier

The agent can:

- maintain or create a local checkout;
- fetch and check out exact SHAs;
- run required commands and services;
- return structured evidence;
- avoid unauthorized writes.

Allowed responsibility: `VERIFY_ONLY` and explicitly authorized mechanical fixes.

### Level 2 — Local Integrator

In addition to Level 1, the agent can:

- diagnose multi-component failures;
- inspect source, logs, generated artifacts and local services;
- distinguish environment defects from product defects;
- make bounded integration fixes under explicit `WRITER_HANDOFF`;
- commit and push a named allowed scope;
- rerun the required checkpoint on the resulting SHA.

Allowed responsibility: bounded integration ownership and mechanical or implementation fixes within a named scope.

### Level 3 — Co-Implementer

In addition to Level 2, the agent can:

- implement a clearly separated workstream from a committed architecture/specification;
- preserve module ownership and dependency direction;
- add tests and documentation for its scope;
- independently review its own diff before handback;
- avoid changing shared contracts or architecture outside its delegated boundary;
- publish semantic commits and exact-SHA handoff evidence.

Allowed responsibility: non-overlapping sub-packets with explicit interfaces and acceptance criteria.

### Level 4 — Delivery Packet Owner

In addition to Level 3, the agent can:

- read and reconcile the normative repository documents;
- define a coherent delivery packet without violating roadmap sequencing;
- make architecture decisions consistent with system invariants and accepted ADRs;
- own implementation, integration, tests, documentation and PR closure;
- coordinate other agents through exact-SHA handoffs;
- keep GitHub issues, PR state and roadmap/status synchronized.

Allowed responsibility: independent packet ownership.

Level 4 is not granted merely because the agent can write code quickly. It requires demonstrated architectural judgment and repository-governance discipline.

## 4. Default authority before qualification

Until qualification evidence is reviewed:

```text
RESPONSIBILITY_LEVEL: 1
DEFAULT_MODE: VERIFY_ONLY
OPTIONAL_MODE: MECHANICAL_FIX_ALLOWED when explicitly scoped
```

The Architect / Implementer may raise or lower the level per delivery packet.

## 5. Qualification evidence

The candidate agent must answer the questionnaire in section 8 and, where possible, demonstrate claims by inspecting its actual environment.

Claims should distinguish:

- **available now** — the tool or action can be used in the current environment;
- **available with user approval** — an explicit confirmation or credential step is required;
- **not available** — the environment cannot perform it;
- **unknown until attempted** — the capability has not been verified.

The candidate must not claim access to credentials, local files, Docker, GitHub write operations or long-running processes unless it can actually perform them.

## 6. Responsibility assignment principles

A higher-capability second agent should not remain artificially limited to passive verification. Responsibility should move toward the agent best positioned to perform the work, provided that:

- overlapping code still has one active primary writer at a time;
- work is partitioned by explicit scope or writer handoff;
- contract and architecture changes have an identified decision owner;
- every pushed change is attributable to an exact SHA;
- independent review/verification is preserved for high-risk changes;
- final GitHub CI remains mandatory.

The preferred mature model may become:

```text
Architect / Lead Agent
  ↔ Local Codex Integrator / Co-Implementer
      ↔ exact-SHA handoffs for shared boundaries
        → independent local verification where useful
          → GitHub exact-head CI
            → merge
```

This is not unrestricted concurrent editing. Parallel work is allowed only on clearly non-overlapping workstreams or separate branches with defined integration ownership.

## 7. Typical delegation by level

| Work | Level 1 | Level 2 | Level 3 | Level 4 |
|---|---:|---:|---:|---:|
| Exact-SHA build/test | yes | yes | yes | yes |
| Local services/PostgreSQL/Docker | yes | yes | yes | yes |
| Mechanical lock/generated refresh | scoped | yes | yes | yes |
| Diagnose compile/integration failures | report | yes | yes | yes |
| Implement bounded fix | no | scoped | yes | yes |
| Own non-overlapping workstream | no | limited | yes | yes |
| Change public contract/architecture | no | no | only delegated | yes |
| Define new delivery packet | no | no | no | yes |
| Merge without required CI | no | no | no | no |

## 8. Qualification questionnaire

The following questionnaire is intended to be sent directly to the candidate ChatGPT Codex agent.

### A. Local repository and persistence

1. Can you create and maintain a persistent local clone of `iamaman11/crm` across multiple work sessions?
2. What exact absolute local directory will you use for the repository? Do not invent a path: create or inspect it and report the real path.
3. Can you run `git remote -v`, `git status --short`, `git branch --show-current` and `git rev-parse HEAD` and report the actual outputs?
4. Can you safely handle an existing dirty worktree without deleting unrelated changes?
5. Can you use multiple Git worktrees or separate local clones when two non-overlapping branches must be active simultaneously?
6. Does your environment persist the checkout, dependency caches and build artifacts between our interactions, or can the environment be reset?

### B. Git and GitHub write capability

7. Can you fetch, checkout exact SHAs, create branches, commit and push to `iamaman11/crm`?
8. Are Git credentials already available in the local environment? If not, what user action is required?
9. Can you use `gh auth status`, `gh pr view`, `gh pr checks`, `gh run view` and `gh run watch`?
10. Can you open or update pull requests and GitHub issues yourself?
11. Can you read and respond to PR review comments and unresolved review threads?
12. Can you perform interactive rebase, squash or cherry-pick safely when explicitly requested?
13. Can you avoid force-pushing unless explicitly authorized?

### C. Toolchain and local services

14. Which of these are available now: Rust/Cargo, rustfmt, Clippy, Python, Node.js, Corepack, pnpm, Buf, protoc, PostgreSQL client/server, Docker, Docker Compose, Playwright and a browser runtime?
15. Can you install missing development dependencies in the environment? What requires elevated privileges or user approval?
16. Can you run PostgreSQL locally and create isolated test databases/roles?
17. Can you run Docker containers and inspect logs/health status?
18. Can you start long-running local processes such as `crm-api`, Vite and Playwright test servers, keep them running while executing other commands, and terminate them cleanly?
19. Can you bind local ports and make HTTP/gRPC/gRPC-Web requests between local processes?
20. Can you capture logs and preserve useful test artifacts without committing transient files?

### D. Codebase understanding and implementation

21. Can you read the repository's normative documents before modifying code and follow their precedence order?
22. Can you inspect a multi-crate Rust workspace and determine dependency direction and composition boundaries?
23. Can you work across Rust, SQL/PostgreSQL, Protobuf, TypeScript/React and CI YAML in one packet?
24. Can you modify code, add tests, run focused checks and return a concise evidence-backed explanation of the root cause?
25. Can you distinguish an architectural defect from a mechanical compile/lint failure?
26. Can you implement a fix from a committed specification without changing unrelated architecture?
27. Can you review your own diff for unintended files, generated artifacts, secrets and scope creep before committing?

### E. Independent verification and review

28. Can you operate in strict `VERIFY_ONLY` mode and refrain from editing even when you know how to fix a failure?
29. Can you verify an exact immutable SHA rather than silently testing the latest moving branch head?
30. Can you return the structured verification report required by `docs/MULTI_AGENT_DEVELOPMENT.md`?
31. Can you rerun a checkpoint after your own permitted commit and correctly treat the resulting SHA as a new verification identity?
32. Can you inspect GitHub Actions failures locally and compare local results with CI logs?
33. Can you perform an independent code review of another agent's diff and identify architecture, security, tenant-isolation, contract and test gaps?

### F. Higher responsibility

34. Are you able to take ownership of a bounded implementation workstream on a separate branch while another agent owns a different non-overlapping workstream?
35. Can you integrate two branches or workstreams and resolve conflicts while preserving the documented architecture?
36. Can you own a complete delivery packet from issue/specification through implementation, local validation, PR, CI fixes and documentation closure?
37. Can you update roadmap/status/module catalog only when actual implementation state justifies it?
38. Can you stop and request an architecture decision rather than inventing one when the committed specification is ambiguous?
39. Can you act as the active primary writer under `WRITER_HANDOFF` and later hand control back with an exact resulting SHA and clean worktree?
40. Which responsibility level from this document do you believe you can perform **right now**: Level 1, 2, 3 or 4? Give evidence for each claimed capability and explicitly list limitations.

## 9. Required response format

The candidate agent should answer using:

```text
CODEX_CAPABILITY_ASSESSMENT

LOCAL_REPOSITORY_PATH: <actual absolute path or NOT_CREATED>
PERSISTENT_ACROSS_SESSIONS: YES | NO | UNKNOWN
REMOTE_ORIGIN: <actual value or NOT_CONFIGURED>
GIT_WRITE_ACCESS: YES | NO | REQUIRES_USER_ACTION | UNKNOWN
GITHUB_CLI_AUTH: AUTHENTICATED | NOT_AUTHENTICATED | UNAVAILABLE | UNKNOWN

AVAILABLE_TOOLCHAIN:
  rust: ...
  cargo: ...
  python: ...
  node: ...
  pnpm: ...
  buf: ...
  protoc: ...
  postgres: ...
  docker: ...
  docker_compose: ...
  playwright: ...

LONG_RUNNING_PROCESSES: YES | NO | LIMITED | UNKNOWN
LOCAL_NETWORK_AND_PORTS: YES | NO | LIMITED | UNKNOWN
CAN_RUN_FULL_REPOSITORY_CHECKS: YES | NO | PARTIAL | UNKNOWN
CAN_PUSH_BRANCHES: YES | NO | REQUIRES_USER_ACTION | UNKNOWN
CAN_MANAGE_PRS: YES | NO | REQUIRES_USER_ACTION | UNKNOWN
CAN_REVIEW_OTHER_AGENT_DIFFS: YES | NO | PARTIAL
CAN_OWN_NON_OVERLAPPING_WORKSTREAM: YES | NO | PARTIAL
CAN_OWN_FULL_DELIVERY_PACKET: YES | NO | PARTIAL

RECOMMENDED_RESPONSIBILITY_LEVEL: 0 | 1 | 2 | 3 | 4

ANSWERS:
  1. ...
  2. ...
  ...
  40. ...

LIMITATIONS:
  - ...

USER_ACTIONS_REQUIRED:
  - ...

PROPOSED_WORKING_MODEL:
  - ...
```

The agent should verify facts from its real environment wherever possible instead of answering hypothetically.

## 10. Qualification outcome

The Architect / Lead Agent reviews the response and assigns a responsibility level per packet. The assigned level may increase after demonstrated successful work or decrease after scope, evidence or coordination failures.

The goal is not to keep the second agent subordinate. The goal is to give it the **maximum responsibility it can safely and productively carry** while preserving architecture, evidence and merge quality.
