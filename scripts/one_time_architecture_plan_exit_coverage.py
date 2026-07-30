from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def replace_exact(path: str, old: str, new: str, *, expected: int = 1) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{path}: expected {expected} exact matches, found {count}")
    target.write_text(text.replace(old, new), encoding="utf-8")


# Architecture stage table and accountability.
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "public-surface/fan-out calibration, measured consolidation at step 13 and remeasurement at steps 20 and 23",
    "public-surface/fan-out calibration and removal of the three direct lint exceptions at step 13, measured consolidation at step 14, and remeasurement at steps 22 and 25",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "destructive owner execution at step 11, Party tombstone/convergence at step 14, complete worker lifecycle at step 17, Phase 8A closure at step 19, and later-owner adoption without forced rewrites",
    "destructive owner execution at step 11, Party tombstone/convergence at step 15, complete worker lifecycle at step 19, Phase 8A closure at step 21, and later-owner adoption without forced rewrites",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "reusable worker conformance at step 15, real worker adoption at step 17, Phase 8A process proof at step 19, plus compatibility, deprecation and retirement enforcement",
    "reusable worker conformance at step 16, contract compatibility/deprecation/retirement enforcement at step 17, real worker adoption at step 19 and Phase 8A process proof at step 21",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "repository step 13 completes at least one behavior-neutral domain-cluster consolidation with measured improvement",
    "repository step 14 completes at least one behavior-neutral domain-cluster consolidation with measured improvement",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "repository step 16 implements and proves `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke`",
    "repository step 18 implements and proves `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke`",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "repository step 18 adds domain-oriented frontend proof, accessibility/browser evidence and restore/SLO/performance/security/supply-chain gates; step 19 proves Phase 8A closure",
    "repository step 20 adds domain-oriented frontend proof, accessibility/browser evidence and restore/SLO/performance/security/supply-chain gates; step 21 proves Phase 8A closure",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| B — dependency, crate and exception governance | steps 12, 13, 20 and 23, plus every structural preflight | no unjustified package growth; calibrated dependency/public-surface/fan-out budgets; expired exceptions zero; measured before/after reports |",
    "| B — dependency, crate and exception governance | steps 12, 13, 14, 22 and 25, plus every structural preflight | step 13 completes calibrated dependency/public-surface/fan-out governance and removes the three direct lint exceptions; no unjustified package growth; expired exceptions zero; measured before/after reports |",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| C — golden owner package and persistence model | steps 11, 14, 17 and 19 | destructive execution, tombstone/convergence, worker lifecycle and Phase 8A closure preserve owner, tenant, RLS, audit and rollback boundaries |",
    "| C — golden owner package and persistence model | steps 11, 15, 19 and 21 | destructive execution, tombstone/convergence, worker lifecycle and Phase 8A closure preserve owner, tenant, RLS, audit and rollback boundaries |",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| E — affected-scope CI | preserved by every packet; rechecked at steps 20 and 23 | every changed path has explainable ownership and executable checks; unknown impact broadens fail closed |",
    "| E — affected-scope CI | preserved by every packet; rechecked at steps 22 and 25 | every changed path has explainable ownership and executable checks; unknown impact broadens fail closed |",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| F — generic conformance and contract lifecycle | steps 15, 17 and 19 | reusable worker conformance is adopted by a real worker; contract compatibility/deprecation/retirement evidence remains enforceable |",
    "| F — generic conformance and contract lifecycle | steps 16, 17, 19 and 21 | reusable worker conformance is adopted by a real worker; published-version compatibility, deprecation telemetry, consumer migration and retirement gates are permanently enforced |",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| G — transitional consolidation | **step 13** |",
    "| G — transitional consolidation | **step 14** |",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| H — reproducible environment and navigation | **step 16** |",
    "| H — reproducible environment and navigation | **step 18** |",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| I — frontend and operations parity | steps 18 and 19 |",
    "| I — frontend and operations parity | steps 20 and 21 |",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| Extensibility cost | 12, 13, 15, 17, 21 and 22 | ordinary capabilities touch one owner closure, zero generic-runtime files, zero new crates and no unrelated workflows; two contrasting later expert-domain waves keep that cost bounded as module count grows |",
    "| Extensibility cost | 12, 14, 16, 19, 23 and 24 | ordinary capabilities touch one owner closure, zero generic-runtime files, zero new crates and no unrelated workflows; two contrasting later expert-domain waves keep that cost bounded as module count grows |",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| Developer comprehension | 16 plus permanent Stage A guards |",
    "| Developer comprehension | 18 plus permanent Stage A guards |",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| Build and CI scalability | 12, 13, 15, 20, 21 and 22 |",
    "| Build and CI scalability | 12, 13, 14, 16, 22, 23 and 24 |",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "| Local development reproducibility | 16 |",
    "| Local development reproducibility | 18 |",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "Repository step 20 is a Phase 8A architecture measurement checkpoint. It cannot by itself declare architecture 10/10 or close issue #194. The earliest final claim is repository step 23, after two contrasting later expert-domain waves at steps 21 and 22",
    "Repository step 22 is a Phase 8A architecture measurement checkpoint. It cannot by itself declare architecture 10/10 or close issue #194. The earliest final claim is repository step 25, after two contrasting later expert-domain waves at steps 23 and 24",
)

