use crm_application_composition::ModuleActivationPort;
use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_customer_privacy::{
    ACTION_PLAN_RECORD_TYPE, DiscoveryScopeSnapshot, MODULE_ID,
    OWNER_OUTCOME_DEFAULT_PAGE_SIZE, OWNER_OUTCOME_MAXIMUM_CURSOR_BYTES,
    OWNER_OUTCOME_MAXIMUM_PAGE_SIZE, PRIVACY_CASE_RECORD_TYPE,
    PrivacyActionPlan, PrivacyCase, PrivacyCaseStatus, discovery_sha256,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, DataClass, ErrorCategory, ModuleId,
    PayloadEncoding, PortFuture, RecordId, RecordRef, RecordType, RequestId, SdkError, TenantId,
    TraceId, TypedPayload,
};
use crm_proto_contracts::crm::customer_privacy::v1 as wire;
use crm_query_runtime::{
    PageSizePolicy, QueryExecutionResult, QueryExecutor, QueryRequest, QuerySemanticValidator,
    QueryVisibilityAuthorizer,
};
use prost::Message;
use std::collections::BTreeSet;
use std::sync::Arc;

pub const GET_PRIVACY_ACTION_PLAN_CAPABILITY: &str = "customer_privacy.case.plan.get";
pub const GET_PRIVACY_ACTION_PLAN_REQUEST_SCHEMA: &str =
    "crm.customer_privacy.v1.GetPrivacyActionPlanRequest";
pub const GET_PRIVACY_ACTION_PLAN_RESPONSE_SCHEMA: &str =
    "crm.customer_privacy.v1.GetPrivacyActionPlanResponse";
pub const LIST_PRIVACY_OWNER_OUTCOMES_CAPABILITY: &str =
    "customer_privacy.case.owner_outcomes.list";
pub const LIST_PRIVACY_OWNER_OUTCOMES_REQUEST_SCHEMA: &str =
    "crm.customer_privacy.v1.ListPrivacyOwnerOutcomesRequest";
pub const LIST_PRIVACY_OWNER_OUTCOMES_RESPONSE_SCHEMA: &str =
    "crm.customer_privacy.v1.ListPrivacyOwnerOutcomesResponse";
pub const PLAN_READ_QUERY_CAPABILITY_IDS: &[&str] = &[
    GET_PRIVACY_ACTION_PLAN_CAPABILITY,
    LIST_PRIVACY_OWNER_OUTCOMES_CAPABILITY,
];

