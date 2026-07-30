from __future__ import annotations

from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[1]


def replace_exact(path: str, old: str, new: str, *, expected: int = 1) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{path}: expected {expected} exact matches, found {count}")
    target.write_text(text.replace(old, new), encoding="utf-8")


STEP10 = (
    "Repository step 10 is accepted through PR #241 / accepted source "
    "`2bb3a671deb18a6ae3bcea228ed01ed287b9de6a` / squash merge "
    "`19232f6f3e2ae87aabeb080257c1aac5477a6616` / 34 of 34 applicable permanent "
    "workflows on one unchanged exact head. It implements trusted-internal, replay-safe "
    "Customer Privacy access/export assembly through "
    "`customer_privacy.access_export.request@1.0.0` and the exact "
    "`customer_data.export.privacy.request@1.0.0` Customer Data Operations boundary. "
    "Customer Privacy persists an immutable strictly rehydrated manifest and stable job/artifact "
    "references before I/O; Customer Data Operations remains the durable job and immutable artifact "
    "owner. Deterministic identities recover pre-target and finalized-artifact/pre-link crash windows "
    "without a second logical job or artifact. Activation, exact case/snapshot/plan/checkpoint lineage, "
    "tenant and canonical-Party locking, registered initiating-capability provenance, FORCE RLS, "
    "transaction/outbox/audit/idempotency evidence, clean PostgreSQL, rollback/reapply and repeated "
    "acceptance are proven. Public inventory remains 7 mutations / 4 permission-aware queries / 0 "
    "workers; no public route or alternate download endpoint, destructive action, crate, dependency, "
    "`Cargo.lock`, Protobuf contract, migration, workspace package or generic-runtime business switch "
    "was introduced."
)

# Architecture plan.
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "Repository step 9 is accepted through PR #239 / accepted source `e7ed45a7da5f14fa79e1ca4d23fc808004b6a642` / squash merge `e40832ae21118dd7f033e2811ca466d1242a19f0` / 8 of 8 applicable permanent workflows on one unchanged exact head. It establishes one declarative affected-scope policy for contracts, Protobuf/API compatibility, database migrations, PostgreSQL acceptance, process/runtime acceptance, product-plane checks, frontend checks and operations checks; preserves deterministic Rust ownership and reverse closure; requires the real permanent workflow filters for every selected scope; blocks unknown non-Rust paths until classified; and records exact pull-request-head evidence. Shared workflow or policy changes widen validation to all 113 Rust workspace packages. The final 12-file packet changes no product behavior, Customer Privacy public inventory, runtime route, worker, contract, Protobuf message, schema, migration, crate, dependency, `Cargo.lock`, workspace package or generic-runtime business algorithm.\n\nThe next permitted implementation packet is repository step 10: governed Customer Privacy access/export assembly.\n",
    "Repository step 9 is accepted through PR #239 / accepted source `e7ed45a7da5f14fa79e1ca4d23fc808004b6a642` / squash merge `e40832ae21118dd7f033e2811ca466d1242a19f0` / 8 of 8 applicable permanent workflows on one unchanged exact head. It establishes one declarative affected-scope policy for contracts, Protobuf/API compatibility, database migrations, PostgreSQL acceptance, process/runtime acceptance, product-plane checks, frontend checks and operations checks; preserves deterministic Rust ownership and reverse closure; requires the real permanent workflow filters for every selected scope; blocks unknown non-Rust paths until classified; and records exact pull-request-head evidence. Shared workflow or policy changes widen validation to all 113 Rust workspace packages. The final 12-file packet changes no product behavior, Customer Privacy public inventory, runtime route, worker, contract, Protobuf message, schema, migration, crate, dependency, `Cargo.lock`, workspace package or generic-runtime business algorithm.\n\n" + STEP10 + "\n\nThe next permitted implementation packet is repository step 11: owner-specific deletion, anonymization and supported crypto-shred execution.\n",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "repository step 8 is complete through PR #237, and repository step 9 is complete through PR #239. None changes the master numbering.",
    "repository step 8 is complete through PR #237, repository step 9 is complete through PR #239, and repository step 10 is complete through PR #241. None changes the master numbering.",
)
replace_exact(
    "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
    "10. governed Customer Privacy access/export assembly — **Next**;\n11. owner-specific deletion, anonymization and supported crypto-shred execution;",
    "10. governed Customer Privacy access/export assembly — **Complete through PR #241**;\n11. owner-specific deletion, anonymization and supported crypto-shred execution — **Next**;",
)

