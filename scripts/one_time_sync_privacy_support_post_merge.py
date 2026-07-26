#!/usr/bin/env python3
from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    if new in text:
        return
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected one anchor in {path}, found {count}: {old[:120]!r}")
    target.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    "docs/PROJECT_STATUS.md",
    "17. `PRIVACY_OWNER_SCOPE_SHARED_SUPPORT_COMPARISON.md` — active two-implementation comparison and PR #176 acceptance contract.",
    "17. `PRIVACY_OWNER_SCOPE_SHARED_SUPPORT_COMPARISON.md` — accepted two-implementation comparison and shared-support boundary.",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is Complete. Phase 8A.11 is In progress; six Customer Privacy runtime coordinates, the immutable owner-scope protocol foundation, nine contract-only owner contribution coordinates and two contrasting authoritative non-runtime owner implementations are merged through PR #175: Parties as a single-record owner and Consents as a relationship-traversed paginated owner. Architecture scalability Phases A–E are accepted through PR #173. Draft PR #176 is the active behavior-neutral packet extracting only query integrity, common lineage/registry/time/page-size validation, canonical Party generation proof and length-framed digests while retaining owner SQL, rehydration, pagination, evidence classification, response contracts and stable errors in the owners.**",
    "**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is Complete. Phase 8A.11 is In progress; six Customer Privacy runtime coordinates, nine contract-only owner contribution coordinates, the contrasting Parties and Consents owner implementations and their mechanically bounded shared support are merged through PR #176. PR #176 accepted unchanged source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8` with 21/21 applicable permanent workflows and merged as `80411d54a3ca45a783d982152c5cd8317f1fd9bd`. The next bounded owner implementation is Customer Accounts; it remains contract-only/non-runtime and must reuse shared support without extending it unless a new repeated seam is independently proven.**",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "- **8A.11 — In progress:** architecture, owner foundation, deterministic domain, canonical persistence, immutable public contracts, FORCE RLS persistence, four public mutations, two permission-aware queries, immutable owner-scope envelopes, nine owner-specific contract-only contribution coordinates and the contrasting Parties and Consents owner implementations are merged; PR #176 is extracting only behavior proven common by both.",
    "- **8A.11 — In progress:** architecture, owner foundation, deterministic domain, canonical persistence, immutable public contracts, FORCE RLS persistence, four public mutations, two permission-aware queries, immutable owner-scope envelopes, nine owner-specific contract-only contribution coordinates, the contrasting Parties/Consents implementations and their accepted shared support are merged through PR #176; Customer Accounts is selected as the next bounded owner implementation.",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "`extract proven shared owner-scope support in PR #176 -> remaining owner privacy contributions -> sufficient owner set and scope discovery/planning -> approval/restriction/legal-hold/plan/outcome/worker lifecycle -> export/deletion/convergence -> Phase 8A closure -> 8B`",
    "`Customer Accounts privacy owner contribution -> remaining owner privacy contributions -> sufficient owner set and scope discovery/planning -> approval/restriction/legal-hold/plan/outcome/worker lifecycle -> export/deletion/convergence -> Phase 8A closure -> 8B`",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "- PR #175 — contrasting authoritative non-runtime Consents privacy scope owner implementation with relationship traversal and keyset pagination.",
    "- PR #175 — contrasting authoritative non-runtime Consents privacy scope owner implementation with relationship traversal and keyset pagination;\n- PR #176 — behavior-neutral shared owner-scope support extraction with mechanical consumer restriction and compatibility proof.",
)
replace_once(
    "docs/PROJECT_STATUS.md",
    "Draft PR #176 compares PRs #156 and #175 and extracts only the shared behavior frozen in `PRIVACY_OWNER_SCOPE_SHARED_SUPPORT_COMPARISON.md`. No third owner, route, worker, migration, contract or runtime promotion belongs in that packet.",
    "PR #176 was accepted on unchanged source SHA `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, passed all 21 applicable permanent workflows and was squash-merged as `80411d54a3ca45a783d982152c5cd8317f1fd9bd`. It extracts only proven common request integrity, lineage/registry/time/page-size validation, canonical Party proof and digest framing; mechanically limits consumers to Parties and Consents; freezes owner error and digest compatibility; and changes no runtime inventory, contract, migration, worker or owner semantics.\n\nCustomer Accounts is selected as the next contract-only owner implementation because its production owner boundary and isolated PostgreSQL baseline are already accepted, while its Party-associated authoritative Account shape provides an independent reuse proof. The packet must not change shared support unless Customer Accounts proves a genuinely repeated seam absent from the accepted boundary.",
)

