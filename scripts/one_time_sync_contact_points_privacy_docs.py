from __future__ import annotations

from pathlib import Path

SOURCE = "00c5b940326b14f5e4aab7d8c8b467ee688f6c9c"
MERGE = "96cd0cf548310592a0718c97242a724a29717a72"


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one occurrence, found {count}: {old[:160]!r}")
    file.write_text(text.replace(old, new, 1))


def append_once(path: str, marker: str, block: str) -> None:
    file = Path(path)
    text = file.read_text()
    if block.strip() in text:
        return
    count = text.count(marker)
    if count != 1:
        raise SystemExit(f"{path}: expected one marker, found {count}: {marker!r}")
    file.write_text(text.replace(marker, block + "\n\n" + marker, 1))


def project_status() -> None:
    path = "docs/PROJECT_STATUS.md"
    replace_once(
        path,
        "17. `PRIVACY_OWNER_SCOPE_SHARED_SUPPORT_COMPARISON.md` — accepted shared-support boundary, compatibility baseline and current three-consumer proof.",
        "17. `PRIVACY_OWNER_SCOPE_SHARED_SUPPORT_COMPARISON.md` — accepted shared-support boundary, compatibility baseline and current four-consumer proof.",
    )
    replace_once(
        path,
        "**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is Complete. Phase 8A.11 is In progress; six Customer Privacy runtime coordinates and nine contract-only owner contribution coordinates are published. Parties, Consents and Customer Accounts now have accepted authoritative contract-only owner implementations, while the shared support remains behavior-neutral and mechanically restricted to those three consumers. PR #179 accepted unchanged source `7d3e44e6dede36f76dfe92145dea6129a2b4639e` with 23/23 applicable permanent workflows and merged as `5b5252a437c6bebbd7afdead0162063af4c0b7e4`. The next bounded owner is Contact Points through `contact_points.privacy.scope.contribute@1.0.0`; it must remain contract-only/non-runtime.**",
        f"**Phases 0.1–7 are complete. Phase 8A is active. Phase 8A.10 is Complete. Phase 8A.11 is In progress; six Customer Privacy runtime coordinates and nine contract-only owner contribution coordinates are published. Parties, Consents, Customer Accounts and Contact Points now have accepted authoritative contract-only owner implementations, while shared support remains behavior-neutral and mechanically restricted to those four consumers. PR #181 accepted unchanged source `{SOURCE}` with 24/24 applicable permanent workflows and merged as `{MERGE}`. The next bounded owner is Party Relationships through `party_relationships.privacy.scope.contribute@1.0.0`; it must remain contract-only/non-runtime.**",
    )
    replace_once(
        path,
        "- **8A.11 — In progress:** architecture, owner foundation, deterministic domain, canonical persistence, immutable public contracts, FORCE RLS persistence, four public mutations, two permission-aware queries, immutable owner-scope envelopes and nine owner-specific contract-only contribution coordinates are merged; authoritative Parties, Consents and Customer Accounts owner implementations are accepted through PR #179, and Contact Points is selected as the next bounded owner implementation.",
        "- **8A.11 — In progress:** architecture, owner foundation, deterministic domain, canonical persistence, immutable public contracts, FORCE RLS persistence, four public mutations, two permission-aware queries, immutable owner-scope envelopes and nine owner-specific contract-only contribution coordinates are merged; authoritative Parties, Consents, Customer Accounts and Contact Points owner implementations are accepted through PR #181, and Party Relationships is selected as the next bounded owner implementation.",
    )
    replace_once(
        path,
        "`Contact Points privacy owner contribution -> remaining owner privacy contributions -> sufficient owner set and scope discovery/planning -> approval/restriction/legal-hold/plan/outcome/worker lifecycle -> export/deletion/convergence -> Phase 8A closure -> 8B`",
        "`Party Relationships privacy owner contribution -> remaining owner privacy contributions -> sufficient owner set and scope discovery/planning -> approval/restriction/legal-hold/plan/outcome/worker lifecycle -> export/deletion/convergence -> Phase 8A closure -> 8B`",
    )
    replace_once(
        path,
        "- PR #179 — authoritative non-runtime Customer Accounts privacy scope owner implementation through embedded Account-owned Party associations and bounded keyset pagination.",
        "- PR #179 — authoritative non-runtime Customer Accounts privacy scope owner implementation through embedded Account-owned Party associations and bounded keyset pagination.\n- PR #181 — authoritative non-runtime Contact Points privacy scope owner implementation through direct persisted Party binding, strict endpoint-state rehydration and bounded keyset pagination.",
    )
    replace_once(
        path,
        "PR #176 was accepted on unchanged source SHA `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, passed all 21 applicable permanent workflows and was squash-merged as `80411d54a3ca45a783d982152c5cd8317f1fd9bd`. It extracts only proven common request integrity, lineage/registry/time/page-size validation, canonical Party proof and digest framing; mechanically limits consumers to Parties and Consents; freezes owner error and digest compatibility; and changes no runtime inventory, contract, migration, worker or owner semantics.",
        "PR #176 was accepted on unchanged source SHA `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, passed all 21 applicable permanent workflows and was squash-merged as `80411d54a3ca45a783d982152c5cd8317f1fd9bd`. It extracts only proven common request integrity, lineage/registry/time/page-size validation, canonical Party proof and digest framing; originally limited consumers to Parties and Consents, while PRs #179 and #181 later extended only the mechanical allowlists to independently proven Customer Accounts and Contact Points adapters; owner errors, digests and runtime inventory remain unchanged.",
    )
    replace_once(
        path,
        "Contact Points is selected as the next bounded contract-only owner implementation through `contact_points.privacy.scope.contribute@1.0.0`. Its packet must preserve authoritative Contact Point ownership, strict rehydration, reference-only evidence, owner-specific pagination/retention/errors and the accepted shared-support boundary without runtime promotion or Customer Privacy orchestration.",
        f"PR #181 was accepted on unchanged user-authored source SHA `{SOURCE}`, passed all 24 applicable permanent workflows and was squash-merged as `{MERGE}`. It implements Contact Points through strict persistence-envelope and full-domain rehydration, exact direct Party binding, bounded owner-specific keyset pagination and cursor/digest domains, deterministic reference-only evidence, endpoint-value byte exclusion, clean rollback/schema-removal/reapply acceptance and zero query-side writes. It remains contract-only/non-runtime and leaves shared support behavior unchanged.\n\nParty Relationships is selected as the next bounded contract-only owner implementation through `party_relationships.privacy.scope.contribute@1.0.0`. Its packet must preserve authoritative two-endpoint temporal relationship semantics, strict rehydration, reference-only evidence, owner-specific matching/pagination/retention/errors and the accepted shared-support boundary without runtime promotion or Customer Privacy orchestration.",
    )