old_sequence = """12. complete first-party contribution aggregation for all currently active owners without behavior changes;
13. first measured behavior-neutral transitional domain-cluster consolidation;
14. Party tombstone, no-orphan proof and projection/search/cache convergence;
15. reusable generic worker conformance suite;
16. deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke`;
17. Customer Privacy worker, disable/uninstall fail-closed semantics and complete process/end-to-end acceptance;
18. Phase 8A frontend, accessibility, browser, restore, SLO, performance, security and supply-chain evidence;
19. Phase 8A closure;
20. Phase 8A architecture remeasurement, remaining-gate review and publication of the measured Phase 8B extension baseline — **not a final 10/10 declaration**;
21. first Phase 8B expert-domain wave proving bounded extension cost;
22. second contrasting expert-domain wave proving bounded extension cost as module count grows;
23. final architecture 10/10 closure review only when every section 12 criterion is mechanically proven."""
new_sequence = """12. complete first-party contribution aggregation for all currently active owners without behavior changes;
13. complete calibrated dependency, Rust public-surface, reverse-fan-out and exception governance, including removal of the three direct lint exceptions;
14. first measured behavior-neutral transitional domain-cluster consolidation;
15. Party tombstone, no-orphan proof and projection/search/cache convergence;
16. reusable generic worker conformance suite;
17. contract compatibility, deprecation, consumer-migration and retirement lifecycle enforcement;
18. deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke`;
19. Customer Privacy worker, disable/uninstall fail-closed semantics and complete process/end-to-end acceptance;
20. Phase 8A frontend, accessibility, browser, restore, SLO, performance, security and supply-chain evidence;
21. Phase 8A closure;
22. Phase 8A architecture remeasurement, remaining-gate review and publication of the measured Phase 8B extension baseline — **not a final 10/10 declaration**;
23. first Phase 8B expert-domain wave proving bounded extension cost;
24. second contrasting expert-domain wave proving bounded extension cost as module count grows;
25. final architecture 10/10 closure review only when every section 12 criterion is mechanically proven."""
replace_exact("docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md", old_sequence, new_sequence)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "Repository step 12 may be delivered as sequential bounded behavior-neutral owner batches, but it remains one unfinished master step until every currently active first-party owner satisfies section 6 completion evidence. Repository step 20 publishes measurements and remaining blockers; it cannot waive the two later-wave requirement or close issue #194.",
    "Repository step 12 may be delivered as sequential bounded behavior-neutral owner batches, but it remains one unfinished master step until every currently active first-party owner satisfies section 6 completion evidence. Repository step 13 is a separate behavior-neutral governance packet and must close the currently named dependency/public-surface/fan-out and direct-lint-exception debt rather than defer it to measurement. Repository step 17 separately enforces the published contract lifecycle. Repository step 22 publishes measurements and remaining blockers; it cannot waive the two later-wave requirement or close issue #194.",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "Repository step 20 performs the first full remeasurement after Phase 8A, but it cannot close issue #194 because the program also requires two later contrasting expert-domain waves. Repository step 23 is the earliest final closure review",
    "Repository step 22 performs the first full remeasurement after Phase 8A, but it cannot close issue #194 because the program also requires two later contrasting expert-domain waves. Repository step 25 is the earliest final closure review",
)

