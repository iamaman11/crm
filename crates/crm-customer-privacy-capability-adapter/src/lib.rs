#![forbid(unsafe_code)]

//! Governed production mutation planning for `crm.customer-privacy`.
//!
//! This crate currently promotes exactly one public coordinate:
//! `customer_privacy.case.create@1.0.0`. Root creation locks the deterministic
//! case record as absent. Lineage creation locks and strictly rehydrates the
//! referenced predecessor, requires a terminal predecessor, and creates the new
//! deterministic case as a separate immutable version-1 record. SQL,
//! authorization, activation and transport remain owned by the shared host.

use crm_capability_plan_support::{self as support, EventSpec};
use crm_capability_runtime::{CapabilityDefinition, CapabilityRequest, CapabilityRisk};
use crm_core_data::{
    AggregatePresence, AggregateTarget, BatchMutationPlan, CapabilityBatchExecutionPlan,
    RecordMutation, TransactionalAggregatePlanner,
};
use crm_customer_privacy::{
    MODULE_ID, PRIVACY_CASE_RECORD_TYPE, PrivacyCase, PrivacyCaseKind, PrivacyCaseStatus,
    PrivacyDomainError,
};
use crm_customer_privacy_persistence_adapter::{
    privacy_case_from_snapshot, privacy_case_persisted_payload, privacy_case_record_ref,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordId, RecordRef,
    RecordSnapshot, SchemaVersion, SdkError,
};
use crm_proto_contracts::crm::customer_privacy::v1 as wire;
use sha2::{Digest, Sha256};

pub const CREATE_PRIVACY_CASE_CAPABILITY: &str = "customer_privacy.case.create";
pub const CREATE_PRIVACY_CASE_REQUEST_SCHEMA: &str =
    "crm.customer_privacy.v1.CreatePrivacyCaseRequest";
pub const CREATE_PRIVACY_CASE_RESPONSE_SCHEMA: &str =
    "crm.customer_privacy.v1.CreatePrivacyCaseResponse";
pub const PRIVACY_CASE_CREATED_EVENT_TYPE: &str = "customer_privacy.case.created";
pub const PRIVACY_CASE_CREATED_EVENT_SCHEMA: &str =
    "crm.customer_privacy.v1.PrivacyCaseCreatedEvent";
pub const IMPLEMENTED_MUTATION_CAPABILITY_IDS: &[&str] = &[CREATE_PRIVACY_CASE_CAPABILITY];

const PRIVACY_CASE_ID_DOMAIN: &[u8] = b"crm.customer-privacy.case/v1";
const PRIVACY_CASE_ID_PREFIX: &str = "privacy-case-";

#[derive(Debug, Default, Clone, Copy)]
pub struct CustomerPrivacyCaseCreateCapabilityPlanner;

impl TransactionalAggregatePlanner for CustomerPrivacyCaseCreateCapabilityPlanner {
    fn target(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
    ) -> Result<AggregateTarget, SdkError> {
        ensure_definition(definition, request)?;
        let privacy_case = privacy_case_from_create_request(request)?;
        match privacy_case.previous_case_id() {
            Some(previous_case_id) => Ok(AggregateTarget {
                reference: privacy_case_ref_from_id(previous_case_id)?,
                presence: AggregatePresence::MustExist,
            }),
            None => Ok(AggregateTarget {
                reference: privacy_case_record_ref(&privacy_case)?,
                presence: AggregatePresence::MustBeAbsent,
            }),
        }
    }

    fn plan(
        &self,
        definition: &CapabilityDefinition,
        request: &CapabilityRequest,
        current: Option<&RecordSnapshot>,
    ) -> Result<CapabilityBatchExecutionPlan, SdkError> {
        ensure_definition(definition, request)?;
        let privacy_case = privacy_case_from_create_request(request)?;

        match privacy_case.previous_case_id() {
            Some(previous_case_id) => {
                validate_previous_case(request, previous_case_id, current)?;
            }
            None if current.is_some() => {
                return Err(plan_invalid(
                    "deterministic root privacy case already exists",
                ));
            }
            None => {}
        }

        plan_case_create(definition, request, privacy_case)
    }
}