def roadmap() -> None:
    path = "docs/IMPLEMENTATION_ROADMAP.md"
    replace_once(
        path,
        "1. **8A.11 / #126 — In progress:** architecture/domain/contracts/FORCE-RLS foundation is merged through PR #145; `case.create`, `case.submit`, `case.subject.verify`, `case.get`, `case.cancel` and `case.list` are merged through PR #152; owner-scope contracts and authoritative Parties, Consents and Customer Accounts implementations are merged through PR #179.",
        "1. **8A.11 / #126 — In progress:** architecture/domain/contracts/FORCE-RLS foundation is merged through PR #145; `case.create`, `case.submit`, `case.subject.verify`, `case.get`, `case.cancel` and `case.list` are merged through PR #152; owner-scope contracts and authoritative Parties, Consents, Customer Accounts and Contact Points implementations are merged through PR #181.",
    )
    replace_once(
        path,
        "3. **Shared owner-scope support — Complete through PR #176:** accepted source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, merge `80411d54a3ca45a783d982152c5cd8317f1fd9bd`; only proven common mechanics were extracted and owner behavior remained unchanged. PR #179 added Customer Accounts as the third mechanically permitted consumer without changing shared behavior.",
        "3. **Shared owner-scope support — Complete through PR #176:** accepted source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, merge `80411d54a3ca45a783d982152c5cd8317f1fd9bd`; only proven common mechanics were extracted and owner behavior remained unchanged. PRs #179 and #181 added Customer Accounts and Contact Points as the third and fourth mechanically permitted consumers without changing shared behavior.",
    )
    replace_once(
        path,
        "5. **Next bounded owner slice — Contact Points:** implement `contact_points.privacy.scope.contribute@1.0.0` through authoritative Contact Point-to-Party ownership, strict state rehydration, deterministic reference-only evidence and tenant/RLS/no-write proof. Add no runtime route, worker, production schema migration, Customer Privacy orchestration or speculative shared abstraction.\n6. **Remaining owner slices:** implement the other privacy owner contributions one bounded owner at a time on the validated support boundary before runtime promotion.\n7. **Scope discovery and lifecycle:** assemble a sufficient owner set, then separately prove discovery/planning, approval, restrictions, legal-hold/retention precedence, plan/outcome reads, resumable orchestration, export/deletion/convergence and workers.\n8. **Phase 8A closure:** only after the complete privacy/customer-master interaction baseline is merged and reconciled.\n9. **8B / #29:** starts only from the completed Phase 8A baseline.",
        f"5. **Contact Points owner slice — Complete through PR #181:** accepted source `{SOURCE}`, merge `{MERGE}`, 24/24 applicable permanent workflows; exact direct Party binding, strict Contact Point rehydration, bounded owner pagination and reference-only endpoint-value-free evidence remain contract-only/non-runtime.\n6. **Next bounded owner slice — Party Relationships:** implement `party_relationships.privacy.scope.contribute@1.0.0` through authoritative two-endpoint relationship state, strict temporal/directional rehydration, deterministic reference-only evidence and tenant/RLS/no-write proof. Add no runtime route, worker, production schema migration, Customer Privacy orchestration or speculative shared abstraction.\n7. **Remaining owner slices:** implement the other privacy owner contributions one bounded owner at a time on the validated support boundary before runtime promotion.\n8. **Scope discovery and lifecycle:** assemble a sufficient owner set, then separately prove discovery/planning, approval, restrictions, legal-hold/retention precedence, plan/outcome reads, resumable orchestration, export/deletion/convergence and workers.\n9. **Phase 8A closure:** only after the complete privacy/customer-master interaction baseline is merged and reconciled.\n10. **8B / #29:** starts only from the completed Phase 8A baseline.",
    )


