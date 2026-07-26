#!/usr/bin/env python3
"""One-time synchronization for the PR #176 privacy owner-scope packet.

This script is intentionally temporary and must be removed with the temporary
validator before authoritative exact-head acceptance.
"""

from pathlib import Path


CHANGED: list[str] = []


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    if new in text:
        return
    count = text.count(old)
    if count != 1:
        raise SystemExit(
            f"expected one synchronization anchor in {path}, found {count}: {old[:120]!r}"
        )
    target.write_text(text.replace(old, new, 1), encoding="utf-8")
    if path not in CHANGED:
        CHANGED.append(path)


replace_once(
    "docs/PROJECT_STATUS.md",
    "16. `MODULE_CATALOG.md` — merged business-module readiness accounting.",
    "16. `MODULE_CATALOG.md` — merged business-module readiness accounting.\n"
    "17. `PRIVACY_OWNER_SCOPE_SHARED_SUPPORT_COMPARISON.md` — active two-implementation comparison and PR #176 acceptance contract.",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is Complete. Phase 8A.11 is In progress; six Customer Privacy runtime coordinates, the immutable owner-scope protocol foundation, nine contract-only owner contribution coordinates and the first authoritative non-runtime Parties owner implementation are merged through PR #156. Architecture scalability Phases A–E are accepted: static/runtime/step telemetry and the cache-free Rust decision through PR #167; contrasting Customer Accounts and Consents module-owned production contributions through PR #170; the mechanically narrow first-party aggregate through PR #171; explainable affected-scope iteration through PR #172; and the bounded Party/Account PostgreSQL isolation pilot through PR #173.**",
    "**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is Complete. Phase 8A.11 is In progress; six Customer Privacy runtime coordinates, the immutable owner-scope protocol foundation, nine contract-only owner contribution coordinates and two contrasting authoritative non-runtime owner implementations are merged through PR #175: Parties as a single-record owner and Consents as a relationship-traversed paginated owner. Architecture scalability Phases A–E are accepted through PR #173. Draft PR #176 is the active behavior-neutral packet extracting only query integrity, common lineage/registry/time/page-size validation, canonical Party generation proof and length-framed digests while retaining owner SQL, rehydration, pagination, evidence classification, response contracts and stable errors in the owners.**",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "- **8A.11 — In progress:** architecture, owner foundation, deterministic domain, canonical persistence, immutable public contracts, FORCE RLS persistence, four public mutations, two permission-aware queries, immutable owner-scope envelopes, nine owner-specific contract-only contribution coordinates and the first proven owner implementation are merged.",
    "- **8A.11 — In progress:** architecture, owner foundation, deterministic domain, canonical persistence, immutable public contracts, FORCE RLS persistence, four public mutations, two permission-aware queries, immutable owner-scope envelopes, nine owner-specific contract-only contribution coordinates and the contrasting Parties and Consents owner implementations are merged; PR #176 is extracting only behavior proven common by both.",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "`second contrasting privacy owner -> compare with Parties -> extract only proven shared privacy protocol support -> remaining owner privacy contributions -> approval/restriction/legal-hold/plan/outcome/worker lifecycle -> Phase 8A closure -> 8B`",
    "`extract proven shared owner-scope support in PR #176 -> remaining owner privacy contributions -> sufficient owner set and scope discovery/planning -> approval/restriction/legal-hold/plan/outcome/worker lifecycle -> export/deletion/convergence -> Phase 8A closure -> 8B`",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "- PR #156 — first authoritative non-runtime Parties privacy scope owner implementation.",
    "- PR #156 — first authoritative non-runtime Parties privacy scope owner implementation;\n"
    "- PR #175 — contrasting authoritative non-runtime Consents privacy scope owner implementation with relationship traversal and keyset pagination.",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "PR #156 was accepted on unchanged source SHA `753acdb2ad2c25b343d0aae3413bb8b5c38581e2`, passed all 18 applicable workflows and was squash-merged as `4368b8c3710e05137b71ba999bf7f3497c0801c8`. It implements the Parties owner contribution in one tenant-bound `REPEATABLE READ, READ ONLY` transaction with transaction-scoped RLS, the shared topology advisory lock, exact generation and canonical-claim proof, strict Party rehydration, reference-only deterministic evidence and clean/reapplied malformed/cross-tenant/stale-lineage/no-write PostgreSQL acceptance. It remains contract-only/non-runtime.",
    "PR #156 was accepted on unchanged source SHA `753acdb2ad2c25b343d0aae3413bb8b5c38581e2`, passed all 18 applicable workflows and was squash-merged as `4368b8c3710e05137b71ba999bf7f3497c0801c8`. It implements the Parties owner contribution in one tenant-bound `REPEATABLE READ, READ ONLY` transaction with transaction-scoped RLS, the shared topology advisory lock, exact generation and canonical-claim proof, strict Party rehydration, reference-only deterministic evidence and clean/reapplied malformed/cross-tenant/stale-lineage/no-write PostgreSQL acceptance. It remains contract-only/non-runtime.\n\n"
    "PR #175 was accepted on unchanged source SHA `b492d5302b421942903be4eb0662522323b05106`, passed all 22 applicable permanent workflows and was squash-merged as `039d6461803208f6cb70ce0fbcfcaffaf59d7125`. It implements Consents as the contrasting multi-record owner through authoritative Party-to-Consent relationships, strict Consent rehydration, bounded keyset pagination, lineage-bound cursor evidence, immutable-required-evidence classification, clean rollback/reapply and repeated no-write PostgreSQL acceptance. It remains contract-only/non-runtime.\n\n"
    "Draft PR #176 compares PRs #156 and #175 and extracts only the shared behavior frozen in `PRIVACY_OWNER_SCOPE_SHARED_SUPPORT_COMPARISON.md`. No third owner, route, worker, migration, contract or runtime promotion belongs in that packet.",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "- **1 implemented owner contribution:** Parties, still contract-only/non-runtime;",
    "- **2 implemented owner contributions:** Parties and Consents, both still contract-only/non-runtime;",
)

