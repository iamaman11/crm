use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_privacy::{
    MODULE_ID, ProcessingRestriction, RestrictionScope, RestrictionStatus,
};
use crm_customer_privacy_persistence_adapter::{
    processing_restriction_persisted_payload, processing_restriction_record_ref,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordId, RecordSnapshot,
    SchemaVersion, SdkError,
};
use crm_proto_contracts::crm::{customer::v1 as customer, customer_privacy::v1 as wire};
use sha2::{Digest, Sha256};

pub const PLACE_PROCESSING_RESTRICTION_CAPABILITY: &str =
    "customer_privacy.restriction.place";
pub const PLACE_PROCESSING_RESTRICTION_REQUEST_SCHEMA: &str =
    "crm.customer_privacy.v1.PlaceProcessingRestrictionRequest";
pub const PLACE_PROCESSING_RESTRICTION_RESPONSE_SCHEMA: &str =
    "crm.customer_privacy.v1.PlaceProcessingRestrictionResponse";
pub const PROCESSING_RESTRICTION_PLACED_EVENT_TYPE: &str =
    "customer_privacy.restriction.placed";
pub const PROCESSING_RESTRICTION_PLACED_EVENT_SCHEMA: &str =
    "crm.customer_privacy.v1.ProcessingRestrictionPlacedEvent";

const RESTRICTION_ID_DOMAIN: &[u8] = b"crm.customer-privacy.restriction/v1";
const RESTRICTION_ID_PREFIX: &str = "processing-restriction-";
const NANOS_PER_MILLISECOND: i64 = 1_000_000;

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerPrivacyRestrictionPlaceCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerPrivacyRestrictionPlaceCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let restriction = processing_restriction_from_place_request(request)?;
        Ok(AggregateTarget {
            reference: processing_restriction_record_ref(&restriction)?,
            presence: AggregatePresence::MustBeAbsent,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_definition(definition, request)?;
        if current.is_some() {
            return Err(plan_invalid(
                "deterministic processing restriction already exists",
            ));
        }
        let command = decode_place_request(request)?;
        let effective_from_unix_nanos = millis_to_nanos(
            command.effective_from_unix_ms,
            "customer_privacy.restriction.effective_from_unix_ms",
        )?;
        let expires_at_unix_nanos = command
            .expires_at_unix_ms
            .map(|value| {
                millis_to_nanos(
                    value,
                    "customer_privacy.restriction.expires_at_unix_ms",
                )
            })
            .transpose()?;
        let restriction = processing_restriction_from_command(
            request,
            &command,
            effective_from_unix_nanos,
            expires_at_unix_nanos,
        )?;
        plan_restriction_place(
            definition,
            request,
            restriction,
            effective_from_unix_nanos,
            expires_at_unix_nanos,
        )
    }
}

pub fn place_processing_restriction_capability_definition(
) -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(
            PLACE_PROCESSING_RESTRICTION_CAPABILITY,
        ))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            PLACE_PROCESSING_RESTRICTION_REQUEST_SCHEMA,
            vec![DataClass::Personal],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            PLACE_PROCESSING_RESTRICTION_RESPONSE_SCHEMA,
            vec![DataClass::Personal],
        )?),
        risk: CapabilityRisk::High,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: PLACE_PROCESSING_RESTRICTION_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

pub fn deterministic_processing_restriction_id(
    tenant_id: &str,
    idempotency_key: &str,
) -> Result<RecordId, SdkError> {
    let mut hasher = Sha256::new();
    hasher.update(RESTRICTION_ID_DOMAIN);
    update_length_framed(&mut hasher, tenant_id.as_bytes());
    update_length_framed(&mut hasher, idempotency_key.as_bytes());
    let digest = hasher.finalize();
    RecordId::try_new(format!("{RESTRICTION_ID_PREFIX}{}", hex(&digest)))
        .map_err(configuration_error)
}

pub fn processing_restriction_canonical_party_id_from_request(
    request: &CapabilityRequest,
) -> Result<RecordId, SdkError> {
    request.context.validate()?;
    canonical_party_id_from_wire(
        decode_place_request(request)?.canonical_party_ref,
        "customer_privacy.restriction.canonical_party_ref",
    )
}

pub fn processing_restriction_from_place_request(
    request: &CapabilityRequest,
) -> Result<ProcessingRestriction, SdkError> {
    request.context.validate()?;
    let command = decode_place_request(request)?;
    let effective_from_unix_nanos = millis_to_nanos(
        command.effective_from_unix_ms,
        "customer_privacy.restriction.effective_from_unix_ms",
    )?;
    let expires_at_unix_nanos = command
        .expires_at_unix_ms
        .map(|value| {
            millis_to_nanos(
                value,
                "customer_privacy.restriction.expires_at_unix_ms",
            )
        })
        .transpose()?;
    processing_restriction_from_command(
        request,
        &command,
        effective_from_unix_nanos,
        expires_at_unix_nanos,
    )
}

