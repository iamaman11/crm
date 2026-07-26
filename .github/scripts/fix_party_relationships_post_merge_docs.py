from pathlib import Path

ACCEPTED = (
    "PR #183 accepted unchanged user-authored source `a431185e01e95dfeffcf7d9c9a440afc8f0c9a57`, "
    "passed all 25 applicable permanent workflows and was squash-merged as "
    "`9ad2aa91321e9edb54cab98218f93143923ef33f`. Party Relationships proves the fifth "
    "authoritative owner shape through strict two-endpoint matching, directional/reciprocal and "
    "temporal domain rehydration, bounded owner-specific pagination/cursors, reference-only "
    "evidence, response-byte non-disclosure, clean rollback/schema-removal/reapply acceptance and "
    "zero query-side writes. It remains contract-only/non-runtime and changes no shared-support behavior."
)


def read(path: str) -> str:
    return Path(path).read_text()


def write(path: str, text: str) -> None:
    Path(path).write_text(text)


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor, found {count}")
    return text.replace(old, new, 1)


def remove_appended_acceptance(text: str) -> str:
    for prefix in ("\n\n\n", "\n\n"):
        candidate = prefix + ACCEPTED
        if candidate in text:
            return text.replace(candidate, "", 1).rstrip() + "\n"
    return text


# Architecture scalability status.
path = "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md"
text = read(path)
text = replace_once(
    text,
    "behavior-neutral shared support accepted through PR #176 and mechanically proven with Customer Accounts and Contact Points as its third and fourth consumers.",
    "behavior-neutral shared support accepted through PR #176 and mechanically proven with Customer Accounts, Contact Points and Party Relationships as its third, fourth and fifth consumers.",
    path,
)
write(path, text)