replace_once(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "1. **8A.11 / #126 — In progress:** architecture/domain/contracts/FORCE-RLS foundation is merged through PR #145; `case.create`, `case.submit`, `case.subject.verify`, `case.get`, `case.cancel` and `case.list` are merged through PR #152; the owner-scope protocol foundation and first Parties implementation are merged through PR #156.\n"
    "2. **Architecture scalability runway — Complete for the next owner packet:** measured complexity/cache decision, Customer Accounts and Consents production-owner proofs, first-party aggregation, explainable affected-scope iteration and the bounded PostgreSQL isolation pilot are accepted through PR #173.\n"
    "3. **Next bounded slice:** implement a contrasting second privacy scope owner, preferably Consents, compare it with Parties and extract only behavior proven common by both implementations.\n"
    "4. **Remaining owner slices:** migrate the other privacy owner contributions one bounded owner at a time before runtime promotion.\n"
    "5. **Remaining 8A.11 lifecycle:** approval, restrictions, legal-hold/retention precedence, plan/outcome reads, orchestration, export/deletion/convergence and workers remain separate production packets.\n"
    "6. **Phase 8A closure:** only after the complete privacy/customer-master interaction baseline is merged and reconciled.\n"
    "7. **8B / #29:** starts only from the completed Phase 8A baseline.",
    "1. **8A.11 / #126 — In progress:** architecture/domain/contracts/FORCE-RLS foundation is merged through PR #145; `case.create`, `case.submit`, `case.subject.verify`, `case.get`, `case.cancel` and `case.list` are merged through PR #152; owner-scope contracts and the contrasting Parties and Consents implementations are merged through PR #175.\n"
    "2. **Architecture scalability runway — Complete:** measured complexity/cache decisions, module-owned contribution proofs, first-party aggregation, explainable affected-scope iteration and the bounded PostgreSQL isolation pilot are accepted through PR #173.\n"
    "3. **Active bounded slice — PR #176:** compare Parties and Consents and extract only proven common query integrity, lineage/registry/time/page-size validation, canonical Party generation proof and digest framing. Preserve owner SQL, rehydration, pagination, evidence classification, response contracts and stable errors; add no runtime route, worker, migration, contract or third owner.\n"
    "4. **Remaining owner slices:** implement the other privacy owner contributions one bounded owner at a time on the validated support boundary before runtime promotion.\n"
    "5. **Scope discovery and lifecycle:** assemble a sufficient owner set, then separately prove discovery/planning, approval, restrictions, legal-hold/retention precedence, plan/outcome reads, resumable orchestration, export/deletion/convergence and workers.\n"
    "6. **Phase 8A closure:** only after the complete privacy/customer-master interaction baseline is merged and reconciled.\n"
    "7. **8B / #29:** starts only from the completed Phase 8A baseline.",
)

