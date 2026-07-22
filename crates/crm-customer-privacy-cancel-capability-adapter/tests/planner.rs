use crm_capability_plan_support as support;
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{AggregatePresence, RecordMutation, TransactionalAggregatePlanner};
use crm_customer_privacy::{
    CompletionOutcome, MODULE_ID, PrivacyCase, PrivacyCaseKind, PrivacyCaseStatus,
    SubjectVerificationMethod,
};
use crm_customer_privacy_cancel_capability_adapter::{
    CANCEL_PRIVACY_CASE_CAPABILITY, CANCEL_PRIVACY_CASE_REQUEST_SCHEMA,
    CANCEL_PRIVACY_CASE_RESPONSE_SCHEMA, CustomerPrivacyCaseCancelCapabilityPlanner,
    PRIVACY_CASE_STATUS_CHANGED_EVENT_SCHEMA, PRIVACY_CASE_STATUS_CHANGED_EVENT_TYPE,
    cancellation_subject_lock_ids, capability_definition,
};
use crm_customer_privacy_persistence_adapter::{
    privacy_case_from_snapshot, privacy_case_persisted_payload, privacy_case_record_ref,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ErrorCategory, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId,
    RecordId, RecordSnapshot, RequestId, SchemaVersion, TenantId, TraceId,
};
use crm_proto_contracts::crm::customer_privacy::v1 as wire;
use prost::Message;

const TENANT: &str = "tenant-a";
const CASE_ID: &str = "privacy-case-a";
const CREATED_AT: i64 = 1_000_000_000;

fn request(
    case_id: Option<&str>,
    expected_version: i64,
    idempotency_key: &str,
    started_at: i64,
) -> CapabilityRequest {
    let command = wire::CancelPrivacyCaseRequest {
        privacy_case_ref: case_id.map(|privacy_case_id| wire::PrivacyCaseRef {
            privacy_case_id: privacy_case_id.to_owned(),
        }),
        expected_version,
    };
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new(MODULE_ID).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new(TENANT).unwrap(),
                actor_id: ActorId::try_new("privacy-officer").unwrap(),
                request_id: RequestId::try_new(format!("request-{idempotency_key}")).unwrap(),
                correlation_id: CorrelationId::try_new("correlation-privacy-cancel").unwrap(),
                causation_id: CausationId::try_new("causation-privacy-cancel").unwrap(),
                trace_id: TraceId::try_new("trace-privacy-cancel").unwrap(),
                capability_id: CapabilityId::try_new(CANCEL_PRIVACY_CASE_CAPABILITY).unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new(idempotency_key).unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(format!(
                    "transaction-{idempotency_key}"
                ))
                .unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: started_at,
            },
        },
        input: support::protobuf_payload(
            MODULE_ID,
            CANCEL_PRIVACY_CASE_REQUEST_SCHEMA,
            DataClass::Confidential,
            &command,
        )
        .unwrap(),
        input_hash: [41; 32],
        approval: None,
    }
}

fn new_case(previous_case_id: Option<&str>) -> PrivacyCase {
    PrivacyCase::new(
        RecordId::try_new(CASE_ID).unwrap(),
        TenantId::try_new(TENANT).unwrap(),
        PrivacyCaseKind::Erasure,
        SchemaVersion::try_new("privacy-policy/1").unwrap(),
        CREATED_AT,
        previous_case_id.map(|value| RecordId::try_new(value).unwrap()),
    )
    .unwrap()
}

fn snapshot(case: &PrivacyCase) -> RecordSnapshot {
    RecordSnapshot {
        reference: privacy_case_record_ref(case).unwrap(),
        version: i64::try_from(case.version()).unwrap(),
        payload: privacy_case_persisted_payload(case).unwrap(),
    }
}

fn verify_subject(case: &mut PrivacyCase, at: i64) {
    case.verify_subject(
        case.version(),
        RecordId::try_new("party-submitted").unwrap(),
        RecordId::try_new("party-canonical").unwrap(),
        7,
        SubjectVerificationMethod::VerifiedDocument,
        ActorId::try_new("privacy-officer").unwrap(),
        at,
    )
    .unwrap();
}

