from __future__ import annotations

import json
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


def require_state(path: str, required: str, forbidden: str) -> None:
    text = (ROOT / path).read_text(encoding="utf-8")
    if required not in text or forbidden in text:
        raise SystemExit(f"{path}: connector-applied permanent guard state is invalid")


# Isolate Customer Privacy access-export ledger evidence from the approval and
# owner-execution transactions that establish the immutable source lineage.
replace_exact(
    "crates/crm-application-runtime/tests/customer_privacy_access_export_postgres.rs",
    "AND capability_id = 'customer_privacy.case.approve'",
    "AND business_transaction_id LIKE 'privacy-access-export-%'",
    expected=2,
)

require_state(
    ".github/workflows/customer-privacy-approval.yml",
    'python scripts/repo.py packet-check --base "origin/${BASE_REF}"',
    'unexpected_runtime_changes=',
)
require_state(
    ".github/workflows/customer-privacy-owner-execution.yml",
    'python scripts/repo.py packet-check --base "${BASE_SHA}"',
    'forbidden="$(printf',
)

packet_path = ROOT / "repository-packet.json"
packet = json.loads(packet_path.read_text(encoding="utf-8"))
for path in (
    ".github/workflows/customer-privacy-approval.yml",
    ".github/workflows/customer-privacy-owner-execution.yml",
):
    if path not in packet["allowed_paths"]:
        packet["allowed_paths"].append(path)
packet["allowed_paths"] = sorted(packet["allowed_paths"])
if "Customer Privacy Approval CI" not in packet["required_checks"]:
    access_index = packet["required_checks"].index("Customer Privacy Access Export CI")
    packet["required_checks"].insert(access_index + 1, "Customer Privacy Approval CI")
packet_path.write_text(json.dumps(packet, indent=2) + "\n", encoding="utf-8")

workflow_paths = (
    '                ".github/workflows/customer-privacy-access-export.yml",\n',
    '                ".github/workflows/customer-privacy-access-export.yml",\n'
    '                ".github/workflows/customer-privacy-approval.yml",\n'
    '                ".github/workflows/customer-privacy-owner-execution.yml",\n',
)
replace_exact("tests/test_repository_navigation.py", *workflow_paths)
replace_exact(
    "tests/test_repository_navigation.py",
    '                "Customer Privacy Access Export CI",\n',
    '                "Customer Privacy Access Export CI",\n'
    '                "Customer Privacy Approval CI",\n',
)

replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '            ".github/workflows/customer-privacy-access-export.yml",\n',
    '            ".github/workflows/customer-privacy-access-export.yml",\n'
    '            ".github/workflows/customer-privacy-approval.yml",\n'
    '            ".github/workflows/customer-privacy-owner-execution.yml",\n',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '            "Customer Privacy Access Export CI",\n',
    '            "Customer Privacy Access Export CI",\n'
    '            "Customer Privacy Approval CI",\n',
)

subprocess.run(
    ["python", "scripts/generate_repository_navigation.py", "--write"],
    cwd=ROOT,
    check=True,
)
