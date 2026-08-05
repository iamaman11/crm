use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_privacy::{
    CustomerDataLegalHold, LEGAL_HOLD_RECORD_TYPE, LegalHoldScope, LegalHoldStatus, MODULE_ID,
    ProcessingRestriction, RESTRICTION_RECORD_TYPE, RestrictionScope, RestrictionStatus,
};
use crm_customer_privacy_persistence_adapter::{
    legal_hold_from_snapshot, legal_hold_persisted_payload, legal_hold_record_ref,
    processing_restriction_from_snapshot, processing_restriction_persisted_payload,
    processing_restriction_record_ref,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordRef, RecordSnapshot,
    SdkError,
};
use crm_proto_contracts::crm::{customer::v1 as customer, customer_privacy::v1 as wire};

pub const RELEASE_PROCESSING_RESTRICTION_CAPABILITY: &str = "customer_privacy.restriction.release";
pub const RELEASE_PROCESSING_RESTRICTION_REQUEST_SCHEMA: &str =
    "crm.customer_privacy.v1.ReleaseProcessingRestrictionRequest";
pub const RELEASE_PROCESSING_RESTRICTION_RESPONSE_SCHEMA: &str =
    "crm.customer_privacy.v1.ReleaseProcessingRestrictionResponse";
pub const PROCESSING_RESTRICTION_RELEASED_EVENT_TYPE: &str =
    "customer_privacy.restriction.released";
pub const PROCESSING_RESTRICTION_RELEASED_EVENT_SCHEMA: &str =
    "crm.customer_privacy.v1.ProcessingRestrictionReleasedEvent";

pub const RELEASE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY: &str = "customer_privacy.legal_hold.release";
pub const RELEASE_CUSTOMER_DATA_LEGAL_HOLD_REQUEST_SCHEMA: &str =
    "crm.customer_privacy.v1.ReleaseCustomerDataLegalHoldRequest";
pub const RELEASE_CUSTOMER_DATA_LEGAL_HOLD_RESPONSE_SCHEMA: &str =
    "crm.customer_privacy.v1.ReleaseCustomerDataLegalHoldResponse";
pub const CUSTOMER_DATA_LEGAL_HOLD_RELEASED_EVENT_TYPE: &str =
    "customer_privacy.legal_hold.released";
pub const CUSTOMER_DATA_LEGAL_HOLD_RELEASED_EVENT_SCHEMA: &str =
    "crm.customer_privacy.v1.CustomerDataLegalHoldReleasedEvent";

const NANOS_PER_MILLISECOND: i64 = 1_000_000;

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerPrivacyRestrictionReleaseCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerPrivacyRestrictionReleaseCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_exact_coordinate(
            definition,
            request,
            RELEASE_PROCESSING_RESTRICTION_CAPABILITY,
        )?;
        Ok(AggregateTarget {
            reference: processing_restriction_ref_from_release_request(request)?,
            presence: AggregatePresence::MustExist,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_exact_coordinate(
            definition,
            request,
            RELEASE_PROCESSING_RESTRICTION_CAPABILITY,
        )?;
        let command = release_processing_restriction_command(request)?;
        let requested_ref = processing_restriction_ref(command.processing_restriction_ref)?;
        let current = current.ok_or_else(control_not_found)?;
        if current.reference != requested_ref {
            return Err(control_not_found());
        }

        let mut restriction = processing_restriction_from_snapshot(current)
            .map_err(|error| control_state_invalid("processing restriction", error))?;
        if restriction.restriction_id() != &requested_ref.record_id
            || restriction.tenant_id() != &request.context.execution.tenant_id
        {
            return Err(control_not_found());
        }
        let previous_version = i64::try_from(restriction.version())
            .map_err(|_| plan_invalid("processing restriction version exceeds i64"))?;
        if current.version != previous_version {
            return Err(control_state_invalid(
                "processing restriction",
                "domain and record versions differ",
            ));
        }
        restriction
            .release(
                positive_version(command.expected_version)?,
                request.context.execution.actor_id.clone(),
                request.context.execution.request_started_at_unix_nanos,
            )
            .map_err(domain_error)?;
        build_restriction_release_plan(definition, request, current, restriction, previous_version)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerPrivacyLegalHoldReleaseCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerPrivacyLegalHoldReleaseCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_exact_coordinate(
            definition,
            request,
            RELEASE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY,
        )?;
        Ok(AggregateTarget {
            reference: legal_hold_ref_from_release_request(request)?,
            presence: AggregatePresence::MustExist,
        })
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_exact_coordinate(
            definition,
            request,
            RELEASE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY,
        )?;
        let command = release_legal_hold_command(request)?;
        let requested_ref = legal_hold_ref(command.customer_data_legal_hold_ref)?;
        let current = current.ok_or_else(control_not_found)?;
        if current.reference != requested_ref {
            return Err(control_not_found());
        }

        let mut hold = legal_hold_from_snapshot(current)
            .map_err(|error| control_state_invalid("customer-data legal hold", error))?;
        if hold.hold_id() != &requested_ref.record_id
            || hold.tenant_id() != &request.context.execution.tenant_id
        {
            return Err(control_not_found());
        }
        let previous_version = i64::try_from(hold.version())
            .map_err(|_| plan_invalid("customer-data legal-hold version exceeds i64"))?;
        if current.version != previous_version {
            return Err(control_state_invalid(
                "customer-data legal hold",
                "domain and record versions differ",
            ));
        }
        hold.release(
            positive_version(command.expected_version)?,
            request.context.execution.actor_id.clone(),
            request.context.execution.request_started_at_unix_nanos,
        )
        .map_err(domain_error)?;
        build_legal_hold_release_plan(definition, request, current, hold, previous_version)
    }
}