const PARTY_RECORD_TYPE: &str = "parties.party";
const CASE_REQUIRED_FIELDS: &[&str] = &[
    "kind",
    "status",
    "version",
    "policy_version",
    "subject_binding",
    "scope_snapshot_id",
    "privacy_action_plan_ref",
];
const PLAN_SUMMARY_FIELDS: &[&str] = &[
    "privacy_action_plan_ref",
    "privacy_case_ref",
    "status",
    "policy_version",
    "version",
    "finalized_at_unix_ms",
];
const OWNER_OUTCOMES_FIELD: &str = "owner_outcomes";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyPlanReplayLink {
    pub source_case_version: u64,
    pub resulting_case_version: u64,
    pub scope_snapshot_id: RecordId,
    pub plan_id: RecordId,
    pub plan_digest: [u8; 32],
    pub approval_required: bool,
    pub planned_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyPlanReadSource {
    pub privacy_case: PrivacyCase,
    pub scope_snapshot: DiscoveryScopeSnapshot,
    pub action_plan: PrivacyActionPlan,
    pub replay_link: PrivacyPlanReplayLink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyReadContext {
    pub tenant_id: TenantId,
    pub actor_id: ActorId,
    pub request_id: RequestId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub capability_id: CapabilityId,
    pub capability_version: CapabilityVersion,
    pub request_started_at_unix_nanos: i64,
}

impl PrivacyReadContext {
    fn from_request(request: &QueryRequest) -> Self {
        Self {
            tenant_id: request.context.tenant_id.clone(),
            actor_id: request.context.actor_id.clone(),
            request_id: request.context.request_id.clone(),
            correlation_id: request.context.correlation_id.clone(),
            trace_id: request.context.trace_id.clone(),
            capability_id: request.context.capability_id.clone(),
            capability_version: request.context.capability_version.clone(),
            request_started_at_unix_nanos: request.context.request_started_at_unix_nanos,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyReadAuditRecord {
    pub context: PrivacyReadContext,
    pub privacy_case_id: RecordId,
    pub plan_id: Option<RecordId>,
    pub plan_digest: Option<[u8; 32]>,
    pub owner_module_filter: Option<ModuleId>,
    pub page_size: Option<u32>,
    pub page_digest: Option<[u8; 32]>,
    pub terminal_digest: Option<[u8; 32]>,
    pub authorization_digest: [u8; 32],
    pub allowed: bool,
    pub result_code: &'static str,
}

pub trait PrivacyReadPersistencePort: Send + Sync {
    fn load_plan_source<'a>(
        &'a self,
        context: &'a PrivacyReadContext,
        privacy_case_id: &'a RecordId,
    ) -> PortFuture<'a, Result<Option<PrivacyPlanReadSource>, SdkError>>;

    fn append_read_audit<'a>(
        &'a self,
        record: &'a PrivacyReadAuditRecord,
    ) -> PortFuture<'a, Result<(), SdkError>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerPrivacyReadVisibilityResource {
    pub owner_module_id: &'static str,
    pub resource_type: &'static str,
    pub allowed_fields: BTreeSet<String>,
}

pub fn plan_read_visibility_resources(
    capability_id: &str,
) -> Vec<CustomerPrivacyReadVisibilityResource> {
    if !PLAN_READ_QUERY_CAPABILITY_IDS.contains(&capability_id) {
        return Vec::new();
    }
    let plan_fields = if capability_id == GET_PRIVACY_ACTION_PLAN_CAPABILITY {
        PLAN_SUMMARY_FIELDS
            .iter()
            .copied()
            .map(str::to_owned)
            .collect()
    } else {
        BTreeSet::from([OWNER_OUTCOMES_FIELD.to_owned()])
    };
    vec![
        CustomerPrivacyReadVisibilityResource {
            owner_module_id: MODULE_ID,
            resource_type: PARTY_RECORD_TYPE,
            allowed_fields: BTreeSet::new(),
        },
        CustomerPrivacyReadVisibilityResource {
            owner_module_id: MODULE_ID,
            resource_type: PRIVACY_CASE_RECORD_TYPE,
            allowed_fields: CASE_REQUIRED_FIELDS
                .iter()
                .copied()
                .map(str::to_owned)
                .collect(),
        },
        CustomerPrivacyReadVisibilityResource {
            owner_module_id: MODULE_ID,
            resource_type: ACTION_PLAN_RECORD_TYPE,
            allowed_fields: plan_fields,
        },
    ]
}

#[derive(Clone)]
pub struct CustomerPrivacyPlanReadAdapter {
    activation: Arc<dyn ModuleActivationPort>,
    persistence: Arc<dyn PrivacyReadPersistencePort>,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
}

impl std::fmt::Debug for CustomerPrivacyPlanReadAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CustomerPrivacyPlanReadAdapter")
            .field("activation", &"dyn ModuleActivationPort")
            .field("persistence", &"dyn PrivacyReadPersistencePort")
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .finish()
    }
}

impl CustomerPrivacyPlanReadAdapter {
    pub fn new(
        activation: Arc<dyn ModuleActivationPort>,
        persistence: Arc<dyn PrivacyReadPersistencePort>,
        visibility: Arc<dyn QueryVisibilityAuthorizer>,
    ) -> Self {
        Self {
            activation,
            persistence,
            visibility,
        }
    }

    async fn ensure_active(&self, tenant_id: &TenantId) -> Result<(), SdkError> {
        let module_id = ModuleId::try_new(MODULE_ID).map_err(configuration_error)?;
        if !self.activation.is_active(tenant_id, &module_id).await? {
            return Err(read_error(
                "CUSTOMER_PRIVACY_READ_DISABLED",
                ErrorCategory::Conflict,
                "Customer Privacy is disabled for the tenant",
            ));
        }
        Ok(())
    }

    async fn execute_plan_get(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::GetPrivacyActionPlanRequest =
            decode_input(request, GET_PRIVACY_ACTION_PLAN_REQUEST_SCHEMA)?;
        let privacy_case_id = privacy_case_id(command.privacy_case_ref)?;
        let source = self
            .authorized_source(
                request,
                &privacy_case_id,
                GET_PRIVACY_ACTION_PLAN_CAPABILITY,
            )
            .await?;
        let output = wire::PrivacyActionPlan {
            privacy_action_plan_ref: Some(wire::PrivacyActionPlanRef {
                privacy_action_plan_id: source.action_plan.plan_id().as_str().to_owned(),
            }),
            privacy_case_ref: Some(wire::PrivacyCaseRef {
                privacy_case_id: privacy_case_id.as_str().to_owned(),
            }),
            status: wire::PrivacyActionPlanStatus::Finalized as i32,
            policy_version: source
                .action_plan
                .lineage()
                .policy_version()
                .as_str()
                .to_owned(),
            version: 1,
            finalized_at_unix_ms: nanos_to_millis(source.action_plan.planned_at_unix_nanos())?,
        };
        self.persistence
            .append_read_audit(&PrivacyReadAuditRecord {
                context: PrivacyReadContext::from_request(request),
                privacy_case_id,
                plan_id: Some(source.action_plan.plan_id().clone()),
                plan_digest: Some(*source.action_plan.digest()),
                owner_module_filter: None,
                page_size: None,
                page_digest: None,
                terminal_digest: None,
                authorization_digest: authorization_digest(request, &source, None, true),
                allowed: true,
                result_code: "plan_read_allowed",
            })
            .await?;
        support::protobuf_payload(
            MODULE_ID,
            GET_PRIVACY_ACTION_PLAN_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &wire::GetPrivacyActionPlanResponse {
                privacy_action_plan: Some(output),
            },
        )
    }

    async fn execute_owner_outcomes(
        &self,
        request: &QueryRequest,
    ) -> Result<TypedPayload, SdkError> {
        let command: wire::ListPrivacyOwnerOutcomesRequest =
            decode_input(request, LIST_PRIVACY_OWNER_OUTCOMES_REQUEST_SCHEMA)?;
        let privacy_case_id = privacy_case_id(command.privacy_case_ref)?;
        let owner_module_filter = owner_module_filter(command.owner_module_id)?;
        let page_size = outcome_page_size(command.page_size)?;
        validate_terminal_cursor(&command.cursor)?;
        let source = self
            .authorized_source(
                request,
                &privacy_case_id,
                LIST_PRIVACY_OWNER_OUTCOMES_CAPABILITY,
            )
            .await?;
        let page_digest = owner_outcome_page_digest(
            &request.context.tenant_id,
            &privacy_case_id,
            source.action_plan.plan_id(),
            owner_module_filter.as_ref(),
            page_size,
        );
        let terminal_digest = owner_outcome_terminal_digest(&page_digest);
        self.persistence
            .append_read_audit(&PrivacyReadAuditRecord {
                context: PrivacyReadContext::from_request(request),
                privacy_case_id,
                plan_id: Some(source.action_plan.plan_id().clone()),
                plan_digest: Some(*source.action_plan.digest()),
                owner_module_filter,
                page_size: Some(page_size),
                page_digest: Some(page_digest),
                terminal_digest: Some(terminal_digest),
                authorization_digest: authorization_digest(
                    request,
                    &source,
                    Some(page_digest),
                    true,
                ),
                allowed: true,
                result_code: "owner_outcomes_empty_terminal_allowed",
            })
            .await?;
        support::protobuf_payload(
            MODULE_ID,
            LIST_PRIVACY_OWNER_OUTCOMES_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &wire::ListPrivacyOwnerOutcomesResponse {
                privacy_owner_outcomes: Vec::new(),
                next_cursor: String::new(),
            },
        )
    }

    async fn authorized_source(
        &self,
        request: &QueryRequest,
        privacy_case_id: &RecordId,
        capability_id: &'static str,
    ) -> Result<PrivacyPlanReadSource, SdkError> {
        let context = PrivacyReadContext::from_request(request);
        let case_ref = record_ref(PRIVACY_CASE_RECORD_TYPE, privacy_case_id)?;
        let case_decision = self
            .visibility
            .authorize_visibility(request, &case_ref)
            .await?;
        let case_allowed = case_decision.resource_visible
            && CASE_REQUIRED_FIELDS
                .iter()
                .all(|field| case_decision.allows_field(field));
        if !case_allowed {
            self.audit_concealed(
                &context,
                privacy_case_id,
                capability_id,
                "case_visibility_denied",
            )
            .await?;
            return Err(read_not_found());
        }

        let source = match self
            .persistence
            .load_plan_source(&context, privacy_case_id)
            .await?
        {
            Some(source) => source,
            None => {
                self.audit_concealed(&context, privacy_case_id, capability_id, "source_not_found")
                    .await?;
                return Err(read_not_found());
            }
        };
        validate_source(&context, privacy_case_id, &source)?;

        let binding = source.privacy_case.subject_binding().ok_or_else(|| {
            evidence_invalid("privacy case has no verified canonical subject binding")
        })?;
        let party_ref = record_ref(PARTY_RECORD_TYPE, &binding.canonical_party_id)?;
        let party_decision = self
            .visibility
            .authorize_visibility(request, &party_ref)
            .await?;
        if !party_decision.resource_visible {
            self.audit_concealed(
                &context,
                privacy_case_id,
                capability_id,
                "party_visibility_denied",
            )
            .await?;
            return Err(read_not_found());
        }

        let plan_ref = record_ref(ACTION_PLAN_RECORD_TYPE, source.action_plan.plan_id())?;
        let plan_decision = self
            .visibility
            .authorize_visibility(request, &plan_ref)
            .await?;
        let required_fields: &[&str] = if capability_id == GET_PRIVACY_ACTION_PLAN_CAPABILITY {
            PLAN_SUMMARY_FIELDS
        } else {
            &[OWNER_OUTCOMES_FIELD]
        };
        let plan_allowed = plan_decision.resource_visible
            && required_fields
                .iter()
                .all(|field| plan_decision.allows_field(field));
        if !plan_allowed {
            self.audit_concealed(
                &context,
                privacy_case_id,
                capability_id,
                "plan_visibility_denied",
            )
            .await?;
            return Err(read_not_found());
        }
        Ok(source)
    }

    async fn audit_concealed(
        &self,
        context: &PrivacyReadContext,
        privacy_case_id: &RecordId,
        capability_id: &'static str,
        result_code: &'static str,
    ) -> Result<(), SdkError> {
        let mut bytes = Vec::new();
        for value in [
            b"crm.customer-privacy.read-authorization/v1".as_slice(),
            context.tenant_id.as_str().as_bytes(),
            privacy_case_id.as_str().as_bytes(),
            capability_id.as_bytes(),
            result_code.as_bytes(),
        ] {
            append_digest_field(&mut bytes, value);
        }
        self.persistence
            .append_read_audit(&PrivacyReadAuditRecord {
                context: context.clone(),
                privacy_case_id: privacy_case_id.clone(),
                plan_id: None,
                plan_digest: None,
                owner_module_filter: None,
                page_size: None,
                page_digest: None,
                terminal_digest: None,
                authorization_digest: discovery_sha256(&bytes),
                allowed: false,
                result_code,
            })
            .await
    }
}

impl QuerySemanticValidator for CustomerPrivacyPlanReadAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            self.ensure_active(&request.context.tenant_id).await?;
            match definition.capability_id.as_str() {
                GET_PRIVACY_ACTION_PLAN_CAPABILITY => {
                    let command: wire::GetPrivacyActionPlanRequest =
                        decode_input(request, GET_PRIVACY_ACTION_PLAN_REQUEST_SCHEMA)?;
                    privacy_case_id(command.privacy_case_ref).map(|_| ())
                }
                LIST_PRIVACY_OWNER_OUTCOMES_CAPABILITY => {
                    let command: wire::ListPrivacyOwnerOutcomesRequest =
                        decode_input(request, LIST_PRIVACY_OWNER_OUTCOMES_REQUEST_SCHEMA)?;
                    privacy_case_id(command.privacy_case_ref)?;
                    owner_module_filter(command.owner_module_id)?;
                    outcome_page_size(command.page_size)?;
                    validate_terminal_cursor(&command.cursor)
                }
                _ => Err(unsupported_query()),
            }
        })
    }
}

