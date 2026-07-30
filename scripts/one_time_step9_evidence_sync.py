from __future__ import annotations

from pathlib import Path
import re


ACCEPTED_SOURCE = "e7ed45a7da5f14fa79e1ca4d23fc808004b6a642"
MERGE_SHA = "e40832ae21118dd7f033e2811ca466d1242a19f0"
ACCEPTED_PARAGRAPH = (
    "Repository step 9 is accepted through PR #239 / accepted source "
    f"`{ACCEPTED_SOURCE}` / squash merge `{MERGE_SHA}` / 8 of 8 applicable "
    "permanent workflows on one unchanged exact head. It establishes one declarative "
    "affected-scope policy for contracts, Protobuf/API compatibility, database migrations, "
    "PostgreSQL acceptance, process/runtime acceptance, product-plane checks, frontend checks "
    "and operations checks; preserves deterministic Rust ownership and reverse closure; requires "
    "the real permanent workflow filters for every selected scope; blocks unknown non-Rust paths "
    "until classified; and records exact pull-request-head evidence. Shared workflow or policy "
    "changes widen validation to all 113 Rust workspace packages. The final 12-file packet changes "
    "no product behavior, Customer Privacy public inventory, runtime route, worker, contract, "
    "Protobuf message, schema, migration, crate, dependency, `Cargo.lock`, workspace package or "
    "generic-runtime business algorithm."
)


def replace_exact(text: str, old: str, new: str, label: str, expected: int = 1) -> str:
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{label}: found {count}, expected {expected}")
    return text.replace(old, new)


def replace_regex(text: str, pattern: str, replacement: str, label: str, expected: int = 1) -> str:
    updated, count = re.subn(pattern, replacement, text, flags=re.MULTILINE)
    if count != expected:
        raise SystemExit(f"{label}: found {count}, expected {expected}")
    return updated


def update(path: str, transform) -> None:
    target = Path(path)
    original = target.read_text(encoding="utf-8")
    revised = transform(original)
    if revised == original:
        raise SystemExit(f"{path}: expected a material change")
    target.write_text(revised, encoding="utf-8")


def architecture(text: str) -> str:
    text = replace_exact(
        text,
        "Current execution checkpoint: **2026-07-29**",
        "Current execution checkpoint: **2026-07-30**",
        "architecture checkpoint",
    )
    text = replace_exact(
        text,
        "- remaining affected-scope coverage outside the accepted real-diff Rust/tooling closure;",
        "- affected-scope ownership and workflow filters that can drift without permanent live compatibility guards;",
        "architecture risk",
    )
    text = replace_exact(
        text,
        "| E — affected-scope CI | **In progress** | changed paths, Rust reverse closure, structural preflight, explainable broadening and real-diff packet-check enforcement are accepted | complete contract, migration, process, product, frontend and operations scope selection with safe fallback |",
        "| E — affected-scope CI | **Complete** | deterministic Rust closure plus declarative contract, Protobuf/API, migration, PostgreSQL, process/runtime, product, frontend and operations ownership; real workflow-filter compatibility; exact-head evidence; unknown-path fail-closed enforcement accepted through PR #239 | preserve policy/workflow compatibility and classify every new repository path before it can be represented as safely skipped |",
        "architecture stage E",
    )
    text = replace_regex(
        text,
        r"^(Repository step 8 is accepted through PR #237[^\n]+)$",
        rf"\1\n\n{ACCEPTED_PARAGRAPH}",
        "architecture accepted step 9",
        expected=2,
    )
    text = replace_exact(
        text,
        "The next permitted implementation packet is repository step 9: affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks.",
        "The next permitted implementation packet is repository step 10: governed Customer Privacy access/export assembly.",
        "architecture next packet",
    )
    text = replace_exact(
        text,
        "The inserted repository step 4 prerequisite is complete through PR #224, repository step 4 is complete through PR #226, repository step 5 is complete through PR #228, the inserted lockfile-preservation prerequisite before repository step 6 is complete through PR #232, repository step 6 is complete through PR #230, repository step 7 is complete through PR #235, and repository step 8 is complete through PR #237. None changes the master numbering.",
        "The inserted repository step 4 prerequisite is complete through PR #224, repository step 4 is complete through PR #226, repository step 5 is complete through PR #228, the inserted lockfile-preservation prerequisite before repository step 6 is complete through PR #232, repository step 6 is complete through PR #230, repository step 7 is complete through PR #235, repository step 8 is complete through PR #237, and repository step 9 is complete through PR #239. None changes the master numbering.",
        "architecture completion summary",
    )
    text = replace_exact(
        text,
        "9. affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks — **Next**;\n10. governed Customer Privacy access/export assembly;",
        "9. affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks — **Complete through PR #239**;\n10. governed Customer Privacy access/export assembly — **Next**;",
        "architecture sequence",
    )
    text = replace_exact(
        text,
        "The next permitted repository packet is **repository step 9: affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks**. No repository step 10 or later work may begin before step 9 has unchanged exact-head acceptance and evidence synchronization.",
        "The next permitted repository packet is **repository step 10: governed Customer Privacy access/export assembly**. No repository step 11 or later work may begin before step 10 has unchanged exact-head acceptance and evidence synchronization.",
        "architecture final next packet",
    )
    return text