replace_once(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "3. **Active bounded slice — PR #176:** compare Parties and Consents and extract only proven common query integrity, lineage/registry/time/page-size validation, canonical Party generation proof and digest framing. Preserve owner SQL, rehydration, pagination, evidence classification, response contracts and stable errors; add no runtime route, worker, migration, contract or third owner.\n4. **Remaining owner slices:** implement the other privacy owner contributions one bounded owner at a time on the validated support boundary before runtime promotion.",
    "3. **Shared owner-scope support — Complete through PR #176:** accepted source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, merge `80411d54a3ca45a783d982152c5cd8317f1fd9bd`; only proven common mechanics were extracted and owner behavior remained unchanged.\n4. **Next bounded owner slice — Customer Accounts:** implement its existing contract-only contribution through authoritative Account/Party association reads, strict rehydration, deterministic reference-only evidence, tenant/RLS/no-write proof and the accepted shared support. Add no runtime route, worker, migration unless independently required, or speculative shared abstraction.\n5. **Remaining owner slices:** implement the other privacy owner contributions one bounded owner at a time on the validated support boundary before runtime promotion.",
)
replace_once(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "5. **Scope discovery and lifecycle:** assemble a sufficient owner set, then separately prove discovery/planning, approval, restrictions, legal-hold/retention precedence, plan/outcome reads, resumable orchestration, export/deletion/convergence and workers.\n6. **Phase 8A closure:** only after the complete privacy/customer-master interaction baseline is merged and reconciled.\n7. **8B / #29:** starts only from the completed Phase 8A baseline.",
    "6. **Scope discovery and lifecycle:** assemble a sufficient owner set, then separately prove discovery/planning, approval, restrictions, legal-hold/retention precedence, plan/outcome reads, resumable orchestration, export/deletion/convergence and workers.\n7. **Phase 8A closure:** only after the complete privacy/customer-master interaction baseline is merged and reconciled.\n8. **8B / #29:** starts only from the completed Phase 8A baseline.",
)

replace_once(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "Active behavior-neutral architecture packet: draft PR #176  ",
    "Shared owner-scope support: **Complete through PR #176**  ",
)
replace_once(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "Next production promotion: intentionally not selected until shared support and the required remaining owner contributions are proven",
    "Next bounded owner packet: Customer Accounts privacy-scope contribution, still contract-only/non-runtime",
)
replace_once(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "Draft PR #176 is now the only active packet in this dependency lane. It is behavior-neutral and extracts only mechanics proven identical in Parties and Consents. Its exact comparison, exclusions, deferred runtime decisions and gate are defined in `PRIVACY_OWNER_SCOPE_SHARED_SUPPORT_COMPARISON.md`. It does not select or promote a Customer Privacy production coordinate.",
    "PR #176 accepted unchanged source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8` with all 21 applicable permanent workflows and merged as `80411d54a3ca45a783d982152c5cd8317f1fd9bd`. It extracts only mechanics proven identical in Parties and Consents, preserves owner-specific contracts and runtime classifications, and mechanically restricts shared-support consumers.\n\nThe next bounded packet is the Customer Accounts privacy-scope owner implementation. It must remain contract-only/non-runtime, use authoritative Account/Party association reads and strict Account rehydration, preserve reference-only evidence, prove tenant/RLS/no-write behavior on clean and reapplied PostgreSQL, and consume the accepted shared support without speculative expansion.",
)