#[test]
fn target_resolution_and_required_request_fields_are_exact() {
    let definition = capability_definition().unwrap();
    let planner = CustomerPrivacyCaseCancelCapabilityPlanner;
    let valid = request(Some(CASE_ID), 1, "cancel-target", 2_000_000_000);
    let target = planner.target(&definition, &valid).unwrap();
    assert_eq!(target.presence, AggregatePresence::MustExist);
    assert_eq!(target.reference.record_type.as_str(), "customer-privacy.case");
    assert_eq!(target.reference.record_id.as_str(), CASE_ID);

    let missing = planner
        .target(
            &definition,
            &request(None, 1, "cancel-missing-ref", 2_000_000_000),
        )
        .unwrap_err();
    assert_eq!(missing.category, ErrorCategory::InvalidArgument);
    assert!(!missing.retryable);

    let non_positive = planner
        .target(
            &definition,
            &request(Some(CASE_ID), 0, "cancel-zero-version", 2_000_000_000),
        )
        .unwrap_err();
    assert_eq!(non_positive.category, ErrorCategory::InvalidArgument);
    assert!(!non_positive.retryable);

    let not_found = planner.plan(&definition, &valid, None).unwrap_err();
    assert_eq!(not_found.code, "CUSTOMER_PRIVACY_CASE_NOT_FOUND");
}

#[test]
fn draft_submitted_and_subject_verified_cases_cancel_by_one_exact_version() {
    let definition = capability_definition().unwrap();
    let planner = CustomerPrivacyCaseCancelCapabilityPlanner;

    let mut submitted = new_case(None);
    submitted.submit(1, 2_000_000_000).unwrap();
    let mut verified = submitted.clone();
    verify_subject(&mut verified, 3_000_000_000);

    for (name, case, expected_locks, at) in [
        ("draft", new_case(None), 0, 2_000_000_000),
        ("submitted", submitted, 0, 3_000_000_000),
        ("verified", verified, 1, 4_000_000_000),
    ] {
        let current = snapshot(&case);
        let expected_version = current.version;
        let plan = planner
            .plan(
                &definition,
                &request(
                    Some(CASE_ID),
                    expected_version,
                    &format!("cancel-{name}"),
                    at,
                ),
                Some(&current),
            )
            .unwrap();

        assert_eq!(plan.batch.records.len(), 1);
        assert_eq!(plan.batch.events.len(), 1);
        assert_eq!(plan.batch.audits.len(), 1);
        assert!(plan.batch.relationships.is_empty());
        assert_eq!(
            cancellation_subject_lock_ids(&current).unwrap().len(),
            expected_locks
        );

        match &plan.batch.records[0] {
            RecordMutation::Update {
                reference,
                expected_version: mutation_version,
                payload,
            } => {
                assert_eq!(reference, &current.reference);
                assert_eq!(*mutation_version, expected_version);
                assert_eq!(payload.data_class, DataClass::Confidential);
                let cancelled = privacy_case_from_snapshot(&RecordSnapshot {
                    reference: reference.clone(),
                    version: expected_version + 1,
                    payload: payload.clone(),
                })
                .unwrap();
                assert_eq!(cancelled.status(), PrivacyCaseStatus::Cancelled);
                assert_eq!(
                    cancelled.version(),
                    u64::try_from(expected_version + 1).unwrap()
                );
            }
            RecordMutation::Create { .. } => panic!("case.cancel must update its exact target"),
        }

        let output_payload = plan.output.as_ref().unwrap();
        assert_eq!(output_payload.owner.as_str(), MODULE_ID);
        assert_eq!(
            output_payload.schema_id.as_str(),
            CANCEL_PRIVACY_CASE_RESPONSE_SCHEMA
        );
        assert_eq!(output_payload.data_class, DataClass::Confidential);
        let output = wire::CancelPrivacyCaseResponse::decode(output_payload.bytes.as_slice())
            .unwrap()
            .privacy_case
            .unwrap();
        assert_eq!(output.status, wire::PrivacyCaseStatus::Cancelled as i32);
        assert_eq!(output.version, expected_version + 1);
        assert_eq!(output.updated_at_unix_ms, at / 1_000_000);

        let event = &plan.batch.events[0];
        assert_eq!(
            event.event.event_type.as_str(),
            PRIVACY_CASE_STATUS_CHANGED_EVENT_TYPE
        );
        assert_eq!(
            event.event.payload.schema_id.as_str(),
            PRIVACY_CASE_STATUS_CHANGED_EVENT_SCHEMA
        );
        assert_eq!(event.aggregate_version, expected_version + 1);
        assert_eq!(event.event_sequence, expected_version + 1);
        assert_eq!(
            event.event.expected_aggregate_version,
            Some(expected_version)
        );
        assert_eq!(event.event.deduplication_key, event.event_id);
        let event_case =
            wire::PrivacyCaseStatusChangedEvent::decode(event.event.payload.bytes.as_slice())
                .unwrap()
                .privacy_case
                .unwrap();
        assert_eq!(event_case, output);
    }
}