pub fn capability_definition() -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: configured(CapabilityId::try_new(CREATE_PRIVACY_CASE_CAPABILITY))?,
        capability_version: configured(CapabilityVersion::try_new(support::CONTRACT_VERSION))?,
        owner_module_id: configured(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            CREATE_PRIVACY_CASE_REQUEST_SCHEMA,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            CREATE_PRIVACY_CASE_RESPONSE_SCHEMA,
            vec![DataClass::Confidential],
        )?),
        risk: CapabilityRisk::High,
        mutation: true,
        requires_idempotency: true,
        requires_approval: false,
        authorization_policy_id: CREATE_PRIVACY_CASE_CAPABILITY.to_owned(),
        rate_limit_policy_id: None,
    })
}

pub fn capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![capability_definition()?])
}

/// Repository-standard deterministic identity: a versioned domain followed by
/// independently length-framed tenant and idempotency-key bytes.
pub fn deterministic_privacy_case_id(
    tenant_id: &str,
    idempotency_key: &str,
) -> Result<RecordId, SdkError> {
    let mut hasher = Sha256::new();
    hasher.update(PRIVACY_CASE_ID_DOMAIN);
    update_length_framed(&mut hasher, tenant_id.as_bytes());
    update_length_framed(&mut hasher, idempotency_key.as_bytes());
    let digest = hasher.finalize();
    RecordId::try_new(format!("{PRIVACY_CASE_ID_PREFIX}{}", hex(&digest)))
        .map_err(configuration_error)
}

pub fn privacy_case_from_create_request(
    request: &CapabilityRequest,
) -> Result<PrivacyCase, SdkError> {
    request.context.validate()?;
    let command: wire::CreatePrivacyCaseRequest =
        support::decode_request(request, MODULE_ID, CREATE_PRIVACY_CASE_REQUEST_SCHEMA)?;
    let kind = privacy_case_kind_from_wire(command.kind)?;
    let policy_version = policy_version(command.policy_version)?;
    let previous_case_id = command
        .previous_privacy_case_ref
        .map(|reference| {
            privacy_case_id_from_wire(reference, "customer_privacy.previous_privacy_case_ref")
        })
        .transpose()?;
    let case_id = deterministic_privacy_case_id(
        request.context.execution.tenant_id.as_str(),
        request.context.execution.idempotency_key.as_str(),
    )?;

    PrivacyCase::new(
        case_id,
        request.context.execution.tenant_id.clone(),
        kind,
        policy_version,
        request.context.execution.request_started_at_unix_nanos,
        previous_case_id,
    )
    .map_err(domain_error)
}

fn plan_case_create(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
    privacy_case: PrivacyCase,
) -> Result<CapabilityBatchExecutionPlan, SdkError> {
    let aggregate = privacy_case_record_ref(&privacy_case)?;
    let public_case = privacy_case_to_wire(&privacy_case)?;
    let output = support::protobuf_payload(
        MODULE_ID,
        CREATE_PRIVACY_CASE_RESPONSE_SCHEMA,
        DataClass::Confidential,
        &wire::CreatePrivacyCaseResponse {
            privacy_case: Some(public_case.clone()),
        },
    )?;
    let event = support::event_evidence(
        request,
        aggregate.clone(),
        MODULE_ID,
        EventSpec {
            event_type: PRIVACY_CASE_CREATED_EVENT_TYPE,
            event_schema_id: PRIVACY_CASE_CREATED_EVENT_SCHEMA,
            aggregate_version: 1,
            previous_version: None,
        },
        &wire::PrivacyCaseCreatedEvent {
            privacy_case: Some(public_case),
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
                payload: privacy_case_persisted_payload(&privacy_case)?,
            }],
            relationships: Vec::new(),
            events: vec![event],
            idempotency: support::capability_idempotency(definition, request)?,
            audits: vec![audit],
        },
        output: Some(output),
    })
}

