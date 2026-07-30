from __future__ import annotations

from pathlib import Path


def replace_exact(path: str, old: str, new: str, label: str, expected: int = 1) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{label}: found {count}, expected {expected}")
    target.write_text(text.replace(old, new), encoding="utf-8")


replace_exact(
    "crates/crm-customer-data-operations-execution-composition/src/lib.rs",
    """//! reconciliation evidence. No direct cross-owner storage path exists here.

pub mod export_execution_reader;
""",
    """//! reconciliation evidence. No direct cross-owner storage path exists here.

pub use crm_core_files::{
    AppendImmutableFileChunk, CreateImmutableFileArtifact, FileArtifactAppendResult,
    FileArtifactMetadata, FileArtifactStatus, FinalizedFileArtifact, ImmutableFileArtifactStore,
};

pub mod export_execution_reader;
""",
    "CDO file-store boundary re-export",
)

replace_exact(
    "crates/crm-customer-privacy-production/src/access_export.rs",
    """use crate::legacy::CustomerPrivacyProductionDependencies;
pub use crm_customer_privacy_application::{
    AccessExportInvocation, AccessExportResult, PrivacyAccessExportService,
    PrivacyExportTargetPort, PrivacyExportTargetRequest, PrivacyExportTargetResult,
};
""",
    """use crate::legacy::CustomerPrivacyProductionDependencies;
pub use crm_customer_privacy::encode_access_export_manifest;
pub use crm_customer_privacy_application::{
    ACCESS_EXPORT_CAPABILITY_VERSION, ACCESS_EXPORT_REQUEST_CAPABILITY, AccessExportInvocation,
    AccessExportPersistencePort, AccessExportPreparation, AccessExportResult,
    PrivacyAccessExportService, PrivacyExportTargetPort, PrivacyExportTargetRequest,
    PrivacyExportTargetResult,
};
""",
    "Customer Privacy production access-export re-exports",
)

postgres_test_path = "crates/crm-application-runtime/tests/customer_privacy_access_export_postgres.rs"
replace_exact(
    postgres_test_path,
    """use crm_core_data::PostgresDataStore;
use crm_core_files::{
    AppendImmutableFileChunk, CreateImmutableFileArtifact, FileArtifactAppendResult,
    FileArtifactMetadata, FileArtifactStatus, FinalizedFileArtifact, ImmutableFileArtifactStore,
};
use crm_customer_data_operations_execution_composition::{
    PrivacyManifestExportPublisher, PrivacyManifestExportRequest,
};
use crm_customer_privacy_application::{
    ACCESS_EXPORT_CAPABILITY_VERSION, ACCESS_EXPORT_REQUEST_CAPABILITY, AccessExportInvocation,
    AccessExportPersistencePort, AccessExportPreparation,
};
use crm_customer_privacy_production::{
    ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES,
    ACTION_PLAN_STATE_RETENTION_POLICY_ID, ACTION_PLAN_STATE_SCHEMA_ID,
    ACTION_PLAN_STATE_SCHEMA_VERSION, ActionPlanningPolicy, ContributionCompletenessProof,
    DiscoveryOwnerScopeContribution, DiscoveryScopeSnapshot, EvidenceClass, ExecutionPreparation,
    OwnerExecutionInvocation, OwnerExecutionPersistencePort, OwnerScopeContract,
    OwnerScopeContribution, OwnerScopeRegistry, PostgresAccessExportPersistence,
    PostgresOwnerExecutionPersistence, PrivacyActionPlan, PrivacyCase, PrivacyCaseKind,
    PrivacyRetentionDecisionSet, ScopeDiscoveryLineage, ScopeResource, SubjectVerificationMethod,
    action_plan_state_descriptor_hash, encode_access_export_manifest, encode_action_plan_state,
    privacy_case_persisted_payload, retention_decision_persisted_payload,
};
""",
    """use crm_core_data::PostgresDataStore;
use crm_customer_data_operations_execution_composition::{
    AppendImmutableFileChunk, CreateImmutableFileArtifact, FileArtifactAppendResult,
    FileArtifactMetadata, FileArtifactStatus, FinalizedFileArtifact, ImmutableFileArtifactStore,
    PrivacyManifestExportPublisher, PrivacyManifestExportRequest,
};
use crm_customer_privacy_production::{
    ACCESS_EXPORT_CAPABILITY_VERSION, ACCESS_EXPORT_REQUEST_CAPABILITY, ACTION_PLAN_RECORD_TYPE,
    ACTION_PLAN_STATE_MAXIMUM_BYTES, ACTION_PLAN_STATE_RETENTION_POLICY_ID,
    ACTION_PLAN_STATE_SCHEMA_ID, ACTION_PLAN_STATE_SCHEMA_VERSION, AccessExportInvocation,
    AccessExportPersistencePort, AccessExportPreparation, ActionPlanningPolicy,
    ContributionCompletenessProof, DiscoveryOwnerScopeContribution, DiscoveryScopeSnapshot,
    EvidenceClass, ExecutionPreparation, OwnerExecutionInvocation, OwnerExecutionPersistencePort,
    OwnerScopeContract, OwnerScopeContribution, OwnerScopeRegistry,
    PostgresAccessExportPersistence, PostgresOwnerExecutionPersistence, PrivacyActionPlan,
    PrivacyCase, PrivacyCaseKind, PrivacyExportTargetResult, PrivacyOwnerActionOutcome,
    PrivacyRetentionDecisionSet, ScopeDiscoveryLineage, ScopeResource,
    SubjectVerificationMethod, action_plan_state_descriptor_hash,
    encode_access_export_manifest, encode_action_plan_state, privacy_case_persisted_payload,
    retention_decision_persisted_payload,
};
""",
    "integration test production-owned imports",
)
replace_exact(
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
replace_exact(
    postgres_test_path,
    "    let application_target = crm_customer_privacy_application::PrivacyExportTargetResult {\n",
    "    let application_target = PrivacyExportTargetResult {\n",
    "production target result type",
)
replace_exact(
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
replace_exact(
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

replace_exact(
    "modules/crm-customer-privacy/tests/access_export.rs",
    '    assert_eq!(error.code(), "CUSTOMER_PRIVACY_ACCESS_EXPORT_CONFLICT");\n',
    '    assert_eq!(error.code.as_str(), "CUSTOMER_PRIVACY_ACCESS_EXPORT_CONFLICT");\n',
    "access export error codes",
    expected=2,
)