impl QueryExecutor for CustomerPrivacyPlanReadAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            ensure_definition(definition)?;
            self.ensure_active(&request.context.tenant_id).await?;
            let output = match definition.capability_id.as_str() {
                GET_PRIVACY_ACTION_PLAN_CAPABILITY => self.execute_plan_get(&request).await?,
                LIST_PRIVACY_OWNER_OUTCOMES_CAPABILITY => {
                    self.execute_owner_outcomes(&request).await?
                }
                _ => return Err(unsupported_query()),
            };
            Ok(QueryExecutionResult { output })
        })
    }
}

pub fn plan_read_query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    Ok(vec![
        query_definition(
            GET_PRIVACY_ACTION_PLAN_CAPABILITY,
            GET_PRIVACY_ACTION_PLAN_REQUEST_SCHEMA,
            GET_PRIVACY_ACTION_PLAN_RESPONSE_SCHEMA,
        )?,
        query_definition(
            LIST_PRIVACY_OWNER_OUTCOMES_CAPABILITY,
            LIST_PRIVACY_OWNER_OUTCOMES_REQUEST_SCHEMA,
            LIST_PRIVACY_OWNER_OUTCOMES_RESPONSE_SCHEMA,
        )?,
    ])
}

fn query_definition(
    capability_id: &'static str,
    request_schema: &'static str,
    response_schema: &'static str,
) -> Result<CapabilityDefinition, SdkError> {
    Ok(CapabilityDefinition {
        capability_id: CapabilityId::try_new(capability_id).map_err(configuration_error)?,
        capability_version: CapabilityVersion::try_new(support::CONTRACT_VERSION)
            .map_err(configuration_error)?,
        owner_module_id: ModuleId::try_new(MODULE_ID).map_err(configuration_error)?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            request_schema,
            vec![DataClass::Confidential],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            response_schema,
            vec![DataClass::Confidential],
        )?),
        risk: CapabilityRisk::Low,
        mutation: false,
        requires_idempotency: false,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

fn validate_source(
    context: &PrivacyReadContext,
    privacy_case_id: &RecordId,
    source: &PrivacyPlanReadSource,
) -> Result<(), SdkError> {
    let case = &source.privacy_case;
    let snapshot = &source.scope_snapshot;
    let plan = &source.action_plan;
    let link = &source.replay_link;
    let binding = case
        .subject_binding()
        .ok_or_else(|| evidence_invalid("privacy case subject binding is missing"))?;
    let snapshot_lineage = snapshot.lineage();
    let plan_lineage = plan.lineage();
    let post_planning_status = matches!(
        case.status(),
        PrivacyCaseStatus::Planned
            | PrivacyCaseStatus::AwaitingApproval
            | PrivacyCaseStatus::Executing
            | PrivacyCaseStatus::Converging
            | PrivacyCaseStatus::FailedRetryable(_)
            | PrivacyCaseStatus::Completed
            | PrivacyCaseStatus::PartiallyCompleted
            | PrivacyCaseStatus::Denied
            | PrivacyCaseStatus::FailedTerminal
    );
    if !post_planning_status
        || case.tenant_id() != &context.tenant_id
        || case.case_id() != privacy_case_id
        || case.action_plan_id() != Some(plan.plan_id())
        || case.scope_snapshot_id() != Some(snapshot.snapshot_id())
        || case.version() < link.resulting_case_version
        || link.source_case_version.checked_add(1) != Some(link.resulting_case_version)
        || link.source_case_version != plan_lineage.source_case_version()
        || link.scope_snapshot_id != *snapshot.snapshot_id()
        || link.plan_id != *plan.plan_id()
        || link.plan_digest != *plan.digest()
        || link.approval_required != plan_lineage.approval_required()
        || link.planned_at_unix_nanos != plan.planned_at_unix_nanos()
        || plan_lineage.tenant_id() != &context.tenant_id
        || plan_lineage.privacy_case_id() != privacy_case_id
        || plan_lineage.scope_snapshot_id() != snapshot.snapshot_id()
        || plan_lineage.case_kind() != case.kind()
        || plan_lineage.policy_version() != case.policy_version()
        || plan_lineage.canonical_party_id() != &binding.canonical_party_id
        || plan_lineage.identity_resolution_generation() != binding.identity_resolution_generation
        || snapshot_lineage.tenant_id() != &context.tenant_id
        || snapshot_lineage.privacy_case_id() != privacy_case_id
        || snapshot_lineage.canonical_party_id() != &binding.canonical_party_id
        || snapshot_lineage.identity_resolution_generation()
            != binding.identity_resolution_generation
        || plan_lineage.scope_snapshot_binding_digest() != snapshot.binding_digest()
        || plan_lineage.scope_completeness_digest() != snapshot.aggregation().completeness_digest()
        || plan_lineage.registry_digest() != snapshot_lineage.registry_digest()
        || plan_lineage.purpose_code() != snapshot_lineage.purpose_code()
        || plan_lineage.effective_request_at_unix_ms()
            != snapshot_lineage.effective_request_at_unix_ms()
        || plan_lineage.snapshot_captured_at_unix_nanos() != snapshot.captured_at_unix_nanos()
    {
        return Err(evidence_invalid(
            "privacy case, immutable snapshot, action plan and replay link do not match",
        ));
    }
    Ok(())
}

pub fn owner_outcome_page_digest(
    tenant_id: &TenantId,
    privacy_case_id: &RecordId,
    plan_id: &RecordId,
    owner_module_filter: Option<&ModuleId>,
    page_size: u32,
) -> [u8; 32] {
    let page_size = page_size.to_string();
    let owner = owner_module_filter
        .map(|value| value.as_str())
        .unwrap_or("");
    let mut bytes = Vec::new();
    for value in [
        b"crm.customer-privacy.owner-outcomes-page/v1".as_slice(),
        tenant_id.as_str().as_bytes(),
        privacy_case_id.as_str().as_bytes(),
        plan_id.as_str().as_bytes(),
        owner.as_bytes(),
        page_size.as_bytes(),
        b"items=0".as_slice(),
        b"next_cursor=terminal".as_slice(),
    ] {
        append_digest_field(&mut bytes, value);
    }
    discovery_sha256(&bytes)
}

pub fn owner_outcome_terminal_digest(page_digest: &[u8; 32]) -> [u8; 32] {
    let mut bytes = Vec::new();
    append_digest_field(
        &mut bytes,
        b"crm.customer-privacy.owner-outcomes-terminal/v1",
    );
    append_digest_field(&mut bytes, page_digest);
    append_digest_field(&mut bytes, b"terminal");
    discovery_sha256(&bytes)
}

fn authorization_digest(
    request: &QueryRequest,
    source: &PrivacyPlanReadSource,
    page_digest: Option<[u8; 32]>,
    allowed: bool,
) -> [u8; 32] {
    let decision = if allowed {
        b"allow".as_slice()
    } else {
        b"deny".as_slice()
    };
    let mut bytes = Vec::new();
    for value in [
        b"crm.customer-privacy.read-authorization/v1".as_slice(),
        request.context.tenant_id.as_str().as_bytes(),
        request.context.actor_id.as_str().as_bytes(),
        request.context.capability_id.as_str().as_bytes(),
        source.privacy_case.case_id().as_str().as_bytes(),
        source.action_plan.plan_id().as_str().as_bytes(),
        decision,
    ] {
        append_digest_field(&mut bytes, value);
    }
    if let Some(page_digest) = page_digest {
        append_digest_field(&mut bytes, &page_digest);
    }
    discovery_sha256(&bytes)
}

fn append_digest_field(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value);
}

