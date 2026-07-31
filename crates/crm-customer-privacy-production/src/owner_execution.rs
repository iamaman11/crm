use crate::legacy::CustomerPrivacyProductionDependencies;
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, TransactionalCapabilityExecutor,
};
use crm_consents_privacy_scope_adapter::{
    consents_privacy_action_definition, consents_privacy_action_planner,
};
use crm_contact_points_privacy_scope_adapter::{
    contact_points_privacy_action_definition, contact_points_privacy_action_planner,
};
use crm_core_data::{
    PostgresDataStore, PostgresPrivacyOwnerActionExecutor, PrivacyOwnerActionPlanner,
    postgres_sqlx::{self, Row},
};
use crm_customer_accounts_privacy_scope_adapter::{
    customer_accounts_privacy_action_definition, customer_accounts_privacy_action_planner,
};
use crm_customer_data_operations_privacy_scope_adapter::{
    customer_data_operations_privacy_action_definition,
    customer_data_operations_privacy_action_planner,
};
use crm_customer_enrichment_privacy_scope_adapter::{
    customer_enrichment_privacy_action_definition, customer_enrichment_privacy_action_planner,
};
use crm_customer_privacy::{
    MODULE_ID as CUSTOMER_PRIVACY_MODULE_ID, OWNER_ACTION_ATTEMPT_STATE_MAXIMUM_BYTES,
    OWNER_ACTION_ATTEMPT_STATE_RETENTION_POLICY_ID, OWNER_ACTION_ATTEMPT_STATE_SCHEMA_ID,
    OWNER_ACTION_ATTEMPT_STATE_SCHEMA_VERSION, OWNER_ACTION_COMMAND_MAXIMUM_BYTES,
    OWNER_ACTION_COMMAND_RETENTION_POLICY_ID, OWNER_ACTION_COMMAND_SCHEMA_ID,
    OWNER_ACTION_COMMAND_SCHEMA_VERSION, PrivacyOwnerActionCommand,
    decode_owner_action_attempt_state, discovery_sha256, encode_owner_action_command,
    owner_action_attempt_state_descriptor_hash, owner_action_command_descriptor_hash,
};
pub use crm_customer_privacy::{
    PrivacyOwnerActionAttempt, PrivacyOwnerActionOutcome, PrivacyOwnerOutcomeStatus,
};
pub use crm_customer_privacy_application::{
    CheckpointAdvance, ExecutionPreparation, OwnerActionEndpoint, OwnerActionEndpoints,
    OwnerActionRequest, OwnerActionResult, OwnerExecutionInvocation, OwnerExecutionPersistencePort,
    OwnerExecutionResult, OwnerPrivacyActionPort, PrivacyOwnerExecutionService,
    PrivacyOwnerOutcomePage, PrivacyOwnerOutcomePosition, PrivacyReadContext,
    PrivacyReadPersistencePort,
};
pub use crm_customer_privacy_postgres::{
    PostgresOwnerExecutionPersistence, PostgresPrivacyReadPersistence,
    retention_decision_persisted_payload,
};
use crm_data_quality_privacy_scope_adapter::{
    data_quality_privacy_action_definition, data_quality_privacy_action_planner,
};
use crm_identity_resolution_privacy_scope_adapter::{
    identity_resolution_privacy_action_definition, identity_resolution_privacy_action_planner,
};
use crm_module_sdk::{
    BusinessTransactionId, CausationId, DataClass, ErrorCategory, ExecutionContext,
    ModuleExecutionContext, PayloadEncoding, PortFuture, RequestId, RetentionPolicyId, SchemaId,
    SchemaVersion, SdkError, TypedPayload,
};
use crm_parties_privacy_scope_adapter::{
    parties_privacy_action_definition, parties_privacy_action_planner,
};
use crm_party_relationships_privacy_scope_adapter::{
    party_relationships_privacy_action_definition, party_relationships_privacy_action_planner,
};
use std::sync::Arc;

const EXECUTION_CONTEXT_SCHEMA_VERSION: &str = "1.0.0";
const FALLBACK_FAILURE_CODE: &str = "PRIVACY_OWNER_ACTION_FAILED";