fn validate_previous_case(
    request: &CapabilityRequest,
    expected_id: &RecordId,
    current: Option<&RecordSnapshot>,
) -> Result<(), SdkError> {
    let expected_reference = privacy_case_ref_from_id(expected_id)?;
    let snapshot = current.ok_or_else(previous_case_not_found)?;
    if snapshot.reference != expected_reference {
        return Err(previous_case_not_found());
    }
    let predecessor = privacy_case_from_snapshot(snapshot).map_err(|error| {
        SdkError::new(
            "CUSTOMER_PRIVACY_PREVIOUS_CASE_INVALID",
            ErrorCategory::Internal,
            false,
            "The previous privacy case could not be loaded safely.",
        )
        .with_internal_reference(error.code)
    })?;
    if predecessor.case_id() != expected_id
        || predecessor.tenant_id() != &request.context.execution.tenant_id
    {
        return Err(previous_case_not_found());
    }
    if !predecessor.status().is_terminal() {
        return Err(SdkError::new(
            "CUSTOMER_PRIVACY_PREVIOUS_CASE_NOT_TERMINAL",
            ErrorCategory::Conflict,
            false,
            "The previous privacy case must be terminal before a successor can be created.",
        ));
    }
    Ok(())
}

fn privacy_case_to_wire(privacy_case: &PrivacyCase) -> Result<wire::PrivacyCase, SdkError> {
    if privacy_case.status() != PrivacyCaseStatus::Draft || privacy_case.version() != 1 {
        return Err(plan_invalid(
            "case.create output must contain a draft version-1 privacy case",
        ));
    }
    let version = i64::try_from(privacy_case.version())
        .map_err(|_| plan_invalid("privacy case version exceeds the wire range"))?;
    let created_at_unix_ms = nanos_to_millis(privacy_case.created_at_unix_nanos())?;
    Ok(wire::PrivacyCase {
        privacy_case_ref: Some(wire::PrivacyCaseRef {
            privacy_case_id: privacy_case.case_id().as_str().to_owned(),
        }),
        kind: privacy_case_kind_to_wire(privacy_case.kind()),
        status: wire::PrivacyCaseStatus::Draft as i32,
        version,
        policy_version: privacy_case.policy_version().as_str().to_owned(),
        created_at_unix_ms,
        updated_at_unix_ms: created_at_unix_ms,
        previous_privacy_case_ref: privacy_case.previous_case_id().map(|value| {
            wire::PrivacyCaseRef {
                privacy_case_id: value.as_str().to_owned(),
            }
        }),
        subject_binding: None,
        pending_rescope: None,
        scope_snapshot_id: String::new(),
        privacy_action_plan_ref: None,
        approval: None,
        retry_resume_stage: None,
    })
}

fn ensure_definition(
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || definition.capability_id.as_str() != CREATE_PRIVACY_CASE_CAPABILITY
        || definition.capability_version.as_str() != support::CONTRACT_VERSION
        || request.context.module_id.as_str() != MODULE_ID
        || request.context.execution.capability_id.as_str() != CREATE_PRIVACY_CASE_CAPABILITY
        || request.context.execution.capability_version.as_str() != support::CONTRACT_VERSION
    {
        return Err(plan_invalid(
            "capability definition does not match the exact request coordinate",
        ));
    }
    Ok(())
}

fn privacy_case_kind_from_wire(value: i32) -> Result<PrivacyCaseKind, SdkError> {
    match wire::PrivacyCaseKind::try_from(value) {
        Ok(wire::PrivacyCaseKind::Access) => Ok(PrivacyCaseKind::Access),
        Ok(wire::PrivacyCaseKind::PortabilityExport) => Ok(PrivacyCaseKind::PortabilityExport),
        Ok(wire::PrivacyCaseKind::RestrictProcessing) => Ok(PrivacyCaseKind::RestrictProcessing),
        Ok(wire::PrivacyCaseKind::Erasure) => Ok(PrivacyCaseKind::Erasure),
        Ok(wire::PrivacyCaseKind::Unspecified) | Err(_) => Err(SdkError::invalid_argument(
            "customer_privacy.case.kind",
            "Privacy case kind is unsupported.",
        )),
    }
}

