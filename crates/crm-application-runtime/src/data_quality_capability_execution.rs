use crm_capability_adapters::semantic_input_hash;
use crm_capability_plan_support as support;
use crm_capability_runtime::{
    CapabilityAuthorizer, CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest,
    TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor, RecordGetQuery};
use crm_data_quality_capability_adapter::{
    DataQualityCompletenessProfileCapabilityPlanner, DataQualityEvaluationJobCapabilityPlanner,
    DataQualityRemediationCompletionPlanner, FINDING_RECORD_TYPE, MODULE_ID,
    PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE, PARTY_RULE_SET_VERSION_RECORD_TYPE,
    PUBLISH_PARTY_COMPLETENESS_PROFILE_CAPABILITY, PartyDisplayNameRemediationAttempt,
    PartyDisplayNameRemediationIdentity, PartyFinding, PartyQualityEvaluator,
    REMEDIATE_PARTY_DISPLAY_NAME_CAPABILITY, REMEDIATE_PARTY_DISPLAY_NAME_REQUEST_SCHEMA,
    REQUEST_PARTY_EVALUATION_CAPABILITY, completeness_profile_reference_scope_from_request,
    evaluation_reference_scope_from_request, party_completeness_profile_from_immutable_snapshot,
    party_finding_from_snapshot, party_rule_set_from_snapshot,
};
use crm_data_quality_query_adapter::registered_party_quality_query_adapter;
use crm_data_quality_source_composition::{
    GovernedPartyQualitySource, PartyQualitySource, PartyQualitySourceRequest,
};
use crm_module_sdk::{
    BusinessTransactionId, DataClass, ErrorCategory, ModuleId, PortFuture, RecordId, RecordType,
    SchemaVersion, SdkError,
};
use crm_parties_capability_adapter::{
    MODULE_ID as PARTIES_MODULE_ID, UPDATE_CAPABILITY as PARTY_UPDATE_CAPABILITY,
    UPDATE_REQUEST_SCHEMA as PARTY_UPDATE_REQUEST_SCHEMA,
    UPDATE_RESPONSE_SCHEMA as PARTY_UPDATE_RESPONSE_SCHEMA,
    capability_definition as party_capability_definition,
};
use crm_proto_contracts::crm::{data_quality::v1 as data_quality, parties::v1 as parties};
use crm_query_runtime::QueryAuthorizer;
use prost::Message;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

static REMEDIATION_FAILPOINT_USED: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
pub struct DataQualityCapabilityExecutor {
    store: PostgresDataStore,
    fallback: Arc<dyn TransactionalCapabilityExecutor>,
    capability_authorizer: Arc<dyn CapabilityAuthorizer>,
    query_authorizer: Arc<dyn QueryAuthorizer>,
}

impl DataQualityCapabilityExecutor {
    pub fn new(
        store: PostgresDataStore,
        fallback: Arc<dyn TransactionalCapabilityExecutor>,
        capability_authorizer: Arc<dyn CapabilityAuthorizer>,
        query_authorizer: Arc<dyn QueryAuthorizer>,
    ) -> Self {
        Self {
            store,
            fallback,
            capability_authorizer,
            query_authorizer,
        }
    }

