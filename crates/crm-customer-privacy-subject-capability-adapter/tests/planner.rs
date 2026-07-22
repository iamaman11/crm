use crm_capability_plan_support as support;
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::{AggregatePresence, RecordMutation, TransactionalAggregatePlanner};
use crm_customer_privacy::{
    MODULE_ID, PrivacyCase, PrivacyCaseKind, PrivacyCaseStatus, SubjectVerificationMethod,
};
use crm_customer_privacy_persistence_adapter::{
    privacy_case_from_snapshot, privacy_case_persisted_payload, privacy_case_record_ref,
};
use crm_customer_privacy_subject_capability_adapter::{
    CustomerPrivacyCaseSubjectVerifyCapabilityPlanner, PRIVACY_CASE_SUBJECT_VERIFIED_EVENT_SCHEMA,
    PRIVACY_CASE_SUBJECT_VERIFIED_EVENT_TYPE, VERIFY_PRIVACY_CASE_SUBJECT_CAPABILITY,
    VERIFY_PRIVACY_CASE_SUBJECT_REQUEST_SCHEMA, capability_definition,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, RecordId,
    RecordSnapshot, RequestId, SchemaVersion, TenantId, TraceId,
};
use crm_proto_contracts::crm::customer::v1 as customer_wire;
use crm_proto_contracts::crm::customer_privacy::v1 as wire;
use prost::Message;

const CREATED_AT: i64 = 1_000_000_000;
const SUBMITTED_AT: i64 = 2_000_000_000;
const VERIFIED_AT: i64 = 3_000_000_000;

#[derive(Clone, Copy)]
struct RequestSpec<'a> {
    tenant: &'a str,
    case_id: Option<&'a str>,
    submitted_party_id: Option<&'a str>,
    canonical_party_id: Option<&'a str>,
    expected_version: i64,
    identity_resolution_generation: u64,
    verification_method: i32,
    idempotency_key: &'a str,
    started_at: i64,
}

fn base_spec() -> RequestSpec<'static> {
    RequestSpec {
        tenant: "tenant-a",
        case_id: Some("privacy-case-a"),
        submitted_party_id: Some("party-submitted"),
        canonical_party_id: Some("party-canonical"),
        expected_version: 2,
        identity_resolution_generation: 7,
        verification_method: wire::SubjectVerificationMethod::VerifiedDocument as i32,
        idempotency_key: "verify-subject-a",
        started_at: VERIFIED_AT,
    }
}

fn request(spec: RequestSpec<'_>) -> CapabilityRequest {
    let command = wire::VerifyPrivacyCaseSubjectRequest {
        privacy_case_ref: spec.case_id.map(|value| wire::PrivacyCaseRef {
            privacy_case_id: value.to_owned(),
        }),
        expected_version: spec.expected_version,
        submitted_party_ref: spec
            .submitted_party_id
            .map(|value| customer_wire::PartyRef {
                party_id: value.to_owned(),
            }),
        canonical_party_ref: spec
            .canonical_party_id
            .map(|value| customer_wire::PartyRef {
                party_id: value.to_owned(),
            }),
        identity_resolution_generation: spec.identity_resolution_generation,
        verification_method: spec.verification_method,
    };
    CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: ModuleId::try_new(MODULE_ID).unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new(spec.tenant).unwrap(),
                actor_id: ActorId::try_new("privacy-officer").unwrap(),
                request_id: RequestId::try_new("request-privacy-subject-verify").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-privacy-subject-verify")
                    .unwrap(),
                causation_id: CausationId::try_new("causation-privacy-subject-verify").unwrap(),
                trace_id: TraceId::try_new("trace-privacy-subject-verify").unwrap(),
                capability_id: CapabilityId::try_new(VERIFY_PRIVACY_CASE_SUBJECT_CAPABILITY)
                    .unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                idempotency_key: IdempotencyKey::try_new(spec.idempotency_key).unwrap(),
                business_transaction_id: BusinessTransactionId::try_new(
                    "transaction-privacy-subject-verify",
                )
                .unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: spec.started_at,
            },
        },
        input: support::protobuf_payload(
            MODULE_ID,
            VERIFY_PRIVACY_CASE_SUBJECT_REQUEST_SCHEMA,
            DataClass::Confidential,
            &command,
        )
        .unwrap(),
        input_hash: [23; 32],
        approval: None,
    }
}