def phase_plan() -> None:
    path = "docs/PHASE8_DELIVERY_PLAN.md"
    replace_once(
        path,
        "Owner-scope foundation and accepted implementations: PRs #154–#156, #175 and #179",
        "Owner-scope foundation and accepted implementations: PRs #154–#156, #175, #179 and #181",
    )
    replace_once(
        path,
        "Next bounded owner packet: Contact Points privacy-scope contribution, still contract-only/non-runtime",
        "Next bounded owner packet: Party Relationships privacy-scope contribution, still contract-only/non-runtime",
    )
    replace_once(
        path,
        "The next bounded packet is the Contact Points privacy-scope owner implementation through `contact_points.privacy.scope.contribute@1.0.0`. It must remain contract-only/non-runtime, use only authoritative Contact Point state and owner-defined Party association semantics, strictly rehydrate persisted state, preserve reference-only evidence, prove tenant/RLS/no-write behavior on clean and reapplied PostgreSQL, and consume accepted shared support without speculative expansion.",
        f"PR #181 accepted unchanged user-authored source `{SOURCE}` with all 24 applicable permanent workflows and squash-merged as `{MERGE}`. Contact Points now proves a fourth authoritative owner shape through exact persisted Party binding, strict endpoint lifecycle/verification rehydration, bounded owner-specific keyset pagination/cursors, deterministic reference-only evidence, response-byte exclusion of endpoint values and verification references, clean rollback/schema-removal/reapply acceptance and zero query-side writes. It remains contract-only/non-runtime and did not extend shared-support behavior.\n\nThe next bounded packet is the Party Relationships privacy-scope owner implementation through `party_relationships.privacy.scope.contribute@1.0.0`. It must remain contract-only/non-runtime, use only authoritative relationship state and owner-defined two-endpoint Party semantics, strictly rehydrate temporal/directional persisted state, preserve reference-only evidence, prove tenant/RLS/no-write behavior on clean and reapplied PostgreSQL, and consume accepted shared support without speculative expansion.",
    )