# Implementation roadmap.
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "legal-hold/mandatory-retention precedence accepted through PR #230, and durable replay-safe owner execution/outcomes accepted through PR #237.",
    "legal-hold/mandatory-retention precedence accepted through PR #230, durable replay-safe owner execution/outcomes accepted through PR #237, and governed access/export assembly accepted through PR #241.",
)
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "PR #230, PR #235, PR #237 and PR #239.",
    "PR #230, PR #235, PR #237, PR #239 and PR #241.",
)
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "Repository steps 1–9 are complete through PR #218, PR #220, PR #222, PR #226, PR #228, PR #230, PR #235, PR #237 and PR #239; PR #224 accepted the smallest inserted prerequisite required by step 4, and PR #232 accepted the smallest lockfile-preservation prerequisite required before step 6. Repository step 10 — governed Customer Privacy access/export assembly — is the current next packet.",
    "Repository steps 1–10 are complete through PR #218, PR #220, PR #222, PR #226, PR #228, PR #230, PR #235, PR #237, PR #239 and PR #241; PR #224 accepted the smallest inserted prerequisite required by step 4, and PR #232 accepted the smallest lockfile-preservation prerequisite required before step 6. Repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — is the current next packet.",
)
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "Latest accepted runtime inventory is seven public mutations, four permission-aware public queries and zero Customer Privacy workers through PR #237. `customer_privacy.plan.build@1.0.0` and `customer_privacy.retention.evaluate@1.0.0` remain accepted trusted-internal runtime without public ingress; repository-step-8 owner execution is also trusted-internal and registers no public route or worker.",
    "Latest accepted runtime inventory is seven public mutations, four permission-aware public queries and zero Customer Privacy workers through PR #241. `customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0`, repository-step-8 owner execution and `customer_privacy.access_export.request@1.0.0` remain accepted trusted-internal runtime without public ingress or a Customer Privacy worker.",
)
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "### 5.14 Accepted multi-plane affected-scope enforcement\n\nRepository step 9 is accepted through PR #239 / accepted source `e7ed45a7da5f14fa79e1ca4d23fc808004b6a642` / squash merge `e40832ae21118dd7f033e2811ca466d1242a19f0` / 8 of 8 applicable permanent workflows on one unchanged exact head. It establishes one declarative affected-scope policy for contracts, Protobuf/API compatibility, database migrations, PostgreSQL acceptance, process/runtime acceptance, product-plane checks, frontend checks and operations checks; preserves deterministic Rust ownership and reverse closure; requires the real permanent workflow filters for every selected scope; blocks unknown non-Rust paths until classified; and records exact pull-request-head evidence. Shared workflow or policy changes widen validation to all 113 Rust workspace packages. The final 12-file packet changes no product behavior, Customer Privacy public inventory, runtime route, worker, contract, Protobuf message, schema, migration, crate, dependency, `Cargo.lock`, workspace package or generic-runtime business algorithm.\n\n## 6. Binding active sequence",
    "### 5.14 Accepted multi-plane affected-scope enforcement\n\nRepository step 9 is accepted through PR #239 / accepted source `e7ed45a7da5f14fa79e1ca4d23fc808004b6a642` / squash merge `e40832ae21118dd7f033e2811ca466d1242a19f0` / 8 of 8 applicable permanent workflows on one unchanged exact head. It establishes one declarative affected-scope policy for contracts, Protobuf/API compatibility, database migrations, PostgreSQL acceptance, process/runtime acceptance, product-plane checks, frontend checks and operations checks; preserves deterministic Rust ownership and reverse closure; requires the real permanent workflow filters for every selected scope; blocks unknown non-Rust paths until classified; and records exact pull-request-head evidence. Shared workflow or policy changes widen validation to all 113 Rust workspace packages. The final 12-file packet changes no product behavior, Customer Privacy public inventory, runtime route, worker, contract, Protobuf message, schema, migration, crate, dependency, `Cargo.lock`, workspace package or generic-runtime business algorithm.\n\n### 5.15 Accepted governed access/export assembly\n\n" + STEP10 + "\n\n## 6. Binding active sequence",
)
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "10. **Repository step 10 — governed Customer Privacy access/export assembly — Next.**\n11–19. **Repository steps 11–19 — continue exactly as numbered in the architecture plan.**",
    "10. **Repository step 10 — governed Customer Privacy access/export assembly — Complete through PR #241.**\n11. **Repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — Next.**\n12–19. **Repository steps 12–19 — continue exactly as numbered in the architecture plan.**",
)
replace_exact(
    "docs/IMPLEMENTATION_ROADMAP.md",
    "Repository step 10 is now the only next permitted implementation packet.",
    "Repository step 11 is now the only next permitted implementation packet.",
)

