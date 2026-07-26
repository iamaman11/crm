from pathlib import Path

FILES = [
    Path("docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md"),
    Path("docs/DELIVERY_GOVERNANCE.md"),
    Path("docs/IMPLEMENTATION_ROADMAP.md"),
    Path("docs/MODULE_CATALOG.md"),
    Path("docs/PARTY_RELATIONSHIPS_PRIVACY_SCOPE_PACKET.md"),
    Path("docs/PHASE8_DELIVERY_PLAN.md"),
    Path("docs/PRIVACY_OWNER_SCOPE_SHARED_SUPPORT_COMPARISON.md"),
    Path("docs/PROJECT_STATUS.md"),
]

REPLACEMENTS = [
    ("current four-consumer proof", "current five-consumer proof"),
    ("four privacy owners accepted through PR #181", "five privacy owners accepted through PR #183"),
    ("fourth consumer accepted through PR #181", "fifth consumer accepted through PR #183"),
    (
        "Parties + Consents + Customer Accounts PR #179 + Contact Points PR #181",
        "Parties + Consents + Customer Accounts PR #179 + Contact Points PR #181 + Party Relationships PR #183",
    ),
    (
        "Parties, Consents, Customer Accounts and Contact Points",
        "Parties, Consents, Customer Accounts, Contact Points and Party Relationships",
    ),
    ("those four consumers", "those five consumers"),
    ("the four path-filter graphs", "the five path-filter graphs"),
    ("all four path-filter graphs", "all five path-filter graphs"),
    (
        "Party Relationships is selected as the next bounded contract-only owner implementation through `party_relationships.privacy.scope.contribute@1.0.0`.",
        "Identity Resolution is selected as the next bounded contract-only owner implementation through `identity_resolution.privacy.scope.contribute@1.0.0`.",
    ),
    (
        "The next bounded owner is Party Relationships through `party_relationships.privacy.scope.contribute@1.0.0`; it must remain contract-only/non-runtime.",
        "The next bounded owner is Identity Resolution through `identity_resolution.privacy.scope.contribute@1.0.0`; it must remain contract-only/non-runtime.",
    ),
    (
        "Next bounded owner packet: Party Relationships privacy-scope contribution, still contract-only/non-runtime",
        "Next bounded owner packet: Identity Resolution privacy-scope contribution, still contract-only/non-runtime",
    ),
    ("**Next bounded owner-scope packet — Party Relationships**", "**Next bounded owner-scope packet — Identity Resolution**"),
    ("**Next bounded owner slice — Party Relationships:**", "**Next bounded owner slice — Identity Resolution:**"),
]

for path in FILES:
    text = path.read_text()
    for old, new in REPLACEMENTS:
        text = text.replace(old, new)
    path.write_text(text)

packet = Path("docs/PARTY_RELATIONSHIPS_PRIVACY_SCOPE_PACKET.md")
text = packet.read_text()
text = text.replace(
    "Status: **Ready after Contact Points PR #181 and post-merge synchronization**",
    "Status: **Complete through Party Relationships PR #183**",
    1,
)
if "Accepted source: `a431185e01e95dfeffcf7d9c9a440afc8f0c9a57`" not in text:
    text = text.replace(
        "Coordinate: `party_relationships.privacy.scope.contribute@1.0.0`",
        "Coordinate: `party_relationships.privacy.scope.contribute@1.0.0`\nAccepted source: `a431185e01e95dfeffcf7d9c9a440afc8f0c9a57`\nMerge: `9ad2aa91321e9edb54cab98218f93143923ef33f`\nPermanent workflows: **25/25 successful on the unchanged accepted source**",
        1,
    )
    text += """

## 12. Accepted result

PR #183 accepted unchanged user-authored source `a431185e01e95dfeffcf7d9c9a440afc8f0c9a57`, passed all 25 applicable permanent workflows and was squash-merged as `9ad2aa91321e9edb54cab98218f93143923ef33f`.

The accepted implementation strictly rehydrates authoritative two-endpoint Party Relationship state, matches the canonical Party through either endpoint, preserves directional/reciprocal, role, Active/Inactive, validity, timestamp and version semantics as owner evidence, emits bounded reference-only resources, excludes counterpart Party identifiers and relationship semantics from encoded response bytes, and proves clean rollback/schema-removal/reapply acceptance with zero query-side writes.

It remains contract-only/non-runtime. Shared support behavior, public contracts, production schema, workers, Customer Privacy orchestration and runtime inventory are unchanged.
"""
packet.write_text(text)

accepted = """

PR #183 accepted unchanged user-authored source `a431185e01e95dfeffcf7d9c9a440afc8f0c9a57`, passed all 25 applicable permanent workflows and was squash-merged as `9ad2aa91321e9edb54cab98218f93143923ef33f`. Party Relationships proves the fifth authoritative owner shape through strict two-endpoint matching, directional/reciprocal and temporal domain rehydration, bounded owner-specific pagination/cursors, reference-only evidence, response-byte non-disclosure, clean rollback/schema-removal/reapply acceptance and zero query-side writes. It remains contract-only/non-runtime and changes no shared-support behavior.
"""
for name in [
    "docs/PROJECT_STATUS.md",
    "docs/PHASE8_DELIVERY_PLAN.md",
    "docs/IMPLEMENTATION_ROADMAP.md",
    "docs/MODULE_CATALOG.md",
    "docs/DELIVERY_GOVERNANCE.md",
]:
    path = Path(name)
    text = path.read_text()
    if "PR #183 accepted unchanged user-authored source" not in text:
        marker = "\n## Architecture scalability governance" if name.endswith("PROJECT_STATUS.md") else None
        if marker and marker in text:
            text = text.replace(marker, accepted + marker, 1)
        else:
            text += accepted
    path.write_text(text)

shared = Path("docs/PRIVACY_OWNER_SCOPE_SHARED_SUPPORT_COMPARISON.md")
text = shared.read_text()
if "## 13. Fifth proven consumer — Party Relationships PR #183" not in text:
    text += """

## 13. Fifth proven consumer — Party Relationships PR #183

PR #183 accepted unchanged user-authored source `a431185e01e95dfeffcf7d9c9a440afc8f0c9a57`, passed all 25 applicable permanent workflows and was squash-merged as `9ad2aa91321e9edb54cab98218f93143923ef33f`.

Party Relationships independently proves the accepted shared boundary against a fifth authoritative shape:

- one relationship resource owns two authoritative Party endpoints;
- the canonical Party may match either fully rehydrated endpoint;
- directional and reciprocal type/role semantics, Active/Inactive state, validity, timestamps and version remain owner-specific;
- bounded record-ID keyset scanning and owner cursor/digest/retention/error domains remain local;
- encoded response bytes exclude the counterpart Party, endpoint position, relationship type, roles, status and validity;
- clean rollback/schema-removal/reapply acceptance proves zero query-side writes.

PR #183 changed no shared-support behavior. It only extended the mechanical consumer and bound-read allowlists to the independently proven Party Relationships adapter. Identity Resolution is the next candidate consumer and must earn the same bounded policy change without moving candidate/merge lineage semantics into shared code.
"""
shared.write_text(text)

architecture = Path("docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md")
text = architecture.read_text().replace(
    "behavior-neutral shared support accepted through PR #176 and mechanically proven with Contact Points as its fourth consumer.",
    "behavior-neutral shared support accepted through PR #176 and mechanically proven with Party Relationships as its fifth consumer.",
)
architecture.write_text(text)