def roadmap(text: str) -> str:
    text = replace_exact(
        text,
        "- Stage E — **In progress**: real-diff packet-check and broadened Rust closure are accepted; database/process/product/frontend/operations selection remains open.",
        "- Stage E — **Complete through PR #239**: deterministic Rust closure, exact eight-category repository ownership, contract/Protobuf/API/migration/PostgreSQL/process/product/frontend/operations selection, live workflow-filter compatibility, exact-head evidence and unknown-path fail-closed enforcement are accepted.",
        "roadmap stage E",
    )
    text = replace_exact(
        text,
        "Accepted architecture evidence includes PR #197, PR #199, PR #200, PR #203, PR #204, PR #205, PR #218, PR #222, PR #224, PR #226, PR #228, PR #232, PR #230, PR #235 and PR #237.",
        "Accepted architecture evidence includes PR #197, PR #199, PR #200, PR #203, PR #204, PR #205, PR #218, PR #222, PR #224, PR #226, PR #228, PR #232, PR #230, PR #235, PR #237 and PR #239.",
        "roadmap evidence list",
    )
    text = replace_regex(
        text,
        r"The architecture stage table is completion accounting, not a set of independent work queues\. The binding repository order is section 2\.4 of the architecture plan\. Repository steps 1–8 are complete through PR #218, PR #220, PR #222, PR #226, PR #228, PR #230, PR #235 and PR #237; PR #224 accepted the smallest inserted prerequisite required by step 4, and PR #232 accepted the smallest lockfile-preservation prerequisite required before step 6\. Repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks — is the current next packet\.",
        "The architecture stage table is completion accounting, not a set of independent work queues. The binding repository order is section 2.4 of the architecture plan. Repository steps 1–9 are complete through PR #218, PR #220, PR #222, PR #226, PR #228, PR #230, PR #235, PR #237 and PR #239; PR #224 accepted the smallest inserted prerequisite required by step 4, and PR #232 accepted the smallest lockfile-preservation prerequisite required before step 6. Repository step 10 — governed Customer Privacy access/export assembly — is the current next packet.",
        "roadmap current packet",
    )
    text = replace_regex(
        text,
        r"^(Repository step 8 is accepted through PR #237[^\n]+)$",
        rf"\1\n\n### 5.14 Accepted multi-plane affected-scope enforcement\n\n{ACCEPTED_PARAGRAPH}",
        "roadmap accepted step 9",
    )
    text = replace_exact(
        text,
        "9. **Repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks — Next.**\n10–19. **Repository steps 10–19 — continue exactly as numbered in the architecture plan.**",
        "9. **Repository step 9 — affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks — Complete through PR #239.**\n10. **Repository step 10 — governed Customer Privacy access/export assembly — Next.**\n11–19. **Repository steps 11–19 — continue exactly as numbered in the architecture plan.**",
        "roadmap sequence",
    )
    text = replace_exact(
        text,
        "The inserted prerequisites did not renumber the master sequence. Repository step 9 is now the only next permitted implementation packet.",
        "The inserted prerequisites did not renumber the master sequence. Repository step 10 is now the only next permitted implementation packet.",
        "roadmap next sentence",
    )
    return text