# Delivery governance active lane and accepted baseline.
path = "docs/DELIVERY_GOVERNANCE.md"
text = remove_appended_acceptance(read(path))
text = replace_once(
    text,
    "6. **8A.11 / #126** — Customer Privacy Lifecycle, Restriction, Deletion and Legal Hold — **In progress**; six runtime coordinates are merged through PR #152, owner-scope contracts through PR #155 and authoritative Parties, Consents, Customer Accounts, Contact Points and Party Relationships owner implementations through PR #181.",
    "6. **8A.11 / #126** — Customer Privacy Lifecycle, Restriction, Deletion and Legal Hold — **In progress**; six runtime coordinates are merged through PR #152, owner-scope contracts through PR #155 and authoritative Parties, Consents, Customer Accounts, Contact Points and Party Relationships owner implementations through PR #183.",
    path,
)
text = replace_once(
    text,
    "7. **Shared owner-scope support / PR #176** — **Complete**; accepted source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, merge `80411d54a3ca45a783d982152c5cd8317f1fd9bd`; PRs #179 and #181 add only the third and fourth mechanically proven consumers.",
    "7. **Shared owner-scope support / PR #176** — **Complete**; accepted source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, merge `80411d54a3ca45a783d982152c5cd8317f1fd9bd`; PRs #179, #181 and #183 add only the third, fourth and fifth mechanically proven consumers.",
    path,
)
old_block = """9. **Contact Points owner-scope packet / PR #181** — **Complete**; accepted source `00c5b940326b14f5e4aab7d8c8b467ee688f6c9c`, merge `96cd0cf548310592a0718c97242a724a29717a72`, 24/24 applicable permanent workflows.
10. **Next bounded owner-scope packet — Identity Resolution** — contract-only/non-runtime authoritative two-endpoint temporal relationship contribution using accepted shared support.
11. **Remaining owner-scope packets** — one authoritative owner at a time on the accepted shared-support boundary.
12. **Remaining 8A.11 lifecycle packets** — sufficient owner set, scope discovery/planning, approval, restrictions, legal-hold/retention precedence, plan/outcome reads, resumable execution, export/deletion and convergence.
13. **Phase 8A closure** — after merged privacy interaction proof.
14. **8B / #29** — Product Catalog, Pricing, CPQ and Quote-to-Revenue."""
new_block = """9. **Contact Points owner-scope packet / PR #181** — **Complete**; accepted source `00c5b940326b14f5e4aab7d8c8b467ee688f6c9c`, merge `96cd0cf548310592a0718c97242a724a29717a72`, 24/24 applicable permanent workflows.
10. **Party Relationships owner-scope packet / PR #183** — **Complete**; accepted source `a431185e01e95dfeffcf7d9c9a440afc8f0c9a57`, merge `9ad2aa91321e9edb54cab98218f93143923ef33f`, 25/25 applicable permanent workflows.
11. **Next bounded owner-scope packet — Identity Resolution** — contract-only/non-runtime contribution across authoritative candidate-case and merge-operation families with bounded alias-aware lineage closure.
12. **Remaining owner-scope packets** — one authoritative owner at a time on the accepted shared-support boundary.
13. **Remaining 8A.11 lifecycle packets** — sufficient owner set, scope discovery/planning, approval, restrictions, legal-hold/retention precedence, plan/outcome reads, resumable execution, export/deletion and convergence.
14. **Phase 8A closure** — after merged privacy interaction proof.
15. **8B / #29** — Product Catalog, Pricing, CPQ and Quote-to-Revenue."""
text = replace_once(text, old_block, new_block, path)
text = replace_once(
    text,
    "- Phase 8A.11 / #126 is **In progress**. PR #152 established four runtime Customer Privacy mutations, two permission-aware queries, ten public non-runtime coordinates and zero Customer Privacy workers. PRs #156, #175, #179 and #181 accepted Parties, Consents, Customer Accounts, Contact Points and Party Relationships privacy-scope owners; all remain contract-only/non-runtime.",
    "- Phase 8A.11 / #126 is **In progress**. PR #152 established four runtime Customer Privacy mutations, two permission-aware queries, ten public non-runtime coordinates and zero Customer Privacy workers. PRs #156, #175, #179, #181 and #183 accepted Parties, Consents, Customer Accounts, Contact Points and Party Relationships privacy-scope owners; all remain contract-only/non-runtime.",
    path,
)
text = replace_once(
    text,
    "- PR #176 accepted shared owner-scope support on unchanged source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, passed 21/21 applicable permanent workflows and merged as `80411d54a3ca45a783d982152c5cd8317f1fd9bd`. PRs #179 and #181 added Customer Accounts and Contact Points as the third and fourth mechanically restricted consumers without changing shared semantics.",
    "- PR #176 accepted shared owner-scope support on unchanged source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, passed 21/21 applicable permanent workflows and merged as `80411d54a3ca45a783d982152c5cd8317f1fd9bd`. PRs #179, #181 and #183 added Customer Accounts, Contact Points and Party Relationships as the third, fourth and fifth mechanically restricted consumers without changing shared semantics.",
    path,
)
anchor = "- PR #181 accepted unchanged user-authored source `00c5b940326b14f5e4aab7d8c8b467ee688f6c9c`, passed 24/24 applicable permanent workflows and merged as `96cd0cf548310592a0718c97242a724a29717a72`."
text = replace_once(
    text,
    anchor,
    anchor + "\n- PR #183 accepted unchanged user-authored source `a431185e01e95dfeffcf7d9c9a440afc8f0c9a57`, passed 25/25 applicable permanent workflows and merged as `9ad2aa91321e9edb54cab98218f93143923ef33f`.",
    path,
)
text = replace_once(
    text,
    "- Party Relationships is the next bounded owner-scope implementation; no Customer Privacy production coordinate is selected until the required owner set establishes a sufficient discovery/planning boundary.",
    "- Identity Resolution is the next bounded owner-scope implementation; no Customer Privacy production coordinate is selected until the required owner set establishes a sufficient discovery/planning boundary.",
    path,
)
write(path, text)