# Phase 8 plan.
replace_exact(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "Latest accepted public runtime inventory is seven mutations, four permission-aware public queries and zero Customer Privacy workers through PR #237. Trusted-internal `customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0` and repository-step-8 owner execution have no public ingress.",
    "Latest accepted public runtime inventory is seven mutations, four permission-aware public queries and zero Customer Privacy workers through PR #241. Trusted-internal `customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0`, repository-step-8 owner execution and `customer_privacy.access_export.request@1.0.0` have no public ingress.",
)
replace_exact(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "Repository step 10 — governed Customer Privacy access/export assembly — is now the next permitted implementation packet. Repository step 11 or later work remains blocked until step 10 is accepted and its evidence is synchronized.",
    STEP10 + "\n\nRepository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — is now the next permitted implementation packet. Repository step 12 or later work remains blocked until step 11 is accepted and its evidence is synchronized.",
)
replace_exact(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "10. repository step 10 — governed access/export assembly — **next**;\n11. repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution;",
    "10. repository step 10 — governed access/export assembly — **complete through PR #241**;\n11. repository step 11 — owner-specific deletion, anonymization and supported crypto-shred execution — **next**;",
)
replace_exact(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "A later step must not start while repository step 10 is unfinished.",
    "A later step must not start while repository step 11 is unfinished.",
)
replace_exact(
    "docs/PHASE8_DELIVERY_PLAN.md",
    "It closes only after restrictions and legal holds are extended with release/read lifecycle where required, access/export, deletion/anonymization/crypto-shred, tombstone/no-orphan behavior, convergence, worker lifecycle, frontend/operations evidence and full process acceptance are merged in the binding repository order.",
    "It closes only after restrictions and legal holds are extended with release/read lifecycle where required, deletion/anonymization/crypto-shred, tombstone/no-orphan behavior, convergence, worker lifecycle, frontend/operations evidence and full process acceptance are merged in the binding repository order.",
)