/// Build the trusted-internal repository-step-eight execution coordinator.
///
/// This compatibility entry point accepts an injected exact owner registry for
/// focused tests and alternative process composition. Production should use
/// [`build_canonical_internal_owner_execution`]. Neither function registers a
/// public route or creates a worker.
pub fn build_internal_owner_execution(
    dependencies: &CustomerPrivacyProductionDependencies,
    endpoints: impl IntoIterator<Item = OwnerActionEndpoint>,
) -> Result<PrivacyOwnerExecutionService, SdkError> {
    Ok(PrivacyOwnerExecutionService::new(
        dependencies.activation.clone(),
        Arc::new(PostgresOwnerExecutionPersistence::new(Arc::new(
            dependencies.store.clone(),
        ))),
        OwnerActionEndpoints::exact_canonical(endpoints)?,
    ))
}

/// Build the exact production registry for all nine authoritative privacy owners.
///
/// Every endpoint receives the same specialized PostgreSQL transaction executor,
/// but all resource/action decisions remain in the owner-local planner. The
/// production port reloads the append-once durable attempt and derives the full
/// canonical owner command from that evidence before any owner record is locked.
pub fn build_canonical_internal_owner_execution(
    dependencies: &CustomerPrivacyProductionDependencies,
) -> Result<PrivacyOwnerExecutionService, SdkError> {
    let store = dependencies.store.clone();
    let endpoints = [
        postgres_owner_endpoint(
            store.clone(),
            consents_privacy_action_definition()?,
            Arc::new(consents_privacy_action_planner()),
        ),
        postgres_owner_endpoint(
            store.clone(),
            contact_points_privacy_action_definition()?,
            Arc::new(contact_points_privacy_action_planner()),
        ),
        postgres_owner_endpoint(
            store.clone(),
            customer_accounts_privacy_action_definition()?,
            Arc::new(customer_accounts_privacy_action_planner()),
        ),
        postgres_owner_endpoint(
            store.clone(),
            customer_data_operations_privacy_action_definition()?,
            Arc::new(customer_data_operations_privacy_action_planner()),
        ),
        postgres_owner_endpoint(
            store.clone(),
            customer_enrichment_privacy_action_definition()?,
            Arc::new(customer_enrichment_privacy_action_planner()),
        ),
        postgres_owner_endpoint(
            store.clone(),
            data_quality_privacy_action_definition()?,
            Arc::new(data_quality_privacy_action_planner()),
        ),
        postgres_owner_endpoint(
            store.clone(),
            identity_resolution_privacy_action_definition()?,
            Arc::new(identity_resolution_privacy_action_planner()),
        ),
        postgres_owner_endpoint(
            store.clone(),
            parties_privacy_action_definition()?,
            Arc::new(parties_privacy_action_planner()),
        ),
        postgres_owner_endpoint(
            store,
            party_relationships_privacy_action_definition()?,
            Arc::new(party_relationships_privacy_action_planner()),
        ),
    ];
    build_internal_owner_execution(dependencies, endpoints)
}

fn postgres_owner_endpoint(
    store: PostgresDataStore,
    definition: CapabilityDefinition,
    planner: Arc<dyn PrivacyOwnerActionPlanner>,
) -> OwnerActionEndpoint {
    let owner_module_id = definition.owner_module_id.clone();
    let executor = PostgresPrivacyOwnerActionExecutor::new(store.clone(), planner);
    OwnerActionEndpoint {
        owner_module_id,
        executor: Arc::new(PostgresOwnerActionPort {
            store,
            definition,
            executor,
        }),
    }
}

#[derive(Clone)]
struct PostgresOwnerActionPort {
    store: PostgresDataStore,
    definition: CapabilityDefinition,
    executor: PostgresPrivacyOwnerActionExecutor,
}

impl std::fmt::Debug for PostgresOwnerActionPort {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PostgresOwnerActionPort")
            .field("definition", &self.definition)
            .field("executor", &self.executor)
            .finish_non_exhaustive()
    }
}