def phase8(text: str) -> str:
    text = replace_regex(
        text,
        r"^(Repository step 8 is accepted through PR #237[^\n]+)$",
        rf"\1\n\n{ACCEPTED_PARAGRAPH}",
        "phase8 accepted step 9",
    )
    text = replace_exact(
        text,
        "Repository step 9 is now the next permitted implementation packet.",
        "Repository step 10 — governed Customer Privacy access/export assembly — is now the next permitted implementation packet. Repository step 11 or later work remains blocked until step 10 is accepted and its evidence is synchronized.",
        "phase8 next packet",
    )
    return text


def project_status(text: str) -> str:
    text = replace_exact(
        text,
        "Latest accepted repository implementation packet is PR #237 / accepted source `f926ece93dc2b24683f982828e72bf9170dc123a` / squash merge `9f21a2b40f6af5ce57045fc4c1fbfc1bd6cb5b90` / 33 of 33 applicable permanent workflows.",
        f"Latest accepted repository implementation packet is PR #239 / accepted source `{ACCEPTED_SOURCE}` / squash merge `{MERGE_SHA}` / 8 of 8 applicable permanent workflows on one unchanged exact head.",
        "status latest repository packet",
    )
    text = replace_regex(
        text,
        r"^(Repository step 8 is accepted through PR #237[^\n]+)$",
        rf"\1\n\n## Accepted multi-plane affected-scope enforcement\n\n{ACCEPTED_PARAGRAPH}",
        "status accepted step 9",
    )
    text = replace_exact(
        text,
        "Repository step 9 is affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks.",
        "Repository step 10 is governed Customer Privacy access/export assembly.",
        "status next packet",
    )
    text = replace_exact(
        text,
        "Repository step 10 is governed Customer Privacy access/export assembly.",
        "Repository step 11 is owner-specific deletion, anonymization and supported crypto-shred execution.",
        "status following packet",
    )
    text = replace_exact(
        text,
        "- Stage E has a working foundation but is incomplete: real-diff packet validation and Rust broadening are accepted, while database/process/product/frontend/operations selection remains incomplete.",
        "- Stage E is complete through PR #239: deterministic Rust closure, exact repository-scope ownership, executable contract/Protobuf/API/migration/PostgreSQL/process/product/frontend/operations workflow coverage, exact-head evidence and unknown-path fail-closed enforcement are accepted.",
        "status stage E",
    )
    text = replace_exact(
        text,
        "-> 9. affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks — next",
        "-> 9. affected-scope expansion for contracts, migrations, PostgreSQL/process and product-plane checks — complete through PR #239\n-> 10. governed Customer Privacy access/export assembly — next",
        "status continuation",
    )
    return text


def architecture_test(text: str) -> str:
    pattern = (
        r"    def test_active_packet_is_machine_declared_and_generated\(self\) -> None:\n"
        r".*?"
        r"\n    def test_repository_map_matches_authoritative_inventory\(self\) -> None:"
    )
    replacement = '''    def test_active_packet_is_machine_declared_and_generated(self) -> None:
        self.assertEqual(self.packet["schema_version"], "crm.repository-packet/v1")
        self.assertEqual(self.packet["packet_id"], "repository-step-9-evidence-sync")
        self.assertEqual(self.packet["status"], "active")
        self.assertEqual(self.packet["baseline"]["sha"], "e40832ae21118dd7f033e2811ca466d1242a19f0")
        self.assertEqual(self.packet["tracking_issues"], [126, 194])
        for path in (
            "docs/ACTIVE_PACKET.md",
            "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
            "docs/IMPLEMENTATION_ROADMAP.md",
            "docs/PHASE8_DELIVERY_PLAN.md",
            "docs/PROJECT_STATUS.md",
            "repository-packet.json",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_repository_navigation.py",
        ):
            self.assertIn(path, self.packet["allowed_paths"])
        for path in (
            ".github/workflows/**",
            "Cargo.lock",
            "Cargo.toml",
            "affected-scope-policy.json",
            "apps/**",
            "contracts/**",
            "crates/**",
            "database/**",
            "modules/**",
            "packages/**",
            "proto/**",
            "schemas/**",
            "scripts/**",
            "services/**",
        ):
            self.assertIn(path, self.packet["forbidden_paths"])
        for check in ("Governance CI", "Affected Scope CI", "Rust CI", "Rust Generated Sync"):
            self.assertIn(check, self.packet["required_checks"])

        self.assertIn("Generated by scripts/generate_repository_navigation.py", self.active_packet)
        self.assertIn("repository-step-9-evidence-sync", self.active_packet)
        self.assertIn(self.packet["baseline"]["sha"], self.active_packet)
        self.assertRegex(self.active_packet, r"sha256:[0-9a-f]{64}")
        self.assertIn("orientation only", self.active_packet)

        for document in self.authoritative_status_documents:
            self.assertIn("PR #239", document)
            self.assertIn("e7ed45a7da5f14fa79e1ca4d23fc808004b6a642", document)
            self.assertIn("e40832ae21118dd7f033e2811ca466d1242a19f0", document)
            self.assertIn("8 of 8", document)
            self.assertIn("repository step 10", document.lower())

    def test_repository_map_matches_authoritative_inventory(self) -> None:'''
    return replace_regex(text, pattern, replacement, "architecture test block")