pub fn release_processing_restriction_capability_definition()
-> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        RELEASE_PROCESSING_RESTRICTION_CAPABILITY,
        RELEASE_PROCESSING_RESTRICTION_REQUEST_SCHEMA,
        RELEASE_PROCESSING_RESTRICTION_RESPONSE_SCHEMA,
    )
}

pub fn release_customer_data_legal_hold_capability_definition()
-> Result<CapabilityDefinition, SdkError> {
    mutation_definition(
        RELEASE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY,
        RELEASE_CUSTOMER_DATA_LEGAL_HOLD_REQUEST_SCHEMA,
        RELEASE_CUSTOMER_DATA_LEGAL_HOLD_RESPONSE_SCHEMA,
    )
}

pub fn release_control_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![
        release_processing_restriction_capability_definition()?,
        release_customer_data_legal_hold_capability_definition()?,
    ])
}

fn mutation_definition(
    capability_id: &'static str,
    request_schema: &'static str,
    response_schema: &'static str,
) -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(capability_id))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            request_schema,
            vec![DataClass::Personal],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            response_schema,
            vec![DataClass::Personal],
        )?),
        risk: CapabilityRisk::High,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

pub fn processing_restriction_ref_from_release_request(
    request: &CapabilityRequest,
) -> Result<RecordRef, SdkError> {
    processing_restriction_ref(
        release_processing_restriction_command(request)?.processing_restriction_ref,
    )
}

pub fn legal_hold_ref_from_release_request(
    request: &CapabilityRequest,
) -> Result<RecordRef, SdkError> {
    legal_hold_ref(release_legal_hold_command(request)?.customer_data_legal_hold_ref)
}