impl OwnerPrivacyActionPort for PostgresOwnerActionPort {
    fn apply<'a>(
        &'a self,
        request: OwnerActionRequest,
    ) -> PortFuture<'a, Result<OwnerActionResult, SdkError>> {
        Box::pin(async move {
            validate_endpoint_coordinate(&self.definition, &request)?;
            let attempt = load_durable_attempt(&self.store, &self.definition, &request).await?;
            validate_request_attempt_binding(&request, &attempt)?;
            let capability_request = capability_request(&self.definition, &request, &attempt)?;
            match self
                .executor
                .execute(&self.definition, capability_request)
                .await
            {
                Ok(_) => Ok(OwnerActionResult {
                    status: PrivacyOwnerOutcomeStatus::Succeeded,
                    safe_failure_code: None,
                }),
                Err(error) => Ok(owner_failure_result(error)),
            }
        })
    }
}

async fn load_durable_attempt(
    store: &PostgresDataStore,
    definition: &CapabilityDefinition,
    request: &OwnerActionRequest,
) -> Result<PrivacyOwnerActionAttempt, SdkError> {
    let request_id = derived_request_id(&request.attempt_id)?;
    let business_transaction_id = derived_business_transaction_id(&request.attempt_id)?;
    let mut transaction = store.pool().begin().await.map_err(database_error)?;
    postgres_sqlx::query(
        r#"
        SELECT set_config('app.tenant_id', $1, true),
               set_config('app.actor_id', $2, true),
               set_config('app.request_id', $3, true),
               set_config('app.capability_id', $4, true),
               set_config('app.capability_version', $5, true),
               set_config('app.business_transaction_id', $6, true)
        "#,
    )
    .bind(request.tenant_id.as_str())
    .bind(request.actor_id.as_str())
    .bind(request_id.as_str())
    .bind(definition.capability_id.as_str())
    .bind(definition.capability_version.as_str())
    .bind(business_transaction_id.as_str())
    .execute(&mut *transaction)
    .await
    .map_err(database_error)?;

    let row = postgres_sqlx::query(
        r#"
        SELECT attempt_id, payload_bytes AS attempt_payload,
               schema_id AS attempt_schema_id,
               schema_version AS attempt_schema_version,
               descriptor_hash AS attempt_descriptor_hash,
               maximum_payload_size AS attempt_maximum,
               retention_policy_id AS attempt_retention
        FROM crm.customer_privacy_owner_action_attempts
        WHERE tenant_id = $1 AND attempt_id = $2
        FOR SHARE
        "#,
    )
    .bind(request.tenant_id.as_str())
    .bind(request.attempt_id.as_str())
    .fetch_optional(&mut *transaction)
    .await
    .map_err(database_error)?
    .ok_or_else(durable_attempt_not_found)?;
    let attempt = attempt_from_row(&row)?;
    transaction.commit().await.map_err(database_error)?;
    Ok(attempt)
}

fn attempt_from_row(
    row: &postgres_sqlx::postgres::PgRow,
) -> Result<PrivacyOwnerActionAttempt, SdkError> {
    let schema_id: String = row.try_get("attempt_schema_id").map_err(database_error)?;
    let schema_version: String = row
        .try_get("attempt_schema_version")
        .map_err(database_error)?;
    let descriptor_hash: Vec<u8> = row
        .try_get("attempt_descriptor_hash")
        .map_err(database_error)?;
    let maximum: i64 = row.try_get("attempt_maximum").map_err(database_error)?;
    let retention: String = row.try_get("attempt_retention").map_err(database_error)?;
    let expected_maximum =
        i64::try_from(OWNER_ACTION_ATTEMPT_STATE_MAXIMUM_BYTES).map_err(configuration_error)?;
    if schema_id != OWNER_ACTION_ATTEMPT_STATE_SCHEMA_ID
        || schema_version != OWNER_ACTION_ATTEMPT_STATE_SCHEMA_VERSION
        || descriptor_hash.as_slice() != owner_action_attempt_state_descriptor_hash()
        || maximum != expected_maximum
        || retention != OWNER_ACTION_ATTEMPT_STATE_RETENTION_POLICY_ID
    {
        return Err(durable_attempt_invalid(
            "durable attempt persistence envelope is not the frozen v1 contract",
        ));
    }
    let bytes: Vec<u8> = row.try_get("attempt_payload").map_err(database_error)?;
    let attempt = decode_owner_action_attempt_state(&bytes)?;
    let envelope_id: String = row.try_get("attempt_id").map_err(database_error)?;
    if attempt.attempt_id().as_str() != envelope_id {
        return Err(durable_attempt_invalid(
            "durable attempt identity differs from its persistence envelope",
        ));
    }
    Ok(attempt)
}