replace_once(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "Owner-scope foundation and first implementation: PRs #154–#156  \n"
    "Accepted architecture runway: PRs #157–#173  \n"
    "Merged production inventory: 4 mutations + 2 permission-aware queries + 10 public non-runtime coordinates + 0 Customer Privacy workers  \n"
    "Next bounded production slice: a contrasting second privacy scope owner, preferably Consents, followed by two-implementation comparison before shared protocol extraction",
    "Owner-scope foundation and contrasting accepted implementations: PRs #154–#156 and #175  \n"
    "Accepted architecture runway: PRs #157–#173  \n"
    "Active behavior-neutral architecture packet: draft PR #176  \n"
    "Merged production inventory: 4 mutations + 2 permission-aware queries + 10 public non-runtime coordinates + 0 Customer Privacy workers  \n"
    "Next production promotion: intentionally not selected until shared support and the required remaining owner contributions are proven",
)
replace_once(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "The active program has completed the prerequisites for the second privacy owner packet:\n\n"
    "- PR #168 — Customer Accounts golden module-owned production contribution;\n"
    "- PR #170 — contrasting Consents module-owned production contribution;\n"
    "- PR #171 — mechanically narrow first-party aggregation with no duplicated owner catalog;\n"
    "- PR #172 — explainable affected-scope iteration with fail-broad shared/unknown impact;\n"
    "- PR #173 — two repeated independent Party/Account PostgreSQL process-isolation samples while retaining the sequential control lane.\n\n"
    "These packets improve production composition and delivery scalability. They do not implement a second privacy scope owner and do not authorize shared privacy protocol extraction before that contrasting implementation exists.",
    "The active program completed the architecture scalability prerequisites through PR #173:\n\n"
    "- PR #168 — Customer Accounts golden module-owned production contribution;\n"
    "- PR #170 — contrasting Consents module-owned production contribution;\n"
    "- PR #171 — mechanically narrow first-party aggregation with no duplicated owner catalog;\n"
    "- PR #172 — explainable affected-scope iteration with fail-broad shared/unknown impact;\n"
    "- PR #173 — two repeated independent Party/Account PostgreSQL process-isolation samples while retaining the sequential control lane.\n\n"
    "PR #175 then completed the required second privacy-scope implementation on accepted source `b492d5302b421942903be4eb0662522323b05106` and merge `039d6461803208f6cb70ce0fbcfcaffaf59d7125`. Consents is materially contrasting: it traverses authoritative Party-to-Consent relationships, strictly rehydrates multiple Consent records, emits immutable-required-evidence references and uses bounded keyset pagination. All 22 applicable permanent workflows passed on the unchanged accepted source.\n\n"
    "Draft PR #176 is now the only active packet in this dependency lane. It is behavior-neutral and extracts only mechanics proven identical in Parties and Consents. Its exact comparison, exclusions, deferred runtime decisions and gate are defined in `PRIVACY_OWNER_SCOPE_SHARED_SUPPORT_COMPARISON.md`. It does not select or promote a Customer Privacy production coordinate.",
)

replace_once(
    "docs/MODULE_CATALOG.md",
    "Phase 8A.11 / issue #126 remains **In progress**. PRs #140–#145 merged the architecture freeze, owner foundation, deterministic domain, canonical persistence, immutable public contracts and FORCE RLS proof.",
    "Phase 8A.11 / issue #126 remains **In progress**. PRs #140–#145 merged the architecture freeze, owner foundation, deterministic domain, canonical persistence, immutable public contracts and FORCE RLS proof. PRs #154–#155 published the owner-scope protocol and nine exact contract-only owner coordinates; PRs #156 and #175 accepted the contrasting Parties and Consents owner implementations. Draft PR #176 is extracting only their proven common support without changing module readiness or runtime inventory.",
)
replace_once(
    "docs/MODULE_CATALOG.md",
    "- PR #152 / `customer_privacy.case.list@1.0.0` — accepted source `9de6048f951c0797a94871457d2bdd73357aee59`, merge `26f5b4644c935001806343b2feaf802a78c90eae`.",
    "- PR #152 / `customer_privacy.case.list@1.0.0` — accepted source `9de6048f951c0797a94871457d2bdd73357aee59`, merge `26f5b4644c935001806343b2feaf802a78c90eae`.\n\n"
    "Accepted contract-only owner-scope evidence:\n\n"
    "- PR #156 — authoritative single-record Parties contribution in one tenant-bound repeatable read-only snapshot;\n"
    "- PR #175 — authoritative multi-record Consents contribution through owner relationships and bounded keyset pagination;\n"
    "- both remain non-runtime and add no Customer Privacy worker or public ingress;\n"
    "- PR #176 is a behavior-neutral shared-support extraction and does not change this module readiness classification.",
)
replace_once(
    "docs/MODULE_CATALOG.md",
    "- 8A.11 / #126 — Customer Privacy. Four production mutations and two permission-aware queries are merged independently; the remaining lifecycle stays incomplete and the next bounded packet is not yet selected.",
    "- 8A.11 / #126 — Customer Privacy. Four production mutations and two permission-aware queries are merged independently; owner-scope contracts plus Parties and Consents implementations are accepted; draft PR #176 is the active behavior-neutral shared-support packet. Remaining owners, discovery/planning, approval, restrictions, legal holds, execution, export/deletion and convergence remain incomplete.",
)