    async fn execute_remediation(
        &self,
        definition: &CapabilityDefinition,
        request: CapabilityRequest,
    ) -> Result<CapabilityExecutionResult, SdkError> {
        let command: data_quality::RemediatePartyDisplayNameRequest =
            support::decode_request_with_data_class(
                &request,
                MODULE_ID,
                REMEDIATE_PARTY_DISPLAY_NAME_REQUEST_SCHEMA,
                DataClass::Personal,
            )?;
        let finding_id = required_finding_id(command.finding_ref.as_ref())?;
        let finding_snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.execution.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: record_type(FINDING_RECORD_TYPE)?,
                record_id: finding_id,
            })
            .await?
            .ok_or_else(remediation_finding_unavailable)?;
        if command.expected_finding_version <= 0
            || command.expected_finding_version != finding_snapshot.version
        {
            return Err(remediation_evidence_conflict());
        }
        let finding = party_finding_from_snapshot(&finding_snapshot)?;
        let observation_id =
            required_observation_id(command.expected_current_observation_ref.as_ref())?;
        if observation_id.as_str() != finding.current_observation_id()
            || command.expected_party_resource_version != finding.evaluated_party_resource_version()
        {
            return Err(remediation_evidence_conflict());
        }
        self.ensure_display_name_rule(&request, &finding).await?;

        let identity = PartyDisplayNameRemediationIdentity::derive(
            &request.context.execution.tenant_id,
            &request.context.execution.idempotency_key,
            &finding,
            command.expected_finding_version,
            observation_id.as_str(),
            command.expected_party_resource_version,
            &command.display_name,
        )?;
        let source_request_identity = format!("dq-remediation-source-{}", identity.attempt_id());
        let governed_source = GovernedPartyQualitySource::new(
            registered_party_quality_query_adapter()?,
            self.query_authorizer.clone(),
        );
        let source = governed_source
            .get(PartyQualitySourceRequest {
                tenant_id: &request.context.execution.tenant_id,
                actor_id: &request.context.execution.actor_id,
                request_identity: &source_request_identity,
                party_id: finding.party_id(),
                request_started_at_unix_nanos: request
                    .context
                    .execution
                    .request_started_at_unix_nanos,
            })
            .await?;
        let expected_party_version = command.expected_party_resource_version;
        let applied_party_version = expected_party_version
            .checked_add(1)
            .ok_or_else(remediation_evidence_conflict)?;
        let target_not_applied = source.resource_version == expected_party_version;
        let target_already_applied = source.resource_version == applied_party_version
            && source.display_name == command.display_name;
        if source.party_id.as_str() != finding.party_id().as_str()
            || (!target_not_applied && !target_already_applied)
        {
            return Err(remediation_evidence_conflict());
        }

        let target_definition = party_capability_definition(PARTY_UPDATE_CAPABILITY)?;
        let target_input = support::protobuf_payload(
            PARTIES_MODULE_ID,
            PARTY_UPDATE_REQUEST_SCHEMA,
            DataClass::Personal,
            &parties::UpdatePartyRequest {
                party_ref: Some(crm_proto_contracts::crm::customer::v1::PartyRef {
                    party_id: finding.party_id().as_str().to_owned(),
                }),
                expected_version: expected_party_version,
                display_name: command.display_name.clone(),
            },
        )?;
        let target_input_hash = semantic_input_hash(&target_input);
        let target_request = bind_target_request(
            &request,
            &target_definition,
            target_input,
            target_input_hash,
            identity.target_idempotency_key(),
        )?;
        authorize_target(
            self.capability_authorizer.as_ref(),
            &target_definition,
            &target_request,
        )
        .await?;
        let target_result = self
            .base
            .execute(&target_definition, target_request)
            .await?;
        let updated_party = decode_updated_party(target_result)?;
        let updated_version = updated_party
            .resource_version
            .as_ref()
            .ok_or_else(|| remediation_target_contract_invalid("updated Party version is missing"))?
            .version;

        if fail_after_target_once() {
            return Err(SdkError::new(
                "DATA_QUALITY_REMEDIATION_OUTCOME_DEFERRED",
                ErrorCategory::Unavailable,
                true,
                "The Party was updated, but the Data Quality remediation outcome must be replayed.",
            ));
        }

        let attempt = PartyDisplayNameRemediationAttempt::complete(
            request.context.execution.tenant_id.clone(),
            identity,
            &finding,
            command.expected_finding_version,
            observation_id.as_str(),
            expected_party_version,
            command.display_name,
            updated_version,
            request.context.execution.request_started_at_unix_nanos,
        )?;
        PostgresTransactionalAggregateExecutor::new(
            self.store.clone(),
            Arc::new(DataQualityRemediationCompletionPlanner::new(
                attempt,
                updated_party,
            )?),
        )
        .execute(definition, request)
        .await
    }

    async fn ensure_display_name_rule(
        &self,
        request: &CapabilityRequest,
        finding: &PartyFinding,
    ) -> Result<(), SdkError> {
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.execution.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: record_type(PARTY_RULE_SET_VERSION_RECORD_TYPE)?,
                record_id: RecordId::try_new(finding.rule_set_version_id())
                    .map_err(reference_configuration_error)?,
            })
            .await?
            .ok_or_else(remediation_rule_unavailable)?;
        let rule_set = party_rule_set_from_snapshot(&snapshot)?;
        let rule = rule_set
            .rule(finding.rule_key())
            .ok_or_else(remediation_rule_unavailable)?;
        if !matches!(
            rule.evaluator(),
            PartyQualityEvaluator::DisplayNameMinUtf8Bytes(_)
                | PartyQualityEvaluator::DisplayNamePlaceholderExactAsciiCasefold(_)
        ) {
            return Err(SdkError::new(
                "DATA_QUALITY_REMEDIATION_RULE_UNSUPPORTED",
                ErrorCategory::InvalidArgument,
                false,
                "The finding is not eligible for Party display-name remediation.",
            ));
        }
        Ok(())
    }
}

