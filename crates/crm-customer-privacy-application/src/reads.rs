use crm_application_composition::ModuleActivationPort;
use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_customer_privacy::{
    ACTION_PLAN_RECORD_TYPE, DiscoveryScopeSnapshot, MODULE_ID, OWNER_ACTION_OUTCOME_RECORD_TYPE,
    OWNER_OUTCOME_DEFAULT_PAGE_SIZE, OWNER_OUTCOME_MAXIMUM_CURSOR_BYTES,
    OWNER_OUTCOME_MAXIMUM_PAGE_SIZE, PRIVACY_CASE_RECORD_TYPE, PrivacyActionPlan,
    PrivacyCase, PrivacyCaseStatus, PrivacyOwnerActionOutcome, PrivacyOwnerOutcomeStatus,
    discovery_sha256,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, DataClass, ErrorCategory, ModuleId,
    PayloadEncoding, PortFuture, RecordId, RecordRef, RecordType, RequestId, SdkError, TenantId,
    TraceId, TypedPayload,
};
use crm_proto_contracts::crm::customer_privacy::v1 as wire;
use crm_query_runtime::{
    CursorBinding, CursorCodec, CursorContinuation, PageSizePolicy, QueryExecutionResult,
    QueryExecutor, QueryRequest, QuerySemanticValidator, QueryVisibilityAuthorizer,
    normalized_filter_hash,
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


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyOwnerOutcomePosition {
    pub item_sequence: u32,
    pub attempt_generation: u32,
    pub outcome_id: RecordId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyOwnerOutcomePage {
    pub outcomes: Vec<PrivacyOwnerActionOutcome>,
    pub has_more: bool,
}

pub trait PrivacyReadPersistencePort: Send + Sync {
    fn load_plan_source<'a>(
        &'a self,
        context: &'a PrivacyReadContext,
        privacy_case_id: &'a RecordId,
    ) -> PortFuture<'a, Result<Option<PrivacyPlanReadSource>, SdkError>>;

    fn load_owner_outcomes<'a>(
        &'a self,
        context: &'a PrivacyReadContext,
        privacy_case_id: &'a RecordId,
        action_plan_id: &'a RecordId,
        owner_module_filter: Option<&'a ModuleId>,
        after: Option<&'a PrivacyOwnerOutcomePosition>,
        page_size: u32,
    ) -> PortFuture<'a, Result<PrivacyOwnerOutcomePage, SdkError>>;

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
    cursor: CursorCodec,
}

impl std::fmt::Debug for CustomerPrivacyPlanReadAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CustomerPrivacyPlanReadAdapter")
            .field("activation", &"dyn ModuleActivationPort")
            .field("persistence", &"dyn PrivacyReadPersistencePort")
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .field("cursor", &self.cursor)
            .finish()
    }
}