fn validate_endpoint_coordinate(
    definition: &CapabilityDefinition,
    request: &OwnerActionRequest,
) -> Result<(), SdkError> {
    if request.owner_module_id != definition.owner_module_id
        || request.owner_capability_id != definition.capability_id.as_str()
        || request.owner_capability_version != definition.capability_version.as_str()
    {
        return Err(durable_attempt_invalid(
            "owner endpoint request differs from the registered capability coordinate",
        ));
    }
    Ok(())
}

fn validate_request_attempt_binding(
    request: &OwnerActionRequest,
    attempt: &PrivacyOwnerActionAttempt,
) -> Result<(), SdkError> {
    if request.tenant_id != *attempt.tenant_id()
        || request.privacy_case_id != *attempt.privacy_case_id()
        || request.action_plan_id != *attempt.action_plan_id()
        || request.retention_decision_id != *attempt.retention_decision_id()
        || request.attempt_id != *attempt.attempt_id()
        || request.owner_module_id != *attempt.owner_module_id()
        || request.owner_capability_id != attempt.owner_capability_id()
        || request.owner_capability_version != attempt.owner_capability_version()
        || request.target_idempotency_key != attempt.target_idempotency_key().as_str()
        || request.resource_type != attempt.resource_type()
        || request.resource_id != *attempt.resource_id()
        || request.resource_version != attempt.resource_version()
        || request.action_code != attempt.action_code()
    {
        return Err(durable_attempt_invalid(
            "owner request differs from append-once durable attempt evidence",
        ));
    }
    Ok(())
}

fn capability_request(
    definition: &CapabilityDefinition,
    request: &OwnerActionRequest,
    attempt: &PrivacyOwnerActionAttempt,
) -> Result<CapabilityRequest, SdkError> {
    let command = PrivacyOwnerActionCommand::from_attempt(attempt)?;
    let bytes = encode_owner_action_command(&command)?;
    let input = TypedPayload {
        owner: crm_module_sdk::ModuleId::try_new(CUSTOMER_PRIVACY_MODULE_ID)
            .map_err(configuration_error)?,
        schema_id: SchemaId::try_new(OWNER_ACTION_COMMAND_SCHEMA_ID)
            .map_err(configuration_error)?,
        schema_version: SchemaVersion::try_new(OWNER_ACTION_COMMAND_SCHEMA_VERSION)
            .map_err(configuration_error)?,
        descriptor_hash: owner_action_command_descriptor_hash(),
        data_class: DataClass::Restricted,
        encoding: PayloadEncoding::Json,
        maximum_size_bytes: OWNER_ACTION_COMMAND_MAXIMUM_BYTES,
        retention_policy_id: RetentionPolicyId::try_new(OWNER_ACTION_COMMAND_RETENTION_POLICY_ID)
            .map_err(configuration_error)?,
        bytes,
    };
    input.validate()?;
    let input_hash = discovery_sha256(&input.bytes);
    Ok(CapabilityRequest {
        context: ModuleExecutionContext {
            module_id: definition.owner_module_id.clone(),
            execution: ExecutionContext {
                tenant_id: command.tenant_id().clone(),
                actor_id: request.actor_id.clone(),
                request_id: derived_request_id(command.attempt_id())?,
                correlation_id: request.correlation_id.clone(),
                causation_id: derived_causation_id(command.attempt_id())?,
                trace_id: request.trace_id.clone(),
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                idempotency_key: command.target_idempotency_key().clone(),
                business_transaction_id: derived_business_transaction_id(command.attempt_id())?,
                schema_version: SchemaVersion::try_new(EXECUTION_CONTEXT_SCHEMA_VERSION)
                    .map_err(configuration_error)?,
                request_started_at_unix_nanos: command.planned_at_unix_nanos(),
            },
        },
        input,
        input_hash,
        approval: None,
    })
}