# Project status.
replace_exact(
    "docs/PROJECT_STATUS.md",
    "Latest accepted Customer Privacy runtime baseline is PR #237 / accepted source `f926ece93dc2b24683f982828e72bf9170dc123a` / squash merge `9f21a2b40f6af5ce57045fc4c1fbfc1bd6cb5b90` / 33 of 33 applicable permanent workflows on one unchanged source-authored head.\n\nLatest accepted repository implementation packet is PR #239 / accepted source `e7ed45a7da5f14fa79e1ca4d23fc808004b6a642` / squash merge `e40832ae21118dd7f033e2811ca466d1242a19f0` / 8 of 8 applicable permanent workflows on one unchanged exact head.",
    "Latest accepted Customer Privacy runtime baseline is PR #241 / accepted source `2bb3a671deb18a6ae3bcea228ed01ed287b9de6a` / squash merge `19232f6f3e2ae87aabeb080257c1aac5477a6616` / 34 of 34 applicable permanent workflows on one unchanged exact head.\n\nLatest accepted repository implementation packet is PR #241 / accepted source `2bb3a671deb18a6ae3bcea228ed01ed287b9de6a` / squash merge `19232f6f3e2ae87aabeb080257c1aac5477a6616` / 34 of 34 applicable permanent workflows on one unchanged exact head.",
)
replace_exact(
    "docs/PROJECT_STATUS.md",
    "`customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0` and repository-step-8 owner execution remain trusted-internal with no public route.",
    "`customer_privacy.plan.build@1.0.0`, `customer_privacy.retention.evaluate@1.0.0`, repository-step-8 owner execution and `customer_privacy.access_export.request@1.0.0` remain trusted-internal with no public route.",
)
replace_exact(
    "docs/PROJECT_STATUS.md",
    "## Next permitted repository packet\n\nRepository step 10 is governed Customer Privacy access/export assembly.\n\n## Following permitted repository packet\n\nRepository step 11 is owner-specific deletion, anonymization and supported crypto-shred execution.",
    "## Accepted governed access/export assembly\n\n" + STEP10 + "\n\n## Next permitted repository packet\n\nRepository step 11 is owner-specific deletion, anonymization and supported crypto-shred execution.\n\n## Following permitted repository packet\n\nRepository step 12 is the first measured behavior-neutral transitional domain-cluster consolidation.",
)
replace_exact(
    "docs/PROJECT_STATUS.md",
    "durable replay-safe owner execution/outcomes and first protected-owner integration are accepted; broader owner adoption",
    "durable replay-safe owner execution/outcomes, governed access/export assembly and first protected-owner integration are accepted; broader owner adoption",
)
replace_exact(
    "docs/PROJECT_STATUS.md",
    "-> 10. governed Customer Privacy access/export assembly — next\n",
    "-> 10. governed Customer Privacy access/export assembly — complete through PR #241\n-> 11. owner-specific deletion, anonymization and supported crypto-shred execution — next\n",
)

# Permanent packet guards.
replace_exact(
    "tests/test_repository_navigation.py",
    '        self.assertEqual(packet["packet_id"], "repository-step-10-access-export-assembly")',
    '        self.assertEqual(packet["packet_id"], "repository-step-10-evidence-sync")',
)
replace_exact(
    "tests/test_repository_navigation.py",
    '            "4e0077fbf09d94e5fd7e4c69e238d6d3878252b0",',
    '            "19232f6f3e2ae87aabeb080257c1aac5477a6616",',
)
start = '            packet["allowed_paths"],\n            [\n'
end = '            ],\n        )\n        for path in (\n'
path = ROOT / "tests/test_repository_navigation.py"
text = path.read_text(encoding="utf-8")
left = text.index(start) + len(start)
right = text.index(end, left)
allowed_block = ''.join(
    f'                "{item}",\n'
    for item in [
        "docs/ACTIVE_PACKET.md",
        "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
        "docs/IMPLEMENTATION_ROADMAP.md",
        "docs/PHASE8_DELIVERY_PLAN.md",
        "docs/PROJECT_STATUS.md",
        "repository-packet.json",
        "tests/test_architecture_documentation_consistency.py",
        "tests/test_repository_navigation.py",
    ]
)
path.write_text(text[:left] + allowed_block + text[right:], encoding="utf-8")
replace_exact(
    "tests/test_repository_navigation.py",
    '            packet["required_checks"],\n            [\n                "Affected Scope CI",\n                "Customer Privacy Access Export CI",\n                "Customer Privacy Approval CI",\n                "Customer Privacy Owner Execution CI",\n                "Generic Mutation Query Conformance CI",\n                "Governance CI",\n                "Rust CI",\n                "Rust Generated Sync",\n            ],',
    '            packet["required_checks"],\n            [\n                "Affected Scope CI",\n                "Governance CI",\n                "Rust CI",\n                "Rust Generated Sync",\n            ],',
)
replace_exact(
    "tests/test_repository_navigation.py",
    '        self.assertIn("repository step 11 is not started", packet["acceptance"])',
    '        self.assertIn(\n            "repository step 11 is the only next implementation packet",\n            packet["acceptance"],\n        )',
)

replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '        self.assertEqual(self.packet["packet_id"], "repository-step-10-access-export-assembly")',
    '        self.assertEqual(self.packet["packet_id"], "repository-step-10-evidence-sync")',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '        self.assertEqual(self.packet["baseline"]["sha"], "4e0077fbf09d94e5fd7e4c69e238d6d3878252b0")',
    '        self.assertEqual(self.packet["baseline"]["sha"], "19232f6f3e2ae87aabeb080257c1aac5477a6616")',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '''        for path in (
            ".github/workflows/customer-privacy-access-export.yml",
            ".github/workflows/customer-privacy-approval.yml",
            ".github/workflows/customer-privacy-owner-execution.yml",
            "crates/crm-application-runtime/tests/customer_privacy_access_export_postgres.rs",
            "crates/crm-customer-data-operations-execution-composition/src/privacy_export.rs",
            "crates/crm-customer-privacy-application/src/access_export.rs",
            "crates/crm-customer-privacy-postgres/src/access_export.rs",
            "crates/crm-customer-privacy-production/src/access_export.rs",
            "docs/generated/REPOSITORY_MAP.md",
            "modules/crm-customer-data-operations/module.yaml",
            "modules/crm-customer-privacy/src/access_export.rs",
            "modules/crm-customer-privacy/tests/access_export.rs",
            "repository-packet.json",
            "services/crm-api/tests/generic_conformance_process_e2e.rs",
        ):
''',
    '''        for path in (
            "docs/ACTIVE_PACKET.md",
            "docs/ARCHITECTURE_COMPLEXITY_AND_SCALABILITY_PLAN.md",
            "docs/IMPLEMENTATION_ROADMAP.md",
            "docs/PHASE8_DELIVERY_PLAN.md",
            "docs/PROJECT_STATUS.md",
            "repository-packet.json",
            "tests/test_architecture_documentation_consistency.py",
            "tests/test_repository_navigation.py",
        ):
''',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '''        for check in (
            "Affected Scope CI",
            "Customer Privacy Access Export CI",
            "Customer Privacy Approval CI",
            "Customer Privacy Owner Execution CI",
            "Generic Mutation Query Conformance CI",
            "Governance CI",
            "Rust CI",
            "Rust Generated Sync",
        ):
''',
    '''        for check in (
            "Affected Scope CI",
            "Governance CI",
            "Rust CI",
            "Rust Generated Sync",
        ):
''',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '        self.assertIn("repository step 11 is not started", self.packet["acceptance"])',
    '        self.assertIn(\n            "repository step 11 is the only next implementation packet",\n            self.packet["acceptance"],\n        )',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '        self.assertIn("repository-step-10-access-export-assembly", self.active_packet)',
    '        self.assertIn("repository-step-10-evidence-sync", self.active_packet)',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '''        for document in self.authoritative_status_documents:
            self.assertIn("PR #239", document)
            self.assertIn("e7ed45a7da5f14fa79e1ca4d23fc808004b6a642", document)
            self.assertIn("e40832ae21118dd7f033e2811ca466d1242a19f0", document)
            self.assertIn("8 of 8", document)
            self.assertIn("repository step 10", document.lower())
''',
    '''        for document in self.authoritative_status_documents:
            self.assertIn("PR #241", document)
            self.assertIn("2bb3a671deb18a6ae3bcea228ed01ed287b9de6a", document)
            self.assertIn("19232f6f3e2ae87aabeb080257c1aac5477a6616", document)
            self.assertIn("34 of 34", document)
            self.assertIn("repository step 11", document.lower())
''',
)

subprocess.run(
    ["python", "scripts/generate_repository_navigation.py", "--write"],
    cwd=ROOT,
    check=True,
)
