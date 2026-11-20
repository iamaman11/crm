use crm_capability_adapters::semantic_input_hash;
use crm_capability_plan_support::{self as support, persisted_json_bytes_with_data_class};
use crm_capability_runtime::{
    CapabilityAuthorizer, CapabilityDefinition, CapabilityRequest, TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor, RecordGetQuery};
use crm_data_quality::{
    FINDING_OBSERVATION_RECORD_TYPE, FINDING_RECORD_TYPE, PartyCompletenessProfileVersion,
    PartyEvaluationInputSnapshot, PartyEvaluationJob, PartyFindingObservation, PartyQualityInput,
    PartyRuleOutcome, PartyRuleSetVersion, decode_finding_observation_state, decode_finding_state,
    party_finding_id,
};
use crm_data_quality_capability_adapter::{
    DataQualityEvaluationMaterializationPlanner, ExistingPartyFinding,
    ExistingPartyFindingObservation, MATERIALIZE_PARTY_EVALUATION_REQUEST_SCHEMA, MODULE_ID,
    evaluation_materialization_capability_definition, party_finding_observation_persisted_contract,
    party_finding_persisted_contract,
};
use crm_module_sdk::{
    BusinessTransactionId, DataClass, ErrorCategory, IdempotencyKey, ModuleExecutionContext,
    ModuleId, PortFuture, RecordId, RecordSnapshot, RecordType, SchemaVersion, SdkError,
    TypedPayload,
};
use crm_proto_contracts::crm::data_quality::v1 as wire;
use std::collections::BTreeMap;
use std::sync::Arc;

type CurrentFindings = BTreeMap<String, ExistingPartyFinding>;
type CurrentObservations = BTreeMap<String, ExistingPartyFindingObservation>;

struct CurrentFindingEvidence {
    findings: CurrentFindings,
    observations: CurrentObservations,
}

#[derive(Clone)]
pub struct PostgresPartyEvaluationMaterializationSink {
    store: PostgresDataStore,
    authorizer: Arc<dyn CapabilityAuthorizer>,
}

impl PostgresPartyEvaluationMaterializationSink {
    pub fn new(store: PostgresDataStore, authorizer: Arc<dyn CapabilityAuthorizer>) -> Self {
        Self { store, authorizer }
    }

    pub fn materialize<'a>(
        &'a self,
        base_context: &'a ModuleExecutionContext,
        job: &'a PartyEvaluationJob,
        expected_job_version: i64,
        rule_set: PartyRuleSetVersion,
        profile: PartyCompletenessProfileVersion,
        input: PartyEvaluationInputSnapshot,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let definition = evaluation_materialization_capability_definition()?;
            let command = wire::MaterializePartyEvaluationRequest {
                evaluation_job_ref: Some(wire::PartyEvaluationJobRef {
                    evaluation_job_id: job.job_id().as_str().to_owned(),
                }),
                expected_job_version,
            };
            let request_payload = support::protobuf_payload(
                MODULE_ID,
                MATERIALIZE_PARTY_EVALUATION_REQUEST_SCHEMA,
                DataClass::Personal,
                &command,
            )?;
            let input_hash = semantic_input_hash(&request_payload);
            let request = bind_materialization_request(
                base_context,
                &definition,
                request_payload,
                input_hash,
            )?;
            let decision = self.authorizer.authorize(&definition, &request).await?;
            if !decision.allowed {
                return Err(SdkError::new(
                    "DATA_QUALITY_EVALUATION_MATERIALIZATION_PERMISSION_DENIED",
                    ErrorCategory::Authorization,
                    false,
                    "The Data Quality worker is not authorized to materialize evaluation outcomes.",
                )
                .with_internal_reference(format!(
                    "decision_id={} reason_code={} policy_version={}",
                    decision.decision_id, decision.reason_code, decision.policy_version
                )));
            }
            let evidence = self
                .load_current_finding_evidence(&request, job, &rule_set, &input)
                .await?;
            let planner = Arc::new(DataQualityEvaluationMaterializationPlanner::new(
                rule_set,
                profile,
                input,
                evidence.findings,
                evidence.observations,
            )?);
            PostgresTransactionalAggregateExecutor::new(self.store.clone(), planner)
                .execute(&definition, request)
                .await
                .map(|_| ())
        })
    }

    async fn load_current_finding_evidence(
        &self,
        request: &CapabilityRequest,
        job: &PartyEvaluationJob,
        rule_set: &PartyRuleSetVersion,
        input: &PartyEvaluationInputSnapshot,
    ) -> Result<CurrentFindingEvidence, SdkError> {
        let quality_input = PartyQualityInput::try_new(input.kind(), input.display_name())?;
        let evaluations = rule_set.evaluate(&quality_input);
        let outcomes = evaluations
            .iter()
            .map(|evaluation| PartyRuleOutcome::evaluate(job, evaluation, input.captured_at()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut findings = BTreeMap::new();
        let mut observations = BTreeMap::new();
        for outcome in &outcomes {
            let finding_id = party_finding_id(
                &request.context.execution.tenant_id,
                outcome.party_id(),
                outcome.rule_set_version_id(),
                outcome.rule_key(),
            );
            if let Some(snapshot) = self
                .load_optional_record(
                    &request.context.execution.tenant_id,
                    FINDING_RECORD_TYPE,
                    &finding_id,
                )
                .await?
            {
                let finding = decode_current_finding(&snapshot)?;
                findings.insert(
                    finding_id,
                    ExistingPartyFinding {
                        version: snapshot.version,
                        finding,
                    },
                );
            }
            if outcome.passed() {
                continue;
            }
            let rule = rule_set
                .rule(outcome.rule_key())
                .ok_or_else(|| materialization_state_invalid("outcome rule is unavailable"))?;
            let observation = PartyFindingObservation::observe_failure(
                request.context.execution.tenant_id.clone(),
                rule,
                outcome,
            )?;
            let observation_id = observation.observation_id().to_owned();
            if let Some(snapshot) = self
                .load_optional_record(
                    &request.context.execution.tenant_id,
                    FINDING_OBSERVATION_RECORD_TYPE,
                    &observation_id,
                )
                .await?
            {
                observations.insert(
                    observation_id,
                    ExistingPartyFindingObservation {
                        observation: decode_current_observation(&snapshot)?,
                    },
                );
            }
        }
        Ok(CurrentFindingEvidence {
            findings,
            observations,
        })
    }

    async fn load_optional_record(
        &self,
        tenant_id: &crm_module_sdk::TenantId,
        record_type_value: &str,
        record_id_value: &str,
    ) -> Result<Option<RecordSnapshot>, SdkError> {
        self.store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: tenant_id.clone(),
                owner_module_id: ModuleId::try_new(MODULE_ID).map_err(configuration_error)?,
                record_type: RecordType::try_new(record_type_value).map_err(configuration_error)?,
                record_id: RecordId::try_new(record_id_value).map_err(configuration_error)?,
            })
            .await
    }
}