fn snapshot(tenant: &str, case_id: &str, status: PrivacyCaseStatus) -> RecordSnapshot {
    let mut privacy_case = PrivacyCase::new(
        RecordId::try_new(case_id).unwrap(),
        TenantId::try_new(tenant).unwrap(),
        PrivacyCaseKind::Erasure,
        SchemaVersion::try_new("privacy-policy/1").unwrap(),
        CREATED_AT,
        None,
    )
    .unwrap();
    match status {
        PrivacyCaseStatus::Draft => {}
        PrivacyCaseStatus::Submitted => privacy_case.submit(1, SUBMITTED_AT).unwrap(),
        PrivacyCaseStatus::SubjectVerified => {
            privacy_case.submit(1, SUBMITTED_AT).unwrap();
            privacy_case
                .verify_subject(
                    2,
                    RecordId::try_new("party-submitted").unwrap(),
                    RecordId::try_new("party-canonical").unwrap(),
                    7,
                    SubjectVerificationMethod::VerifiedDocument,
                    ActorId::try_new("privacy-officer").unwrap(),
                    VERIFIED_AT,
                )
                .unwrap();
        }
        other => panic!("unsupported fixture state: {other:?}"),
    }
    RecordSnapshot {
        reference: privacy_case_record_ref(&privacy_case).unwrap(),
        version: i64::try_from(privacy_case.version()).unwrap(),
        payload: privacy_case_persisted_payload(&privacy_case).unwrap(),
    }
}

#[test]
fn successful_transition_binds_subject_and_emits_exact_evidence() {
    let definition = capability_definition().unwrap();
    let request = request(base_spec());
    let current = snapshot("tenant-a", "privacy-case-a", PrivacyCaseStatus::Submitted);
    let planner = CustomerPrivacyCaseSubjectVerifyCapabilityPlanner;

    let target = planner.target(&definition, &request).unwrap();
    assert_eq!(target.presence, AggregatePresence::MustExist);
    assert_eq!(target.reference, current.reference);

    let plan = planner.plan(&definition, &request, Some(&current)).unwrap();
    assert_eq!(plan.batch.records.len(), 1);
    assert_eq!(plan.batch.events.len(), 1);
    assert_eq!(plan.batch.audits.len(), 1);
    assert!(plan.batch.relationships.is_empty());

    match &plan.batch.records[0] {
        RecordMutation::Update {
            reference,
            expected_version,
            payload,
        } => {
            assert_eq!(reference, &current.reference);
            assert_eq!(*expected_version, 2);
            assert_eq!(payload.data_class, DataClass::Confidential);
            let verified = privacy_case_from_snapshot(&RecordSnapshot {
                reference: reference.clone(),
                version: 3,
                payload: payload.clone(),
            })
            .unwrap();
            assert_eq!(verified.status(), PrivacyCaseStatus::SubjectVerified);
            assert_eq!(verified.version(), 3);
            let binding = verified.subject_binding().unwrap();
            assert_eq!(binding.submitted_party_id.as_str(), "party-submitted");
            assert_eq!(binding.canonical_party_id.as_str(), "party-canonical");
            assert_eq!(binding.identity_resolution_generation, 7);
            assert_eq!(
                binding.verification_method,
                SubjectVerificationMethod::VerifiedDocument
            );
            assert_eq!(binding.verified_by.as_str(), "privacy-officer");
            assert_eq!(binding.verified_at_unix_nanos, VERIFIED_AT);
        }
        RecordMutation::Create { .. } => panic!("subject verification must update the case"),
    }

    let event = &plan.batch.events[0];
    assert_eq!(
        event.event.event_type.as_str(),
        PRIVACY_CASE_SUBJECT_VERIFIED_EVENT_TYPE
    );
    assert_eq!(
        event.event.payload.schema_id.as_str(),
        PRIVACY_CASE_SUBJECT_VERIFIED_EVENT_SCHEMA
    );
    assert_eq!(event.aggregate_version, 3);
    assert_eq!(event.event_sequence, 3);
    assert_eq!(event.event.expected_aggregate_version, Some(2));
    assert_eq!(event.event.deduplication_key, event.event_id);
    let event_message =
        wire::PrivacyCaseSubjectVerifiedEvent::decode(event.event.payload.bytes.as_slice())
            .unwrap();
    let event_binding = event_message.subject_binding.unwrap();
    assert_eq!(
        event_binding.submitted_party_ref.unwrap().party_id,
        "party-submitted"
    );
    assert_eq!(
        event_binding.canonical_party_ref.unwrap().party_id,
        "party-canonical"
    );
    assert_eq!(event_binding.identity_resolution_generation, 7);
    assert_eq!(
        event_binding.verification_method,
        wire::SubjectVerificationMethod::VerifiedDocument as i32
    );
    assert_eq!(event_binding.verified_by_actor_id, "privacy-officer");
    assert_eq!(event_binding.verified_at_unix_ms, 3_000);

    let output = wire::VerifyPrivacyCaseSubjectResponse::decode(
        plan.output.as_ref().unwrap().bytes.as_slice(),
    )
    .unwrap()
    .privacy_case
    .unwrap();
    assert_eq!(
        output.status,
        wire::PrivacyCaseStatus::SubjectVerified as i32
    );
    assert_eq!(output.version, 3);
    assert_eq!(output.updated_at_unix_ms, 3_000);
    assert_eq!(
        plan.batch.idempotency.scope,
        "capability:customer_privacy.case.subject.verify:1.0.0"
    );
    assert_eq!(plan.batch.idempotency.key, "verify-subject-a");
}