fn outcome_page_size(requested: i32) -> Result<u32, SdkError> {
    PageSizePolicy {
        default_size: OWNER_OUTCOME_DEFAULT_PAGE_SIZE,
        maximum_size: OWNER_OUTCOME_MAXIMUM_PAGE_SIZE,
    }
    .resolve(requested)
    .map_err(|error| {
        SdkError::new(
            error.code(),
            ErrorCategory::InvalidArgument,
            false,
            error.safe_message(),
        )
    })
}

fn validate_terminal_cursor(cursor: &str) -> Result<(), SdkError> {
    if cursor.len() > OWNER_OUTCOME_MAXIMUM_CURSOR_BYTES {
        return Err(SdkError::new(
            "QUERY_CURSOR_TOO_LARGE",
            ErrorCategory::InvalidArgument,
            false,
            "The page cursor is too large.",
        ));
    }
    if !cursor.is_empty() {
        return Err(SdkError::new(
            "QUERY_CURSOR_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The terminal owner-outcome page has no continuation cursor.",
        ));
    }
    Ok(())
}

fn owner_module_filter(value: Option<String>) -> Result<Option<ModuleId>, SdkError> {
    value
        .filter(|value| !value.is_empty())
        .map(|value| {
            ModuleId::try_new(value).map_err(|error| {
                SdkError::invalid_argument("customer_privacy.owner_module_id", error.to_string())
            })
        })
        .transpose()
}