impl fmt::Debug for DataQualityCapabilityExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DataQualityCapabilityExecutor")
            .field("store", &self.store)
            .field("fallback", &"dyn TransactionalCapabilityExecutor")
            .field("capability_authorizer", &"dyn CapabilityAuthorizer")
            .field("query_authorizer", &"dyn QueryAuthorizer")
            .finish()
    }
}

impl TransactionalCapabilityExecutor for DataQualityCapabilityExecutor {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        match definition.capability_id.as_str() {
            PUBLISH_PARTY_COMPLETENESS_PROFILE_CAPABILITY => Box::pin(async move {
                let scope = completeness_profile_reference_scope_from_request(&request)?;
                let snapshot = self
                    .store
                    .get_record_for_query(&RecordGetQuery {
                        tenant_id: request.context.execution.tenant_id.clone(),
                        owner_module_id: module_id()?,
                        record_type: record_type(PARTY_RULE_SET_VERSION_RECORD_TYPE)?,
                        record_id: RecordId::try_new(scope.rule_set_version_id)
                            .map_err(|_| rule_set_unavailable())?,
                    })
                    .await?
                    .ok_or_else(rule_set_unavailable)?;
                let rule_set = party_rule_set_from_snapshot(&snapshot)?;
                PostgresTransactionalAggregateExecutor::new(
                    self.store.clone(),
                    Arc::new(DataQualityCompletenessProfileCapabilityPlanner::new(
                        rule_set,
                    )),
                )
                .execute(definition, request)
                .await
            }),
            REQUEST_PARTY_EVALUATION_CAPABILITY => Box::pin(async move {
                let scope = evaluation_reference_scope_from_request(&request)?;
                let rule_set_snapshot = self
                    .store
                    .get_record_for_query(&RecordGetQuery {
                        tenant_id: request.context.execution.tenant_id.clone(),
                        owner_module_id: module_id()?,
                        record_type: record_type(PARTY_RULE_SET_VERSION_RECORD_TYPE)?,
                        record_id: scope.rule_set_version_id,
                    })
                    .await?
                    .ok_or_else(evaluation_definitions_unavailable)?;
                let rule_set = party_rule_set_from_snapshot(&rule_set_snapshot)?;
                let profile_snapshot = self
                    .store
                    .get_record_for_query(&RecordGetQuery {
                        tenant_id: request.context.execution.tenant_id.clone(),
                        owner_module_id: module_id()?,
                        record_type: record_type(PARTY_COMPLETENESS_PROFILE_VERSION_RECORD_TYPE)?,
                        record_id: scope.profile_version_id,
                    })
                    .await?
                    .ok_or_else(evaluation_definitions_unavailable)?;
                let profile = party_completeness_profile_from_immutable_snapshot(
                    &profile_snapshot,
                    &rule_set,
                )?;
                PostgresTransactionalAggregateExecutor::new(
                    self.store.clone(),
                    Arc::new(DataQualityEvaluationJobCapabilityPlanner::new(
                        rule_set, profile,
                    )?),
                )
                .execute(definition, request)
                .await
            }),
            REMEDIATE_PARTY_DISPLAY_NAME_CAPABILITY => {
                Box::pin(async move { self.execute_remediation(definition, request).await })
            }
            _ => self.fallback.execute(definition, request),
        }
    }
}

fn bind_target_request(
    original: &CapabilityRequest,
    definition: &CapabilityDefinition,
    input: crm_module_sdk::TypedPayload,
    input_hash: [u8; 32],
    idempotency_key: &crm_module_sdk::IdempotencyKey,
) -> Result<CapabilityRequest, SdkError> {
    let mut context = original.context.clone();
    context.module_id = definition.owner_module_id.clone();
    context.execution.capability_id = definition.capability_id.clone();
    context.execution.capability_version = definition.capability_version.clone();
    context.execution.schema_version =
        SchemaVersion::try_new(support::CONTRACT_VERSION).map_err(reference_configuration_error)?;
    context.execution.idempotency_key = idempotency_key.clone();
    context.execution.business_transaction_id =
        BusinessTransactionId::try_new(idempotency_key.as_str())
            .map_err(reference_configuration_error)?;
    Ok(CapabilityRequest {
        context,
        input,
        input_hash,
        approval: None,
    })
}