#[test]
fn cancellation_preserves_complete_immutable_lineage() {
    let definition = capability_definition().unwrap();
    let planner = CustomerPrivacyCaseCancelCapabilityPlanner;
    let mut case = new_case(Some("privacy-case-predecessor"));
    case.submit(1, 2_000_000_000).unwrap();
    verify_subject(&mut case, 3_000_000_000);
    case.begin_scoping(3, 4_000_000_000).unwrap();
    case.record_scope(
        4,
        RecordId::try_new("scope-snapshot-a").unwrap(),
        5_000_000_000,
    )
    .unwrap();
    case.record_plan(
        5,
        RecordId::try_new("privacy-action-plan-a").unwrap(),
        true,
        6_000_000_000,
    )
    .unwrap();
    case.approve(
        6,
        ActorId::try_new("privacy-approver").unwrap(),
        7_000_000_000,
    )
    .unwrap();

    let current = snapshot(&case);
    let plan = planner
        .plan(
            &definition,
            &request(Some(CASE_ID), 7, "cancel-lineage", 8_000_000_000),
            Some(&current),
        )
        .unwrap();
    let output = wire::CancelPrivacyCaseResponse::decode(
        plan.output.as_ref().unwrap().bytes.as_slice(),
    )
    .unwrap()
    .privacy_case
    .unwrap();

    assert_eq!(
        output
            .previous_privacy_case_ref
            .unwrap()
            .privacy_case_id,
        "privacy-case-predecessor"
    );
    let binding = output.subject_binding.unwrap();
    assert_eq!(
        binding.submitted_party_ref.unwrap().party_id,
        "party-submitted"
    );
    assert_eq!(
        binding.canonical_party_ref.unwrap().party_id,
        "party-canonical"
    );
    assert_eq!(binding.identity_resolution_generation, 7);
    assert_eq!(output.scope_snapshot_id, "scope-snapshot-a");
    assert_eq!(
        output
            .privacy_action_plan_ref
            .unwrap()
            .privacy_action_plan_id,
        "privacy-action-plan-a"
    );
    let approval = output.approval.unwrap();
    assert_eq!(approval.approved_by_actor_id, "privacy-approver");
    assert_eq!(approval.approved_at_unix_ms, 7_000);
}

#[test]
fn pending_rescope_is_preserved_and_produces_sorted_deduplicated_subject_locks() {
    let definition = capability_definition().unwrap();
    let planner = CustomerPrivacyCaseCancelCapabilityPlanner;
    let mut case = new_case(None);
    case.submit(1, 2_000_000_000).unwrap();
    verify_subject(&mut case, 3_000_000_000);
    case.require_rescope(
        3,
        RecordId::try_new("party-proposed").unwrap(),
        8,
        4_000_000_000,
    )
    .unwrap();
    let current = snapshot(&case);
    assert_eq!(
        cancellation_subject_lock_ids(&current)
            .unwrap()
            .into_iter()
            .map(|value| value.as_str().to_owned())
            .collect::<Vec<_>>(),
        vec!["party-canonical".to_owned(), "party-proposed".to_owned()]
    );

    let plan = planner
        .plan(
            &definition,
            &request(Some(CASE_ID), 4, "cancel-rescope", 5_000_000_000),
            Some(&current),
        )
        .unwrap();
    let output = wire::CancelPrivacyCaseResponse::decode(
        plan.output.as_ref().unwrap().bytes.as_slice(),
    )
    .unwrap()
    .privacy_case
    .unwrap();
    let rescope = output.pending_rescope.unwrap();
    assert_eq!(
        rescope
            .previous_canonical_party_ref
            .unwrap()
            .party_id,
        "party-canonical"
    );
    assert_eq!(
        rescope
            .proposed_canonical_party_ref
            .unwrap()
            .party_id,
        "party-proposed"
    );
    assert_eq!(rescope.previous_identity_resolution_generation, 7);
    assert_eq!(rescope.proposed_identity_resolution_generation, 8);
}

