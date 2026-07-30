from __future__ import annotations

import json
from pathlib import Path
import re
import subprocess


ROOT = Path(__file__).resolve().parents[1]


def replace_exact(path: str, old: str, new: str, *, expected: int = 1) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{path}: expected {expected} exact matches, found {count}")
    target.write_text(text.replace(old, new), encoding="utf-8")


def replace_regex(path: str, pattern: str, replacement: str, *, expected: int = 1) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    updated, count = re.subn(pattern, replacement, text, flags=re.DOTALL)
    if count != expected:
        raise SystemExit(f"{path}: expected {expected} regex matches, found {count}")
    target.write_text(updated, encoding="utf-8")


# Isolate Customer Privacy access-export ledger evidence from the approval and
# owner-execution transactions that establish the immutable source lineage.
replace_exact(
    "crates/crm-application-runtime/tests/customer_privacy_access_export_postgres.rs",
    "AND capability_id = 'customer_privacy.case.approve'",
    "AND business_transaction_id LIKE 'privacy-access-export-%'",
    expected=2,
)

# Historical approval CI must validate its frozen approval behavior and the
# currently active packet, not reject every later runtime file forever.
replace_exact(
    ".github/workflows/customer-privacy-approval.yml",
    "      - name: Verify approval and directive invariants\n",
    "      - name: Verify approval invariants and active packet boundaries\n",
)
replace_regex(
    ".github/workflows/customer-privacy-approval.yml",
    r'''          runtime_changes=.*?          python - <<'PY'\n''',
    '''          python scripts/repo.py packet-check --base "origin/${BASE_REF}"\n          python - <<'PY'\n''',
)

# The authoritative packet checker supersedes the old step-8 path blacklist.
replace_exact(
    ".github/workflows/customer-privacy-owner-execution.yml",
    "      - name: Prove bounded non-effects\n",
    "      - name: Prove active packet boundaries and owner-execution non-effects\n",
)
replace_exact(
    ".github/workflows/customer-privacy-owner-execution.yml",
    '''          changed="$(git diff --name-only "${BASE_SHA}" HEAD)"\n          forbidden="$(printf '%s\\n' "${changed}" | grep -E '(^|/)Cargo\\.(toml|lock)$|modules/.*/module\\.yaml|^proto/|^services/crm-api/src/' || true)"\n          test -z "${forbidden}"\n''',
    '''          python scripts/repo.py packet-check --base "${BASE_SHA}"\n''',
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
    ["python", "scripts/generate_repository_navigation.py"],
    cwd=ROOT,
    check=True,
)