replace_once(
    "docs/MODULE_CATALOG.md",
    "Draft PR #176 is extracting only their proven common support without changing module readiness or runtime inventory.",
    "PR #176 accepted and merged their proven common support without changing module readiness or runtime inventory. Customer Accounts is selected as the next contract-only owner implementation.",
)
replace_once(
    "docs/MODULE_CATALOG.md",
    "- PR #176 is a behavior-neutral shared-support extraction and does not change this module readiness classification.",
    "- PR #176 / source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8` / merge `80411d54a3ca45a783d982152c5cd8317f1fd9bd` accepted the behavior-neutral shared-support extraction and did not change this module readiness classification.",
)
replace_once(
    "docs/MODULE_CATALOG.md",
    "- 8A.11 / #126 — Customer Privacy. Four production mutations and two permission-aware queries are merged independently; owner-scope contracts plus Parties and Consents implementations are accepted; draft PR #176 is the active behavior-neutral shared-support packet. Remaining owners, discovery/planning, approval, restrictions, legal holds, execution, export/deletion and convergence remain incomplete.",
    "- 8A.11 / #126 — Customer Privacy. Four production mutations and two permission-aware queries are merged independently; owner-scope contracts, Parties/Consents implementations and shared support are accepted through PR #176. Customer Accounts is the next bounded contract-only owner packet. Remaining owners, discovery/planning, approval, restrictions, legal holds, execution, export/deletion and convergence remain incomplete.",
)

replace_once(
    "docs/DELIVERY_GOVERNANCE.md",
    "7. **Active bounded architecture packet / PR #176** — behavior-neutral extraction of only support proven common by Parties and Consents; no runtime coordinate, worker, migration, contract or third owner.\n8. **Remaining owner-scope packets** — one authoritative owner at a time on the accepted shared-support boundary.\n9. **Remaining 8A.11 lifecycle packets** — sufficient owner set, scope discovery/planning, approval, restrictions, legal-hold/retention precedence, plan/outcome reads, resumable execution, export/deletion and convergence.\n10. **Phase 8A closure** — after merged privacy interaction proof.\n11. **8B / #29** — Product Catalog, Pricing, CPQ and Quote-to-Revenue.",
    "7. **Shared owner-scope support / PR #176** — **Complete**; accepted source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, merge `80411d54a3ca45a783d982152c5cd8317f1fd9bd`.\n8. **Next bounded owner-scope packet — Customer Accounts** — contract-only/non-runtime authoritative Account/Party association contribution using accepted shared support.\n9. **Remaining owner-scope packets** — one authoritative owner at a time on the accepted shared-support boundary.\n10. **Remaining 8A.11 lifecycle packets** — sufficient owner set, scope discovery/planning, approval, restrictions, legal-hold/retention precedence, plan/outcome reads, resumable execution, export/deletion and convergence.\n11. **Phase 8A closure** — after merged privacy interaction proof.\n12. **8B / #29** — Product Catalog, Pricing, CPQ and Quote-to-Revenue.",
)
replace_once(
    "docs/DELIVERY_GOVERNANCE.md",
    "- Draft PR #176 is the active dependency-lane packet. It must synchronize documentation, preserve exact owner behavior and errors, remove its temporary validator and pass all applicable checks on one unchanged SHA before merge.\n- The next Customer Privacy production coordinate remains intentionally unselected until shared support and the required remaining owner implementations establish a sufficient discovery/planning boundary.",
    "- PR #176 accepted shared owner-scope support on unchanged source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, passed 21/21 applicable permanent workflows and merged as `80411d54a3ca45a783d982152c5cd8317f1fd9bd`.\n- Customer Accounts is the next bounded owner-scope implementation; no Customer Privacy production coordinate is selected until the required owner set establishes a sufficient discovery/planning boundary.",
)

replace_once(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "Execution status: **Scalability runway accepted through PR #173; two privacy owners accepted through PR #175; shared-support extraction active in draft PR #176.**  ",
    "Execution status: **Scalability runway accepted through PR #173; two privacy owners accepted through PR #175; behavior-neutral shared support accepted through PR #176.**  ",
)

replace_once(
    "docs/PRIVACY_OWNER_SCOPE_SHARED_SUPPORT_COMPARISON.md",
    "Status: **Gate candidate — exact-head permanent validation pending for PR #176**  ",
    "Status: **Accepted through PR #176**  \nAccepted source: `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`  \nMerge: `80411d54a3ca45a783d982152c5cd8317f1fd9bd`  \nPermanent workflows: **21/21 successful on the unchanged accepted source**  ",
)

print("post-merge privacy support documentation synchronized")