fn decode_input<M>(request: &QueryRequest, schema: &'static str) -> Result<M, SdkError>
where
    M: Message + Default,
{
    let payload = &request.input;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != schema
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(schema)
        || payload.data_class != DataClass::Confidential
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes != support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            "CUSTOMER_PRIVACY_READ_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The Customer Privacy read input does not match the required contract.",
        ));
    }
    M::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "CUSTOMER_PRIVACY_READ_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Customer Privacy read input is not valid Protobuf.",
        )
    })
}

fn privacy_case_id(value: Option<wire::PrivacyCaseRef>) -> Result<RecordId, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_privacy.privacy_case_ref",
            "Privacy case reference is required.",
        )
    })?;
    RecordId::try_new(value.privacy_case_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_privacy.privacy_case_ref.privacy_case_id",
            error.to_string(),
        )
    })
}

fn record_ref(record_type: &str, record_id: &RecordId) -> Result<RecordRef, SdkError> {
    Ok(RecordRef {
        record_type: RecordType::try_new(record_type).map_err(configuration_error)?,
        record_id: record_id.clone(),
    })
}

fn ensure_definition(definition: &CapabilityDefinition) -> Result<(), SdkError> {
    if definition.owner_module_id.as_str() != MODULE_ID
        || !PLAN_READ_QUERY_CAPABILITY_IDS.contains(&definition.capability_id.as_str())
        || definition.capability_version.as_str() != support::CONTRACT_VERSION
        || definition.mutation
    {
        return Err(unsupported_query());
    }
    Ok(())
}