# Roadmap.
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "blocked on repository steps 1–20 and completed 8A; first extension wave is step 21",
    "blocked on repository steps 1–22 and completed 8A; first extension wave is step 23",
)
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "repository step 16 owns local lifecycle completion",
    "repository step 18 owns local lifecycle completion",
)
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "repository step 15 owns worker conformance and step 17 proves real adoption",
    "repository step 16 owns worker conformance, step 17 owns contract lifecycle enforcement and step 19 proves real worker adoption",
)
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "repository step 13 owns the first measured behavior-neutral consolidation",
    "repository step 14 owns the first measured behavior-neutral consolidation",
)
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "repository step 18 owns frontend/operations evidence and step 19 proves Phase 8A closure",
    "repository step 20 owns frontend/operations evidence and step 21 proves Phase 8A closure",
)
old_mapping = """| 13 — measured consolidation | G | B, D |
| 14 — tombstone/no-orphan/convergence | C | F |
| 15 — generic worker conformance | F | E |
| 16 — local lifecycle commands | H | A |
| 17 — Customer Privacy worker and full E2E | C, F | D, H |
| 18 — frontend and operations evidence | I | E |
| 19 — Phase 8A closure | C, F, I | A, E |
| 20 — Phase 8A architecture remeasurement | all stages | measurement only; no automatic 10/10 claim |
| 21–22 — contrasting later expert-domain waves | C, D | B, E, F, H, I |
| 23 — final 10/10 closure review | all stages | succeeds only if every normative criterion is proven |"""
new_mapping = """| 13 — dependency/public-surface/fan-out/exception governance completion | B | A, E |
| 14 — measured consolidation | G | B, D |
| 15 — tombstone/no-orphan/convergence | C | F |
| 16 — generic worker conformance | F | E |
| 17 — contract lifecycle enforcement | F | A, E |
| 18 — local lifecycle commands | H | A |
| 19 — Customer Privacy worker and full E2E | C, F | D, H |
| 20 — frontend and operations evidence | I | E |
| 21 — Phase 8A closure | C, F, I | A, E |
| 22 — Phase 8A architecture remeasurement | all stages | measurement only; no automatic 10/10 claim |
| 23–24 — contrasting later expert-domain waves | C, D | B, E, F, H, I |
| 25 — final 10/10 closure review | all stages | succeeds only if every normative criterion is proven |"""
replace_exact("docs/IMPLEMENTATION_ROADMAP.md", old_mapping, new_mapping)
old_roadmap_sequence = """13. **Repository step 13 — first measured behavior-neutral transitional domain-cluster consolidation.**
14. **Repository step 14 — Party tombstone, no-orphan proof and projection/search/cache convergence.**
15. **Repository step 15 — reusable generic worker conformance.**
16. **Repository step 16 — deterministic local lifecycle commands.**
17. **Repository step 17 — Customer Privacy worker, disable/uninstall fail-closed semantics and full process acceptance.**
18. **Repository step 18 — Phase 8A frontend and operations evidence.**
19. **Repository step 19 — Phase 8A closure.**
20. **Repository step 20 — Phase 8A architecture remeasurement; not a final 10/10 declaration.**
21. **Repository step 21 — first Phase 8B expert-domain wave.**
22. **Repository step 22 — second contrasting expert-domain wave.**
23. **Repository step 23 — final architecture 10/10 closure review only after every criterion is mechanically proven.**"""
new_roadmap_sequence = """13. **Repository step 13 — complete calibrated dependency, Rust public-surface, reverse-fan-out and exception governance, including removal of the three direct lint exceptions.**
14. **Repository step 14 — first measured behavior-neutral transitional domain-cluster consolidation.**
15. **Repository step 15 — Party tombstone, no-orphan proof and projection/search/cache convergence.**
16. **Repository step 16 — reusable generic worker conformance.**
17. **Repository step 17 — contract compatibility, deprecation, consumer-migration and retirement lifecycle enforcement.**
18. **Repository step 18 — deterministic local lifecycle commands.**
19. **Repository step 19 — Customer Privacy worker, disable/uninstall fail-closed semantics and full process acceptance.**
20. **Repository step 20 — Phase 8A frontend and operations evidence.**
21. **Repository step 21 — Phase 8A closure.**
22. **Repository step 22 — Phase 8A architecture remeasurement; not a final 10/10 declaration.**
23. **Repository step 23 — first Phase 8B expert-domain wave.**
24. **Repository step 24 — second contrasting expert-domain wave.**
25. **Repository step 25 — final architecture 10/10 closure review only after every criterion is mechanically proven.**"""
replace_exact("docs/IMPLEMENTATION_ROADMAP.md", old_roadmap_sequence, new_roadmap_sequence)
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "blocked on repository steps 1–20 and completed Phase 8A. Repository step 21 begins the first measured extension wave; step 22 must provide a contrasting second wave before the final step 23 architecture closure review.",
    "blocked on repository steps 1–22 and completed Phase 8A. Repository step 23 begins the first measured extension wave; step 24 must provide a contrasting second wave before the final step 25 architecture closure review.",
)
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "Repository step 20 remeasures the Phase 8A architecture but cannot itself claim architecture 10/10. Final closure is no earlier than step 23",
    "Repository step 22 remeasures the Phase 8A architecture but cannot itself claim architecture 10/10. Final closure is no earlier than step 25",
)