fn owner_failure_result(error: SdkError) -> OwnerActionResult {
    OwnerActionResult {
        status: if error.retryable {
            PrivacyOwnerOutcomeStatus::FailedRetryable
        } else {
            PrivacyOwnerOutcomeStatus::FailedTerminal
        },
        safe_failure_code: Some(canonical_failure_code(&error.code)),
    }
}

fn canonical_failure_code(code: &str) -> String {
    if !code.is_empty()
        && code.len() <= 96
        && code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        code.to_owned()
    } else {
        FALLBACK_FAILURE_CODE.to_owned()
    }
}

fn derived_request_id(attempt_id: &crm_module_sdk::RecordId) -> Result<RequestId, SdkError> {
    RequestId::try_new(format!("privacy-owner-request-{attempt_id}")).map_err(configuration_error)
}

fn derived_causation_id(attempt_id: &crm_module_sdk::RecordId) -> Result<CausationId, SdkError> {
    CausationId::try_new(format!("privacy-owner-causation-{attempt_id}"))
        .map_err(configuration_error)
}

fn derived_business_transaction_id(
    attempt_id: &crm_module_sdk::RecordId,
) -> Result<BusinessTransactionId, SdkError> {
    BusinessTransactionId::try_new(format!("privacy-owner-transaction-{attempt_id}"))
        .map_err(configuration_error)
}

fn durable_attempt_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_ATTEMPT_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The durable owner action attempt was not found.",
    )
}

fn durable_attempt_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_ATTEMPT_INVALID",
        ErrorCategory::Internal,
        false,
        "The durable owner action attempt is invalid.",
    )
    .with_internal_reference(reference)
}

fn database_error(error: postgres_sqlx::Error) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_ACTION_DATABASE_ERROR",
        ErrorCategory::Dependency,
        true,
        "The owner action could not access durable state.",
    )
    .with_internal_reference(error.to_string())
}

fn configuration_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_OWNER_ACTION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "Customer Privacy owner action composition is invalid.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_owner_action_definitions_remain_frozen() {
        let definitions = [
            consents_privacy_action_definition().unwrap(),
            contact_points_privacy_action_definition().unwrap(),
            customer_accounts_privacy_action_definition().unwrap(),
            customer_data_operations_privacy_action_definition().unwrap(),
            customer_enrichment_privacy_action_definition().unwrap(),
            data_quality_privacy_action_definition().unwrap(),
            identity_resolution_privacy_action_definition().unwrap(),
            parties_privacy_action_definition().unwrap(),
            party_relationships_privacy_action_definition().unwrap(),
        ];
        let mut owners = definitions
            .iter()
            .map(|definition| definition.owner_module_id.as_str())
            .collect::<Vec<_>>();
        owners.sort_unstable();
        assert_eq!(
            owners,
            vec![
                "crm.consents",
                "crm.contact-points",
                "crm.customer-accounts",
                "crm.customer-data-operations",
                "crm.customer-enrichment",
                "crm.data-quality",
                "crm.identity-resolution",
                "crm.parties",
                "crm.party-relationships",
            ]
        );
        assert!(definitions.iter().all(|definition| {
            definition.mutation
                && definition.requires_idempotency
                && definition.capability_version.as_str() == "1.0.0"
        }));
    }

    #[test]
    fn failure_codes_are_safe_and_bounded() {
        assert_eq!(
            canonical_failure_code("PRIVACY_OWNER_ACTION_UNSUPPORTED"),
            "PRIVACY_OWNER_ACTION_UNSUPPORTED"
        );
        assert_eq!(canonical_failure_code("not-safe"), FALLBACK_FAILURE_CODE);
        assert_eq!(
            canonical_failure_code(&"A".repeat(97)),
            FALLBACK_FAILURE_CODE
        );
    }
}