# Implementation roadmap active sequence.
path = "docs/IMPLEMENTATION_ROADMAP.md"
text = remove_appended_acceptance(read(path))
text = replace_once(
    text,
    "1. **8A.11 / #126 — In progress:** architecture/domain/contracts/FORCE-RLS foundation is merged through PR #145; `case.create`, `case.submit`, `case.subject.verify`, `case.get`, `case.cancel` and `case.list` are merged through PR #152; owner-scope contracts and authoritative Parties, Consents, Customer Accounts, Contact Points and Party Relationships implementations are merged through PR #181.",
    "1. **8A.11 / #126 — In progress:** architecture/domain/contracts/FORCE-RLS foundation is merged through PR #145; `case.create`, `case.submit`, `case.subject.verify`, `case.get`, `case.cancel` and `case.list` are merged through PR #152; owner-scope contracts and authoritative Parties, Consents, Customer Accounts, Contact Points and Party Relationships implementations are merged through PR #183.",
    path,
)
text = replace_once(
    text,
    "3. **Shared owner-scope support — Complete through PR #176:** accepted source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, merge `80411d54a3ca45a783d982152c5cd8317f1fd9bd`; only proven common mechanics were extracted and owner behavior remained unchanged. PRs #179 and #181 added Customer Accounts and Contact Points as the third and fourth mechanically permitted consumers without changing shared behavior.",
    "3. **Shared owner-scope support — Complete through PR #176:** accepted source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, merge `80411d54a3ca45a783d982152c5cd8317f1fd9bd`; only proven common mechanics were extracted and owner behavior remained unchanged. PRs #179, #181 and #183 added Customer Accounts, Contact Points and Party Relationships as the third, fourth and fifth mechanically permitted consumers without changing shared behavior.",
    path,
)
old_block = """5. **Contact Points owner slice — Complete through PR #181:** accepted source `00c5b940326b14f5e4aab7d8c8b467ee688f6c9c`, merge `96cd0cf548310592a0718c97242a724a29717a72`, 24/24 applicable permanent workflows; exact direct Party binding, strict Contact Point rehydration, bounded owner pagination and reference-only endpoint-value-free evidence remain contract-only/non-runtime.
6. **Next bounded owner slice — Identity Resolution:** implement `party_relationships.privacy.scope.contribute@1.0.0` through authoritative two-endpoint relationship state, strict temporal/directional rehydration, deterministic reference-only evidence and tenant/RLS/no-write proof. Add no runtime route, worker, production schema migration, Customer Privacy orchestration or speculative shared abstraction.
7. **Remaining owner slices:** implement the other privacy owner contributions one bounded owner at a time on the validated support boundary before runtime promotion.
8. **Scope discovery and lifecycle:** assemble a sufficient owner set, then separately prove discovery/planning, approval, restrictions, legal-hold/retention precedence, plan/outcome reads, resumable orchestration, export/deletion/convergence and workers.
9. **Phase 8A closure:** only after the complete privacy/customer-master interaction baseline is merged and reconciled."""
new_block = """5. **Contact Points owner slice — Complete through PR #181:** accepted source `00c5b940326b14f5e4aab7d8c8b467ee688f6c9c`, merge `96cd0cf548310592a0718c97242a724a29717a72`, 24/24 applicable permanent workflows; exact direct Party binding, strict Contact Point rehydration, bounded owner pagination and reference-only endpoint-value-free evidence remain contract-only/non-runtime.
6. **Party Relationships owner slice — Complete through PR #183:** accepted source `a431185e01e95dfeffcf7d9c9a440afc8f0c9a57`, merge `9ad2aa91321e9edb54cab98218f93143923ef33f`, 25/25 applicable permanent workflows; authoritative two-endpoint directional/reciprocal state, strict temporal rehydration, bounded owner pagination and reference-only relationship-semantic-free evidence remain contract-only/non-runtime.
7. **Next bounded owner slice — Identity Resolution:** implement `identity_resolution.privacy.scope.contribute@1.0.0` across authoritative duplicate-candidate cases and merge operations with bounded active-lineage resolution, heterogeneous pagination, reference-only evidence and tenant/RLS/no-write proof. Add no runtime route, worker, production schema migration, Customer Privacy orchestration or speculative shared abstraction.
8. **Remaining owner slices:** implement the other privacy owner contributions one bounded owner at a time on the validated support boundary before runtime promotion.
9. **Scope discovery and lifecycle:** assemble a sufficient owner set, then separately prove discovery/planning, approval, restrictions, legal-hold/retention precedence, plan/outcome reads, resumable orchestration, export/deletion/convergence and workers.
10. **Phase 8A closure:** only after the complete privacy/customer-master interaction baseline is merged and reconciled."""
text = replace_once(text, old_block, new_block, path)
write(path, text)