impl std::fmt::Debug for PostgresPartyEvaluationMaterializationSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PostgresPartyEvaluationMaterializationSink")
            .field("store", &self.store)
            .field("authorizer", &"dyn CapabilityAuthorizer")
            .finish()
    }
}

fn decode_current_finding(
    snapshot: &RecordSnapshot,
) -> Result<crm_data_quality::PartyFinding, SdkError> {
    if snapshot.version <= 0 {
        return Err(materialization_state_invalid(
            "current finding record version is invalid",
        ));
    }
    let bytes = persisted_json_bytes_with_data_class(
        snapshot,
        party_finding_persisted_contract(),
        DataClass::Personal,
    )?;
    let finding = decode_finding_state(bytes)?;
    if finding.finding_id() != snapshot.reference.record_id.as_str() {
        return Err(materialization_state_invalid(
            "current finding identity differs from its record",
        ));
    }
    Ok(finding)
}

fn decode_current_observation(
    snapshot: &RecordSnapshot,
) -> Result<PartyFindingObservation, SdkError> {
    if snapshot.version != 1 {
        return Err(materialization_state_invalid(
            "finding observation is not immutable version one",
        ));
    }
    let bytes = persisted_json_bytes_with_data_class(
        snapshot,
        party_finding_observation_persisted_contract(),
        DataClass::Personal,
    )?;
    let observation = decode_finding_observation_state(bytes)?;
    if observation.observation_id() != snapshot.reference.record_id.as_str() {
        return Err(materialization_state_invalid(
            "finding observation identity differs from its record",
        ));
    }
    Ok(observation)
}

fn bind_materialization_request(
    base_context: &ModuleExecutionContext,
    definition: &CapabilityDefinition,
    input: TypedPayload,
    input_hash: [u8; 32],
) -> Result<CapabilityRequest, SdkError> {
    base_context.validate()?;
    let mut context = base_context.clone();
    context.module_id = definition.owner_module_id.clone();
    context.execution.capability_id = definition.capability_id.clone();
    context.execution.capability_version = definition.capability_version.clone();
    context.execution.schema_version =
        SchemaVersion::try_new(support::CONTRACT_VERSION).map_err(configuration_error)?;
    let identity = format!("dq-evaluation-materialize-{}", hex(&input_hash));
    context.execution.idempotency_key =
        IdempotencyKey::try_new(identity.clone()).map_err(configuration_error)?;
    context.execution.business_transaction_id =
        BusinessTransactionId::try_new(identity).map_err(configuration_error)?;
    Ok(CapabilityRequest {
        context,
        input,
        input_hash,
        approval: None,
    })
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn configuration_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_EVALUATION_MATERIALIZATION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Data Quality evaluation materialization is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

fn materialization_state_invalid(reference: &str) -> SdkError {
    SdkError::new(
        "DATA_QUALITY_EVALUATION_MATERIALIZATION_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The durable Party evaluation finding evidence is invalid.",
    )
    .with_internal_reference(reference)
}