#[test]
fn planning_is_stable_for_exact_replay_input() {
    let definition = capability_definition().unwrap();
    let request = request(base_spec());
    let current = snapshot("tenant-a", "privacy-case-a", PrivacyCaseStatus::Submitted);
    let planner = CustomerPrivacyCaseSubjectVerifyCapabilityPlanner;

    let first = planner.plan(&definition, &request, Some(&current)).unwrap();
    let second = planner.plan(&definition, &request, Some(&current)).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first.batch.events[0].event_id,
        second.batch.events[0].event_id
    );
    assert_eq!(
        first.batch.audits[0].audit_record_id,
        second.batch.audits[0].audit_record_id
    );
}

#[test]
fn missing_references_and_invalid_scalars_fail_closed() {
    let definition = capability_definition().unwrap();
    let current = snapshot("tenant-a", "privacy-case-a", PrivacyCaseStatus::Submitted);
    let planner = CustomerPrivacyCaseSubjectVerifyCapabilityPlanner;

    let mut spec = base_spec();
    spec.case_id = None;
    assert_eq!(
        planner
            .target(&definition, &request(spec))
            .unwrap_err()
            .code,
        "SDK_INVALID_ARGUMENT"
    );

    let mut spec = base_spec();
    spec.submitted_party_id = None;
    assert_eq!(
        planner
            .plan(&definition, &request(spec), Some(&current))
            .unwrap_err()
            .code,
        "SDK_INVALID_ARGUMENT"
    );

    let mut spec = base_spec();
    spec.canonical_party_id = None;
    assert_eq!(
        planner
            .plan(&definition, &request(spec), Some(&current))
            .unwrap_err()
            .code,
        "SDK_INVALID_ARGUMENT"
    );

    for invalid_version in [0, -1] {
        let mut spec = base_spec();
        spec.expected_version = invalid_version;
        assert_eq!(
            planner
                .target(&definition, &request(spec))
                .unwrap_err()
                .code,
            "SDK_INVALID_ARGUMENT"
        );
    }

    let mut spec = base_spec();
    spec.identity_resolution_generation = 0;
    assert_eq!(
        planner
            .target(&definition, &request(spec))
            .unwrap_err()
            .code,
        "SDK_INVALID_ARGUMENT"
    );

    let mut spec = base_spec();
    spec.verification_method = i32::MAX;
    assert_eq!(
        planner
            .target(&definition, &request(spec))
            .unwrap_err()
            .code,
        "SDK_INVALID_ARGUMENT"
    );
}