fn privacy_case_kind_to_wire(value: PrivacyCaseKind) -> i32 {
    match value {
        PrivacyCaseKind::Access => wire::PrivacyCaseKind::Access as i32,
        PrivacyCaseKind::PortabilityExport => wire::PrivacyCaseKind::PortabilityExport as i32,
        PrivacyCaseKind::RestrictProcessing => wire::PrivacyCaseKind::RestrictProcessing as i32,
        PrivacyCaseKind::Erasure => wire::PrivacyCaseKind::Erasure as i32,
    }
}

fn policy_version(value: String) -> Result<SchemaVersion, SdkError> {
    if value.trim().is_empty() {
        return Err(SdkError::invalid_argument(
            "customer_privacy.case.policy_version",
            "Policy version is required.",
        ));
    }
    SchemaVersion::try_new(value).map_err(|error| {
        SdkError::invalid_argument("customer_privacy.case.policy_version", error.to_string())
    })
}

fn privacy_case_id_from_wire(
    reference: wire::PrivacyCaseRef,
    field: &'static str,
) -> Result<RecordId, SdkError> {
    RecordId::try_new(reference.privacy_case_id)
        .map_err(|error| SdkError::invalid_argument(field, error.to_string()))
}

fn privacy_case_ref_from_id(case_id: &RecordId) -> Result<RecordRef, SdkError> {
    support::record_ref(
        PRIVACY_CASE_RECORD_TYPE,
        case_id.as_str(),
        "customer_privacy.privacy_case_ref.privacy_case_id",
    )
}

fn nanos_to_millis(value: i64) -> Result<i64, SdkError> {
    if value < 0 {
        return Err(SdkError::invalid_argument(
            "execution_context.request_started_at_unix_nanos",
            "Request start time must not be negative.",
        ));
    }
    Ok(value / 1_000_000)
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

fn domain_error(error: PrivacyDomainError) -> SdkError {
    let category = match error {
        PrivacyDomainError::VersionConflict { .. }
        | PrivacyDomainError::InvalidTransition { .. } => ErrorCategory::Conflict,
        PrivacyDomainError::InvalidArgument { .. } => ErrorCategory::InvalidArgument,
    };
    SdkError::new(
        error.code(),
        category,
        error.retryable(),
        "The customer privacy case request is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn previous_case_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_PREVIOUS_CASE_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The previous privacy case was not found.",
    )
}

fn plan_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_CREATE_PLAN_INVALID",
        ErrorCategory::Internal,
        false,
        "The privacy case could not be planned safely.",
    )
    .with_internal_reference(reference.into())
}

fn configured<T>(value: Result<T, crm_module_sdk::IdentifierError>) -> Result<T, SdkError> {
    value.map_err(configuration_error)
}