# Module catalog owner readiness accounting.
path = "docs/MODULE_CATALOG.md"
text = remove_appended_acceptance(read(path))
old = "Phase 8A.11 / issue #126 remains **In progress**. PRs #140–#145 merged the architecture freeze, owner foundation, deterministic domain, canonical persistence, immutable public contracts and FORCE RLS proof. PRs #154–#155 published the owner-scope protocol and nine exact contract-only owner coordinates; PRs #156, #175, #179 and #181 accepted authoritative Parties, Consents, Customer Accounts, Contact Points and Party Relationships owner implementations. PR #176 accepted their behavior-neutral common support; PRs #179 and #181 added the third and fourth mechanically permitted consumers without changing module readiness or runtime inventory. Party Relationships is selected as the next contract-only owner implementation."
new = "Phase 8A.11 / issue #126 remains **In progress**. PRs #140–#145 merged the architecture freeze, owner foundation, deterministic domain, canonical persistence, immutable public contracts and FORCE RLS proof. PRs #154–#155 published the owner-scope protocol and nine exact contract-only owner coordinates; PRs #156, #175, #179, #181 and #183 accepted authoritative Parties, Consents, Customer Accounts, Contact Points and Party Relationships owner implementations. PR #176 accepted their behavior-neutral common support; PRs #179, #181 and #183 added the third, fourth and fifth mechanically permitted consumers without changing module readiness or runtime inventory. Identity Resolution is selected as the next contract-only owner implementation through `identity_resolution.privacy.scope.contribute@1.0.0`."
text = replace_once(text, old, new, path)
write(path, text)