def navigation_test(text: str) -> str:
    pattern = (
        r"    def test_active_packet_declaration_is_valid_and_exact\(self\) -> None:\n"
        r".*?"
        r"\n    def test_affected_scope_workflow_executes_real_packet_check\(self\) -> None:"
    )
    replacement = '''    def test_active_packet_declaration_is_valid_and_exact(self) -> None:
        packet = load_packet(ROOT)
        self.assertEqual(packet["packet_id"], "repository-step-9-evidence-sync")
        self.assertEqual(packet["status"], "active")
        self.assertEqual(packet["baseline"]["ref"], "main")
        self.assertEqual(
            packet["baseline"]["sha"],
            "e40832ae21118dd7f033e2811ca466d1242a19f0",
        )
        self.assertEqual(packet["tracking_issues"], [126, 194])
        self.assertEqual(
            packet["allowed_paths"],
            [
                "docs/ACTIVE_PACKET.md",
                "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
                "docs/IMPLEMENTATION_ROADMAP.md",
                "docs/PHASE8_DELIVERY_PLAN.md",
                "docs/PROJECT_STATUS.md",
                "repository-packet.json",
                "tests/test_architecture_documentation_consistency.py",
                "tests/test_repository_navigation.py",
            ],
        )
        self.assertIn("Cargo.lock", packet["forbidden_paths"])
        self.assertEqual(
            packet["required_checks"],
            [
                "Affected Scope CI",
                "Governance CI",
                "Rust CI",
                "Rust Generated Sync",
            ],
        )
        self.assertIn(
            "repository step 10 is the only next implementation packet",
            packet["acceptance"],
        )

    def test_affected_scope_workflow_executes_real_packet_check(self) -> None:'''
    text = replace_regex(text, pattern, replacement, "navigation active packet block")
    changed_pattern = r"        changed_paths = \[\n.*?\n        \]\n        affected = \{"
    changed_replacement = '''        changed_paths = [
            "docs/ACTIVE_PACKET.md",
            "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
            "docs/IMPLEMENTATION_ROADMAP.md",
            "docs/PHASE8_DELIVERY_PLAN.md",
            "docs/PROJECT_STATUS.md",
            "repository-packet.json",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_repository_navigation.py",
        ]
        affected = {'''
    text = replace_regex(text, changed_pattern, changed_replacement, "navigation changed paths")
    text = replace_exact(
        text,
        '                    "c9f5bd515b2104ea172ca3089b8a0cdd5f152d9c"',
        '                    "e40832ae21118dd7f033e2811ca466d1242a19f0"',
        "navigation baseline fixture",
    )
    return text


update("docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md", architecture)
update("docs/IMPLEMENTATION_ROADMAP.md", roadmap)
update("docs/PHASE8_DELIVERY_PLAN.md", phase8)
update("docs/PROJECT_STATUS.md", project_status)
update("tests/test_architecture_documentation_consistency.py", architecture_test)
update("tests/test_repository_navigation.py", navigation_test)
