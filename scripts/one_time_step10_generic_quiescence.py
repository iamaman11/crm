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


TEST = "services/crm-api/tests/generic_conformance_process_e2e.rs"
replace_exact(
    TEST,
    "use std::collections::BTreeSet;\nuse tonic::Code;\n",
    "use std::collections::BTreeSet;\nuse std::time::Duration;\nuse tokio::time::{Instant, sleep};\nuse tonic::Code;\n",
)
replace_exact(
    TEST,
    "    mutation_suite.assert_denied(\n        &conflicting_replay,\n        Code::Aborted,\n        \"CAPABILITY_IDEMPOTENCY_KEY_REUSED\",\n        false,\n        committed,\n        evidence_snapshot(&admin).await,\n    );\n\n    set_module_status(&admin, CUSTOMER_ENRICHMENT_MODULE, \"suspended\").await;\n",
    "    mutation_suite.assert_denied(\n        &conflicting_replay,\n        Code::Aborted,\n        \"CAPABILITY_IDEMPOTENCY_KEY_REUSED\",\n        false,\n        committed,\n        evidence_snapshot(&admin).await,\n    );\n\n    wait_for_customer_enrichment_dispatch(&admin).await;\n\n    set_module_status(&admin, CUSTOMER_ENRICHMENT_MODULE, \"suspended\").await;\n",
)
replace_exact(
    TEST,
    "async fn evidence_snapshot(pool: &PgPool) -> EvidenceSnapshot {\n",
    '''async fn wait_for_customer_enrichment_dispatch(pool: &PgPool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let completed: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND idempotency_scope = 'capability:customer_enrichment.request.dispatch:1.0.0' AND status = 'completed'",
        )
        .bind(TENANT_A)
        .fetch_one(pool)
        .await
        .expect("read Customer Enrichment dispatch completion evidence");
        if completed == 1 {
            return;
        }
        assert_eq!(completed, 0, "generic conformance created multiple dispatches");
        assert!(
            Instant::now() < deadline,
            "Customer Enrichment dispatch did not quiesce before query conformance"
        );
        sleep(Duration::from_millis(50)).await;
    }
}

async fn evidence_snapshot(pool: &PgPool) -> EvidenceSnapshot {
''',
)

packet_path = ROOT / "repository-packet.json"
packet = json.loads(packet_path.read_text(encoding="utf-8"))
allowed = packet["allowed_paths"]
for stale in (
    "crates/crm-application-runtime/src/customer_privacy_case_create_promotion.rs",
):
    if stale in allowed:
        allowed.remove(stale)
for path in (
    TEST,
):
    if path not in allowed:
        allowed.append(path)
packet["allowed_paths"] = sorted(allowed)
packet["forbidden_paths"] = [
    "services/crm-api/src/**" if value == "services/**" else value
    for value in packet["forbidden_paths"]
]
if "Generic Mutation Query Conformance CI" not in packet["required_checks"]:
    governance_index = packet["required_checks"].index("Governance CI")
    packet["required_checks"].insert(
        governance_index,
        "Generic Mutation Query Conformance CI",
    )
packet_path.write_text(json.dumps(packet, indent=2) + "\n", encoding="utf-8")

replace_exact(
    "tests/test_repository_navigation.py",
    '                "crates/crm-application-runtime/src/customer_privacy_case_create_promotion.rs",\n',
    "",
)
replace_exact(
    "tests/test_repository_navigation.py",
    '                "repository-packet.json",\n',
    '                "repository-packet.json",\n'
    '                "services/crm-api/tests/generic_conformance_process_e2e.rs",\n',
)
replace_exact(
    "tests/test_repository_navigation.py",
    '            "services/**",\n',
    '            "services/crm-api/src/**",\n',
)
replace_exact(
    "tests/test_repository_navigation.py",
    '                "Governance CI",\n',
    '                "Generic Mutation Query Conformance CI",\n'
    '                "Governance CI",\n',
)

replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '            "repository-packet.json",\n',
    '            "repository-packet.json",\n'
    '            "services/crm-api/tests/generic_conformance_process_e2e.rs",\n',
)
replace_exact(
    "tests/test_architecture_documentation_consistency.py",
    '            "Governance CI",\n',
    '            "Generic Mutation Query Conformance CI",\n'
    '            "Governance CI",\n',
)

subprocess.run(
    ["python", "scripts/generate_repository_navigation.py", "--write"],
    cwd=ROOT,
    check=True,
)