# Project status authoritative summary.
path = "docs/PROJECT_STATUS.md"
text = read(path)
text = replace_once(
    text,
    "17. `PRIVACY_OWNER_SCOPE_SHARED_SUPPORT_COMPARISON.md` — accepted shared-support boundary, compatibility baseline and current five-consumer proof.",
    "17. `PRIVACY_OWNER_SCOPE_SHARED_SUPPORT_COMPARISON.md` — accepted shared-support boundary, compatibility baseline and current five-consumer proof.\n18. `IDENTITY_RESOLUTION_PRIVACY_SCOPE_PACKET.md` — frozen entry packet for the next bounded authoritative owner contribution.",
    path,
)
old = "**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is Complete. Phase 8A.11 is In progress; six Customer Privacy runtime coordinates and nine contract-only owner contribution coordinates are published. Parties, Consents, Customer Accounts, Contact Points and Party Relationships now have accepted authoritative contract-only owner implementations, while shared support remains behavior-neutral and mechanically restricted to those five consumers. PR #181 accepted unchanged source `00c5b940326b14f5e4aab7d8c8b467ee688f6c9c` with 24/24 applicable permanent workflows and merged as `96cd0cf548310592a0718c97242a724a29717a72`. The next bounded owner is Identity Resolution through `identity_resolution.privacy.scope.contribute@1.0.0`; it must remain contract-only/non-runtime.**"
new = "**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is Complete. Phase 8A.11 is In progress; six Customer Privacy runtime coordinates and nine contract-only owner contribution coordinates are published. Parties, Consents, Customer Accounts, Contact Points and Party Relationships now have accepted authoritative contract-only owner implementations, while shared support remains behavior-neutral and mechanically restricted to those five consumers. PR #183 accepted unchanged source `a431185e01e95dfeffcf7d9c9a440afc8f0c9a57` with 25/25 applicable permanent workflows and merged as `9ad2aa91321e9edb54cab98218f93143923ef33f`. The next bounded owner is Identity Resolution through `identity_resolution.privacy.scope.contribute@1.0.0`; it must remain contract-only/non-runtime.**"
text = replace_once(text, old, new, path)
text = replace_once(
    text,
    "- **8A.11 — In progress:** architecture, owner foundation, deterministic domain, canonical persistence, immutable public contracts, FORCE RLS persistence, four public mutations, two permission-aware queries, immutable owner-scope envelopes and nine owner-specific contract-only contribution coordinates are merged; authoritative Parties, Consents, Customer Accounts, Contact Points and Party Relationships owner implementations are accepted through PR #181, and Party Relationships is selected as the next bounded owner implementation.",
    "- **8A.11 — In progress:** architecture, owner foundation, deterministic domain, canonical persistence, immutable public contracts, FORCE RLS persistence, four public mutations, two permission-aware queries, immutable owner-scope envelopes and nine owner-specific contract-only contribution coordinates are merged; authoritative Parties, Consents, Customer Accounts, Contact Points and Party Relationships owner implementations are accepted through PR #183, and Identity Resolution is selected as the next bounded owner implementation.",
    path,
)
text = replace_once(
    text,
    "Identity Resolution is selected as the next bounded contract-only owner implementation through `identity_resolution.privacy.scope.contribute@1.0.0`. Its packet must preserve authoritative two-endpoint temporal relationship semantics, strict rehydration, reference-only evidence, owner-specific matching/pagination/retention/errors and the accepted shared-support boundary without runtime promotion or Customer Privacy orchestration.",
    "Identity Resolution is selected as the next bounded contract-only owner implementation through `identity_resolution.privacy.scope.contribute@1.0.0`. Its packet freezes authoritative duplicate-candidate and merge-operation families, bounded alias-aware lineage resolution, heterogeneous pagination, strict rehydration, reference-only evidence and owner-specific retention/errors without runtime promotion or Customer Privacy orchestration.",
    path,
)
write(path, text)

# Phase 8 delivery plan: place PR #183 next to the accepted owner sequence.
path = "docs/PHASE8_DELIVERY_PLAN.md"
text = remove_appended_acceptance(read(path))
text = text.replace(
    "Owner-scope foundation and accepted implementations: PRs #154–#156, #175, #179 and #181",
    "Owner-scope foundation and accepted implementations: PRs #154–#156, #175, #179, #181 and #183",
    1,
)
anchor = "PR #181 accepted unchanged user-authored source `00c5b940326b14f5e4aab7d8c8b467ee688f6c9c` with all 24 applicable permanent workflows and squash-merged as `96cd0cf548310592a0718c97242a724a29717a72`. Contact Points now proves a fourth authoritative owner shape through exact persisted Party binding, strict endpoint lifecycle/verification rehydration, bounded owner-specific keyset pagination/cursors, deterministic reference-only evidence, response-byte exclusion of endpoint values and verification references, clean rollback/schema-removal/reapply acceptance and zero query-side writes. It remains contract-only/non-runtime and did not extend shared-support behavior."
insert = anchor + "\n\nPR #183 accepted unchanged user-authored source `a431185e01e95dfeffcf7d9c9a440afc8f0c9a57` with all 25 applicable permanent workflows and squash-merged as `9ad2aa91321e9edb54cab98218f93143923ef33f`. Party Relationships now proves a fifth authoritative owner shape through strict two-endpoint directional/reciprocal and temporal rehydration, bounded owner-specific keyset pagination/cursors, deterministic reference-only evidence, response-byte exclusion of counterpart Party and relationship semantics, clean rollback/schema-removal/reapply acceptance and zero query-side writes. It remains contract-only/non-runtime and did not extend shared-support behavior."
text = replace_once(text, anchor, insert, path)
write(path, text)