def module_catalog() -> None:
    path = "docs/MODULE_CATALOG.md"
    replace_once(
        path,
        "Phase 8A.11 / issue #126 remains **In progress**. PRs #140–#145 merged the architecture freeze, owner foundation, deterministic domain, canonical persistence, immutable public contracts and FORCE RLS proof. PRs #154–#155 published the owner-scope protocol and nine exact contract-only owner coordinates; PRs #156, #175 and #179 accepted authoritative Parties, Consents and Customer Accounts owner implementations. PR #176 accepted their behavior-neutral common support; PR #179 added Customer Accounts as the third mechanically permitted consumer without changing module readiness or runtime inventory. Contact Points is selected as the next contract-only owner implementation.",
        "Phase 8A.11 / issue #126 remains **In progress**. PRs #140–#145 merged the architecture freeze, owner foundation, deterministic domain, canonical persistence, immutable public contracts and FORCE RLS proof. PRs #154–#155 published the owner-scope protocol and nine exact contract-only owner coordinates; PRs #156, #175, #179 and #181 accepted authoritative Parties, Consents, Customer Accounts and Contact Points owner implementations. PR #176 accepted their behavior-neutral common support; PRs #179 and #181 added the third and fourth mechanically permitted consumers without changing module readiness or runtime inventory. Party Relationships is selected as the next contract-only owner implementation.",
    )
    replace_once(
        path,
        "- PR #179 — authoritative Customer Accounts contribution through strict Account rehydration, embedded `Primary`/`Member` Party associations and bounded owner-specific keyset pagination; accepted source `7d3e44e6dede36f76dfe92145dea6129a2b4639e`, merge `5b5252a437c6bebbd7afdead0162063af4c0b7e4`, 23/23 applicable workflows;\n- all three remain non-runtime and add no Customer Privacy worker or public ingress;\n- PR #176 / source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8` / merge `80411d54a3ca45a783d982152c5cd8317f1fd9bd` accepted the behavior-neutral shared-support extraction; PR #179 changed only its mechanical consumer allowlist, not this module readiness classification.",
        f"- PR #179 — authoritative Customer Accounts contribution through strict Account rehydration, embedded `Primary`/`Member` Party associations and bounded owner-specific keyset pagination; accepted source `7d3e44e6dede36f76dfe92145dea6129a2b4639e`, merge `5b5252a437c6bebbd7afdead0162063af4c0b7e4`, 23/23 applicable workflows;\n- PR #181 — authoritative Contact Points contribution through strict endpoint-state rehydration, exact direct Party binding and bounded owner-specific keyset pagination; accepted source `{SOURCE}`, merge `{MERGE}`, 24/24 applicable workflows;\n- all four remain non-runtime and add no Customer Privacy worker or public ingress;\n- PR #176 / source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8` / merge `80411d54a3ca45a783d982152c5cd8317f1fd9bd` accepted the behavior-neutral shared-support extraction; PRs #179 and #181 changed only its mechanical consumer allowlists, not this module readiness classification.",
    )
    replace_once(
        path,
        "- 8A.11 / #126 — Customer Privacy. Four production mutations and two permission-aware queries are merged independently; owner-scope contracts, Parties/Consents/Customer Accounts implementations and shared support are accepted through PR #179. Contact Points is the next bounded contract-only owner packet. Remaining owners, discovery/planning, approval, restrictions, legal holds, execution, export/deletion and convergence remain incomplete.",
        "- 8A.11 / #126 — Customer Privacy. Four production mutations and two permission-aware queries are merged independently; owner-scope contracts, Parties/Consents/Customer Accounts/Contact Points implementations and shared support are accepted through PR #181. Party Relationships is the next bounded contract-only owner packet. Remaining owners, discovery/planning, approval, restrictions, legal holds, execution, export/deletion and convergence remain incomplete.",
    )


