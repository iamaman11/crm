use crate::{
    CREATE_CAPABILITY, CREATE_REQUEST_SCHEMA, CREATE_RESPONSE_SCHEMA, CREATED_EVENT_SCHEMA,
    CREATED_EVENT_TYPE, MODULE_ID, MUTATION_CAPABILITY_IDS, RECORD_TYPE, WITHDRAW_CAPABILITY,
    WITHDRAW_REQUEST_SCHEMA, WITHDRAW_RESPONSE_SCHEMA, WITHDRAWN_EVENT_SCHEMA,
    WITHDRAWN_EVENT_TYPE,
};
use crm_capability_plan_support::{self as support, EventSpec, PersistedPayloadContract};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest};
use crm_consents::{
    CONSENT_AUTHORIZATION_STATE_MAXIMUM_BYTES, CONSENT_AUTHORIZATION_STATE_RETENTION_POLICY_ID,
    CONSENT_AUTHORIZATION_STATE_SCHEMA_ID, CONSENT_AUTHORIZATION_STATE_SCHEMA_VERSION,
    CommunicationChannel, ConsentAuthorization, ConsentAuthorizationId, ConsentEffect,
    ContactPointReference, CreateConsentAuthorization, EvidenceReference, JurisdictionCode,
    LegalBasisCode, PartyReference, PurposeCode, SourceCode, WithdrawConsentAuthorization,
    consent_authorization_state_descriptor_hash, decode_consent_authorization_state,
    encode_consent_authorization_state,
};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_module_sdk::{DataClass, RecordSnapshot, SdkError};
use crm_proto_contracts::crm::{consents::v1 as wire, core::v1 as core, customer::v1 as customer};

#[derive(Debug, Default, Clone, Copy)]
pub struct ConsentCapabilityPlanner;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateConsentReferenceScope {
    pub party_ref: PartyReference,
    pub contact_point_ref: Option<ContactPointReference>,
    pub channel: CommunicationChannel,
}