fn nanos_to_millis(value: i64) -> Result<i64, SdkError> {
    if value <= 0 || value % 1_000 != 0 {
        return Err(evidence_invalid(
            "action plan finalization time is not positive and microsecond aligned",
        ));
    }
    Ok(value / 1_000_000)
}

fn unsupported_query() -> SdkError {
    read_error(
        "CUSTOMER_PRIVACY_READ_UNSUPPORTED",
        ErrorCategory::InvalidArgument,
        "requested capability is not a supported Customer Privacy read",
    )
}

fn read_not_found() -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_CASE_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested privacy case was not found.",
    )
}

fn evidence_invalid(reference: impl Into<String>) -> SdkError {
    read_error(
        "CUSTOMER_PRIVACY_READ_EVIDENCE_INVALID",
        ErrorCategory::Internal,
        reference,
    )
}

fn configuration_error(reference: impl std::fmt::Display) -> SdkError {
    read_error(
        "CUSTOMER_PRIVACY_READ_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        reference.to_string(),
    )
}

fn read_error(
    code: &'static str,
    category: ErrorCategory,
    reference: impl Into<String>,
) -> SdkError {
    SdkError::new(
        code,
        category,
        false,
        "The Customer Privacy read failed closed.",
    )
    .with_internal_reference(reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_coordinates_and_visibility_are_exact() {
        assert_eq!(
            ACTION_PLAN_GET_COORDINATE,
            "customer_privacy.case.plan.get@1.0.0"
        );
        assert_eq!(
            OWNER_OUTCOMES_LIST_COORDINATE,
            "customer_privacy.case.owner_outcomes.list@1.0.0"
        );
        let definitions = plan_read_query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        assert_eq!(
            plan_read_visibility_resources(GET_PRIVACY_ACTION_PLAN_CAPABILITY).len(),
            3
        );
        assert_eq!(
            plan_read_visibility_resources(LIST_PRIVACY_OWNER_OUTCOMES_CAPABILITY).len(),
            3
        );
    }

    #[test]
    fn empty_terminal_page_digests_are_stable_and_filter_sensitive() {
        let tenant = TenantId::try_new("tenant-a").unwrap();
        let case = RecordId::try_new("case-a").unwrap();
        let plan = RecordId::try_new("plan-a").unwrap();
        let owner = ModuleId::try_new("crm.parties").unwrap();
        let first = owner_outcome_page_digest(&tenant, &case, &plan, None, 64);
        assert_eq!(
            first,
            owner_outcome_page_digest(&tenant, &case, &plan, None, 64)
        );
        assert_ne!(
            first,
            owner_outcome_page_digest(&tenant, &case, &plan, Some(&owner), 64)
        );
        assert_ne!(owner_outcome_terminal_digest(&first), first);
    }

    #[test]
    fn outcome_bounds_are_frozen_and_nonempty_cursor_is_terminally_invalid() {
        assert_eq!(outcome_page_size(0).unwrap(), 64);
        assert_eq!(outcome_page_size(128).unwrap(), 128);
        assert_eq!(
            outcome_page_size(129).unwrap_err().code,
            "QUERY_PAGE_SIZE_EXCEEDS_LIMIT"
        );
        assert!(validate_terminal_cursor("").is_ok());
        assert_eq!(
            validate_terminal_cursor("continuation").unwrap_err().code,
            "QUERY_CURSOR_INVALID"
        );
    }
}