async fn authorize_target(
    authorizer: &dyn CapabilityAuthorizer,
    definition: &CapabilityDefinition,
    request: &CapabilityRequest,
) -> Result<(), SdkError> {
    let decision = authorizer.authorize(definition, request).await?;
    if decision.allowed {
        return Ok(());
    }
    Err(SdkError::new(
        "DATA_QUALITY_REMEDIATION_TARGET_PERMISSION_DENIED",
        ErrorCategory::Authorization,
        false,
        "The Party update required by remediation is not authorized.",
    )
    .with_internal_reference(format!(
        "decision_id={} reason_code={} policy_version={}",
        decision.decision_id, decision.reason_code, decision.policy_version
    )))
}

fn decode_updated_party(result: CapabilityExecutionResult) -> Result<parties::Party, SdkError> {
    let payload = result
        .output
        .ok_or_else(|| remediation_target_contract_invalid("Party update output is missing"))?;
    if payload.owner.as_str() != PARTIES_MODULE_ID
        || payload.schema_id.as_str() != PARTY_UPDATE_RESPONSE_SCHEMA
        || payload.data_class != DataClass::Personal
    {
        return Err(remediation_target_contract_invalid(
            "Party update output contract differs from the exact owner response",
        ));
    }
    parties::UpdatePartyResponse::decode(payload.bytes.as_slice())
        .map_err(|error| remediation_target_contract_invalid(error.to_string()))?
        .party
        .ok_or_else(|| remediation_target_contract_invalid("updated Party is missing"))
}

fn required_finding_id(
    value: Option<&data_quality::DataQualityFindingRef>,
) -> Result<RecordId, SdkError> {
    RecordId::try_new(
        value
            .ok_or_else(remediation_evidence_conflict)?
            .finding_id
            .clone(),
    )
    .map_err(|_| remediation_evidence_conflict())
}

fn required_observation_id(
    value: Option<&data_quality::DataQualityFindingObservationRef>,
) -> Result<RecordId, SdkError> {
    RecordId::try_new(
        value
            .ok_or_else(remediation_evidence_conflict)?
            .finding_observation_id
            .clone(),
    )
    .map_err(|_| remediation_evidence_conflict())
}

fn fail_after_target_once() -> bool {
    std::env::var_os("CRM_DATA_QUALITY_REMEDIATION_FAIL_AFTER_TARGET_ONCE").is_some()
        && !REMEDIATION_FAILPOINT_USED.swap(true, Ordering::AcqRel)
}

fn module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(MODULE_ID).map_err(reference_configuration_error)
}

fn record_type(value: &str) -> Result<RecordType, SdkError> {
    RecordType::try_new(value).map_err(reference_configuration_error)
}

fn rule_set_unavailable() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_COMPLETENESS_RULE_SET_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "The referenced Party rule-set version is unavailable.",
    )
}

fn evaluation_definitions_unavailable() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_EVALUATION_DEFINITIONS_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "The referenced Party evaluation definitions are unavailable.",
    )
}

fn remediation_finding_unavailable() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_FINDING_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested Data Quality finding was not found.",
    )
}

fn remediation_evidence_conflict() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_REMEDIATION_EVIDENCE_CONFLICT",
        ErrorCategory::Conflict,
        false,
        "The Data Quality finding or Party evidence changed before remediation.",
    )
}

fn remediation_rule_unavailable() -> SdkError {
    SdkError::new(
        "DATA_QUALITY_REMEDIATION_RULE_UNAVAILABLE",
        ErrorCategory::Internal,
        false,
        "The Data Quality finding rule is unavailable.",
    )
}

fn remediation_target_contract_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_REMEDIATION_TARGET_CONTRACT_INVALID",
        ErrorCategory::Internal,
        false,
        "The Party update required by remediation returned invalid evidence.",
    )
    .with_internal_reference(reference.into())
}

fn reference_configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_REFERENCE_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Data Quality reference boundary is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}
