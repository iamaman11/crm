//! One-Worker in-process original Sales -> Activities link.
//! Original SalesActivitiesLink::handle owns delivery/semantic decisions.
//! Original ProtobufSalesActivitiesLinkContractAdapter owns wire contracts.
//! Original crm-activities::Task::create owns Task validity.
//! Ephemeral SDK ports are only a transactional staging area; their complete
//! result is committed together with the Sales change in ONE D1 batch.
use crm_activities::{CreateTask, Task, TaskOwner, TaskPriority};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityClient, CapabilityId, CapabilityInvocation,
    CapabilityOutcome, CapabilityVersion, CausationId, CorrelationId, DataClass, DeliveryId,
    ErrorCategory, EventDelivery, EventId, EventType, EventVersion, ExecutionContext,
    IdempotencyKey, ModuleExecutionContext, ModuleId, ModuleStateEntry, ModuleStateStore,
    PayloadEncoding, PortFuture, PortResult, PutModuleStateRequest, RecordId, RecordRef,
    RecordType, RequestId, ResourceRef, RetentionPolicyId, SchemaId, SchemaVersion, SdkError,
    StateKey, TenantId, TraceId, TypedPayload,
};
use crm_proto_contracts::{
    MAX_PROTOBUF_BYTES,
    crm::{activities::v1 as activities, core::v1 as core, sales::v1 as sales},
    message_descriptor_hash,
};
use crm_sales::{Deal, DealStatus};
use crm_sales_activities_link::{
    MODULE_ID, SOURCE_EVENT_TYPE, SOURCE_EVENT_VERSION, SOURCE_MODULE_ID, SOURCE_RECORD_TYPE,
    SalesActivitiesLink, TARGET_CAPABILITY_ID, TARGET_CAPABILITY_VERSION, TARGET_MODULE_ID,
    TARGET_REQUEST_SCHEMA_ID,
};
use crm_sales_activities_link_contract_adapter::{
    ProtobufSalesActivitiesLinkContractAdapter, SOURCE_EVENT_SCHEMA_ID,
};
use prost::Message;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Mutex;

fn invalid(code: &str) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::InvalidArgument,
        false,
        "Invalid typed link boundary.",
    )
}
fn internal(code: &str) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::Internal,
        false,
        "Internal linked transaction failed.",
    )
}
// SDK identifiers return IdentifierError, while link ports return SdkError.
fn ident<T, E>(v: Result<T, E>) -> Result<T, SdkError> {
    v.map_err(|_| invalid("LINK_IDENTIFIER_INVALID"))
}
fn id_hash(tenant: &str, deal: &str, version: i64) -> String {
    let mut hash = Sha256::new();
    hash.update(tenant.as_bytes());
    hash.update(b"\0");
    hash.update(deal.as_bytes());
    hash.update(b"\0");
    hash.update(version.to_be_bytes());
    let digest = hash.finalize();
    let full = format!("{digest:x}");
    full[..40].to_owned()
}
fn deal_stage(stage: &crm_sales::DealStage) -> sales::DealStage {
    sales::DealStage {
        pipeline_id: stage.pipeline_id().as_str().to_owned(),
        stage_id: stage.stage_id().as_str().to_owned(),
        ordinal: stage.ordinal() as u32,
    }
}
fn status(value: DealStatus) -> sales::DealStatus {
    match value {
        DealStatus::Open => sales::DealStatus::Open,
        DealStatus::Won => sales::DealStatus::Won,
        DealStatus::Lost => sales::DealStatus::Lost,
    }
}
fn make_delivery(
    before: &Deal,
    after: &Deal,
    tenant: &str,
    actor: &str,
    trace: &str,
    now: i64,
) -> Result<(EventDelivery, ModuleExecutionContext), SdkError> {
    if before.deal_id() != after.deal_id() || before.version() + 1 != after.version() {
        return Err(invalid("LINK_SALES_SOURCE_VERSION_INVALID"));
    }
    let marker = id_hash(tenant, after.deal_id().as_str(), after.version());
    let delivery_id = format!("delivery-{marker}");
    let event_id = format!("event-{marker}");
    let closed = after.close_outcome().map(|x| sales::DealCloseOutcome {
        status: status(x.status()) as i32,
        reason_code: x.reason_code().as_str().to_owned(),
        closed_at: Some(core::UnixTime {
            unix_nanos: x.closed_at_unix_nanos(),
        }),
    });
    let message = sales::DealStageChangedEvent {
        deal_id: after.deal_id().as_str().to_owned(),
        previous_stage: Some(deal_stage(before.stage())),
        current_stage: Some(deal_stage(after.stage())),
        status: status(after.status()) as i32,
        close_outcome: closed,
        version: after.version(),
    };
    let delivery = EventDelivery {
        delivery_id: ident(DeliveryId::try_new(&delivery_id))?,
        event_id: ident(EventId::try_new(&event_id))?,
        tenant_id: ident(TenantId::try_new(tenant))?,
        source_module_id: ident(ModuleId::try_new(SOURCE_MODULE_ID))?,
        consumer_module_id: ident(ModuleId::try_new(MODULE_ID))?,
        source_actor_id: ident(ActorId::try_new(actor))?,
        event_type: ident(EventType::try_new(SOURCE_EVENT_TYPE))?,
        event_version: ident(EventVersion::try_new(SOURCE_EVENT_VERSION))?,
        aggregate: RecordRef {
            record_type: ident(RecordType::try_new(SOURCE_RECORD_TYPE))?,
            record_id: ident(RecordId::try_new(after.deal_id().as_str()))?,
        },
        aggregate_version: after.version(),
        occurred_at_unix_nanos: now,
        correlation_id: ident(CorrelationId::try_new(trace))?,
        trace_id: ident(TraceId::try_new(trace))?,
        payload: TypedPayload {
            owner: ident(ModuleId::try_new(SOURCE_MODULE_ID))?,
            schema_id: ident(SchemaId::try_new(SOURCE_EVENT_SCHEMA_ID))?,
            schema_version: ident(SchemaVersion::try_new(SOURCE_EVENT_VERSION))?,
            descriptor_hash: message_descriptor_hash(SOURCE_EVENT_SCHEMA_ID),
            data_class: DataClass::Confidential,
            encoding: PayloadEncoding::Protobuf,
            maximum_size_bytes: MAX_PROTOBUF_BYTES,
            retention_policy_id: ident(RetentionPolicyId::try_new("standard"))?,
            bytes: message.encode_to_vec(),
        },
    };
    let context = ModuleExecutionContext {
        module_id: ident(ModuleId::try_new(MODULE_ID))?,
        execution: ExecutionContext {
            tenant_id: delivery.tenant_id.clone(),
            actor_id: ident(ActorId::try_new(actor))?,
            request_id: ident(RequestId::try_new(trace))?,
            correlation_id: delivery.correlation_id.clone(),
            causation_id: ident(CausationId::try_new(&event_id))?,
            trace_id: delivery.trace_id.clone(),
            capability_id: ident(CapabilityId::try_new("link.sales.stage_changed.process"))?,
            capability_version: ident(CapabilityVersion::try_new("1.0.0"))?,
            idempotency_key: ident(IdempotencyKey::try_new(&delivery_id))?,
            business_transaction_id: ident(BusinessTransactionId::try_new(format!(
                "txn-{marker}"
            )))?,
            schema_version: ident(SchemaVersion::try_new("1.0.0"))?,
            request_started_at_unix_nanos: now,
        },
    };
    Ok((delivery, context))
}