impl CustomerPrivacyPlanReadAdapter {
    pub fn new(
        activation: Arc<dyn ModuleActivationPort>,
        persistence: Arc<dyn PrivacyReadPersistencePort>,
        visibility: Arc<dyn QueryVisibilityAuthorizer>,
        cursor: CursorCodec,
    ) -> Self {
        Self {
            activation,
            persistence,
            visibility,
            cursor,
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
        validate_cursor_size(&command.cursor)?;
        let source = self
            .authorized_source(
                request,
                &privacy_case_id,
                LIST_PRIVACY_OWNER_OUTCOMES_CAPABILITY,
            )
            .await?;
        let binding = owner_outcome_cursor_binding(
            request,
            &privacy_case_id,
            source.action_plan.plan_id(),
            owner_module_filter.as_ref(),
            page_size,
        )?;
        let after = decode_outcome_cursor(&self.cursor, &binding, &command.cursor)?;
        let page = self
            .persistence
            .load_owner_outcomes(
                &PrivacyReadContext::from_request(request),
                &privacy_case_id,
                source.action_plan.plan_id(),
                owner_module_filter.as_ref(),
                after.as_ref(),
                page_size,
            )
            .await?;
        validate_outcome_page(
            request,
            &privacy_case_id,
            source.action_plan.plan_id(),
            owner_module_filter.as_ref(),
            after.as_ref(),
            page_size,
            &page,
        )?;
        let next_cursor = if page.has_more {
            let last = page
                .outcomes
                .last()
                .ok_or_else(|| evidence_invalid("non-terminal outcome page is empty"))?;
            encode_outcome_cursor(&self.cursor, &binding, last)?
        } else {
            String::new()
        };
        let page_digest = owner_outcome_page_digest_for_items(
            &PrivacyOwnerOutcomePageDigestContext {
                tenant_id: &request.context.tenant_id,
                privacy_case_id: &privacy_case_id,
                action_plan_id: source.action_plan.plan_id(),
                owner_module_filter: owner_module_filter.as_ref(),
                page_size,
            },
            after.as_ref(),
            &page.outcomes,
            &next_cursor,
        );
        let terminal_digest = (!page.has_more)
            .then(|| owner_outcome_terminal_digest(&page_digest));
        let output_items = page
            .outcomes
            .iter()
            .map(owner_outcome_to_wire)
            .collect::<Result<Vec<_>, _>>()?;
        self.persistence
            .append_read_audit(&PrivacyReadAuditRecord {
                context: PrivacyReadContext::from_request(request),
                privacy_case_id,
                plan_id: Some(source.action_plan.plan_id().clone()),
                plan_digest: Some(*source.action_plan.digest()),
                owner_module_filter,
                page_size: Some(page_size),
                page_digest: Some(page_digest),
                terminal_digest,
                authorization_digest: authorization_digest(
                    request,
                    &source,
                    Some(page_digest),
                    true,
                ),
                allowed: true,
                result_code: if output_items.is_empty() {
                    "owner_outcomes_empty_terminal_allowed"
                } else {
                    "owner_outcomes_page_allowed"
                },
            })
            .await?;
        support::protobuf_payload(
            MODULE_ID,
            LIST_PRIVACY_OWNER_OUTCOMES_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &wire::ListPrivacyOwnerOutcomesResponse {
                privacy_owner_outcomes: output_items,
                next_cursor,
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
                    validate_cursor_size(&command.cursor)
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

#[derive(Debug, Clone, Copy)]
pub struct PrivacyOwnerOutcomePageDigestContext<'a> {
    pub tenant_id: &'a TenantId,
    pub privacy_case_id: &'a RecordId,
    pub action_plan_id: &'a RecordId,
    pub owner_module_filter: Option<&'a ModuleId>,
    pub page_size: u32,
}

pub fn owner_outcome_page_digest(
    tenant_id: &TenantId,
    privacy_case_id: &RecordId,
    plan_id: &RecordId,
    owner_module_filter: Option<&ModuleId>,
    page_size: u32,
) -> [u8; 32] {
    owner_outcome_page_digest_for_items(
        &PrivacyOwnerOutcomePageDigestContext {
            tenant_id,
            privacy_case_id,
            action_plan_id: plan_id,
            owner_module_filter,
            page_size,
        },
        None,
        &[],
        "",
    )
}

pub fn owner_outcome_page_digest_for_items(
    context: &PrivacyOwnerOutcomePageDigestContext<'_>,
    after: Option<&PrivacyOwnerOutcomePosition>,
    outcomes: &[PrivacyOwnerActionOutcome],
    next_cursor: &str,
) -> [u8; 32] {
    let page_size = context.page_size.to_string();
    let owner = context
        .owner_module_filter
        .map(ModuleId::as_str)
        .unwrap_or("");
    let item_count = outcomes.len().to_string();
    let mut bytes = Vec::new();
    for value in [
        b"crm.customer-privacy.owner-outcomes-page/v2".as_slice(),
        context.tenant_id.as_str().as_bytes(),
        context.privacy_case_id.as_str().as_bytes(),
        context.action_plan_id.as_str().as_bytes(),
        owner.as_bytes(),
        page_size.as_bytes(),
        item_count.as_bytes(),
    ] {
        append_digest_field(&mut bytes, value);
    }
    if let Some(after) = after {
        append_digest_field(&mut bytes, &after.item_sequence.to_be_bytes());
        append_digest_field(&mut bytes, &after.attempt_generation.to_be_bytes());
        append_digest_field(&mut bytes, after.outcome_id.as_str().as_bytes());
    } else {
        append_digest_field(&mut bytes, b"initial");
    }
    for outcome in outcomes {
        append_digest_field(&mut bytes, outcome.digest());
    }
    append_digest_field(&mut bytes, next_cursor.as_bytes());
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

fn owner_outcome_cursor_binding(
    request: &QueryRequest,
    privacy_case_id: &RecordId,
    plan_id: &RecordId,
    owner_module_filter: Option<&ModuleId>,
    page_size: u32,
) -> Result<CursorBinding, SdkError> {
    let owner = owner_module_filter.map(ModuleId::as_str).unwrap_or("");
    Ok(CursorBinding {
        tenant_id: request.context.tenant_id.clone(),
        actor_id: Some(request.context.actor_id.clone()),
        capability_id: request.context.capability_id.clone(),
        capability_version: request.context.capability_version.clone(),
        resource_type: RecordType::try_new(OWNER_ACTION_OUTCOME_RECORD_TYPE)
            .map_err(configuration_error)?,
        normalized_filter_hash: normalized_filter_hash([
            ("privacy_case_id", privacy_case_id.as_str().as_bytes()),
            ("action_plan_id", plan_id.as_str().as_bytes()),
            ("owner_module_id", owner.as_bytes()),
        ]),
        sort_id: "customer_privacy.owner_outcome.sequence_generation.v1".to_owned(),
        page_size,
    })
}

fn decode_outcome_cursor(
    codec: &CursorCodec,
    binding: &CursorBinding,
    token: &str,
) -> Result<Option<PrivacyOwnerOutcomePosition>, SdkError> {
    if token.is_empty() {
        return Ok(None);
    }
    let continuation = codec.decode(token, binding).map_err(cursor_error)?;
    if continuation.sort_key.len() != 8 {
        return Err(cursor_error("owner-outcome cursor sort key is invalid"));
    }
    let sequence = u32::from_be_bytes(
        continuation.sort_key[..4]
            .try_into()
            .map_err(|_| cursor_error("owner-outcome cursor sequence is invalid"))?,
    );
    let generation = u32::from_be_bytes(
        continuation.sort_key[4..]
            .try_into()
            .map_err(|_| cursor_error("owner-outcome cursor generation is invalid"))?,
    );
    if sequence == 0 || generation > 100 {
        return Err(cursor_error("owner-outcome cursor position is out of bounds"));
    }
    Ok(Some(PrivacyOwnerOutcomePosition {
        item_sequence: sequence,
        attempt_generation: generation,
        outcome_id: continuation.record_id,
    }))
}

fn encode_outcome_cursor(
    codec: &CursorCodec,
    binding: &CursorBinding,
    outcome: &PrivacyOwnerActionOutcome,
) -> Result<String, SdkError> {
    let mut sort_key = Vec::with_capacity(8);
    sort_key.extend_from_slice(&outcome.item_sequence().to_be_bytes());
    sort_key.extend_from_slice(&outcome.attempt_generation().to_be_bytes());
    codec
        .encode(
            binding,
            &CursorContinuation {
                sort_key,
                record_id: outcome.outcome_id().clone(),
            },
        )
        .map_err(cursor_error)
}

#[allow(clippy::too_many_arguments)]
fn validate_outcome_page(
    request: &QueryRequest,
    privacy_case_id: &RecordId,
    plan_id: &RecordId,
    owner_module_filter: Option<&ModuleId>,
    after: Option<&PrivacyOwnerOutcomePosition>,
    page_size: u32,
    page: &PrivacyOwnerOutcomePage,
) -> Result<(), SdkError> {
    if page.outcomes.len() > usize::try_from(page_size).unwrap_or(usize::MAX)
        || (page.has_more && page.outcomes.is_empty())
    {
        return Err(evidence_invalid("owner-outcome page exceeds its governed bounds"));
    }
    let mut previous = after.map(|value| {
        (
            value.item_sequence,
            value.attempt_generation,
            value.outcome_id.as_str().to_owned(),
        )
    });
    for outcome in &page.outcomes {
        if outcome.tenant_id() != &request.context.tenant_id
            || outcome.privacy_case_id() != privacy_case_id
            || outcome.action_plan_id() != plan_id
            || owner_module_filter
                .is_some_and(|owner| outcome.owner_module_id() != owner)
        {
            return Err(evidence_invalid("owner-outcome page lineage is invalid"));
        }
        let current = (
            outcome.item_sequence(),
            outcome.attempt_generation(),
            outcome.outcome_id().as_str().to_owned(),
        );
        if previous.as_ref().is_some_and(|value| value >= &current) {
            return Err(evidence_invalid("owner-outcome page is not in canonical order"));
        }
        previous = Some(current);
    }
    Ok(())
}

fn owner_outcome_to_wire(
    outcome: &PrivacyOwnerActionOutcome,
) -> Result<wire::PrivacyOwnerOutcome, SdkError> {
    let status = match outcome.status() {
        PrivacyOwnerOutcomeStatus::Succeeded => wire::PrivacyOwnerOutcomeStatus::Succeeded,
        PrivacyOwnerOutcomeStatus::Retained => wire::PrivacyOwnerOutcomeStatus::Retained,
        PrivacyOwnerOutcomeStatus::BlockedByHold => {
            wire::PrivacyOwnerOutcomeStatus::BlockedByHold
        }
        PrivacyOwnerOutcomeStatus::BlockedByRetention => {
            wire::PrivacyOwnerOutcomeStatus::BlockedByRetention
        }
        PrivacyOwnerOutcomeStatus::FailedRetryable => {
            wire::PrivacyOwnerOutcomeStatus::FailedRetryable
        }
        PrivacyOwnerOutcomeStatus::FailedTerminal => {
            wire::PrivacyOwnerOutcomeStatus::FailedTerminal
        }
    };
    Ok(wire::PrivacyOwnerOutcome {
        privacy_owner_outcome_ref: Some(wire::PrivacyOwnerOutcomeRef {
            privacy_owner_outcome_id: outcome.outcome_id().as_str().to_owned(),
        }),
        privacy_action_plan_ref: Some(wire::PrivacyActionPlanRef {
            privacy_action_plan_id: outcome.action_plan_id().as_str().to_owned(),
        }),
        owner_module_id: outcome.owner_module_id().as_str().to_owned(),
        action_code: outcome.action_code().to_owned(),
        status: status as i32,
        safe_failure_code: outcome.safe_failure_code().map(str::to_owned),
        recorded_at_unix_ms: nanos_to_millis(outcome.recorded_at_unix_nanos())?,
    })
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

fn validate_cursor_size(cursor: &str) -> Result<(), SdkError> {
    if cursor.len() > OWNER_OUTCOME_MAXIMUM_CURSOR_BYTES {
        return Err(SdkError::new(
            "QUERY_CURSOR_TOO_LARGE",
            ErrorCategory::InvalidArgument,
            false,
            "The page cursor is too large.",
        ));
    }
    Ok(())
}

fn cursor_error(reference: impl std::fmt::Display) -> SdkError {
    read_error(
        "QUERY_CURSOR_INVALID",
        ErrorCategory::InvalidArgument,
        reference.to_string(),
    )
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
    fn outcome_bounds_and_cursor_size_are_frozen() {
        assert_eq!(outcome_page_size(0).unwrap(), 64);
        assert_eq!(outcome_page_size(128).unwrap(), 128);
        assert_eq!(
            outcome_page_size(129).unwrap_err().code,
            "QUERY_PAGE_SIZE_EXCEEDS_LIMIT"
        );
        assert_eq!(
            outcome_page_size(-1).unwrap_err().code,
            "QUERY_PAGE_SIZE_INVALID"
        );
        assert!(validate_cursor_size("").is_ok());
        assert!(validate_cursor_size("continuation").is_ok());
        assert_eq!(
            validate_cursor_size(&"x".repeat(OWNER_OUTCOME_MAXIMUM_CURSOR_BYTES + 1))
                .unwrap_err()
                .code,
            "QUERY_CURSOR_TOO_LARGE"
        );
    }

    fn outcome_binding(
        tenant_id: &str,
        privacy_case_id: &str,
        action_plan_id: &str,
        owner_module_id: Option<&str>,
        page_size: u32,
    ) -> CursorBinding {
        CursorBinding {
            tenant_id: TenantId::try_new(tenant_id).unwrap(),
            actor_id: Some(ActorId::try_new("actor-a").unwrap()),
            capability_id: CapabilityId::try_new(LIST_PRIVACY_OWNER_OUTCOMES_CAPABILITY).unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            resource_type: RecordType::try_new(OWNER_ACTION_OUTCOME_RECORD_TYPE).unwrap(),
            normalized_filter_hash: normalized_filter_hash([
                ("privacy_case_id", privacy_case_id.as_bytes()),
                ("action_plan_id", action_plan_id.as_bytes()),
                ("owner_module_id", owner_module_id.unwrap_or("").as_bytes()),
            ]),
            sort_id: "customer_privacy.owner_outcome.sequence_generation.v1".to_owned(),
            page_size,
        }
    }

    fn outcome_token(
        codec: &CursorCodec,
        binding: &CursorBinding,
        sequence: u32,
        generation: u32,
    ) -> String {
        let mut sort_key = Vec::with_capacity(8);
        sort_key.extend_from_slice(&sequence.to_be_bytes());
        sort_key.extend_from_slice(&generation.to_be_bytes());
        codec
            .encode(
                binding,
                &CursorContinuation {
                    sort_key,
                    record_id: RecordId::try_new("privacy-owner-outcome-a").unwrap(),
                },
            )
            .unwrap()
    }

    #[test]
    fn owner_outcome_cursor_round_trip_is_keyset_exact() {
        let codec = CursorCodec::new([0x42; 32]).unwrap();
        let binding = outcome_binding(
            "tenant-a",
            "privacy-case-a",
            "action-plan-a",
            Some("crm.parties"),
            64,
        );
        let token = outcome_token(&codec, &binding, 7, 2);
        let decoded = decode_outcome_cursor(&codec, &binding, &token)
            .unwrap()
            .unwrap();
        assert_eq!(decoded.item_sequence, 7);
        assert_eq!(decoded.attempt_generation, 2);
        assert_eq!(decoded.outcome_id.as_str(), "privacy-owner-outcome-a");
    }

    #[test]
    fn outcome_cursor_is_bound_to_tenant_case_plan_filter_actor_and_page_size() {
        let codec = CursorCodec::new([0x42; 32]).unwrap();
        let binding = outcome_binding(
            "tenant-a",
            "privacy-case-a",
            "action-plan-a",
            Some("crm.parties"),
            64,
        );
        let token = outcome_token(&codec, &binding, 1, 0);
        for wrong in [
            outcome_binding(
                "tenant-b",
                "privacy-case-a",
                "action-plan-a",
                Some("crm.parties"),
                64,
            ),
            outcome_binding(
                "tenant-a",
                "privacy-case-b",
                "action-plan-a",
                Some("crm.parties"),
                64,
            ),
            outcome_binding(
                "tenant-a",
                "privacy-case-a",
                "action-plan-b",
                Some("crm.parties"),
                64,
            ),
            outcome_binding(
                "tenant-a",
                "privacy-case-a",
                "action-plan-a",
                Some("crm.consents"),
                64,
            ),
            outcome_binding(
                "tenant-a",
                "privacy-case-a",
                "action-plan-a",
                Some("crm.parties"),
                128,
            ),
        ] {
            assert_eq!(
                decode_outcome_cursor(&codec, &wrong, &token)
                    .unwrap_err()
                    .code,
                "QUERY_CURSOR_INVALID"
            );
        }

        let mut wrong_actor = binding.clone();
        wrong_actor.actor_id = Some(ActorId::try_new("actor-b").unwrap());
        assert_eq!(
            decode_outcome_cursor(&codec, &wrong_actor, &token)
                .unwrap_err()
                .code,
            "QUERY_CURSOR_INVALID"
        );
    }

    #[test]
    fn malformed_tampered_and_out_of_bounds_outcome_cursors_fail_closed() {
        let codec = CursorCodec::new([0x42; 32]).unwrap();
        let binding = outcome_binding("tenant-a", "privacy-case-a", "action-plan-a", None, 64);
        assert_eq!(
            decode_outcome_cursor(&codec, &binding, "not-a-signed-cursor")
                .unwrap_err()
                .code,
            "QUERY_CURSOR_INVALID"
        );

        let token = outcome_token(&codec, &binding, 1, 0);
        let mut tampered = token.into_bytes();
        let index = tampered.len() / 2;
        tampered[index] = if tampered[index] == b'A' { b'B' } else { b'A' };
        let tampered = String::from_utf8(tampered).unwrap();
        assert_eq!(
            decode_outcome_cursor(&codec, &binding, &tampered)
                .unwrap_err()
                .code,
            "QUERY_CURSOR_INVALID"
        );

        for (sequence, generation) in [(0, 0), (1, 101)] {
            let token = outcome_token(&codec, &binding, sequence, generation);
            assert_eq!(
                decode_outcome_cursor(&codec, &binding, &token)
                    .unwrap_err()
                    .code,
                "QUERY_CURSOR_INVALID"
            );
        }

        let invalid_sort = codec
            .encode(
                &binding,
                &CursorContinuation {
                    sort_key: vec![0; 7],
                    record_id: RecordId::try_new("privacy-owner-outcome-a").unwrap(),
                },
            )
            .unwrap();
        assert_eq!(
            decode_outcome_cursor(&codec, &binding, &invalid_sort)
                .unwrap_err()
                .code,
            "QUERY_CURSOR_INVALID"
        );
    }
}