#[test]
fn missing_cross_tenant_malformed_and_wrong_lifecycle_snapshots_are_bounded() {
    let definition = capability_definition().unwrap();
    let base_request = request(base_spec());
    let planner = CustomerPrivacyCaseSubjectVerifyCapabilityPlanner;

    assert_eq!(
        planner
            .plan(&definition, &base_request, None)
            .unwrap_err()
            .code,
        "CUSTOMER_PRIVACY_CASE_NOT_FOUND"
    );

    let cross_tenant = snapshot("tenant-b", "privacy-case-a", PrivacyCaseStatus::Submitted);
    assert_eq!(
        planner
            .plan(&definition, &base_request, Some(&cross_tenant))
            .unwrap_err()
            .code,
        "CUSTOMER_PRIVACY_CASE_NOT_FOUND"
    );

    let mut malformed = snapshot("tenant-a", "privacy-case-a", PrivacyCaseStatus::Submitted);
    malformed.payload.bytes = b"{\"raw_secret\":\"must-not-leak\"}".to_vec();
    let malformed_error = planner
        .plan(&definition, &base_request, Some(&malformed))
        .unwrap_err();
    assert_eq!(malformed_error.code, "CUSTOMER_PRIVACY_CASE_INVALID");
    assert_eq!(
        malformed_error.safe_message,
        "The privacy case could not be loaded safely."
    );
    assert!(!malformed_error.safe_message.contains("raw_secret"));

    let mut draft_spec = base_spec();
    draft_spec.expected_version = 1;
    let draft = snapshot("tenant-a", "privacy-case-a", PrivacyCaseStatus::Draft);
    assert_eq!(
        planner
            .plan(&definition, &request(draft_spec), Some(&draft))
            .unwrap_err()
            .code,
        "CUSTOMER_PRIVACY_INVALID_TRANSITION"
    );

    let mut verified_spec = base_spec();
    verified_spec.expected_version = 3;
    verified_spec.started_at = 4_000_000_000;
    let verified = snapshot(
        "tenant-a",
        "privacy-case-a",
        PrivacyCaseStatus::SubjectVerified,
    );
    assert_eq!(
        planner
            .plan(&definition, &request(verified_spec), Some(&verified))
            .unwrap_err()
            .code,
        "CUSTOMER_PRIVACY_INVALID_TRANSITION"
    );
}

#[test]
fn stale_version_and_non_monotonic_time_are_rejected() {
    let definition = capability_definition().unwrap();
    let current = snapshot("tenant-a", "privacy-case-a", PrivacyCaseStatus::Submitted);
    let planner = CustomerPrivacyCaseSubjectVerifyCapabilityPlanner;

    let mut stale_spec = base_spec();
    stale_spec.expected_version = 3;
    stale_spec.idempotency_key = "verify-subject-stale";
    let stale = planner
        .plan(&definition, &request(stale_spec), Some(&current))
        .unwrap_err();
    assert_eq!(stale.code, "CUSTOMER_PRIVACY_VERSION_CONFLICT");
    assert!(stale.retryable);

    let mut non_monotonic_spec = base_spec();
    non_monotonic_spec.started_at = SUBMITTED_AT - 1;
    non_monotonic_spec.idempotency_key = "verify-subject-time";
    let non_monotonic = planner
        .plan(&definition, &request(non_monotonic_spec), Some(&current))
        .unwrap_err();
    assert_eq!(non_monotonic.code, "CUSTOMER_PRIVACY_INVALID_ARGUMENT");
    assert!(!non_monotonic.retryable);
    assert_eq!(
        non_monotonic.safe_message,
        "The customer privacy subject could not be verified."
    );
}
