from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one anchor, found {count}")
    file.write_text(text.replace(old, new, 1))


replace_once(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "10. **8B / #29:** starts only from the completed Phase 8A baseline.",
    "11. **8B / #29:** starts only from the completed Phase 8A baseline.",
)

replace_once(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "The next bounded packet is the Party Relationships privacy-scope owner implementation through `party_relationships.privacy.scope.contribute@1.0.0`. It must remain contract-only/non-runtime, use only authoritative relationship state and owner-defined two-endpoint Party semantics, strictly rehydrate temporal/directional persisted state, preserve reference-only evidence, prove tenant/RLS/no-write behavior on clean and reapplied PostgreSQL, and consume accepted shared support without speculative expansion.",
    "The next bounded packet is the Identity Resolution privacy-scope owner implementation through `identity_resolution.privacy.scope.contribute@1.0.0`. It must remain contract-only/non-runtime, strictly rehydrate authoritative duplicate-candidate cases and merge operations, derive bounded alias-aware lineage from active merge state, paginate deterministically across both resource families, preserve reference-only evidence, prove tenant/RLS/no-write behavior on clean and reapplied PostgreSQL, and consume accepted shared support without speculative expansion.",
)

path = Path("docs/PROJECT_STATUS.md")
text = path.read_text()
old = """PR #181 was accepted on unchanged user-authored source SHA `00c5b940326b14f5e4aab7d8c8b467ee688f6c9c`, passed all 24 applicable permanent workflows and was squash-merged as `96cd0cf548310592a0718c97242a724a29717a72`. It implements Contact Points through strict persistence-envelope and full-domain rehydration, exact direct Party binding, bounded owner-specific keyset pagination and cursor/digest domains, deterministic reference-only evidence, endpoint-value byte exclusion, clean rollback/schema-removal/reapply acceptance and zero query-side writes. It remains contract-only/non-runtime and leaves shared support behavior unchanged.

Identity Resolution is selected as the next bounded contract-only owner implementation through `identity_resolution.privacy.scope.contribute@1.0.0`. Its packet freezes authoritative duplicate-candidate and merge-operation families, bounded alias-aware lineage resolution, heterogeneous pagination, strict rehydration, reference-only evidence and owner-specific retention/errors without runtime promotion or Customer Privacy orchestration.


PR #183 accepted unchanged user-authored source `a431185e01e95dfeffcf7d9c9a440afc8f0c9a57`, passed all 25 applicable permanent workflows and was squash-merged as `9ad2aa91321e9edb54cab98218f93143923ef33f`. Party Relationships proves the fifth authoritative owner shape through strict two-endpoint matching, directional/reciprocal and temporal domain rehydration, bounded owner-specific pagination/cursors, reference-only evidence, response-byte non-disclosure, clean rollback/schema-removal/reapply acceptance and zero query-side writes. It remains contract-only/non-runtime and changes no shared-support behavior.
"""
new = """PR #181 was accepted on unchanged user-authored source SHA `00c5b940326b14f5e4aab7d8c8b467ee688f6c9c`, passed all 24 applicable permanent workflows and was squash-merged as `96cd0cf548310592a0718c97242a724a29717a72`. It implements Contact Points through strict persistence-envelope and full-domain rehydration, exact direct Party binding, bounded owner-specific keyset pagination and cursor/digest domains, deterministic reference-only evidence, endpoint-value byte exclusion, clean rollback/schema-removal/reapply acceptance and zero query-side writes. It remains contract-only/non-runtime and leaves shared support behavior unchanged.

PR #183 accepted unchanged user-authored source `a431185e01e95dfeffcf7d9c9a440afc8f0c9a57`, passed all 25 applicable permanent workflows and was squash-merged as `9ad2aa91321e9edb54cab98218f93143923ef33f`. Party Relationships proves the fifth authoritative owner shape through strict two-endpoint matching, directional/reciprocal and temporal domain rehydration, bounded owner-specific pagination/cursors, reference-only evidence, response-byte non-disclosure, clean rollback/schema-removal/reapply acceptance and zero query-side writes. It remains contract-only/non-runtime and changes no shared-support behavior.

Identity Resolution is selected as the next bounded contract-only owner implementation through `identity_resolution.privacy.scope.contribute@1.0.0`. Its packet freezes authoritative duplicate-candidate and merge-operation families, bounded alias-aware lineage resolution, heterogeneous pagination, strict rehydration, reference-only evidence and owner-specific retention/errors without runtime promotion or Customer Privacy orchestration.
"""
count = text.count(old)
if count != 1:
    raise SystemExit(f"docs/PROJECT_STATUS.md: expected one sequence, found {count}")
path.write_text(text.replace(old, new, 1))