def governance() -> None:
    path = "docs/DELIVERY_GOVERNANCE.md"
    replace_once(
        path,
        "6. **8A.11 / #126** — Customer Privacy Lifecycle, Restriction, Deletion and Legal Hold — **In progress**; six runtime coordinates are merged through PR #152, owner-scope contracts through PR #155 and authoritative Parties, Consents and Customer Accounts owner implementations through PR #179.\n7. **Shared owner-scope support / PR #176** — **Complete**; accepted source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, merge `80411d54a3ca45a783d982152c5cd8317f1fd9bd`; PR #179 adds only the third mechanically proven consumer.\n8. **Customer Accounts owner-scope packet / PR #179** — **Complete**; accepted source `7d3e44e6dede36f76dfe92145dea6129a2b4639e`, merge `5b5252a437c6bebbd7afdead0162063af4c0b7e4`, 23/23 applicable permanent workflows.\n9. **Next bounded owner-scope packet — Contact Points** — contract-only/non-runtime authoritative Contact Point contribution using accepted shared support.\n10. **Remaining owner-scope packets** — one authoritative owner at a time on the accepted shared-support boundary.\n11. **Remaining 8A.11 lifecycle packets** — sufficient owner set, scope discovery/planning, approval, restrictions, legal-hold/retention precedence, plan/outcome reads, resumable execution, export/deletion and convergence.\n12. **Phase 8A closure** — after merged privacy interaction proof.\n13. **8B / #29** — Product Catalog, Pricing, CPQ and Quote-to-Revenue.",
        f"6. **8A.11 / #126** — Customer Privacy Lifecycle, Restriction, Deletion and Legal Hold — **In progress**; six runtime coordinates are merged through PR #152, owner-scope contracts through PR #155 and authoritative Parties, Consents, Customer Accounts and Contact Points owner implementations through PR #181.\n7. **Shared owner-scope support / PR #176** — **Complete**; accepted source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, merge `80411d54a3ca45a783d982152c5cd8317f1fd9bd`; PRs #179 and #181 add only the third and fourth mechanically proven consumers.\n8. **Customer Accounts owner-scope packet / PR #179** — **Complete**; accepted source `7d3e44e6dede36f76dfe92145dea6129a2b4639e`, merge `5b5252a437c6bebbd7afdead0162063af4c0b7e4`, 23/23 applicable permanent workflows.\n9. **Contact Points owner-scope packet / PR #181** — **Complete**; accepted source `{SOURCE}`, merge `{MERGE}`, 24/24 applicable permanent workflows.\n10. **Next bounded owner-scope packet — Party Relationships** — contract-only/non-runtime authoritative two-endpoint temporal relationship contribution using accepted shared support.\n11. **Remaining owner-scope packets** — one authoritative owner at a time on the accepted shared-support boundary.\n12. **Remaining 8A.11 lifecycle packets** — sufficient owner set, scope discovery/planning, approval, restrictions, legal-hold/retention precedence, plan/outcome reads, resumable execution, export/deletion and convergence.\n13. **Phase 8A closure** — after merged privacy interaction proof.\n14. **8B / #29** — Product Catalog, Pricing, CPQ and Quote-to-Revenue.",
    )
    replace_once(
        path,
        "- Phase 8A.11 / #126 is **In progress**. PR #152 established four runtime Customer Privacy mutations, two permission-aware queries, ten public non-runtime coordinates and zero Customer Privacy workers. PRs #156, #175 and #179 accepted Parties, Consents and Customer Accounts privacy-scope owners; all remain contract-only/non-runtime.\n- PR #176 accepted shared owner-scope support on unchanged source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, passed 21/21 applicable permanent workflows and merged as `80411d54a3ca45a783d982152c5cd8317f1fd9bd`. PR #179 added Customer Accounts as the third mechanically restricted consumer without changing shared semantics.\n- PR #179 accepted unchanged user-authored source `7d3e44e6dede36f76dfe92145dea6129a2b4639e`, passed 23/23 applicable permanent workflows and merged as `5b5252a437c6bebbd7afdead0162063af4c0b7e4`.\n- Contact Points is the next bounded owner-scope implementation; no Customer Privacy production coordinate is selected until the required owner set establishes a sufficient discovery/planning boundary.",
        f"- Phase 8A.11 / #126 is **In progress**. PR #152 established four runtime Customer Privacy mutations, two permission-aware queries, ten public non-runtime coordinates and zero Customer Privacy workers. PRs #156, #175, #179 and #181 accepted Parties, Consents, Customer Accounts and Contact Points privacy-scope owners; all remain contract-only/non-runtime.\n- PR #176 accepted shared owner-scope support on unchanged source `eb8e6b6f2edf038485e5c64014d7d28dba302ce8`, passed 21/21 applicable permanent workflows and merged as `80411d54a3ca45a783d982152c5cd8317f1fd9bd`. PRs #179 and #181 added Customer Accounts and Contact Points as the third and fourth mechanically restricted consumers without changing shared semantics.\n- PR #179 accepted unchanged user-authored source `7d3e44e6dede36f76dfe92145dea6129a2b4639e`, passed 23/23 applicable permanent workflows and merged as `5b5252a437c6bebbd7afdead0162063af4c0b7e4`.\n- PR #181 accepted unchanged user-authored source `{SOURCE}`, passed 24/24 applicable permanent workflows and merged as `{MERGE}`.\n- Party Relationships is the next bounded owner-scope implementation; no Customer Privacy production coordinate is selected until the required owner set establishes a sufficient discovery/planning boundary.",
    )