/// A deliberately non-durable staging store. This is NOT a second store:
/// only its final typed entry is committed by D1 along with Deal and Task.
#[derive(Default)]
struct StagedState(Mutex<BTreeMap<String, ModuleStateEntry>>);
impl StagedState {
    fn final_entry(&self, key: &str) -> Result<ModuleStateEntry, SdkError> {
        self.0
            .lock()
            .map_err(|_| internal("LINK_STATE_POISONED"))?
            .get(key)
            .cloned()
            .ok_or_else(|| internal("LINK_STATE_MISSING"))
    }
}
impl ModuleStateStore for StagedState {
    fn get<'a>(
        &'a self,
        _: &'a ModuleExecutionContext,
        key: StateKey,
    ) -> PortFuture<'a, PortResult<Option<ModuleStateEntry>>> {
        Box::pin(async move {
            Ok(self
                .0
                .lock()
                .map_err(|_| internal("LINK_STATE_POISONED"))?
                .get(key.as_str())
                .cloned())
        })
    }
    fn put<'a>(
        &'a self,
        _: &'a ModuleExecutionContext,
        req: PutModuleStateRequest,
    ) -> PortFuture<'a, PortResult<ModuleStateEntry>> {
        Box::pin(async move {
            let mut states = self.0.lock().map_err(|_| internal("LINK_STATE_POISONED"))?;
            let old = states.get(req.key.as_str());
            if old.map(|x| x.version) != req.expected_version {
                return Err(SdkError::new(
                    "SDK_VERSION_CONFLICT",
                    ErrorCategory::Conflict,
                    true,
                    "Delivery version changed.",
                ));
            }
            let entry = ModuleStateEntry {
                key: req.key.clone(),
                version: old.map_or(1, |x| x.version + 1),
                value: req.value,
            };
            states.insert(req.key.as_str().to_owned(), entry.clone());
            Ok(entry)
        })
    }
    fn delete<'a>(
        &'a self,
        _: &'a ModuleExecutionContext,
        _: StateKey,
        _: Option<i64>,
    ) -> PortFuture<'a, PortResult<()>> {
        Box::pin(async { Err(internal("LINK_STATE_DELETE_UNSUPPORTED")) })
    }
}
struct StagedActivities {
    task: Mutex<Option<Task>>,
    now: i64,
}
impl StagedActivities {
    fn new(now: i64) -> Self {
        Self {
            task: Mutex::new(None),
            now,
        }
    }
    fn take_task(&self) -> Result<Option<Task>, SdkError> {
        Ok(self
            .task
            .lock()
            .map_err(|_| internal("LINK_TASK_LOCK"))?
            .clone())
    }
}
impl CapabilityClient for StagedActivities {
    fn invoke<'a>(
        &'a self,
        ctx: &'a ModuleExecutionContext,
        invocation: CapabilityInvocation,
    ) -> PortFuture<'a, PortResult<CapabilityOutcome>> {
        Box::pin(async move {
            if invocation.capability_id.as_str() != TARGET_CAPABILITY_ID
                || invocation.capability_version.as_str() != TARGET_CAPABILITY_VERSION
                || invocation.input.owner.as_str() != TARGET_MODULE_ID
                || invocation.input.schema_id.as_str() != TARGET_REQUEST_SCHEMA_ID
                || invocation.input.encoding != PayloadEncoding::Protobuf
                || invocation.input.descriptor_hash
                    != message_descriptor_hash(TARGET_REQUEST_SCHEMA_ID)
            {
                return Err(invalid("LINK_ACTIVITIES_CONTRACT_INVALID"));
            }
            let request = activities::CreateTaskRequest::decode(invocation.input.bytes.as_slice())
                .map_err(|_| invalid("LINK_ACTIVITIES_PROTOBUF_INVALID"))?;
            let actor = match request.owner.as_ref().and_then(|x| x.owner.as_ref()) {
                Some(core::actor_or_team_owner::Owner::ActorId(id))
                    if id == ctx.execution.actor_id.as_str() =>
                {
                    ident(ActorId::try_new(id.clone()))?
                }
                _ => return Err(invalid("LINK_ACTIVITIES_ACTOR_MISMATCH")),
            };
            if request.related_resources.len() != 1
                || request.related_resources[0].tenant_id != ctx.execution.tenant_id.as_str()
                || request.related_resources[0].resource_type != SOURCE_RECORD_TYPE
                || request.priority != activities::TaskPriority::Normal as i32
                || request.description.is_some()
                || request.due_at.is_some()
                || request.reminder_at.is_some()
            {
                return Err(invalid("LINK_ACTIVITIES_SCOPE_INVALID"));
            }
            let related = &request.related_resources[0];
            let task = Task::create(CreateTask {
                task_id: ident(RecordId::try_new(request.task_id))?,
                subject: request.subject,
                description: request.description,
                owner: TaskOwner::Actor(actor),
                related_resources: vec![ResourceRef {
                    resource_type: related.resource_type.clone(),
                    resource_id: related.resource_id.clone(),
                    version: related.version,
                }],
                priority: TaskPriority::Normal,
                due_at_unix_nanos: None,
                reminder_at_unix_nanos: None,
                occurred_at_unix_nanos: self.now,
            })?;
            let resource = ResourceRef {
                resource_type: "activities.task".to_owned(),
                resource_id: task.task_id().as_str().to_owned(),
                version: Some(task.version()),
            };
            let mut slot = self.task.lock().map_err(|_| internal("LINK_TASK_LOCK"))?;
            if slot.is_some() {
                return Err(internal("LINK_TASK_DUPLICATED"));
            }
            *slot = Some(task);
            Ok(CapabilityOutcome {
                output: None,
                affected_resources: vec![resource],
            })
        })
    }
}
pub(super) struct Prepared {
    pub delivery_id: String,
    pub event_id: String,
    pub state_json: String,
    pub task: Option<Task>,
}
pub(super) async fn prepare(
    before: &Deal,
    after: &Deal,
    tenant: &str,
    actor: &str,
    trace: &str,
    now: i64,
) -> Result<Prepared, SdkError> {
    let (delivery, context) = make_delivery(before, after, tenant, actor, trace, now)?;
    let adapter = ProtobufSalesActivitiesLinkContractAdapter;
    let event = adapter.decode_sales_deal_stage_changed(&delivery)?;
    let staged = StagedState::default();
    let capability = StagedActivities::new(now);
    let link = SalesActivitiesLink::new(adapter);
    let result = link
        .handle(&context, &delivery, &event, &capability, &staged)
        .await?;
    let task = capability.take_task()?;
    if (result.disposition == crm_sales_activities_link::LinkDisposition::Applied) != task.is_some()
    {
        return Err(internal("LINK_TASK_DISPOSITION_MISMATCH"));
    }
    let entry = staged.final_entry(delivery.delivery_id.as_str())?;
    let state_json =
        serde_json::to_string(&entry).map_err(|_| internal("LINK_STATE_SERIALIZATION"))?;
    Ok(Prepared {
        delivery_id: delivery.delivery_id.as_str().to_owned(),
        event_id: delivery.event_id.as_str().to_owned(),
        state_json,
        task,
    })
}