fn build_restriction_release_plan(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: &RecordSnapshot,
    restriction: ProcessingRestriction,
    previous_version: i64,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let next_version = i64::try_from(restriction.version())
        .map_err(|_| plan_invalid("released processing restriction version exceeds i64"))?;
    if restriction.status() != RestrictionStatus::Released || next_version != previous_version + 1 {
        return Err(plan_invalid(
            "restriction.release must advance the active aggregate by one version",
        ));
    }
    let aggregate = processing_restriction_record_ref(&restriction)?;
    if aggregate != current.reference {
        return Err(control_not_found());
    }
    let public = processing_restriction_to_wire(&restriction)?;
    let output = support::protobuf_payload(
        MODULE_ID,
        RELEASE_PROCESSING_RESTRICTION_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::ReleaseProcessingRestrictionResponse {
            processing_restriction: Some(public.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PROCESSING_RESTRICTION_RELEASED_EVENT_TYPE,
            event_schema_id: PROCESSING_RESTRICTION_RELEASED_EVENT_SCHEMA,
            aggregate_version: next_version,
            previous_version: Some(previous_version),
        },
        DataClass::Personal,
        &wire::ProcessingRestrictionReleasedEvent {
            processing_restriction: Some(public),
        },
    )?;
    let audit = support::audit_intent(
        request,
        &aggregate,
        next_version,
        definition.capability_id.as_str(),
        &output.bytes,
    )?;
    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records: vec![RecordMutation::Update {
                reference: aggregate,
                expected_version: previous_version,
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

fn build_legal_hold_release_plan(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    current: &RecordSnapshot,
    hold: CustomerDataLegalHold,
    previous_version: i64,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let next_version = i64::try_from(hold.version())
        .map_err(|_| plan_invalid("released customer-data legal-hold version exceeds i64"))?;
    if hold.status() != LegalHoldStatus::Released || next_version != previous_version + 1 {
        return Err(plan_invalid(
            "legal_hold.release must advance the active aggregate by one version",
        ));
    }
    let aggregate = legal_hold_record_ref(&hold)?;
    if aggregate != current.reference {
        return Err(control_not_found());
    }
    let public = customer_data_legal_hold_to_wire(&hold)?;
    let output = support::protobuf_payload(
        MODULE_ID,
        RELEASE_CUSTOMER_DATA_LEGAL_HOLD_RESPONSE_SCHEMA,
        DataClass::Personal,
        &wire::ReleaseCustomerDataLegalHoldResponse {
            customer_data_legal_hold: Some(public.clone()),
        },
    )?;
    let event = support::event_evidence_with_data_class(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: CUSTOMER_DATA_LEGAL_HOLD_RELEASED_EVENT_TYPE,
            event_schema_id: CUSTOMER_DATA_LEGAL_HOLD_RELEASED_EVENT_SCHEMA,
            aggregate_version: next_version,
            previous_version: Some(previous_version),
        },
        DataClass::Personal,
        &wire::CustomerDataLegalHoldReleasedEvent {
            customer_data_legal_hold: Some(public),
        },
    )?;
    let audit = support::audit_intent(
        request,
        &aggregate,
        next_version,
        definition.capability_id.as_str(),
        &output.bytes,
    )?;
    Ok(CapabilityBatchExecutionPlan {
        batch: BatchMutationPlan {
            context: request.context.clone(),
            records: vec![RecordMutation::Update {
                reference: aggregate,
                expected_version: previous_version,
                payload: legal_hold_persisted_payload(&hold)?,
            }],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(definition, request)?,
            audits: vec![audit],
        },
        output: Some(output),
    })
}

pub fn processing_restriction_to_wire(
    restriction: &ProcessingRestriction,
) -> Result<wire::ProcessingRestriction, SdkError> {
    Ok(wire::ProcessingRestriction {
        processing_restriction_ref: Some(wire::ProcessingRestrictionRef {
            processing_restriction_id: restriction.restriction_id().as_str().to_owned(),
        }),
        canonical_party_ref: Some(customer::PartyRef {
            party_id: restriction.canonical_party_id().as_str().to_owned(),
        }),
        scope: match restriction.scope() {
            RestrictionScope::Processing => wire::ProcessingRestrictionScope::Processing as i32,
            RestrictionScope::Communication => {
                wire::ProcessingRestrictionScope::Communication as i32
            }
            RestrictionScope::ProcessingAndCommunication => {
                wire::ProcessingRestrictionScope::ProcessingAndCommunication as i32
            }
        },
        status: match restriction.status() {
            RestrictionStatus::Active => wire::ProcessingRestrictionStatus::Active as i32,
            RestrictionStatus::Released => wire::ProcessingRestrictionStatus::Released as i32,
            RestrictionStatus::Expired => wire::ProcessingRestrictionStatus::Expired as i32,
        },
        version: i64::try_from(restriction.version())
            .map_err(|_| plan_invalid("processing restriction version exceeds wire range"))?,
        policy_version: restriction.policy_version().as_str().to_owned(),
        placed_by_actor_id: restriction.placed_by().as_str().to_owned(),
        placed_at_unix_ms: nanos_to_millis(
            restriction.placed_at_unix_nanos(),
            "processing restriction placement timestamp",
        )?,
        effective_from_unix_ms: nanos_to_millis(
            restriction.effective_from_unix_nanos(),
            "processing restriction effective timestamp",
        )?,
        expires_at_unix_ms: restriction
            .expires_at_unix_nanos()
            .map(|value| nanos_to_millis(value, "processing restriction expiry timestamp"))
            .transpose()?,
        released_by_actor_id: restriction
            .released_by()
            .map(|value| value.as_str().to_owned()),
        released_at_unix_ms: restriction
            .released_at_unix_nanos()
            .map(|value| nanos_to_millis(value, "processing restriction release timestamp"))
            .transpose()?,
    })
}

pub fn customer_data_legal_hold_to_wire(
    hold: &CustomerDataLegalHold,
) -> Result<wire::CustomerDataLegalHold, SdkError> {
    Ok(wire::CustomerDataLegalHold {
        customer_data_legal_hold_ref: Some(wire::CustomerDataLegalHoldRef {
            customer_data_legal_hold_id: hold.hold_id().as_str().to_owned(),
        }),
        canonical_party_ref: Some(customer::PartyRef {
            party_id: hold.canonical_party_id().as_str().to_owned(),
        }),
        scope: Some(legal_hold_scope_to_wire(hold.scope())),
        authority_reference_id: hold.authority_reference().as_str().to_owned(),
        reason_code: hold.reason_code().to_owned(),
        policy_version: hold.policy_version().as_str().to_owned(),
        status: match hold.status() {
            LegalHoldStatus::Active => wire::CustomerDataLegalHoldStatus::Active as i32,
            LegalHoldStatus::Released => wire::CustomerDataLegalHoldStatus::Released as i32,
        },
        version: i64::try_from(hold.version())
            .map_err(|_| plan_invalid("customer-data legal-hold version exceeds wire range"))?,
        placed_by_actor_id: hold.placed_by().as_str().to_owned(),
        effective_from_unix_ms: nanos_to_millis(
            hold.effective_from_unix_nanos(),
            "customer-data legal-hold effective timestamp",
        )?,
        effective_until_unix_ms: hold
            .effective_until_unix_nanos()
            .map(|value| nanos_to_millis(value, "customer-data legal-hold end timestamp"))
            .transpose()?,
        released_by_actor_id: hold.released_by().map(|value| value.as_str().to_owned()),
        released_at_unix_ms: hold
            .released_at_unix_nanos()
            .map(|value| nanos_to_millis(value, "customer-data legal-hold release timestamp"))
            .transpose()?,
    })
}

fn release_processing_restriction_command(
    request: &CapabilityRequest,
) -> Result<wire::ReleaseProcessingRestrictionRequest, SdkError> {
    request.context.validate()?;
    let command: wire::ReleaseProcessingRestrictionRequest =
        support::decode_request_with_data_class(
            request,
            MODULE_ID,
            RELEASE_PROCESSING_RESTRICTION_REQUEST_SCHEMA,
            DataClass::Personal,
        )?;
    positive_version(command.expected_version)?;
    Ok(command)
}

fn release_legal_hold_command(
    request: &CapabilityRequest,
) -> Result<wire::ReleaseCustomerDataLegalHoldRequest, SdkError> {
    request.context.validate()?;
    let command: wire::ReleaseCustomerDataLegalHoldRequest =
        support::decode_request_with_data_class(
            request,
            MODULE_ID,
            RELEASE_CUSTOMER_DATA_LEGAL_HOLD_REQUEST_SCHEMA,
            DataClass::Personal,
        )?;
    positive_version(command.expected_version)?;
    Ok(command)
}

fn processing_restriction_ref(
    value: Option<wire::ProcessingRestrictionRef>,
) -> Result<RecordRef, SdkError> {
    let value = value.ok_or_else(|| required("customer_privacy.restriction.ref"))?;
    support::record_ref(
        RESTRICTION_RECORD_TYPE,
        &value.processing_restriction_id,
        "customer_privacy.restriction.ref.processing_restriction_id",
    )
}

fn legal_hold_ref(value: Option<wire::CustomerDataLegalHoldRef>) -> Result<RecordRef, SdkError> {
    let value = value.ok_or_else(|| required("customer_privacy.legal_hold.ref"))?;
    support::record_ref(
        LEGAL_HOLD_RECORD_TYPE,
        &value.customer_data_legal_hold_id,
        "customer_privacy.legal_hold.ref.customer_data_legal_hold_id",
    )
}

fn legal_hold_scope_to_wire(value: &LegalHoldScope) -> wire::CustomerDataLegalHoldScope {
    let scope = match value {
        LegalHoldScope::AllCustomerData => {
            wire::customer_data_legal_hold_scope::Scope::AllCustomerData(true)
        }
        LegalHoldScope::DataClass(value) => {
            wire::customer_data_legal_hold_scope::Scope::DataClass(data_class_to_wire(*value))
        }
        LegalHoldScope::Owner(value) => {
            wire::customer_data_legal_hold_scope::Scope::OwnerModuleId(value.as_str().to_owned())
        }
    };
    wire::CustomerDataLegalHoldScope { scope: Some(scope) }
}

fn data_class_to_wire(value: DataClass) -> i32 {
    match value {
        DataClass::Public => wire::CustomerDataClass::Public as i32,
        DataClass::Internal => wire::CustomerDataClass::Internal as i32,
        DataClass::Confidential => wire::CustomerDataClass::Confidential as i32,
        DataClass::Restricted => wire::CustomerDataClass::Restricted as i32,
        DataClass::Personal => wire::CustomerDataClass::Personal as i32,
        DataClass::SensitivePersonal => wire::CustomerDataClass::SensitivePersonal as i32,
        DataClass::Biometric => wire::CustomerDataClass::Biometric as i32,
        DataClass::Financial => wire::CustomerDataClass::Financial as i32,
        DataClass::Credential => wire::CustomerDataClass::Credential as i32,
    }
}

fn ensure_exact_coordinate(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    capability_id: &'static str,
) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != capability_id
        || definition.capability_version.as_str() != support::CONTRACT_VERSION
        || request.context.module_id.as_str() != MODULE_ID
        || request.context.execution.capability_id.as_str() != capability_id
        || request.context.execution.capability_version.as_str() != support::CONTRACT_VERSION
    {
        return Err(plan_invalid(
            "capability definition does not match the exact control release coordinate",
        ));
    }
    Ok(())
}

fn positive_version(value: i64) -> Result<u64, SdkError> {
    if value <= 0 {
        return Err(SdkError::invalid_argument(
            "customer_privacy.expected_version",
            "Expected version must be positive.",
        ));
    }
    u64::try_from(value).map_err(|error| {
        SdkError::invalid_argument("customer_privacy.expected_version", error.to_string())
    })
}

fn nanos_to_millis(value: i64, field: &'static str) -> Result<i64, SdkError> {
    if value < 0 {
        return Err(plan_invalid(format!("{field} is negative")));
    }
    Ok(value / NANOS_PER_MILLISECOND)
}

fn required(field: &'static str) -> SdkError {
    SdkError::invalid_argument(field, "The control reference is required.")
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(|error| {
        SdkError::new(
            "CUSTOMER_PRIVACY_CONTROL_RELEASE_CONFIGURATION_INVALID",
            ErrorCategory::Internal,
            false,
            "The Customer Privacy control release capability is not configured safely.",
        )
        .with_internal_reference(error.to_string())
    })
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
        "The Customer Privacy control release request is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn control_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CONTROL_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested Customer Privacy control was not found.",
    )
}

fn control_state_invalid(label: &'static str, reference: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CONTROL_STATE_INVALID",
        ErrorCategory::Unavailable,
        true,
        "The requested Customer Privacy control is temporarily unavailable.",
    )
    .with_internal_reference(format!("{label}: {reference}"))
}

fn plan_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CONTROL_RELEASE_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Privacy control release plan is invalid.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_definitions_are_exact_high_risk_idempotent_mutations() {
        let definitions = release_control_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        assert_eq!(
            definitions[0].capability_id.as_str(),
            RELEASE_PROCESSING_RESTRICTION_CAPABILITY
        );
        assert_eq!(
            definitions[1].capability_id.as_str(),
            RELEASE_CUSTOMER_DATA_LEGAL_HOLD_CAPABILITY
        );
        for definition in definitions {
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
    }

    #[test]
    fn expected_versions_are_strictly_positive() {
        assert_eq!(positive_version(1).unwrap(), 1);
        assert!(positive_version(0).is_err());
        assert!(positive_version(-1).is_err());
    }
}