def complexity() -> None:
    replace_once(
        "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
        "Execution status: **Scalability runway accepted through PR #173; three privacy owners accepted through PR #179; behavior-neutral shared support accepted through PR #176 and mechanically proven with Customer Accounts as its third consumer.**",
        "Execution status: **Scalability runway accepted through PR #173; four privacy owners accepted through PR #181; behavior-neutral shared support accepted through PR #176 and mechanically proven with Customer Accounts and Contact Points as its third and fourth consumers.**",
    )


def shared_comparison() -> None:
    path = "docs/PRIVACY_OWNER_SCOPE_SHARED_SUPPORT_COMPARISON.md"
    replace_once(
        path,
        "Status: **Shared boundary accepted through PR #176; third consumer accepted through PR #179**",
        "Status: **Shared boundary accepted through PR #176; fourth consumer accepted through PR #181**",
    )
    replace_once(
        path,
        "Current proven consumers: **Parties + Consents + Customer Accounts PR #179**",
        "Current proven consumers: **Parties + Consents + Customer Accounts PR #179 + Contact Points PR #181**",
    )
    replace_once(
        path,
        "The support crate must not begin, commit or otherwise own the database transaction. The exact allowlist for `begin_bound_read_transaction` is restricted to the Parties, Consents and Customer Accounts PostgreSQL adapters.",
        "The support crate must not begin, commit or otherwise own the database transaction. The exact allowlist for `begin_bound_read_transaction` is restricted to the Parties, Consents, Customer Accounts and Contact Points PostgreSQL adapters.",
    )
    replace_once(
        path,
        "The dependency boundary is also mechanical. `architecture-policy.json` names the Parties, Consents and Customer Accounts adapter manifests as the only required consumers, and `scripts/check_architecture.py` rejects missing allowed manifests, missing required consumers and every unexpected consumer. A future owner may use this crate only through an explicit policy change reviewed together with that independently proven owner implementation.",
        "The dependency boundary is also mechanical. `architecture-policy.json` names the Parties, Consents, Customer Accounts and Contact Points adapter manifests as the only required consumers, and `scripts/check_architecture.py` rejects missing allowed manifests, missing required consumers and every unexpected consumer. A future owner may use this crate only through an explicit policy change reviewed together with that independently proven owner implementation.",
    )
    replace_once(
        path,
        "The Parties, Consents and Customer Accounts permanent workflows remain authoritative. The shared crate is present in all three path-filter graphs so a change to common behavior re-runs every proven owner acceptance suite.",
        "The Parties, Consents, Customer Accounts and Contact Points permanent workflows remain authoritative. The shared crate is present in all four path-filter graphs so a change to common behavior re-runs every proven owner acceptance suite.",
    )
    replace_once(
        path,
        "- **Expected consumers:** Parties, Consents and Customer Accounts are proven; any additional consumer requires an explicit architecture-policy change with its independently accepted owner packet.",
        "- **Expected consumers:** Parties, Consents, Customer Accounts and Contact Points are proven; any additional consumer requires an explicit architecture-policy change with its independently accepted owner packet.",
    )
    replace_once(
        path,
        "PR #179 changed no shared-support behavior. It only extended the mechanical consumer and bound-read allowlists to the independently proven Customer Accounts adapter. The next candidate consumer is Contact Points, but it must earn the same explicit policy change in its own bounded packet; shared support must not be expanded speculatively.",
        "PR #179 changed no shared-support behavior. It only extended the mechanical consumer and bound-read allowlists to the independently proven Customer Accounts adapter. PR #181 later earned the same explicit policy change for Contact Points without extending shared behavior. The next candidate consumer is Party Relationships, but it must be proven independently in its own bounded packet; shared support must not be expanded speculatively.",
    )
    append_once(
        path,
        "## 11. Third proven consumer — Customer Accounts PR #179",
        f"""## 12. Fourth proven consumer — Contact Points PR #181

PR #181 accepted unchanged user-authored source `{SOURCE}`, passed all 24 applicable permanent workflows and was squash-merged as `{MERGE}`.

Contact Points independently proves the accepted shared boundary against a fourth authoritative resource shape:

- authoritative Contact Point records with a direct persisted Party reference;
- strict endpoint kind, normalized/display value, lifecycle, validity, verification, timestamp and version rehydration before evidence emission;
- inclusion of owner-approved Active/Inactive and verified/unverified shapes without exposing endpoint values or verification references;
- owner-specific bounded keyset scan, continuation cursor, page/cursor digest domains, evidence classification, retention and stable error prefixes;
- deterministic reference-only response bytes with email, phone, postal, URL, messaging and verification fixture values excluded;
- one caller-opened tenant-bound `REPEATABLE READ, READ ONLY` PostgreSQL transaction;
- clean migration, full rollback/schema removal, reapply and repeated acceptance with zero writes across record, relationship, transaction, idempotency, outbox and audit surfaces.

PR #181 changed no shared-support behavior. It only extended the mechanical consumer and bound-read allowlists to the independently proven Contact Points adapter. Party Relationships is the next candidate consumer and must earn the same bounded policy change without moving two-endpoint temporal relationship semantics into shared code.""",
    )