#[test]
fn every_terminal_state_rejects_cancellation() {
    let definition = capability_definition().unwrap();
    let planner = CustomerPrivacyCaseCancelCapabilityPlanner;

    let mut denied = new_case(None);
    denied.submit(1, 2_000_000_000).unwrap();
    denied.deny(2, 3_000_000_000).unwrap();

    let mut cancelled = new_case(None);
    cancelled.cancel(1, 2_000_000_000).unwrap();

    let mut failed_terminal = new_case(None);
    failed_terminal
        .fail_terminal(1, 2_000_000_000)
        .unwrap();

    let completed = completed_case(CompletionOutcome::Completed);
    let partially_completed = completed_case(CompletionOutcome::PartiallyCompleted);

    for (name, case) in [
        ("completed", completed),
        ("partially-completed", partially_completed),
        ("denied", denied),
        ("cancelled", cancelled),
        ("failed-terminal", failed_terminal),
    ] {
        let current = snapshot(&case);
        let error = planner
            .plan(
                &definition,
                &request(
                    Some(CASE_ID),
                    current.version,
                    &format!("cancel-terminal-{name}"),
                    20_000_000_000,
                ),
                Some(&current),
            )
            .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_PRIVACY_INVALID_TRANSITION");
        assert!(!error.retryable);
    }
}

fn completed_case(outcome: CompletionOutcome) -> PrivacyCase {
    let mut case = new_case(None);
    case.submit(1, 2_000_000_000).unwrap();
    verify_subject(&mut case, 3_000_000_000);
    case.begin_scoping(3, 4_000_000_000).unwrap();
    case.record_scope(
        4,
        RecordId::try_new("scope-snapshot-terminal").unwrap(),
        5_000_000_000,
    )
    .unwrap();
    case.record_plan(
        5,
        RecordId::try_new("privacy-action-plan-terminal").unwrap(),
        false,
        6_000_000_000,
    )
    .unwrap();
    case.begin_execution(6, 7_000_000_000).unwrap();
    case.begin_convergence(7, 8_000_000_000).unwrap();
    case.complete(8, outcome, 9_000_000_000).unwrap();
    case
}

#[test]
fn stale_cross_tenant_and_malformed_snapshots_fail_closed() {
    let definition = capability_definition().unwrap();
    let planner = CustomerPrivacyCaseCancelCapabilityPlanner;
    let current = snapshot(&new_case(None));

    let stale = planner
        .plan(
            &definition,
            &request(Some(CASE_ID), 2, "cancel-stale", 2_000_000_000),
            Some(&current),
        )
        .unwrap_err();
    assert_eq!(stale.code, "CUSTOMER_PRIVACY_VERSION_CONFLICT");
    assert!(stale.retryable);

    let cross_tenant_case = PrivacyCase::new(
        RecordId::try_new(CASE_ID).unwrap(),
        TenantId::try_new("tenant-b").unwrap(),
        PrivacyCaseKind::Erasure,
        SchemaVersion::try_new("privacy-policy/1").unwrap(),
        CREATED_AT,
        None,
    )
    .unwrap();
    let cross_tenant = planner
        .plan(
            &definition,
            &request(
                Some(CASE_ID),
                1,
                "cancel-cross-tenant",
                2_000_000_000,
            ),
            Some(&snapshot(&cross_tenant_case)),
        )
        .unwrap_err();
    assert_eq!(cross_tenant.code, "CUSTOMER_PRIVACY_CASE_NOT_FOUND");

    let mut malformed = current;
    malformed.payload.bytes = b"{\"raw_secret\":\"must-not-leak\"}".to_vec();
    let malformed_error = planner
        .plan(
            &definition,
            &request(Some(CASE_ID), 1, "cancel-malformed", 2_000_000_000),
            Some(&malformed),
        )
        .unwrap_err();
    assert_eq!(malformed_error.code, "CUSTOMER_PRIVACY_CASE_INVALID");
    assert!(!malformed_error.safe_message.contains("raw_secret"));
}