replace_once(
    "docs/DELIVERY_GOVERNANCE.md",
    "6. **8A.11 / #126** — Customer Privacy Lifecycle, Restriction, Deletion and Legal Hold — **In progress**; foundation plus `case.create`, `case.submit`, `case.subject.verify`, `case.get`, `case.cancel` and `case.list` are merged through PR #152.\n"
    "7. **Next bounded 8A.11 slice** — not selected until approval, restriction placement, legal-hold precedence, plan/outcome reads and worker dependencies are compared.\n"
    "8. **Phase 8A closure** — after merged privacy interaction proof.\n"
    "9. **8B / #29** — Product Catalog, Pricing, CPQ and Quote-to-Revenue.",
    "6. **8A.11 / #126** — Customer Privacy Lifecycle, Restriction, Deletion and Legal Hold — **In progress**; six runtime coordinates are merged through PR #152, owner-scope contracts through PR #155 and the contrasting Parties/Consents owner implementations through PR #175.\n"
    "7. **Active bounded architecture packet / PR #176** — behavior-neutral extraction of only support proven common by Parties and Consents; no runtime coordinate, worker, migration, contract or third owner.\n"
    "8. **Remaining owner-scope packets** — one authoritative owner at a time on the accepted shared-support boundary.\n"
    "9. **Remaining 8A.11 lifecycle packets** — sufficient owner set, scope discovery/planning, approval, restrictions, legal-hold/retention precedence, plan/outcome reads, resumable execution, export/deletion and convergence.\n"
    "10. **Phase 8A closure** — after merged privacy interaction proof.\n"
    "11. **8B / #29** — Product Catalog, Pricing, CPQ and Quote-to-Revenue.",
)
replace_once(
    "docs/DELIVERY_GOVERNANCE.md",
    "As of 2026-07-22:",
    "As of 2026-07-26:",
)
replace_once(
    "docs/DELIVERY_GOVERNANCE.md",
    "- Phase 8A.11 / #126 is **In progress**. PR #152 accepted source `9de6048f951c0797a94871457d2bdd73357aee59` and merge `26f5b4644c935001806343b2feaf802a78c90eae` established four runtime Customer Privacy mutations, two permission-aware queries, ten public non-runtime coordinates and zero Customer Privacy workers.\n"
    "- The next Customer Privacy production coordinate is not selected until this post-merge synchronization is complete and the remaining trust boundaries are compared.\n"
    "- Phase 8B / #29 remains planned after Phase 8A closure.",
    "- Phase 8A.11 / #126 is **In progress**. PR #152 established four runtime Customer Privacy mutations, two permission-aware queries, ten public non-runtime coordinates and zero Customer Privacy workers. PR #175 accepted the second contrasting privacy-scope owner on source `b492d5302b421942903be4eb0662522323b05106` and merge `039d6461803208f6cb70ce0fbcfcaffaf59d7125`; Parties and Consents remain contract-only/non-runtime.\n"
    "- Draft PR #176 is the active dependency-lane packet. It must synchronize documentation, preserve exact owner behavior and errors, remove its temporary validator and pass all applicable checks on one unchanged SHA before merge.\n"
    "- The next Customer Privacy production coordinate remains intentionally unselected until shared support and the required remaining owner implementations establish a sufficient discovery/planning boundary.\n"
    "- Phase 8B / #29 remains planned after Phase 8A closure.",
)

replace_once(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "Status: **Proposed normative architecture evolution plan**  \nAudit baseline: **2026-07-25**  ",
    "Status: **Normative architecture evolution plan**  \n"
    "Audit baseline: **2026-07-25**  \n"
    "Execution status: **Scalability runway accepted through PR #173; two privacy owners accepted through PR #175; shared-support extraction active in draft PR #176.**  ",
)

print("synchronized:", ", ".join(CHANGED) if CHANGED else "already current")