def contact_packet() -> None:
    path = "docs/CONTACT_POINTS_PRIVACY_SCOPE_PACKET.md"
    replace_once(
        path,
        "Status: **Ready after Customer Accounts PR #179 and post-merge synchronization**  \nParent program: #126  \nPrerequisites: Parties PR #156, Consents PR #175, shared support PR #176, Customer Accounts PR #179  \nCoordinate: `contact_points.privacy.scope.contribute@1.0.0`",
        f"Status: **Complete through PR #181**  \nParent program: #126  \nPrerequisites: Parties PR #156, Consents PR #175, shared support PR #176, Customer Accounts PR #179  \nAccepted source: `{SOURCE}`  \nMerge: `{MERGE}`  \nPermanent workflows: **24/24 successful on the unchanged accepted source**  \nCoordinate: `contact_points.privacy.scope.contribute@1.0.0`",
    )
    replace_once(
        path,
        "The packet is complete only after its implementation PR is merged to `main`, exact-head source and merge SHAs are recorded in issue #126, permanent owner CI is authoritative, and post-merge documentation selects the following bounded owner.\n\nCompletion does not authorize Customer Privacy discovery, planning, approval, restriction, legal-hold, action execution or worker runtime. Those remain separate packets after a sufficient owner set is explicitly reviewed and accepted.",
        f"The packet is complete through PR #181 on accepted source `{SOURCE}` and merge `{MERGE}`. `Contact Points Privacy Scope CI` is authoritative, all 24 applicable permanent workflows passed on the unchanged source, and the coordinate remains contract-only/non-runtime.\n\nCompletion does not authorize Customer Privacy discovery, planning, approval, restriction, legal-hold, action execution or worker runtime. Party Relationships is the next bounded contract-only owner packet, and those lifecycle capabilities remain separate until a sufficient owner set is explicitly reviewed and accepted.",
    )


def main() -> None:
    project_status()
    roadmap()
    phase_plan()
    module_catalog()
    governance()
    complexity()
    shared_comparison()
    contact_packet()


if __name__ == "__main__":
    main()