fn processing_restriction_from_command(
    request: &CapabilityRequest,
    command: &wire::PlaceProcessingRestrictionRequest,
    effective_from_unix_nanos: i64,
    expires_at_unix_nanos: Option<i64>,
) -> Result<ProcessingRestriction, SdkError> {
    ProcessingRestriction::place(
        deterministic_processing_restriction_id(
            request.context.execution.tenant_id.as_str(),
            request.context.execution.idempotency_key.as_str(),
        )?,
        request.context.execution.tenant_id.clone(),
        canonical_party_id_from_wire(
            command.canonical_party_ref.clone(),
            "customer_privacy.restriction.canonical_party_ref",
        )?,
        restriction_scope_from_wire(command.scope)?,
        policy_version(command.policy_version.clone())?,
        request.context.execution.actor_id.clone(),
        request.context.execution.request_started_at_unix_nanos,
        effective_from_unix_nanos,
        expires_at_unix_nanos,
    )
    .map_err(domain_error)
}

fn plan_restriction_place(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    restriction: ProcessingRestriction,
    effective_from_unix_nanos: i64,
    expires_at_unix_nanos: Option<i64>,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let aggregate = processing_restriction_record_ref(&restriction)?;
    let public_restriction = processing_restriction_to_wire(
        &restriction,
        effective_from_unix_nanos,
        expires_at_unix_nanos,
    )?;
    let output = support::protobuf_payload(
        MODULE_ID,
        PLACE_PROCESSING_RESTRICTION_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::PlaceProcessingRestrictionResponse {
            processing_restriction: Some(public_restriction.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PROCESSING_RESTRICTION_PLACED_EVENT_TYPE,
            event_schema_id: PROCESSING_RESTRICTION_PLACED_EVENT_SCHEMA,
            aggregate_version: 1,
            previous_version: None,
        },
        DataClass::Personal,
        &wire::ProcessingRestrictionPlacedEvent {
            processing_restriction: Some(public_restriction),
        },
    )?;
    let audit = support::audit_intent(
        request,
        &aggregate,
        1,
        definition.capability_id.as_str(),
        &output.bytes,
    )?;

    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records: vec![RecordMutation::Create {
                reference: aggregate,
                payload: processing_restriction_persisted_payload(&restriction)?,
            }],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(definition, request)?,
            audits: vec![audit],
        },
        output: Some(output),
    })
}

fn processing_restriction_to_wire(
    restriction: &ProcessingRestriction,
    effective_from_unix_nanos: i64,
    expires_at_unix_nanos: Option<i64>,
) -> Result<wire::ProcessingRestriction, SdkError> {
    if restriction.status() != RestrictionStatus::Active || restriction.version() != 1 {
        return Err(plan_invalid(
            "restriction.place output must contain an active version-1 restriction",
        ));
    }
    Ok(wire::ProcessingRestriction {
        processing_restriction_ref: Some(wire::ProcessingRestrictionRef {
            processing_restriction_id: restriction.restriction_id().as_str().to_owned(),
        }),
        canonical_party_ref: Some(customer::PartyRef {
            party_id: restriction.canonical_party_id().as_str().to_owned(),
        }),
        scope: restriction_scope_to_wire(restriction.scope()),
        status: wire::ProcessingRestrictionStatus::Active as i32,
        version: i64::try_from(restriction.version())
            .map_err(|_| plan_invalid("restriction version exceeds the wire range"))?,
        policy_version: restriction.policy_version().as_str().to_owned(),
        placed_by_actor_id: restriction.placed_by().as_str().to_owned(),
        placed_at_unix_ms: nanos_to_millis(
            restriction.placed_at_unix_nanos(),
            "execution_context.request_started_at_unix_nanos",
        )?,
        effective_from_unix_ms: nanos_to_millis(
            effective_from_unix_nanos,
            "customer_privacy.restriction.effective_from_unix_ms",
        )?,
        expires_at_unix_ms: expires_at_unix_nanos
            .map(|value| {
                nanos_to_millis(
                    value,
                    "customer_privacy.restriction.expires_at_unix_ms",
                )
            })
            .transpose()?,
        released_by_actor_id: None,
        released_at_unix_ms: None,
    })
}

fn decode_place_request(
    request: &CapabilityRequest,
) -> Result<wire::PlaceProcessingRestrictionRequest, SdkError> {
    support::decode_request(
        request,
        MODULE_ID,
        PLACE_PROCESSING_RESTRICTION_REQUEST_SCHEMA,
    )
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != PLACE_PROCESSING_RESTRICTION_CAPABILITY
        || definition.capability_version.as_str() != support::CONTRACT_VERSION
        || request.context.module_id.as_str() != MODULE_ID
        || request.context.execution.capability_id.as_str()
            != PLACE_PROCESSING_RESTRICTION_CAPABILITY
        || request.context.execution.capability_version.as_str() != support::CONTRACT_VERSION
    {
        return Err(plan_invalid(
            "capability definition does not match the exact restriction.place request coordinate",
        ));
    }
    Ok(())
}