# Phase 8 plan.
old_phase_sequence = """13. repository step 13 — first measured behavior-neutral consolidation;
14. repository step 14 — Party tombstone, no-orphan proof and projection/search/cache convergence;
15. repository step 15 — reusable generic worker conformance;
16. repository step 16 — deterministic local lifecycle commands;
17. repository step 17 — Customer Privacy worker, disable/uninstall fail-closed semantics and complete process/end-to-end acceptance;
18. repository step 18 — Phase 8A frontend and operations evidence;
19. repository step 19 — Phase 8A closure;
20. repository step 20 — Phase 8A architecture remeasurement and remaining-gate review, not a final 10/10 declaration;
21. repository step 21 — first Phase 8B expert-domain wave;
22. repository step 22 — second contrasting expert-domain wave;
23. repository step 23 — final architecture 10/10 closure review only after every criterion is mechanically proven."""
new_phase_sequence = """13. repository step 13 — complete calibrated dependency, Rust public-surface, reverse-fan-out and exception governance, including removal of the three direct lint exceptions;
14. repository step 14 — first measured behavior-neutral consolidation;
15. repository step 15 — Party tombstone, no-orphan proof and projection/search/cache convergence;
16. repository step 16 — reusable generic worker conformance;
17. repository step 17 — contract compatibility, deprecation, consumer-migration and retirement lifecycle enforcement;
18. repository step 18 — deterministic local lifecycle commands;
19. repository step 19 — Customer Privacy worker, disable/uninstall fail-closed semantics and complete process/end-to-end acceptance;
20. repository step 20 — Phase 8A frontend and operations evidence;
21. repository step 21 — Phase 8A closure;
22. repository step 22 — Phase 8A architecture remeasurement and remaining-gate review, not a final 10/10 declaration;
23. repository step 23 — first Phase 8B expert-domain wave;
24. repository step 24 — second contrasting expert-domain wave;
25. repository step 25 — final architecture 10/10 closure review only after every criterion is mechanically proven."""
replace_exact("docs/PHASE8_DELIVERY_PLAN.md", old_phase_sequence, new_phase_sequence)
old_phase_mapping = """| 13 | G — measured behavior-neutral consolidation |
| 14 | C — tombstone, no-orphan and convergence persistence model |
| 15 | F — reusable worker conformance |
| 16 | H — reproducible local lifecycle |
| 17 | C + F — real Customer Privacy worker and lifecycle proof |
| 18 | I — frontend and operations parity |
| 19 | C + F + I — Phase 8A closure |
| 20 | all stages — measurement checkpoint only |
| 21–22 | later-domain proof that extension cost remains bounded |
| 23 | final architecture closure review |"""
new_phase_mapping = """| 13 | B — dependency/public-surface/fan-out/exception governance completion |
| 14 | G — measured behavior-neutral consolidation |
| 15 | C — tombstone, no-orphan and convergence persistence model |
| 16 | F — reusable worker conformance |
| 17 | F — contract compatibility/deprecation/retirement enforcement |
| 18 | H — reproducible local lifecycle |
| 19 | C + F — real Customer Privacy worker and lifecycle proof |
| 20 | I — frontend and operations parity |
| 21 | C + F + I — Phase 8A closure |
| 22 | all stages — measurement checkpoint only |
| 23–24 | later-domain proof that extension cost remains bounded |
| 25 | final architecture closure review |"""
replace_exact("docs/PHASE8_DELIVERY_PLAN.md", old_phase_mapping, new_phase_mapping)
replace_exact(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "Step 12 is architecture refactoring only and must not change Customer Privacy product behavior. Step 13 is physical consolidation only and remains separate from feature behavior. Step 20 cannot close issue #194 or declare 10/10 before steps 21 and 22 provide contrasting later-domain evidence.",
    "Step 12 is architecture refactoring only and must not change Customer Privacy product behavior. Step 13 is a separate governance packet that closes the currently named dependency/public-surface/fan-out and lint-exception debt. Step 14 is physical consolidation only and remains separate from feature behavior. Step 17 separately enforces contract lifecycle. Step 22 cannot close issue #194 or declare 10/10 before steps 23 and 24 provide contrasting later-domain evidence.",
)
replace_exact(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "until repository step 20 and Phase 8A closure permit step 21. Two contrasting later expert-domain waves at steps 21 and 22 are required before the step 23 final architecture 10/10 review.",
    "until repository step 22 and Phase 8A closure permit step 23. Two contrasting later expert-domain waves at steps 23 and 24 are required before the step 25 final architecture 10/10 review.",
)

