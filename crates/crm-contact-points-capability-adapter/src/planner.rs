use crate::{
    CREATE_CAPABILITY, CREATE_REQUEST_SCHEMA, CREATE_RESPONSE_SCHEMA, CREATED_EVENT_SCHEMA,
    CREATED_EVENT_TYPE, MODULE_ID, MUTATION_CAPABILITY_IDS, RECORD_TYPE, UPDATE_CAPABILITY,
    UPDATE_REQUEST_SCHEMA, UPDATE_RESPONSE_SCHEMA, UPDATED_EVENT_SCHEMA, UPDATED_EVENT_TYPE,
    VERIFIED_EVENT_SCHEMA, VERIFIED_EVENT_TYPE, VERIFY_CAPABILITY, VERIFY_REQUEST_SCHEMA,
    VERIFY_RESPONSE_SCHEMA,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_contact_points::{
    CONTACT_POINT_STATE_MAXIMUM_BYTES, CONTACT_POINT_STATE_RETENTION_POLICY_ID,
    CONTACT_POINT_STATE_SCHEMA_ID, CONTACT_POINT_STATE_SCHEMA_VERSION, ContactPoint, ContactPointId,
    ContactPointKind, ContactPointStatus, CreateContactPoint, PartyReference, UpdateContactPoint,
    VerificationState, VerifyContactPoint, contact_point_state_descriptor_hash,
    decode_contact_point_state, encode_contact_point_state,
};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_module_sdk::{DataClass, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::{
    contact_points::v1 as wire, core::v1 as core, customer::v1 as customer,
};

#[derive(Debug, Default, Clone, Copy)]
pub struct ContactPointCapabilityPlanner;

impl TransactionalAggregatePlanner for ContactPointCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let (contact_point_id, presence) = match definition.capability_id.as_str() {
            CREATE_CAPABILITY => {
                let command: wire::CreateContactPointRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        CREATE_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                (
                    contact_point_id_from_ref(
                        command.contact_point_ref,
                        "contact_point.contact_point_ref",
                    )?,
                    AggregatePresence::MustBeAbsent,
                )
            }
            UPDATE_CAPABILITY => {
                let command: wire::UpdateContactPointRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        UPDATE_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                (
                    contact_point_id_from_ref(
                        command.contact_point_ref,
                        "contact_point.contact_point_ref",
                    )?,
                    AggregatePresence::MustExist,
                )
            }
            VERIFY_CAPABILITY => {
                let command: wire::VerifyContactPointRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        VERIFY_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                (
                    contact_point_id_from_ref(
                        command.contact_point_ref,
                        "contact_point.contact_point_ref",
                    )?,
                    AggregatePresence::MustExist,
                )
            }
            _ => return Err(unsupported_capability()),
        };

        Ok(AggregateTarget {
            reference: support::record_ref(
                RECORD_TYPE,
                contact_point_id.as_str(),
                "contact_point.contact_point_ref.contact_point_id",
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
            CREATE_CAPABILITY => plan_create(definition, request, current),
            UPDATE_CAPABILITY => plan_update(definition, request, current),
            VERIFY_CAPABILITY => plan_verify(definition, request, current),
            _ => Err(unsupported_capability()),
        }
    }
}

fn plan_create(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    if current.is_some() {
        return Err(invalid_plan());
    }

    let command: wire::CreateContactPointRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        CREATE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let contact_point = ContactPoint::create(CreateContactPoint {
        contact_point_id: contact_point_id_from_ref(
            command.contact_point_ref,
            "contact_point.contact_point_ref",
        )?,
        party_ref: party_reference_from_ref(command.party_ref, "contact_point.party_ref")?,
        kind: contact_point_kind_from_wire(command.kind)?,
        value: command.value,
        preferred: command.preferred,
        valid_from_unix_nanos: optional_time(command.valid_from),
        valid_until_unix_nanos: optional_time(command.valid_until),
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;

    let aggregate = support::record_ref(
        RECORD_TYPE,
        contact_point.contact_point_id().as_str(),
        "contact_point.contact_point_ref.contact_point_id",
    )?;
    let public_contact_point = contact_point_to_wire(&contact_point);
    let output = support::protobuf_payload(
        MODULE_ID,
        CREATE_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::CreateContactPointResponse {
            contact_point: Some(public_contact_point.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: CREATED_EVENT_TYPE,
            event_schema_id: CREATED_EVENT_SCHEMA,
            aggregate_version: contact_point.version(),
            previous_version: None,
        },
        DataClass::Personal,
        &wire::ContactPointCreatedEvent {
            contact_point: Some(public_contact_point),
        },
    )?;

    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Create {
            reference: aggregate,
            payload: persisted_payload(&contact_point)?,
        },
        event,
        output,
    )
}

fn plan_update(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::UpdateContactPointRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        UPDATE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let requested_id = contact_point_id_from_ref(
        command.contact_point_ref,
        "contact_point.contact_point_ref",
    )?;
    if requested_id.as_str() != current.reference.record_id.as_str() {
        return Err(invalid_plan());
    }

    let mut contact_point = contact_point_from_snapshot(current)?;
    contact_point.apply_update(UpdateContactPoint {
        expected_version: command.expected_version,
        value: command.value,
        status: contact_point_status_from_wire(command.status)?,
        preferred: command.preferred,
        valid_from_unix_nanos: optional_time(command.valid_from),
        valid_until_unix_nanos: optional_time(command.valid_until),
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;

    let aggregate = current.reference.clone();
    let public_contact_point = contact_point_to_wire(&contact_point);
    let output = support::protobuf_payload(
        MODULE_ID,
        UPDATE_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::UpdateContactPointResponse {
            contact_point: Some(public_contact_point.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: UPDATED_EVENT_TYPE,
            event_schema_id: UPDATED_EVENT_SCHEMA,
            aggregate_version: contact_point.version(),
            previous_version: Some(current.version),
        },
        DataClass::Personal,
        &wire::ContactPointUpdatedEvent {
            contact_point: Some(public_contact_point),
        },
    )?;

    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version,
            payload: persisted_payload(&contact_point)?,
        },
        event,
        output,
    )
}

fn plan_verify(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::VerifyContactPointRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        VERIFY_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let requested_id = contact_point_id_from_ref(
        command.contact_point_ref,
        "contact_point.contact_point_ref",
    )?;
    if requested_id.as_str() != current.reference.record_id.as_str() {
        return Err(invalid_plan());
    }

    let mut contact_point = contact_point_from_snapshot(current)?;
    contact_point.verify(VerifyContactPoint {
        expected_version: command.expected_version,
        evidence_ref: command.evidence_ref,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;

    let aggregate = current.reference.clone();
    let public_contact_point = contact_point_to_wire(&contact_point);
    let output = support::protobuf_payload(
        MODULE_ID,
        VERIFY_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::VerifyContactPointResponse {
            contact_point: Some(public_contact_point.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: VERIFIED_EVENT_TYPE,
            event_schema_id: VERIFIED_EVENT_SCHEMA,
            aggregate_version: contact_point.version(),
            previous_version: Some(current.version),
        },
        DataClass::Personal,
        &wire::ContactPointVerifiedEvent {
            contact_point: Some(public_contact_point),
        },
    )?;

    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version,
            payload: persisted_payload(&contact_point)?,
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

pub fn contact_point_to_wire(contact_point: &ContactPoint) -> wire::ContactPoint {
    wire::ContactPoint {
        contact_point_ref: Some(customer::ContactPointRef {
            contact_point_id: contact_point.contact_point_id().as_str().to_owned(),
        }),
        party_ref: Some(customer::PartyRef {
            party_id: contact_point.party_ref().as_str().to_owned(),
        }),
        kind: match contact_point.kind() {
            ContactPointKind::Email => wire::ContactPointKind::Email as i32,
            ContactPointKind::Phone => wire::ContactPointKind::Phone as i32,
            ContactPointKind::Postal => wire::ContactPointKind::Postal as i32,
            ContactPointKind::Web => wire::ContactPointKind::Web as i32,
            ContactPointKind::Messaging => wire::ContactPointKind::Messaging as i32,
        },
        normalized_value: contact_point.normalized_value().to_owned(),
        display_value: contact_point.display_value().to_owned(),
        status: match contact_point.status() {
            ContactPointStatus::Active => wire::ContactPointStatus::Active as i32,
            ContactPointStatus::Inactive => wire::ContactPointStatus::Inactive as i32,
        },
        preferred: contact_point.preferred(),
        valid_from: contact_point
            .valid_from_unix_nanos()
            .map(|unix_nanos| core::UnixTime { unix_nanos }),
        valid_until: contact_point
            .valid_until_unix_nanos()
            .map(|unix_nanos| core::UnixTime { unix_nanos }),
        verification: Some(match contact_point.verification() {
            VerificationState::Unverified => wire::ContactPointVerification {
                status: wire::ContactPointVerificationStatus::Unverified as i32,
                evidence_ref: None,
                verified_at: None,
            },
            VerificationState::Verified(evidence) => wire::ContactPointVerification {
                status: wire::ContactPointVerificationStatus::Verified as i32,
                evidence_ref: Some(evidence.evidence_ref().to_owned()),
                verified_at: Some(core::UnixTime {
                    unix_nanos: evidence.verified_at_unix_nanos(),
                }),
            },
        }),
        resource_version: Some(customer::CustomerResourceVersion {
            version: contact_point.version(),
            created_at: Some(core::UnixTime {
                unix_nanos: contact_point.created_at_unix_nanos(),
            }),
            updated_at: Some(core::UnixTime {
                unix_nanos: contact_point.updated_at_unix_nanos(),
            }),
        }),
    }
}

pub fn persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: CONTACT_POINT_STATE_SCHEMA_ID,
        schema_version: CONTACT_POINT_STATE_SCHEMA_VERSION,
        descriptor_hash: contact_point_state_descriptor_hash(),
        maximum_size_bytes: CONTACT_POINT_STATE_MAXIMUM_BYTES,
        retention_policy_id: CONTACT_POINT_STATE_RETENTION_POLICY_ID,
    }
}

pub fn persisted_payload(
    contact_point: &ContactPoint,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        persisted_contract(),
        DataClass::Personal,
        encode_contact_point_state(contact_point)?,
    )
}

pub fn contact_point_from_snapshot(snapshot: &RecordSnapshot) -> Result<ContactPoint, SdkError> {
    let contact_point = decode_contact_point_state(support::persisted_json_bytes_with_data_class(
        snapshot,
        persisted_contract(),
        DataClass::Personal,
    )?)?;
    if contact_point.contact_point_id().as_str() != snapshot.reference.record_id.as_str()
        || contact_point.version() != snapshot.version
    {
        return Err(support::stored_data_error(
            "CONTACT_POINTS_PERSISTED_CONTACT_POINT_IDENTITY_INVALID",
        ));
    }
    Ok(contact_point)
}

pub fn referenced_party_id_from_create(
    request: &CapabilityRequest,
) -> Result<PartyReference, SdkError> {
    let command: wire::CreateContactPointRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        CREATE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    party_reference_from_ref(command.party_ref, "contact_point.party_ref")
}

fn contact_point_id_from_ref(
    contact_point_ref: Option<customer::ContactPointRef>,
    field: &'static str,
) -> Result<ContactPointId, SdkError> {
    let contact_point_ref = contact_point_ref.ok_or_else(|| {
        SdkError::invalid_argument(field, "Contact Point reference is required")
    })?;
    ContactPointId::try_new(contact_point_ref.contact_point_id)
}

fn party_reference_from_ref(
    party_ref: Option<customer::PartyRef>,
    field: &'static str,
) -> Result<PartyReference, SdkError> {
    let party_ref =
        party_ref.ok_or_else(|| SdkError::invalid_argument(field, "Party reference is required"))?;
    PartyReference::try_new(party_ref.party_id)
}

fn contact_point_kind_from_wire(value: i32) -> Result<ContactPointKind, SdkError> {
    match wire::ContactPointKind::try_from(value) {
        Ok(wire::ContactPointKind::Email) => Ok(ContactPointKind::Email),
        Ok(wire::ContactPointKind::Phone) => Ok(ContactPointKind::Phone),
        Ok(wire::ContactPointKind::Postal) => Ok(ContactPointKind::Postal),
        Ok(wire::ContactPointKind::Web) => Ok(ContactPointKind::Web),
        Ok(wire::ContactPointKind::Messaging) => Ok(ContactPointKind::Messaging),
        Ok(wire::ContactPointKind::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "contact_point.kind",
            "Contact Point kind must be EMAIL, PHONE, POSTAL, WEB, or MESSAGING",
        )),
    }
}

fn contact_point_status_from_wire(value: i32) -> Result<ContactPointStatus, SdkError> {
    match wire::ContactPointStatus::try_from(value) {
        Ok(wire::ContactPointStatus::Active) => Ok(ContactPointStatus::Active),
        Ok(wire::ContactPointStatus::Inactive) => Ok(ContactPointStatus::Inactive),
        Ok(wire::ContactPointStatus::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "contact_point.status",
            "Contact Point status must be ACTIVE or INACTIVE",
        )),
    }
}

fn optional_time(value: Option<core::UnixTime>) -> Option<i64> {
    value.map(|value| value.unix_nanos)
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if !MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
        || definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != request.context.execution.capability_id.as_str()
    {
        return Err(invalid_plan());
    }
    Ok(())
}

fn invalid_plan() -> SdkError {
    SdkError::new(
        "CONTACT_POINTS_CAPABILITY_PLAN_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Contact Point capability could not be planned safely.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "CONTACT_POINTS_CAPABILITY_UNSUPPORTED",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Contact Point capability is not configured.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_mapping_preserves_endpoint_verification_and_version_metadata() {
        let mut contact_point = ContactPoint::create(CreateContactPoint {
            contact_point_id: ContactPointId::try_new("contact-point-wire-1").unwrap(),
            party_ref: PartyReference::try_new("party-1").unwrap(),
            kind: ContactPointKind::Email,
            value: "Ada@EXAMPLE.COM".to_owned(),
            preferred: true,
            valid_from_unix_nanos: Some(10),
            valid_until_unix_nanos: Some(1_000),
            occurred_at_unix_nanos: 10,
        })
        .unwrap();
        contact_point
            .verify(VerifyContactPoint {
                expected_version: 1,
                evidence_ref: "evidence-1".to_owned(),
                occurred_at_unix_nanos: 20,
            })
            .unwrap();

        let wire = contact_point_to_wire(&contact_point);
        assert_eq!(
            wire.contact_point_ref.unwrap().contact_point_id,
            "contact-point-wire-1"
        );
        assert_eq!(wire.party_ref.unwrap().party_id, "party-1");
        assert_eq!(wire.kind, wire::ContactPointKind::Email as i32);
        assert_eq!(wire.normalized_value, "Ada@example.com");
        assert_eq!(
            wire.verification.unwrap().status,
            wire::ContactPointVerificationStatus::Verified as i32
        );
        assert_eq!(wire.resource_version.unwrap().version, 2);
    }
}
