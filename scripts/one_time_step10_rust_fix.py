from __future__ import annotations

from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: found {count}, expected 1")
    target.write_text(text.replace(old, new), encoding="utf-8")


postgres_test_path = "crates/crm-application-runtime/tests/customer_privacy_access_export_postgres.rs"
replace_once(
    postgres_test_path,
    """    PostgresOwnerExecutionPersistence, PrivacyActionPlan, PrivacyCase, PrivacyCaseKind,
    PrivacyRetentionDecisionSet, ScopeDiscoveryLineage, ScopeResource, SubjectVerificationMethod,
""",
    """    PostgresOwnerExecutionPersistence, PrivacyActionPlan, PrivacyCase, PrivacyCaseKind,
    PrivacyOwnerActionOutcome, PrivacyRetentionDecisionSet, ScopeDiscoveryLineage, ScopeResource,
    SubjectVerificationMethod,
""",
    "owner outcome import",
)
replace_once(
    postgres_test_path,
    """    let owner_execution = PostgresOwnerExecutionPersistence::new(store.clone());
    let owner_result = owner_execution
        .prepare_next(&owner_invocation(
            plan.plan_id().clone(),
            decision.decision_id().clone(),
        ))
        .await
        .expect("complete zero-action Access owner execution");
    assert!(matches!(
        owner_result,
        ExecutionPreparation::Complete { .. }
    ));
""",
    """    let owner_execution = PostgresOwnerExecutionPersistence::new(store.clone());
    let owner_invocation = owner_invocation(
        plan.plan_id().clone(),
        decision.decision_id().clone(),
    );
    let retained_attempt = match owner_execution
        .prepare_next(&owner_invocation)
        .await
        .expect("prepare Access retain outcome")
    {
        ExecutionPreparation::Ready { attempt, .. } => attempt,
        other => panic!("expected Access retain attempt, got {other:?}"),
    };
    let retained_outcome = PrivacyOwnerActionOutcome::record(
        &retained_attempt,
        retained_attempt
            .coordinator_outcome_status()
            .expect("Access Retain is coordinator-owned"),
        None,
        EXECUTED_AT,
    )
    .unwrap();
    assert!(
        owner_execution
            .record_outcome(&owner_invocation, &retained_attempt, &retained_outcome)
            .await
            .expect("record Access retained outcome")
    );
    let owner_result = owner_execution
        .prepare_next(&owner_invocation)
        .await
        .expect("complete Access owner execution after retained outcome");
    assert!(matches!(owner_result, ExecutionPreparation::Complete { .. }));
""",
    "Access retained execution protocol",
)
replace_once(
    postgres_test_path,
    """            if command.chunk_index != artifact.metadata.next_chunk_index
                || Sha256::digest(&command.bytes).as_slice() != command.chunk_sha256
            {
""",
    """            let chunk_sha256: [u8; 32] = Sha256::digest(&command.bytes).into();
            if command.chunk_index != artifact.metadata.next_chunk_index
                || chunk_sha256 != command.chunk_sha256
            {
""",
    "test chunk digest comparison",
)
replace_once(
    postgres_test_path,
    """            if artifact.bytes.len() as u64 != artifact.metadata.expected_size_bytes
                || Sha256::digest(&artifact.bytes).as_slice() != artifact.metadata.expected_sha256
            {
""",
    """            let artifact_sha256: [u8; 32] = Sha256::digest(&artifact.bytes).into();
            if artifact.bytes.len() as u64 != artifact.metadata.expected_size_bytes
                || artifact_sha256 != artifact.metadata.expected_sha256
            {
""",
    "test artifact digest comparison",
)

pure_test_path = "modules/crm-customer-privacy/tests/access_export.rs"
replace_once(
    pure_test_path,
    '    assert_eq!(error.code(), "CUSTOMER_PRIVACY_ACCESS_EXPORT_CONFLICT");\n',
    '    assert_eq!(error.code.as_str(), "CUSTOMER_PRIVACY_ACCESS_EXPORT_CONFLICT");\n',
    "completed reference conflict code",
)
replace_once(
    pure_test_path,
    '    assert_eq!(error.code(), "CUSTOMER_PRIVACY_ACCESS_EXPORT_CONFLICT");\n',
    '    assert_eq!(error.code.as_str(), "CUSTOMER_PRIVACY_ACCESS_EXPORT_CONFLICT");\n',
    "erasure rejection code",
)
