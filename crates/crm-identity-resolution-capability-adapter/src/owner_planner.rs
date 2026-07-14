use crate::{
    CONFIRM_CAPABILITY, CONFIRM_REQUEST_SCHEMA, CONFIRM_RESPONSE_SCHEMA, CONFIRMED_EVENT_SCHEMA,
    CONFIRMED_EVENT_TYPE, DISMISS_CAPABILITY, DISMISS_REQUEST_SCHEMA, DISMISS_RESPONSE_SCHEMA,
    DISMISSED_EVENT_SCHEMA, DISMISSED_EVENT_TYPE, MODULE_ID, MUTATION_CAPABILITY_IDS,
    RECORD_TYPE, REFRESH_CAPABILITY, REFRESH_REQUEST_SCHEMA, REFRESH_RESPONSE_SCHEMA,
    REFRESHED_EVENT_SCHEMA, REFRESHED_EVENT_TYPE, REGISTER_CAPABILITY, REGISTER_REQUEST_SCHEMA,
    REGISTER_RESPONSE_SCHEMA, REGISTERED_EVENT_SCHEMA, REGISTERED_EVENT_TYPE,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_identity_resolution::{
    DUPLICATE_CANDIDATE_CASE_STATE_MAXIMUM_BYTES,
    DUPLICATE_CANDIDATE_CASE_STATE_RETENTION_POLICY_ID, DUPLICATE_CANDIDATE_CASE_STATE_SCHEMA_ID,
    DUPLICATE_CANDIDATE_CASE_STATE_SCHEMA_VERSION, CreateDuplicateCandidateCase,
    DecisionReasonCode, DecideDuplicateCandidateCase, DuplicateCandidateCase,
    DuplicateCandidateCaseId, DuplicateCandidateCaseStatus, EvidenceReference,
    MatchEvidenceSnapshot, MatchSignal, MatcherProfileCode, PartyReference,
    RefreshDuplicateCandidateEvidence, SignalKindCode, SignalSourceCode,
    decode_duplicate_candidate_case_state, duplicate_candidate_case_state_descriptor_hash,
    encode_duplicate_candidate_case_state,
};
use crm_module_sdk::{DataClass, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::{
    core::v1 as core, customer::v1 as customer, identity_resolution::v1 as wire,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct IdentityResolutionCapabilityPlanner;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyVersionExpectation {
    pub party_ref: PartyReference,
    pub expected_version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceReferenceScope {
    pub parties: Vec<PartyVersionExpectation>,
}

impl TransactionalAggregatePlanner for IdentityResolutionCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let (case_id, presence) = match definition.capability_id.as_str() {
            REGISTER_CAPABILITY => {
                let command: wire::RegisterDuplicateCandidateRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        REGISTER_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                let evidence = evidence_from_wire(
                    command.evidence,
                    "identity_resolution.candidate.evidence",
                )?;
                (
                    DuplicateCandidateCaseId::for_pair(evidence.pair())?,
                    AggregatePresence::MustBeAbsent,
                )
            }
            REFRESH_CAPABILITY => {
                let command: wire::RefreshDuplicateCandidateEvidenceRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        REFRESH_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                (
                    case_id_from_ref(command.case_ref, "identity_resolution.candidate.case_ref")?,
                    AggregatePresence::MustExist,
                )
            }
            DISMISS_CAPABILITY => {
                let command: wire::DismissDuplicateCandidateRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        DISMISS_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                (
                    case_id_from_ref(command.case_ref, "identity_resolution.candidate.case_ref")?,
                    AggregatePresence::MustExist,
                )
            }
            CONFIRM_CAPABILITY => {
                let command: wire::ConfirmDuplicateCandidateRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        CONFIRM_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                (
                    case_id_from_ref(command.case_ref, "identity_resolution.candidate.case_ref")?,
                    AggregatePresence::MustExist,
                )
            }
            _ => return Err(unsupported_capability()),
        };

        Ok(AggregateTarget {
            reference: support::record_ref(
                RECORD_TYPE,
                case_id.as_str(),
                "identity_resolution.candidate.case_ref.case_id",
            )?,
            presence,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_definition(definition, request)?;
        match definition.capability_id.as_str() {
            REGISTER_CAPABILITY => plan_register(definition, request, current),
            REFRESH_CAPABILITY => plan_refresh(definition, request, current),
            DISMISS_CAPABILITY => plan_dismiss(definition, request, current),
            CONFIRM_CAPABILITY => plan_confirm(definition, request, current),
            _ => Err(unsupported_capability()),
        }
    }
}

fn plan_register(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    if current.is_some() {
        return Err(invalid_plan());
    }
    let command: wire::RegisterDuplicateCandidateRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        REGISTER_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let candidate = DuplicateCandidateCase::create(CreateDuplicateCandidateCase {
        evidence: evidence_from_wire(command.evidence, "identity_resolution.candidate.evidence")?,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;
    let aggregate = support::record_ref(
        RECORD_TYPE,
        candidate.case_id().as_str(),
        "identity_resolution.candidate.case_ref.case_id",
    )?;
    let public_candidate = duplicate_candidate_case_to_wire(&candidate);
    let output = support::protobuf_payload(
        MODULE_ID,
        REGISTER_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::RegisterDuplicateCandidateResponse {
            candidate_case: Some(public_candidate.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: REGISTERED_EVENT_TYPE,
            event_schema_id: REGISTERED_EVENT_SCHEMA,
            aggregate_version: candidate.version(),
            previous_version: None,
        },
        DataClass::Personal,
        &wire::DuplicateCandidateRegisteredEvent {
            candidate_case: Some(public_candidate),
        },
    )?;
    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Create {
            reference: aggregate,
            payload: persisted_payload(&candidate)?,
        },
        event,
        output,
    )
}

fn plan_refresh(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::RefreshDuplicateCandidateEvidenceRequest =
        support::decode_request_with_data_class(
            request,
            MODULE_ID,
            REFRESH_REQUEST_SCHEMA,
            DataClass::Personal,
        )?;
    ensure_requested_case_matches(current, command.case_ref)?;
    let mut candidate = duplicate_candidate_case_from_snapshot(current)?;
    candidate.refresh_evidence(RefreshDuplicateCandidateEvidence {
        expected_version: command.expected_version,
        evidence: evidence_from_wire(command.evidence, "identity_resolution.candidate.evidence")?,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;
    update_plan(
        definition,
        request,
        current,
        candidate,
        REFRESH_RESPONSE_SCHEMA,
        REFRESHED_EVENT_TYPE,
        REFRESHED_EVENT_SCHEMA,
        UpdateKind::Refreshed,
    )
}

fn plan_dismiss(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::DismissDuplicateCandidateRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        DISMISS_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    ensure_requested_case_matches(current, command.case_ref)?;
    let mut candidate = duplicate_candidate_case_from_snapshot(current)?;
    candidate.dismiss(DecideDuplicateCandidateCase {
        expected_version: command.expected_version,
        reason: DecisionReasonCode::try_new(command.reason)?,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;
    update_plan(
        definition,
        request,
        current,
        candidate,
        DISMISS_RESPONSE_SCHEMA,
        DISMISSED_EVENT_TYPE,
        DISMISSED_EVENT_SCHEMA,
        UpdateKind::Dismissed,
    )
}

fn plan_confirm(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::ConfirmDuplicateCandidateRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        CONFIRM_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    ensure_requested_case_matches(current, command.case_ref)?;
    let mut candidate = duplicate_candidate_case_from_snapshot(current)?;
    candidate.confirm_duplicate(DecideDuplicateCandidateCase {
        expected_version: command.expected_version,
        reason: DecisionReasonCode::try_new(command.reason)?,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;
    update_plan(
        definition,
        request,
        current,
        candidate,
        CONFIRM_RESPONSE_SCHEMA,
        CONFIRMED_EVENT_TYPE,
        CONFIRMED_EVENT_SCHEMA,
        UpdateKind::Confirmed,
    )
}

#[derive(Debug, Clone, Copy)]
enum UpdateKind {
    Refreshed,
    Dismissed,
    Confirmed,
}

#[allow(clippy::too_many_arguments)]
fn update_plan(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: &RecordSnapshot,
    candidate: DuplicateCandidateCase,
    response_schema: &'static str,
    event_type: &'static str,
    event_schema: &'static str,
    kind: UpdateKind,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let aggregate = current.reference.clone();
    let public_candidate = duplicate_candidate_case_to_wire(&candidate);
    let output = match kind {
        UpdateKind::Refreshed => support::protobuf_payload(
            MODULE_ID,
            response_schema,
            DataClass::Personal,
            &wire::RefreshDuplicateCandidateEvidenceResponse {
                candidate_case: Some(public_candidate.clone()),
            },
        )?,
        UpdateKind::Dismissed => support::protobuf_payload(
            MODULE_ID,
            response_schema,
            DataClass::Personal,
            &wire::DismissDuplicateCandidateResponse {
                candidate_case: Some(public_candidate.clone()),
            },
        )?,
        UpdateKind::Confirmed => support::protobuf_payload(
            MODULE_ID,
            response_schema,
            DataClass::Personal,
            &wire::ConfirmDuplicateCandidateResponse {
                candidate_case: Some(public_candidate.clone()),
            },
        )?,
    };
    let event = match kind {
        UpdateKind::Refreshed => support::event_evidence_with_data_class(
            request,
            aggregate.clone(),
            MODULE_ID,
            EventSpec {
                event_type,
                event_schema_id: event_schema,
                aggregate_version: candidate.version(),
                previous_version: Some(current.version),
            },
            DataClass::Personal,
            &wire::DuplicateCandidateEvidenceRefreshedEvent {
                candidate_case: Some(public_candidate),
            },
        )?,
        UpdateKind::Dismissed => support::event_evidence_with_data_class(
            request,
            aggregate.clone(),
            MODULE_ID,
            EventSpec {
                event_type,
                event_schema_id: event_schema,
                aggregate_version: candidate.version(),
                previous_version: Some(current.version),
            },
            DataClass::Personal,
            &wire::DuplicateCandidateDismissedEvent {
                candidate_case: Some(public_candidate),
            },
        )?,
        UpdateKind::Confirmed => support::event_evidence_with_data_class(
            request,
            aggregate.clone(),
            MODULE_ID,
            EventSpec {
                event_type,
                event_schema_id: event_schema,
                aggregate_version: candidate.version(),
                previous_version: Some(current.version),
            },
            DataClass::Personal,
            &wire::DuplicateCandidateConfirmedEvent {
                candidate_case: Some(public_candidate),
            },
        )?,
    };
    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version,
            payload: persisted_payload(&candidate)?,
        },
        event,
        output,
    )
}

fn mutation_plan(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    aggregate: crm_module_sdk::RecordRef,
    mutation: RecordMutation,
    event: crm_core_data::EventEvidence,
    output: crm_module_sdk::TypedPayload,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let audit = support::audit_intent(
        request,
        &aggregate,
        event.aggregate_version,
        definition.capability_id.as_str(),
        &output.bytes,
    )?;
    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records: vec![mutation],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(definition, request)?,
            audits: vec![audit],
        },
        output: Some(output),
    })
}

pub fn evidence_reference_scope_from_request(
    capability_id: &str,
    request: &CapabilityRequest,
) -> Result<Option<EvidenceReferenceScope>, SdkError> {
    let evidence = match capability_id {
        REGISTER_CAPABILITY => {
            let command: wire::RegisterDuplicateCandidateRequest =
                support::decode_request_with_data_class(
                    request,
                    MODULE_ID,
                    REGISTER_REQUEST_SCHEMA,
                    DataClass::Personal,
                )?;
            Some(evidence_from_wire(
                command.evidence,
                "identity_resolution.candidate.evidence",
            )?)
        }
        REFRESH_CAPABILITY => {
            let command: wire::RefreshDuplicateCandidateEvidenceRequest =
                support::decode_request_with_data_class(
                    request,
                    MODULE_ID,
                    REFRESH_REQUEST_SCHEMA,
                    DataClass::Personal,
                )?;
            Some(evidence_from_wire(
                command.evidence,
                "identity_resolution.candidate.evidence",
            )?)
        }
        DISMISS_CAPABILITY | CONFIRM_CAPABILITY => None,
        _ => return Err(unsupported_capability()),
    };
    Ok(evidence.map(|evidence| EvidenceReferenceScope {
        parties: vec![
            PartyVersionExpectation {
                party_ref: evidence.pair().left().clone(),
                expected_version: evidence.left_party_version(),
            },
            PartyVersionExpectation {
                party_ref: evidence.pair().right().clone(),
                expected_version: evidence.right_party_version(),
            },
        ],
    }))
}

pub fn duplicate_candidate_case_to_wire(
    candidate: &DuplicateCandidateCase,
) -> wire::DuplicateCandidateCase {
    wire::DuplicateCandidateCase {
        case_ref: Some(wire::DuplicateCandidateCaseRef {
            case_id: candidate.case_id().as_str().to_owned(),
        }),
        left_party_ref: Some(customer::PartyRef {
            party_id: candidate.pair().left().as_str().to_owned(),
        }),
        right_party_ref: Some(customer::PartyRef {
            party_id: candidate.pair().right().as_str().to_owned(),
        }),
        evidence_history: candidate
            .evidence_history()
            .iter()
            .map(evidence_to_wire)
            .collect(),
        status: match candidate.status() {
            DuplicateCandidateCaseStatus::Open => wire::DuplicateCandidateCaseStatus::Open as i32,
            DuplicateCandidateCaseStatus::Dismissed => {
                wire::DuplicateCandidateCaseStatus::Dismissed as i32
            }
            DuplicateCandidateCaseStatus::ConfirmedDuplicate => {
                wire::DuplicateCandidateCaseStatus::ConfirmedDuplicate as i32
            }
        },
        decision_reason: candidate
            .decision_reason()
            .map(|reason| reason.as_str().to_owned())
            .unwrap_or_default(),
        resource_version: Some(customer::CustomerResourceVersion {
            version: candidate.version(),
            created_at: Some(core::UnixTime {
                unix_nanos: candidate.created_at_unix_nanos(),
            }),
            updated_at: Some(core::UnixTime {
                unix_nanos: candidate.updated_at_unix_nanos(),
            }),
        }),
    }
}

fn evidence_to_wire(evidence: &MatchEvidenceSnapshot) -> wire::MatchEvidenceSnapshot {
    wire::MatchEvidenceSnapshot {
        first_party_ref: Some(customer::PartyRef {
            party_id: evidence.pair().left().as_str().to_owned(),
        }),
        first_party_version: evidence.left_party_version(),
        second_party_ref: Some(customer::PartyRef {
            party_id: evidence.pair().right().as_str().to_owned(),
        }),
        second_party_version: evidence.right_party_version(),
        matcher_profile: evidence.matcher_profile().as_str().to_owned(),
        score_basis_points: u32::from(evidence.score_basis_points()),
        signals: evidence
            .signals()
            .iter()
            .map(|signal| wire::MatchSignal {
                kind: signal.kind().as_str().to_owned(),
                source: signal.source().as_str().to_owned(),
                evidence_ref: signal.evidence_ref().as_str().to_owned(),
                contribution_basis_points: i32::from(signal.contribution_basis_points()),
            })
            .collect(),
        generated_at: Some(core::UnixTime {
            unix_nanos: evidence.generated_at_unix_nanos(),
        }),
    }
}

fn evidence_from_wire(
    value: Option<wire::MatchEvidenceSnapshot>,
    field: &'static str,
) -> Result<MatchEvidenceSnapshot, SdkError> {
    let value =
        value.ok_or_else(|| SdkError::invalid_argument(field, "match evidence is required"))?;
    let score_basis_points = u16::try_from(value.score_basis_points).map_err(|_| {
        SdkError::invalid_argument(
            "identity_resolution.candidate.evidence.score_basis_points",
            "score is outside the supported range",
        )
    })?;
    let signals = value
        .signals
        .into_iter()
        .map(|signal| {
            let contribution_basis_points = i16::try_from(signal.contribution_basis_points).map_err(
                |_| {
                    SdkError::invalid_argument(
                        "identity_resolution.candidate.evidence.signal.contribution_basis_points",
                        "signal contribution is outside the supported range",
                    )
                },
            )?;
            MatchSignal::try_new(
                SignalKindCode::try_new(signal.kind)?,
                SignalSourceCode::try_new(signal.source)?,
                EvidenceReference::try_new(signal.evidence_ref)?,
                contribution_basis_points,
            )
        })
        .collect::<Result<Vec<_>, SdkError>>()?;
    MatchEvidenceSnapshot::try_new(
        party_reference_from_ref(
            value.first_party_ref,
            "identity_resolution.candidate.evidence.first_party_ref",
        )?,
        value.first_party_version,
        party_reference_from_ref(
            value.second_party_ref,
            "identity_resolution.candidate.evidence.second_party_ref",
        )?,
        value.second_party_version,
        MatcherProfileCode::try_new(value.matcher_profile)?,
        score_basis_points,
        signals,
        required_time(
            value.generated_at,
            "identity_resolution.candidate.evidence.generated_at",
        )?,
    )
}

pub fn persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: DUPLICATE_CANDIDATE_CASE_STATE_SCHEMA_ID,
        schema_version: DUPLICATE_CANDIDATE_CASE_STATE_SCHEMA_VERSION,
        descriptor_hash: duplicate_candidate_case_state_descriptor_hash(),
        maximum_size_bytes: DUPLICATE_CANDIDATE_CASE_STATE_MAXIMUM_BYTES,
        retention_policy_id: DUPLICATE_CANDIDATE_CASE_STATE_RETENTION_POLICY_ID,
    }
}

pub fn persisted_payload(
    candidate: &DuplicateCandidateCase,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        persisted_contract(),
        DataClass::Personal,
        encode_duplicate_candidate_case_state(candidate)?,
    )
}

pub fn duplicate_candidate_case_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<DuplicateCandidateCase, SdkError> {
    let candidate = decode_duplicate_candidate_case_state(
        support::persisted_json_bytes_with_data_class(
            snapshot,
            persisted_contract(),
            DataClass::Personal,
        )?,
    )?;
    if candidate.case_id().as_str() != snapshot.reference.record_id.as_str()
        || candidate.version() != snapshot.version
    {
        return Err(support::stored_data_error(
            "IDENTITY_RESOLUTION_PERSISTED_CASE_IDENTITY_INVALID",
        ));
    }
    Ok(candidate)
}

fn ensure_requested_case_matches(
    current: &RecordSnapshot,
    value: Option<wire::DuplicateCandidateCaseRef>,
) -> Result<(), SdkError> {
    let case_id = case_id_from_ref(value, "identity_resolution.candidate.case_ref")?;
    if case_id.as_str() != current.reference.record_id.as_str() {
        return Err(invalid_plan());
    }
    Ok(())
}

fn case_id_from_ref(
    value: Option<wire::DuplicateCandidateCaseRef>,
    field: &'static str,
) -> Result<DuplicateCandidateCaseId, SdkError> {
    let value =
        value.ok_or_else(|| SdkError::invalid_argument(field, "candidate case ref is required"))?;
    DuplicateCandidateCaseId::try_new(value.case_id)
}

fn party_reference_from_ref(
    value: Option<customer::PartyRef>,
    field: &'static str,
) -> Result<PartyReference, SdkError> {
    let value = value.ok_or_else(|| SdkError::invalid_argument(field, "Party ref is required"))?;
    PartyReference::try_new(value.party_id)
}

fn required_time(value: Option<core::UnixTime>, field: &'static str) -> Result<i64, SdkError> {
    value
        .map(|value| value.unix_nanos)
        .ok_or_else(|| SdkError::invalid_argument(field, "time is required"))
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if !MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
        || definition.owner_module_id.as_str() != MODULE_ID
        || request.context.module_id.as_str() != MODULE_ID
        || definition.capability_id != request.context.execution.capability_id
        || definition.capability_version != request.context.execution.capability_version
    {
        return Err(invalid_plan());
    }
    Ok(())
}

fn invalid_plan() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MUTATION_PLAN_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Identity Resolution mutation plan is invalid.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_CAPABILITY_UNSUPPORTED",
        crm_module_sdk::ErrorCategory::InvalidArgument,
        false,
        "The Identity Resolution mutation capability is unsupported.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persisted_contract_is_exact_and_nonzero() {
        let contract = persisted_contract();
        assert_eq!(contract.owner, MODULE_ID);
        assert_eq!(contract.schema_id, DUPLICATE_CANDIDATE_CASE_STATE_SCHEMA_ID);
        assert_ne!(contract.descriptor_hash, [0; 32]);
    }
}