fn configuration_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_CREATE_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The privacy case capability is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_capability_runtime::CapabilityRequest;
    use crm_core_data::RecordMutation;
    use crm_customer_privacy_persistence_adapter::privacy_case_persisted_payload;
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CausationId, CorrelationId, ExecutionContext,
        IdempotencyKey, ModuleExecutionContext, RequestId, TenantId, TraceId,
    };
    use prost::Message;

    fn request(
        tenant: &str,
        idempotency_key: &str,
        previous_case_id: Option<&str>,
    ) -> CapabilityRequest {
        request_with(
            tenant,
            idempotency_key,
            previous_case_id,
            wire::PrivacyCaseKind::Erasure as i32,
            "privacy-policy/1",
            1_000_000_000,
        )
    }

    fn request_with(
        tenant: &str,
        idempotency_key: &str,
        previous_case_id: Option<&str>,
        kind: i32,
        policy_version: &str,
        started_at: i64,
    ) -> CapabilityRequest {
        let command = wire::CreatePrivacyCaseRequest {
            kind,
            policy_version: policy_version.to_owned(),
            previous_privacy_case_ref: previous_case_id.map(|value| wire::PrivacyCaseRef {
                privacy_case_id: value.to_owned(),
            }),
        };
        CapabilityRequest {
            context: ModuleExecutionContext {
                module_id: ModuleId::try_new(MODULE_ID).unwrap(),
                execution: ExecutionContext {
                    tenant_id: TenantId::try_new(tenant).unwrap(),
                    actor_id: ActorId::try_new("privacy-officer").unwrap(),
                    request_id: RequestId::try_new("request-privacy-create").unwrap(),
                    correlation_id: CorrelationId::try_new("correlation-privacy-create").unwrap(),
                    causation_id: CausationId::try_new("causation-privacy-create").unwrap(),
                    trace_id: TraceId::try_new("trace-privacy-create").unwrap(),
                    capability_id: CapabilityId::try_new(CREATE_PRIVACY_CASE_CAPABILITY).unwrap(),
                    capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                    idempotency_key: IdempotencyKey::try_new(idempotency_key).unwrap(),
                    business_transaction_id: BusinessTransactionId::try_new(
                        "transaction-privacy-create",
                    )
                    .unwrap(),
                    schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                    request_started_at_unix_nanos: started_at,
                },
            },
            input: support::protobuf_payload(
                MODULE_ID,
                CREATE_PRIVACY_CASE_REQUEST_SCHEMA,
                DataClass::Confidential,
                &command,
            )
            .unwrap(),
            input_hash: [7; 32],
            approval: None,
        }
    }

    fn predecessor_snapshot(tenant: &str, case_id: &str, terminal: bool) -> RecordSnapshot {
        let mut predecessor = PrivacyCase::new(
            RecordId::try_new(case_id).unwrap(),
            TenantId::try_new(tenant).unwrap(),
            PrivacyCaseKind::Access,
            SchemaVersion::try_new("privacy-policy/1").unwrap(),
            100,
            None,
        )
        .unwrap();
        if terminal {
            predecessor.cancel(1, 101).unwrap();
        }
        RecordSnapshot {
            reference: privacy_case_record_ref(&predecessor).unwrap(),
            version: i64::try_from(predecessor.version()).unwrap(),
            payload: privacy_case_persisted_payload(&predecessor).unwrap(),
        }
    }

    #[test]
    fn deterministic_identity_is_tenant_and_idempotency_bound() {
        let first = deterministic_privacy_case_id("tenant-a", "key-a").unwrap();
        assert_eq!(
            first,
            deterministic_privacy_case_id("tenant-a", "key-a").unwrap()
        );
        assert_ne!(
            first,
            deterministic_privacy_case_id("tenant-b", "key-a").unwrap()
        );
        assert_ne!(
            first,
            deterministic_privacy_case_id("tenant-a", "key-b").unwrap()
        );
    }

    #[test]
    fn root_create_is_one_confidential_atomic_draft_plan() {
        let request = request("tenant-a", "key-a", None);
        let definition = capability_definition().unwrap();
        let target = CustomerPrivacyCaseCreateCapabilityPlanner
            .target(&definition, &request)
            .unwrap();
        assert_eq!(target.presence, AggregatePresence::MustBeAbsent);

        let plan = CustomerPrivacyCaseCreateCapabilityPlanner
            .plan(&definition, &request, None)
            .unwrap();
        assert_eq!(plan.batch.records.len(), 1);
        assert_eq!(plan.batch.relationships.len(), 0);
        assert_eq!(plan.batch.events.len(), 1);
        assert_eq!(plan.batch.audits.len(), 1);
        assert_eq!(plan.batch.idempotency.key, "key-a");

        let RecordMutation::Create { reference, payload } = &plan.batch.records[0] else {
            panic!("case.create must create exactly one record");
        };
        assert_eq!(payload.data_class, DataClass::Confidential);
        assert_eq!(plan.batch.events[0].event.aggregate, *reference);

        let response =
            wire::CreatePrivacyCaseResponse::decode(plan.output.as_ref().unwrap().bytes.as_slice())
                .unwrap();
        let privacy_case = response.privacy_case.unwrap();
        assert_eq!(
            privacy_case.privacy_case_ref.unwrap().privacy_case_id,
            reference.record_id.as_str()
        );
        assert_eq!(privacy_case.status, wire::PrivacyCaseStatus::Draft as i32);
        assert_eq!(privacy_case.version, 1);

        let event = wire::PrivacyCaseCreatedEvent::decode(
            plan.batch.events[0].event.payload.bytes.as_slice(),
        )
        .unwrap();
        assert_eq!(
            event
                .privacy_case
                .unwrap()
                .privacy_case_ref
                .unwrap()
                .privacy_case_id,
            reference.record_id.as_str()
        );
    }

    #[test]
    fn terminal_predecessor_allows_separate_successor_without_mutating_lineage() {
        let predecessor_id = "privacy-case-predecessor";
        let request = request("tenant-a", "key-successor", Some(predecessor_id));
        let definition = capability_definition().unwrap();
        let predecessor = predecessor_snapshot("tenant-a", predecessor_id, true);
        let target = CustomerPrivacyCaseCreateCapabilityPlanner
            .target(&definition, &request)
            .unwrap();
        assert_eq!(target.presence, AggregatePresence::MustExist);
        assert_eq!(target.reference, predecessor.reference);

        let plan = CustomerPrivacyCaseCreateCapabilityPlanner
            .plan(&definition, &request, Some(&predecessor))
            .unwrap();
        let RecordMutation::Create { reference, .. } = &plan.batch.records[0] else {
            panic!("successor must be a new record");
        };
        assert_ne!(reference, &predecessor.reference);
        let response =
            wire::CreatePrivacyCaseResponse::decode(plan.output.unwrap().bytes.as_slice()).unwrap();
        assert_eq!(
            response
                .privacy_case
                .unwrap()
                .previous_privacy_case_ref
                .unwrap()
                .privacy_case_id,
            predecessor_id
        );
    }

    #[test]
    fn nonterminal_cross_tenant_and_malformed_predecessors_fail_closed() {
        let predecessor_id = "privacy-case-predecessor";
        let definition = capability_definition().unwrap();
        let request = request("tenant-a", "key-successor", Some(predecessor_id));

        let nonterminal = predecessor_snapshot("tenant-a", predecessor_id, false);
        let error = CustomerPrivacyCaseCreateCapabilityPlanner
            .plan(&definition, &request, Some(&nonterminal))
            .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_PRIVACY_PREVIOUS_CASE_NOT_TERMINAL");

        let other_tenant = predecessor_snapshot("tenant-b", predecessor_id, true);
        let error = CustomerPrivacyCaseCreateCapabilityPlanner
            .plan(&definition, &request, Some(&other_tenant))
            .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_PRIVACY_PREVIOUS_CASE_NOT_FOUND");

        let mut malformed = predecessor_snapshot("tenant-a", predecessor_id, true);
        malformed.payload.bytes = b"{}".to_vec();
        let error = CustomerPrivacyCaseCreateCapabilityPlanner
            .plan(&definition, &request, Some(&malformed))
            .unwrap_err();
        assert_eq!(error.code, "CUSTOMER_PRIVACY_PREVIOUS_CASE_INVALID");
        assert!(!error.safe_message.contains('{'));
    }

    #[test]
    fn invalid_enum_policy_and_timestamp_produce_no_plan() {
        let definition = capability_definition().unwrap();
        for request in [
            request_with("tenant-a", "key-enum", None, 99, "privacy-policy/1", 1),
            request_with(
                "tenant-a",
                "key-policy",
                None,
                wire::PrivacyCaseKind::Access as i32,
                "",
                1,
            ),
            request_with(
                "tenant-a",
                "key-time",
                None,
                wire::PrivacyCaseKind::Access as i32,
                "privacy-policy/1",
                -1,
            ),
        ] {
            let error = CustomerPrivacyCaseCreateCapabilityPlanner
                .plan(&definition, &request, None)
                .unwrap_err();
            assert_eq!(error.category, ErrorCategory::InvalidArgument);
            assert!(!error.retryable);
        }
    }
}