# Project status.
replace_exact(
    "docs/PROJECT_STATUS.md",
    "Repository step 13 is the first measured behavior-neutral transitional domain-cluster consolidation.",
    "Repository step 13 completes calibrated dependency, Rust public-surface, reverse-fan-out and exception governance, including removal of the three direct lint exceptions. Repository step 14 is the first measured behavior-neutral transitional domain-cluster consolidation.",
)
replace_exact(
    "docs/PROJECT_STATUS.md",
    "Repository step 20 is a measured Phase 8A checkpoint, not an automatic success declaration. Architecture 10/10 requires the completed Stage D packet at step 12, measured consolidation at step 13, worker/local/frontend/operations closure through step 19, two contrasting later expert-domain waves at steps 21 and 22, and a separate final review at step 23.",
    "Repository step 22 is a measured Phase 8A checkpoint, not an automatic success declaration. Architecture 10/10 requires the completed Stage D packet at step 12, explicit Stage B governance closure at step 13, measured consolidation at step 14, worker/contract/local/frontend/operations closure through step 21, two contrasting later expert-domain waves at steps 23 and 24, and a separate final review at step 25.",
)
replace_exact(
    "docs/PROJECT_STATUS.md",
    "- Stage H is in progress: deterministic explain, packet-check and generated navigation are accepted through PR #228; local lifecycle commands are repository step 16.",
    "- Stage H is in progress: deterministic explain, packet-check and generated navigation are accepted through PR #228; local lifecycle commands are repository step 18.",
)
replace_exact(
    "docs/PROJECT_STATUS.md",
    "- Stage F is in progress: reusable generic mutation/query conformance is accepted through PR #235; generic worker conformance is repository step 15 and real Customer Privacy worker adoption is step 17; contract lifecycle enforcement remains open.",
    "- Stage F is in progress: reusable generic mutation/query conformance is accepted through PR #235; generic worker conformance is repository step 16, contract lifecycle enforcement is step 17 and real Customer Privacy worker adoption is step 19.",
)
replace_exact(
    "docs/PROJECT_STATUS.md",
    "- Stage G remains unstarted and is owned by repository step 13. Stage I remains incomplete and is owned by steps 18–19.",
    "- Stage G remains unstarted and is owned by repository step 14. Stage I remains incomplete and is owned by steps 20–21.",
)
old_status_tail = """-> 13. first measured behavior-neutral transitional domain-cluster consolidation
-> 14. Party tombstone, no-orphan proof and projection/search/cache convergence
-> 15. reusable generic worker conformance
-> 16. deterministic local lifecycle commands
-> 17. Customer Privacy worker and complete process/end-to-end acceptance
-> 18. Phase 8A frontend and operations evidence
-> 19. Phase 8A closure
-> 20. Phase 8A architecture remeasurement — checkpoint, not final 10/10
-> 21–22. two contrasting later expert-domain waves
-> 23. final architecture 10/10 closure review only if every criterion is mechanically proven"""
new_status_tail = """-> 13. complete dependency/public-surface/reverse-fan-out/exception governance
-> 14. first measured behavior-neutral transitional domain-cluster consolidation
-> 15. Party tombstone, no-orphan proof and projection/search/cache convergence
-> 16. reusable generic worker conformance
-> 17. contract compatibility, deprecation, consumer-migration and retirement enforcement
-> 18. deterministic local lifecycle commands
-> 19. Customer Privacy worker and complete process/end-to-end acceptance
-> 20. Phase 8A frontend and operations evidence
-> 21. Phase 8A closure
-> 22. Phase 8A architecture remeasurement — checkpoint, not final 10/10
-> 23–24. two contrasting later expert-domain waves
-> 25. final architecture 10/10 closure review only if every criterion is mechanically proven"""
replace_exact("docs/PROJECT_STATUS.md", old_status_tail, new_status_tail)