fn canonical_party_id_from_wire(
    reference: Option<customer::PartyRef>,
    field: &'static str,
) -> Result<RecordId, SdkError> {
    let reference = reference.ok_or_else(|| {
        SdkError::invalid_argument(field, "Canonical Party reference is required.")
    })?;
    RecordId::try_new(reference.party_id)
        .map_err(|error| SdkError::invalid_argument(field, error.to_string()))
}

fn restriction_scope_from_wire(value: i32) -> Result<RestrictionScope, SdkError> {
    match wire::ProcessingRestrictionScope::try_from(value) {
        Ok(wire::ProcessingRestrictionScope::Processing) => Ok(RestrictionScope::Processing),
        Ok(wire::ProcessingRestrictionScope::Communication) => Ok(RestrictionScope::Communication),
        Ok(wire::ProcessingRestrictionScope::ProcessingAndCommunication) => {
            Ok(RestrictionScope::ProcessingAndCommunication)
        }
        Ok(wire::ProcessingRestrictionScope::Unspecified) | Err(_) => Err(
            SdkError::invalid_argument(
                "customer_privacy.restriction.scope",
                "Processing restriction scope is unsupported.",
            ),
        ),
    }
}

fn restriction_scope_to_wire(value: RestrictionScope) -> i32 {
    match value {
        RestrictionScope::Processing => wire::ProcessingRestrictionScope::Processing as i32,
        RestrictionScope::Communication => wire::ProcessingRestrictionScope::Communication as i32,
        RestrictionScope::ProcessingAndCommunication => {
            wire::ProcessingRestrictionScope::ProcessingAndCommunication as i32
        }
    }
}

fn policy_version(value: String) -> Result<SchemaVersion, SdkError> {
    if value.trim().is_empty() {
        return Err(SdkError::invalid_argument(
            "customer_privacy.restriction.policy_version",
            "Policy version is required.",
        ));
    }
    SchemaVersion::try_new(value).map_err(|error| {
        SdkError::invalid_argument(
            "customer_privacy.restriction.policy_version",
            error.to_string(),
        )
    })
}

fn millis_to_nanos(value: i64, field: &'static str) -> Result<i64, SdkError> {
    if value < 0 {
        return Err(SdkError::invalid_argument(
            field,
            "Unix timestamp must not be negative.",
        ));
    }
    value.checked_mul(NANOS_PER_MILLISECOND).ok_or_else(|| {
        SdkError::invalid_argument(field, "Unix timestamp exceeds the supported range.")
    })
}

fn nanos_to_millis(value: i64, field: &'static str) -> Result<i64, SdkError> {
    if value < 0 || value % NANOS_PER_MILLISECOND != 0 {
        return Err(plan_invalid(format!(
            "{field} is negative or not millisecond aligned"
        )));
    }
    Ok(value / NANOS_PER_MILLISECOND)
}

fn update_length_framed(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(configuration_error)
}

fn configuration_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_RESTRICTION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The processing restriction capability is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

fn domain_error(error: crm_customer_privacy::PrivacyDomainError) -> SdkError {
    let category = match error {
        crm_customer_privacy::PrivacyDomainError::VersionConflict { .. }
        | crm_customer_privacy::PrivacyDomainError::InvalidTransition { .. } => {
            ErrorCategory::Conflict
        }
        crm_customer_privacy::PrivacyDomainError::InvalidArgument { .. } => {
            ErrorCategory::InvalidArgument
        }
    };
    SdkError::new(
        error.code(),
        category,
        error.retryable(),
        "The processing restriction request is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn plan_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_RESTRICTION_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The processing restriction mutation plan is invalid.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definition_is_exact_high_risk_personal_mutation() {
        let definition = place_processing_restriction_capability_definition().unwrap();
        assert_eq!(
            definition.capability_id.as_str(),
            PLACE_PROCESSING_RESTRICTION_CAPABILITY
        );
        assert_eq!(definition.owner_module_id.as_str(), MODULE_ID);
        assert_eq!(definition.capability_version.as_str(), "1.0.0");
        assert_eq!(
            definition.input_contract.allowed_data_classes,
            vec![DataClass::Personal]
        );
        assert_eq!(definition.risk, CapabilityRisk::High);
        assert!(definition.mutation);
        assert!(definition.requires_idempotency);
        assert!(!definition.requires_approval);
    }

    #[test]
    fn deterministic_identity_is_tenant_and_idempotency_bound() {
        let first = deterministic_processing_restriction_id("tenant-a", "same-key").unwrap();
        let replay = deterministic_processing_restriction_id("tenant-a", "same-key").unwrap();
        let other_tenant =
            deterministic_processing_restriction_id("tenant-b", "same-key").unwrap();
        assert_eq!(first, replay);
        assert_ne!(first, other_tenant);
        assert!(first.as_str().starts_with(RESTRICTION_ID_PREFIX));
    }
}