impl TransactionalAggregatePlanner for ConsentCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let (authorization_id, presence) = match definition.capability_id.as_str() {
            CREATE_CAPABILITY => {
                let command: wire::CreateConsentAuthorizationRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        CREATE_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                (
                    authorization_id_from_ref(
                        command.authorization_ref,
                        "consent_authorization.authorization_ref",
                    )?,
                    AggregatePresence::MustBeAbsent,
                )
            }
            WITHDRAW_CAPABILITY => {
                let command: wire::WithdrawConsentAuthorizationRequest =
                    support::decode_request_with_data_class(
                        request,
                        MODULE_ID,
                        WITHDRAW_REQUEST_SCHEMA,
                        DataClass::Personal,
                    )?;
                (
                    authorization_id_from_ref(
                        command.authorization_ref,
                        "consent_authorization.authorization_ref",
                    )?,
                    AggregatePresence::MustExist,
                )
            }
            _ => return Err(unsupported_capability()),
        };

        Ok(AggregateTarget {
            reference: support::record_ref(
                RECORD_TYPE,
                authorization_id.as_str(),
                "consent_authorization.authorization_ref.authorization_id",
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
            WITHDRAW_CAPABILITY => plan_withdraw(definition, request, current),
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
    let command: wire::CreateConsentAuthorizationRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        CREATE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    let authorization = ConsentAuthorization::create(CreateConsentAuthorization {
        authorization_id: authorization_id_from_ref(
            command.authorization_ref,
            "consent_authorization.authorization_ref",
        )?,
        party_ref: party_reference_from_ref(command.party_ref, "consent_authorization.party_ref")?,
        contact_point_ref: optional_contact_point_reference_from_ref(command.contact_point_ref)?,
        purpose: PurposeCode::try_new(command.purpose)?,
        channel: channel_from_wire(command.channel)?,
        effect: effect_from_wire(command.effect)?,
        legal_basis: LegalBasisCode::try_new(command.legal_basis)?,
        jurisdiction: JurisdictionCode::try_new(command.jurisdiction)?,
        source: SourceCode::try_new(command.source)?,
        evidence_ref: EvidenceReference::try_new(command.evidence_ref)?,
        effective_from_unix_nanos: required_time(
            command.effective_from,
            "consent_authorization.effective_from",
        )?,
        expires_at_unix_nanos: optional_time(command.expires_at),
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;

    let aggregate = support::record_ref(
        RECORD_TYPE,
        authorization.authorization_id().as_str(),
        "consent_authorization.authorization_ref.authorization_id",
    )?;
    let public_authorization = consent_authorization_to_wire(&authorization);
    let output = support::protobuf_payload(
        MODULE_ID,
        CREATE_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::CreateConsentAuthorizationResponse {
            authorization: Some(public_authorization.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: CREATED_EVENT_TYPE,
            event_schema_id: CREATED_EVENT_SCHEMA,
            aggregate_version: authorization.version(),
            previous_version: None,
        },
        DataClass::Personal,
        &wire::ConsentAuthorizationCreatedEvent {
            authorization: Some(public_authorization),
        },
    )?;

    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Create {
            reference: aggregate,
            payload: persisted_payload(&authorization)?,
        },
        event,
        output,
    )
}

fn plan_withdraw(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: Option<&RecordSnapshot>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let current = current.ok_or_else(invalid_plan)?;
    let command: wire::WithdrawConsentAuthorizationRequest =
        support::decode_request_with_data_class(
            request,
            MODULE_ID,
            WITHDRAW_REQUEST_SCHEMA,
            DataClass::Personal,
        )?;
    let requested_id = authorization_id_from_ref(
        command.authorization_ref,
        "consent_authorization.authorization_ref",
    )?;
    if requested_id.as_str() != current.reference.record_id.as_str() {
        return Err(invalid_plan());
    }

    let mut authorization = consent_authorization_from_snapshot(current)?;
    authorization.withdraw(WithdrawConsentAuthorization {
        expected_version: command.expected_version,
        occurred_at_unix_nanos: request.context.execution.request_started_at_unix_nanos,
    })?;

    let aggregate = current.reference.clone();
    let public_authorization = consent_authorization_to_wire(&authorization);
    let output = support::protobuf_payload(
        MODULE_ID,
        WITHDRAW_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::WithdrawConsentAuthorizationResponse {
            authorization: Some(public_authorization.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: WITHDRAWN_EVENT_TYPE,
            event_schema_id: WITHDRAWN_EVENT_SCHEMA,
            aggregate_version: authorization.version(),
            previous_version: Some(current.version),
        },
        DataClass::Personal,
        &wire::ConsentAuthorizationWithdrawnEvent {
            authorization: Some(public_authorization),
        },
    )?;

    mutation_plan(
        definition,
        request,
        aggregate.clone(),
        RecordMutation::Update {
            reference: aggregate,
            expected_version: current.version,
            payload: persisted_payload(&authorization)?,
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

pub fn referenced_scope_from_create(
    request: &CapabilityRequest,
) -> Result<CreateConsentReferenceScope, SdkError> {
    let command: wire::CreateConsentAuthorizationRequest = support::decode_request_with_data_class(
        request,
        MODULE_ID,
        CREATE_REQUEST_SCHEMA,
        DataClass::Personal,
    )?;
    Ok(CreateConsentReferenceScope {
        party_ref: party_reference_from_ref(command.party_ref, "consent_authorization.party_ref")?,
        contact_point_ref: optional_contact_point_reference_from_ref(command.contact_point_ref)?,
        channel: channel_from_wire(command.channel)?,
    })
}

pub fn consent_authorization_to_wire(
    authorization: &ConsentAuthorization,
) -> wire::ConsentAuthorization {
    wire::ConsentAuthorization {
        authorization_ref: Some(wire::ConsentAuthorizationRef {
            authorization_id: authorization.authorization_id().as_str().to_owned(),
        }),
        party_ref: Some(customer::PartyRef {
            party_id: authorization.party_ref().as_str().to_owned(),
        }),
        contact_point_ref: authorization.contact_point_ref().map(|reference| {
            customer::ContactPointRef {
                contact_point_id: reference.as_str().to_owned(),
            }
        }),
        purpose: authorization.purpose().as_str().to_owned(),
        channel: match authorization.channel() {
            CommunicationChannel::Email => wire::CommunicationChannel::Email as i32,
            CommunicationChannel::Phone => wire::CommunicationChannel::Phone as i32,
            CommunicationChannel::Sms => wire::CommunicationChannel::Sms as i32,
            CommunicationChannel::Postal => wire::CommunicationChannel::Postal as i32,
            CommunicationChannel::Messaging => wire::CommunicationChannel::Messaging as i32,
            CommunicationChannel::Push => wire::CommunicationChannel::Push as i32,
        },
        effect: match authorization.effect() {
            ConsentEffect::Grant => wire::ConsentEffect::Grant as i32,
            ConsentEffect::Deny => wire::ConsentEffect::Deny as i32,
        },
        legal_basis: authorization.legal_basis().as_str().to_owned(),
        jurisdiction: authorization.jurisdiction().as_str().to_owned(),
        source: authorization.source().as_str().to_owned(),
        evidence_ref: authorization.evidence_ref().as_str().to_owned(),
        effective_from: Some(core::UnixTime {
            unix_nanos: authorization.effective_from_unix_nanos(),
        }),
        expires_at: authorization
            .expires_at_unix_nanos()
            .map(|unix_nanos| core::UnixTime { unix_nanos }),
        status: match authorization.status() {
            crm_consents::ConsentAuthorizationStatus::Active => {
                wire::ConsentAuthorizationStatus::Active as i32
            }
            crm_consents::ConsentAuthorizationStatus::Withdrawn => {
                wire::ConsentAuthorizationStatus::Withdrawn as i32
            }
        },
        withdrawn_at: authorization
            .withdrawn_at_unix_nanos()
            .map(|unix_nanos| core::UnixTime { unix_nanos }),
        resource_version: Some(customer::CustomerResourceVersion {
            version: authorization.version(),
            created_at: Some(core::UnixTime {
                unix_nanos: authorization.created_at_unix_nanos(),
            }),
            updated_at: Some(core::UnixTime {
                unix_nanos: authorization.updated_at_unix_nanos(),
            }),
        }),
    }
}

pub fn persisted_contract() -> PersistedPayloadContract<'static> {
    PersistedPayloadContract {
        owner: MODULE_ID,
        schema_id: CONSENT_AUTHORIZATION_STATE_SCHEMA_ID,
        schema_version: CONSENT_AUTHORIZATION_STATE_SCHEMA_VERSION,
        descriptor_hash: consent_authorization_state_descriptor_hash(),
        maximum_size_bytes: CONSENT_AUTHORIZATION_STATE_MAXIMUM_BYTES,
        retention_policy_id: CONSENT_AUTHORIZATION_STATE_RETENTION_POLICY_ID,
    }
}

pub fn persisted_payload(
    authorization: &ConsentAuthorization,
) -> Result<crm_module_sdk::TypedPayload, SdkError> {
    support::persisted_json_payload_with_data_class(
        persisted_contract(),
        DataClass::Personal,
        encode_consent_authorization_state(authorization)?,
    )
}

pub fn consent_authorization_from_snapshot(
    snapshot: &RecordSnapshot,
) -> Result<ConsentAuthorization, SdkError> {
    let authorization =
        decode_consent_authorization_state(support::persisted_json_bytes_with_data_class(
            snapshot,
            persisted_contract(),
            DataClass::Personal,
        )?)?;
    if authorization.authorization_id().as_str() != snapshot.reference.record_id.as_str()
        || authorization.version() != snapshot.version
    {
        return Err(support::stored_data_error(
            "CONSENTS_PERSISTED_AUTHORIZATION_IDENTITY_INVALID",
        ));
    }
    Ok(authorization)
}

fn authorization_id_from_ref(
    value: Option<wire::ConsentAuthorizationRef>,
    field: &'static str,
) -> Result<ConsentAuthorizationId, SdkError> {
    let value =
        value.ok_or_else(|| SdkError::invalid_argument(field, "authorization ref is required"))?;
    ConsentAuthorizationId::try_new(value.authorization_id)
}

fn party_reference_from_ref(
    value: Option<customer::PartyRef>,
    field: &'static str,
) -> Result<PartyReference, SdkError> {
    let value = value.ok_or_else(|| SdkError::invalid_argument(field, "Party ref is required"))?;
    PartyReference::try_new(value.party_id)
}

fn optional_contact_point_reference_from_ref(
    value: Option<customer::ContactPointRef>,
) -> Result<Option<ContactPointReference>, SdkError> {
    value
        .map(|value| ContactPointReference::try_new(value.contact_point_id))
        .transpose()
}

fn required_time(value: Option<core::UnixTime>, field: &'static str) -> Result<i64, SdkError> {
    value
        .map(|value| value.unix_nanos)
        .ok_or_else(|| SdkError::invalid_argument(field, "time is required"))
}

fn optional_time(value: Option<core::UnixTime>) -> Option<i64> {
    value.map(|value| value.unix_nanos)
}

fn channel_from_wire(value: i32) -> Result<CommunicationChannel, SdkError> {
    match wire::CommunicationChannel::try_from(value) {
        Ok(wire::CommunicationChannel::Email) => Ok(CommunicationChannel::Email),
        Ok(wire::CommunicationChannel::Phone) => Ok(CommunicationChannel::Phone),
        Ok(wire::CommunicationChannel::Sms) => Ok(CommunicationChannel::Sms),
        Ok(wire::CommunicationChannel::Postal) => Ok(CommunicationChannel::Postal),
        Ok(wire::CommunicationChannel::Messaging) => Ok(CommunicationChannel::Messaging),
        Ok(wire::CommunicationChannel::Push) => Ok(CommunicationChannel::Push),
        Ok(wire::CommunicationChannel::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "consent_authorization.channel",
            "communication channel is invalid",
        )),
    }
}

fn effect_from_wire(value: i32) -> Result<ConsentEffect, SdkError> {
    match wire::ConsentEffect::try_from(value) {
        Ok(wire::ConsentEffect::Grant) => Ok(ConsentEffect::Grant),
        Ok(wire::ConsentEffect::Deny) => Ok(ConsentEffect::Deny),
        Ok(wire::ConsentEffect::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "consent_authorization.effect",
            "Consent effect is invalid",
        )),
    }
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
        "CONSENTS_MUTATION_PLAN_INVALID",
        crm_module_sdk::ErrorCategory::Internal,
        false,
        "The Consent mutation plan is invalid.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "CONSENTS_CAPABILITY_UNSUPPORTED",
        crm_module_sdk::ErrorCategory::InvalidArgument,
        false,
        "The Consent mutation capability is unsupported.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_wire_projection_preserves_immutable_scope_and_lifecycle() {
        let authorization = ConsentAuthorization::create(CreateConsentAuthorization {
            authorization_id: ConsentAuthorizationId::try_new("consent-auth-1").unwrap(),
            party_ref: PartyReference::try_new("party-1").unwrap(),
            contact_point_ref: Some(ContactPointReference::try_new("contact-point-1").unwrap()),
            purpose: PurposeCode::try_new("marketing.newsletter").unwrap(),
            channel: CommunicationChannel::Email,
            effect: ConsentEffect::Grant,
            legal_basis: LegalBasisCode::try_new("consent").unwrap(),
            jurisdiction: JurisdictionCode::try_new("eu-lt").unwrap(),
            source: SourceCode::try_new("web.form").unwrap(),
            evidence_ref: EvidenceReference::try_new("evidence://1").unwrap(),
            effective_from_unix_nanos: 100,
            expires_at_unix_nanos: Some(1_000),
            occurred_at_unix_nanos: 100,
        })
        .unwrap();
        let projected = consent_authorization_to_wire(&authorization);
        assert_eq!(projected.purpose, "marketing.newsletter");
        assert_eq!(projected.channel, wire::CommunicationChannel::Email as i32);
        assert_eq!(projected.effect, wire::ConsentEffect::Grant as i32);
        assert_eq!(
            projected
                .resource_version
                .expect("resource version")
                .version,
            1
        );
    }

    #[test]
    fn persisted_contract_is_exact_and_nonzero() {
        let contract = persisted_contract();
        assert_eq!(contract.owner, MODULE_ID);
        assert_eq!(contract.schema_id, CONSENT_AUTHORIZATION_STATE_SCHEMA_ID);
        assert_ne!(contract.descriptor_hash, [0; 32]);
    }
}