# Machine packet.
replace_exact(
    "repository-packet.json",
    '    "renumber the later sequence through a measured Phase 8A checkpoint, two contrasting expert-domain waves and a final 10/10 closure review",',
    '    "add explicit Stage B dependency/public-surface/fan-out/exception governance and Stage F contract-lifecycle enforcement packets",\n    "renumber the later sequence through a measured Phase 8A checkpoint, two contrasting expert-domain waves and a final 10/10 closure review",',
)
replace_exact(
    "repository-packet.json",
    '    "repository step 20 is a measurement checkpoint rather than an automatic 10/10 declaration",',
    '    "repository step 13 explicitly closes the named Stage B governance debt",\n    "repository step 17 explicitly enforces contract compatibility, deprecation, migration and retirement lifecycle",\n    "repository step 22 is a measurement checkpoint rather than an automatic 10/10 declaration",',
)
replace_exact(
    "repository-packet.json",
    '    "final architecture 10/10 requires two contrasting later expert-domain waves and a separate closure review",',
    '    "final architecture 10/10 requires two contrasting later expert-domain waves and a separate step 25 closure review",',
)

# Permanent tests.
replace_exact(
    "tests/test_repository_navigation.py",
    '            "repository step 20 is a measurement checkpoint rather than an automatic 10/10 declaration",',
    '            "repository step 22 is a measurement checkpoint rather than an automatic 10/10 declaration",',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '            "13. first measured behavior-neutral transitional domain-cluster consolidation;",\n            "16. deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke`;",\n            "20. Phase 8A architecture remeasurement, remaining-gate review and publication of the measured Phase 8B extension baseline — **not a final 10/10 declaration**;",\n            "21. first Phase 8B expert-domain wave proving bounded extension cost;",\n            "22. second contrasting expert-domain wave proving bounded extension cost as module count grows;",\n            "23. final architecture 10/10 closure review only when every section 12 criterion is mechanically proven.",',
    '            "13. complete calibrated dependency, Rust public-surface, reverse-fan-out and exception governance, including removal of the three direct lint exceptions;",\n            "14. first measured behavior-neutral transitional domain-cluster consolidation;",\n            "17. contract compatibility, deprecation, consumer-migration and retirement lifecycle enforcement;",\n            "18. deterministic local lifecycle commands: `doctor`, `bootstrap`, `dev-up`, `dev-reset`, `seed-demo` and `smoke`;",\n            "22. Phase 8A architecture remeasurement, remaining-gate review and publication of the measured Phase 8B extension baseline — **not a final 10/10 declaration**;",\n            "23. first Phase 8B expert-domain wave proving bounded extension cost;",\n            "24. second contrasting expert-domain wave proving bounded extension cost as module count grows;",\n            "25. final architecture 10/10 closure review only when every section 12 criterion is mechanically proven.",',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '            "20. repository step 20 — Phase 8A architecture remeasurement and remaining-gate review, not a final 10/10 declaration;",',
    '            "22. repository step 22 — Phase 8A architecture remeasurement and remaining-gate review, not a final 10/10 declaration;",',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '            "repository step 20 is a measurement checkpoint rather than an automatic 10/10 declaration",',
    '            "repository step 22 is a measurement checkpoint rather than an automatic 10/10 declaration",',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '            "two contrasting later expert-domain waves",\n            "step 23",',
    '            "two contrasting later expert-domain waves",\n            "contract compatibility, deprecation, consumer-migration and retirement lifecycle enforcement",\n            "removal of the three direct lint exceptions",\n            "step 25",',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '        self.assertIn("repository step 20 is a phase 8a architecture measurement checkpoint", self.plan.lower())\n        self.assertIn("repository step 23", self.plan.lower())',
    '        self.assertIn("repository step 22 is a phase 8a architecture measurement checkpoint", self.plan.lower())\n        self.assertIn("repository step 25", self.plan.lower())',
)
