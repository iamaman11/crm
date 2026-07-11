use crate::definitions::*;
use crate::wire::{
    activities_record_type, deal_from_snapshot, deal_matches, deal_to_wire, sales_record_type,
    task_from_snapshot, task_matches, task_to_wire, validate_related_resource_tenant,
};
use crm_capability_plan_support as support;
use crm_capability_runtime::CapabilityDefinition;
use crm_core_data::{
    PostgresDataStore, RecordGetQuery, RecordListQuery, RecordQueryContinuation, RecordQuerySort,
};
use crm_module_sdk::{
    ActorId, DataClass, ErrorCategory, ModuleId, PortFuture, RecordId, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::{activities::v1 as activities, core::v1 as core, sales::v1 as sales};
use crm_query_runtime::{
    CursorBinding, CursorCodec, CursorContinuation, PageSizePolicy, QueryExecutionResult,
    QueryExecutor, QueryRequest, QuerySemanticValidator, QueryVisibilityAuthorizer,
    normalized_filter_hash,
};
use prost::Message;
use std::sync::Arc;

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAXIMUM_PAGE_SIZE: u32 = 200;
const MAXIMUM_VISIBILITY_SCAN_RECORDS: usize = 10_000;

#[derive(Clone)]
pub struct SalesActivitiesQueryAdapter {
    store: PostgresDataStore,
    cursor_codec: CursorCodec,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
    page_policy: PageSizePolicy,
}

impl std::fmt::Debug for SalesActivitiesQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SalesActivitiesQueryAdapter")
            .field("store", &self.store)
            .field("cursor_codec", &self.cursor_codec)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .field("page_policy", &self.page_policy)
            .finish()
    }
}

impl SalesActivitiesQueryAdapter {
    pub fn new(
        store: PostgresDataStore,
        cursor_codec: CursorCodec,
        visibility: Arc<dyn QueryVisibilityAuthorizer>,
    ) -> Result<Self, SdkError> {
        let page_policy = PageSizePolicy {
            default_size: DEFAULT_PAGE_SIZE,
            maximum_size: MAXIMUM_PAGE_SIZE,
        }
        .validate()
        .map_err(cursor_error)?;
        Ok(Self {
            store,
            cursor_codec,
            visibility,
            page_policy,
        })
    }
}

impl QuerySemanticValidator for SalesActivitiesQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            match definition.capability_id.as_str() {
                SALES_GET_CAPABILITY => {
                    let command: sales::GetDealRequest =
                        decode_input(request, SALES_MODULE_ID, SALES_GET_REQUEST_SCHEMA)?;
                    validate_record_id(&command.deal_id, "deal.deal_id")?;
                }
                SALES_LIST_CAPABILITY => {
                    let command: sales::ListDealsRequest =
                        decode_input(request, SALES_MODULE_ID, SALES_LIST_REQUEST_SCHEMA)?;
                    validate_sales_list(self, request, &command)?;
                }
                ACTIVITIES_GET_CAPABILITY => {
                    let command: activities::GetTaskRequest =
                        decode_input(request, ACTIVITIES_MODULE_ID, ACTIVITIES_GET_REQUEST_SCHEMA)?;
                    validate_record_id(&command.task_id, "task.task_id")?;
                }
                ACTIVITIES_LIST_CAPABILITY => {
                    let command: activities::ListTasksRequest =
                        decode_input(request, ACTIVITIES_MODULE_ID, ACTIVITIES_LIST_REQUEST_SCHEMA)?;
                    validate_activities_list(self, request, &command)?;
                }
                _ => return Err(unsupported_query()),
            }
            Ok(())
        })
    }
}

impl QueryExecutor for SalesActivitiesQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            let output = match definition.capability_id.as_str() {
                SALES_GET_CAPABILITY => self.execute_get_deal(&request).await?,
                SALES_LIST_CAPABILITY => self.execute_list_deals(&request).await?,
                ACTIVITIES_GET_CAPABILITY => self.execute_get_task(&request).await?,
                ACTIVITIES_LIST_CAPABILITY => self.execute_list_tasks(&request).await?,
                _ => return Err(unsupported_query()),
            };
            Ok(QueryExecutionResult { output })
        })
    }
}

impl SalesActivitiesQueryAdapter {
    async fn execute_get_deal(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: sales::GetDealRequest =
            decode_input(request, SALES_MODULE_ID, SALES_GET_REQUEST_SCHEMA)?;
        let record_id = validate_record_id(&command.deal_id, "deal.deal_id")?;
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: ModuleId::try_new(SALES_MODULE_ID).map_err(config_error)?,
                record_type: sales_record_type(),
                record_id,
            })
            .await?
            .ok_or_else(resource_not_found)?;
        let visibility = self
            .visibility
            .authorize_visibility(request, &snapshot.reference)
            .await?;
        if !visibility.resource_visible {
            return Err(resource_not_found());
        }
        let deal = deal_from_snapshot(&snapshot)?;
        support::protobuf_payload(
            SALES_MODULE_ID,
            SALES_GET_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &sales::GetDealResponse {
                deal: Some(deal_to_wire(&deal, &request.context.tenant_id, &visibility)),
            },
        )
    }

    async fn execute_get_task(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: activities::GetTaskRequest =
            decode_input(request, ACTIVITIES_MODULE_ID, ACTIVITIES_GET_REQUEST_SCHEMA)?;
        let record_id = validate_record_id(&command.task_id, "task.task_id")?;
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: ModuleId::try_new(ACTIVITIES_MODULE_ID).map_err(config_error)?,
                record_type: activities_record_type(),
                record_id,
            })
            .await?
            .ok_or_else(resource_not_found)?;
        let visibility = self
            .visibility
            .authorize_visibility(request, &snapshot.reference)
            .await?;
        if !visibility.resource_visible {
            return Err(resource_not_found());
        }
        let task = task_from_snapshot(&snapshot)?;
        support::protobuf_payload(
            ACTIVITIES_MODULE_ID,
            ACTIVITIES_GET_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &activities::GetTaskResponse {
                task: Some(task_to_wire(&task, &request.context.tenant_id, &visibility)),
            },
        )
    }

    async fn execute_list_deals(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: sales::ListDealsRequest =
            decode_input(request, SALES_MODULE_ID, SALES_LIST_REQUEST_SCHEMA)?;
        let page_size = resolve_page_size(self.page_policy, command.page.as_ref())?;
        let filter_hash = sales_filter_hash(&command);
        let binding = cursor_binding(
            request,
            sales_record_type(),
            filter_hash,
            RecordQuerySort::UpdatedAtDescending,
            page_size,
        );
        let after = decode_after(self, command.page.as_ref(), &binding)?;
        let owner = command.owner.as_ref();
        let pipeline_id = command.pipeline_id.as_deref();
        let status = command.status;
        let (deals, next) = self
            .collect_deals(request, page_size, after, owner, pipeline_id, status)
            .await?;
        let next_page_token = encode_next(self, &binding, next.as_ref())?;
        support::protobuf_payload(
            SALES_MODULE_ID,
            SALES_LIST_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &sales::ListDealsResponse {
                deals,
                page: Some(core::PageInfo {
                    next_page_token,
                    total_size: 0,
                }),
            },
        )
    }

    async fn execute_list_tasks(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: activities::ListTasksRequest =
            decode_input(request, ACTIVITIES_MODULE_ID, ACTIVITIES_LIST_REQUEST_SCHEMA)?;
        let page_size = resolve_page_size(self.page_policy, command.page.as_ref())?;
        let filter_hash = activities_filter_hash(&command);
        let binding = cursor_binding(
            request,
            activities_record_type(),
            filter_hash,
            RecordQuerySort::UpdatedAtDescending,
            page_size,
        );
        let after = decode_after(self, command.page.as_ref(), &binding)?;
        let owner = command.owner.as_ref();
        let status = command.status;
        let related_resource = command.related_resource.as_ref();
        let (tasks, next) = self
            .collect_tasks(request, page_size, after, owner, status, related_resource)
            .await?;
        let next_page_token = encode_next(self, &binding, next.as_ref())?;
        support::protobuf_payload(
            ACTIVITIES_MODULE_ID,
            ACTIVITIES_LIST_RESPONSE_SCHEMA,
            DataClass::Confidential,
            &activities::ListTasksResponse {
                tasks,
                page: Some(core::PageInfo {
                    next_page_token,
                    total_size: 0,
                }),
            },
        )
    }

    async fn collect_deals(
        &self,
        request: &QueryRequest,
        page_size: u32,
        mut after: Option<RecordQueryContinuation>,
        owner: Option<&core::ActorOrTeamOwner>,
        pipeline_id: Option<&str>,
        status: Option<i32>,
    ) -> Result<(Vec<sales::Deal>, Option<RecordQueryContinuation>), SdkError> {
        let mut output = Vec::with_capacity(page_size as usize);
        let mut scanned = 0_usize;
        loop {
            let remaining = page_size as usize - output.len();
            if remaining == 0 {
                let anchor = after.clone();
                let has_more = self
                    .has_more_visible_deal(request, anchor.clone(), owner, pipeline_id, status, &mut scanned)
                    .await?;
                return Ok((output, has_more.then_some(anchor).flatten()));
            }
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: ModuleId::try_new(SALES_MODULE_ID).map_err(config_error)?,
                    record_type: sales_record_type(),
                    page_size: u32::try_from(remaining).map_err(|_| scan_limit_error())?,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(scanned)?;
            for snapshot in &page.records {
                let deal = deal_from_snapshot(snapshot)?;
                if !deal_matches(&deal, owner, pipeline_id, status) {
                    continue;
                }
                let visibility = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if visibility.resource_visible {
                    output.push(deal_to_wire(
                        &deal,
                        &request.context.tenant_id,
                        &visibility,
                    ));
                }
            }
            after = page.next;
            if after.is_none() {
                return Ok((output, None));
            }
        }
    }

    async fn collect_tasks(
        &self,
        request: &QueryRequest,
        page_size: u32,
        mut after: Option<RecordQueryContinuation>,
        owner: Option<&core::ActorOrTeamOwner>,
        status: Option<i32>,
        related_resource: Option<&core::ResourceRef>,
    ) -> Result<(Vec<activities::Task>, Option<RecordQueryContinuation>), SdkError> {
        let mut output = Vec::with_capacity(page_size as usize);
        let mut scanned = 0_usize;
        loop {
            let remaining = page_size as usize - output.len();
            if remaining == 0 {
                let anchor = after.clone();
                let has_more = self
                    .has_more_visible_task(
                        request,
                        anchor.clone(),
                        owner,
                        status,
                        related_resource,
                        &mut scanned,
                    )
                    .await?;
                return Ok((output, has_more.then_some(anchor).flatten()));
            }
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: ModuleId::try_new(ACTIVITIES_MODULE_ID).map_err(config_error)?,
                    record_type: activities_record_type(),
                    page_size: u32::try_from(remaining).map_err(|_| scan_limit_error())?,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(scanned)?;
            for snapshot in &page.records {
                let task = task_from_snapshot(snapshot)?;
                if !task_matches(&task, owner, status, related_resource) {
                    continue;
                }
                let visibility = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if visibility.resource_visible {
                    output.push(task_to_wire(
                        &task,
                        &request.context.tenant_id,
                        &visibility,
                    ));
                }
            }
            after = page.next;
            if after.is_none() {
                return Ok((output, None));
            }
        }
    }

    async fn has_more_visible_deal(
        &self,
        request: &QueryRequest,
        mut after: Option<RecordQueryContinuation>,
        owner: Option<&core::ActorOrTeamOwner>,
        pipeline_id: Option<&str>,
        status: Option<i32>,
        scanned: &mut usize,
    ) -> Result<bool, SdkError> {
        while after.is_some() {
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: ModuleId::try_new(SALES_MODULE_ID).map_err(config_error)?,
                    record_type: sales_record_type(),
                    page_size: MAXIMUM_PAGE_SIZE,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            *scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(*scanned)?;
            for snapshot in &page.records {
                let deal = deal_from_snapshot(snapshot)?;
                if deal_matches(&deal, owner, pipeline_id, status)
                    && self
                        .visibility
                        .authorize_visibility(request, &snapshot.reference)
                        .await?
                        .resource_visible
                {
                    return Ok(true);
                }
            }
            after = page.next;
        }
        Ok(false)
    }

    async fn has_more_visible_task(
        &self,
        request: &QueryRequest,
        mut after: Option<RecordQueryContinuation>,
        owner: Option<&core::ActorOrTeamOwner>,
        status: Option<i32>,
        related_resource: Option<&core::ResourceRef>,
        scanned: &mut usize,
    ) -> Result<bool, SdkError> {
        while after.is_some() {
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: ModuleId::try_new(ACTIVITIES_MODULE_ID).map_err(config_error)?,
                    record_type: activities_record_type(),
                    page_size: MAXIMUM_PAGE_SIZE,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            *scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(*scanned)?;
            for snapshot in &page.records {
                let task = task_from_snapshot(snapshot)?;
                if task_matches(&task, owner, status, related_resource)
                    && self
                        .visibility
                        .authorize_visibility(request, &snapshot.reference)
                        .await?
                        .resource_visible
                {
                    return Ok(true);
                }
            }
            after = page.next;
        }
        Ok(false)
    }
}

fn validate_sales_list(
    adapter: &SalesActivitiesQueryAdapter,
    request: &QueryRequest,
    command: &sales::ListDealsRequest,
) -> Result<(), SdkError> {
    validate_owner(command.owner.as_ref(), "deal.owner")?;
    if command.pipeline_id.as_deref().is_some_and(str::is_empty) {
        return Err(SdkError::invalid_argument(
            "deal.pipeline_id",
            "pipeline ID must not be empty",
        ));
    }
    if let Some(status) = command.status {
        match sales::DealStatus::try_from(status).ok() {
            Some(sales::DealStatus::Open | sales::DealStatus::Won | sales::DealStatus::Lost) => {}
            _ => {
                return Err(SdkError::invalid_argument(
                    "deal.status",
                    "deal status filter is invalid",
                ));
            }
        }
    }
    validate_sales_sort(command.sort)?;
    let page_size = resolve_page_size(adapter.page_policy, command.page.as_ref())?;
    let binding = cursor_binding(
        request,
        sales_record_type(),
        sales_filter_hash(command),
        RecordQuerySort::UpdatedAtDescending,
        page_size,
    );
    let _ = decode_after(adapter, command.page.as_ref(), &binding)?;
    Ok(())
}

fn validate_activities_list(
    adapter: &SalesActivitiesQueryAdapter,
    request: &QueryRequest,
    command: &activities::ListTasksRequest,
) -> Result<(), SdkError> {
    validate_owner(command.owner.as_ref(), "task.owner")?;
    if let Some(status) = command.status {
        match activities::TaskStatus::try_from(status).ok() {
            Some(activities::TaskStatus::Open | activities::TaskStatus::Completed) => {}
            _ => {
                return Err(SdkError::invalid_argument(
                    "task.status",
                    "task status filter is invalid",
                ));
            }
        }
    }
    validate_related_resource_tenant(command.related_resource.as_ref(), &request.context.tenant_id)?;
    validate_activities_sort(command.sort)?;
    let page_size = resolve_page_size(adapter.page_policy, command.page.as_ref())?;
    let binding = cursor_binding(
        request,
        activities_record_type(),
        activities_filter_hash(command),
        RecordQuerySort::UpdatedAtDescending,
        page_size,
    );
    let _ = decode_after(adapter, command.page.as_ref(), &binding)?;
    Ok(())
}

fn validate_sales_sort(sort: i32) -> Result<(), SdkError> {
    match sales::DealSort::try_from(sort).ok() {
        Some(sales::DealSort::Unspecified | sales::DealSort::UpdatedAtDescending) => Ok(()),
        Some(_) => Err(unsupported_sort("deal.sort")),
        None => Err(SdkError::invalid_argument(
            "deal.sort",
            "deal sort is invalid",
        )),
    }
}

fn validate_activities_sort(sort: i32) -> Result<(), SdkError> {
    match activities::TaskSort::try_from(sort).ok() {
        Some(activities::TaskSort::Unspecified | activities::TaskSort::UpdatedAtDescending) => Ok(()),
        Some(_) => Err(unsupported_sort("task.sort")),
        None => Err(SdkError::invalid_argument(
            "task.sort",
            "task sort is invalid",
        )),
    }
}

fn validate_owner(value: Option<&core::ActorOrTeamOwner>, field: &'static str) -> Result<(), SdkError> {
    use core::actor_or_team_owner::Owner;
    if let Some(value) = value {
        match value.owner.as_ref() {
            Some(Owner::ActorId(actor_id)) => {
                ActorId::try_new(actor_id.clone())
                    .map_err(|error| SdkError::invalid_argument(field, error.to_string()))?;
            }
            Some(Owner::TeamId(team_id)) if !team_id.is_empty() => {}
            _ => return Err(SdkError::invalid_argument(field, "owner filter is invalid")),
        }
    }
    Ok(())
}

fn validate_record_id(value: &str, field: &'static str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value.to_owned()).map_err(|error| SdkError::invalid_argument(field, error.to_string()))
}

fn resolve_page_size(
    policy: PageSizePolicy,
    page: Option<&core::PageRequest>,
) -> Result<u32, SdkError> {
    policy
        .resolve(page.map_or(0, |value| value.page_size))
        .map_err(cursor_error)
}

fn cursor_binding(
    request: &QueryRequest,
    resource_type: crm_module_sdk::RecordType,
    filter_hash: [u8; 32],
    sort: RecordQuerySort,
    page_size: u32,
) -> CursorBinding {
    CursorBinding {
        tenant_id: request.context.tenant_id.clone(),
        actor_id: Some(request.context.actor_id.clone()),
        capability_id: request.context.capability_id.clone(),
        capability_version: request.context.capability_version.clone(),
        resource_type,
        normalized_filter_hash: filter_hash,
        sort_id: sort.id().to_owned(),
        page_size,
    }
}

fn decode_after(
    adapter: &SalesActivitiesQueryAdapter,
    page: Option<&core::PageRequest>,
    binding: &CursorBinding,
) -> Result<Option<RecordQueryContinuation>, SdkError> {
    let token = page.map(|value| value.page_token.as_str()).unwrap_or("");
    if token.is_empty() {
        return Ok(None);
    }
    let continuation = adapter
        .cursor_codec
        .decode(token, binding)
        .map_err(cursor_error)?;
    let sort_value = String::from_utf8(continuation.sort_key).map_err(|_| {
        SdkError::new(
            "QUERY_CURSOR_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The page cursor is invalid.",
        )
    })?;
    let after = RecordQueryContinuation {
        sort_value,
        record_id: continuation.record_id,
    };
    after.validate()?;
    Ok(Some(after))
}

fn encode_next(
    adapter: &SalesActivitiesQueryAdapter,
    binding: &CursorBinding,
    next: Option<&RecordQueryContinuation>,
) -> Result<String, SdkError> {
    next.map(|next| {
        adapter
            .cursor_codec
            .encode(
                binding,
                &CursorContinuation {
                    sort_key: next.sort_value.as_bytes().to_vec(),
                    record_id: next.record_id.clone(),
                },
            )
            .map_err(cursor_error)
    })
    .transpose()
    .map(|value| value.unwrap_or_default())
}

fn sales_filter_hash(command: &sales::ListDealsRequest) -> [u8; 32] {
    let owner = command.owner.as_ref().map(Message::encode_to_vec).unwrap_or_default();
    let pipeline = command.pipeline_id.as_deref().unwrap_or_default().as_bytes();
    let status = command.status.unwrap_or_default().to_be_bytes();
    normalized_filter_hash([
        ("owner", owner.as_slice()),
        ("pipeline_id", pipeline),
        ("status", status.as_slice()),
    ])
}

fn activities_filter_hash(command: &activities::ListTasksRequest) -> [u8; 32] {
    let owner = command.owner.as_ref().map(Message::encode_to_vec).unwrap_or_default();
    let status = command.status.unwrap_or_default().to_be_bytes();
    let related = command
        .related_resource
        .as_ref()
        .map(Message::encode_to_vec)
        .unwrap_or_default();
    normalized_filter_hash([
        ("owner", owner.as_slice()),
        ("status", status.as_slice()),
        ("related_resource", related.as_slice()),
    ])
}

fn decode_input<M: Message + Default>(
    request: &QueryRequest,
    owner: &str,
    schema_id: &str,
) -> Result<M, SdkError> {
    let payload = &request.input;
    if payload.owner.as_str() != owner
        || payload.schema_id.as_str() != schema_id
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(schema_id)
        || payload.data_class != DataClass::Confidential
        || payload.encoding != crm_module_sdk::PayloadEncoding::Protobuf
        || payload.maximum_size_bytes > support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            "QUERY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The query input does not match the required contract.",
        ));
    }
    M::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "QUERY_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The query input is not valid Protobuf.",
        )
    })
}

fn enforce_scan_limit(scanned: usize) -> Result<(), SdkError> {
    if scanned > MAXIMUM_VISIBILITY_SCAN_RECORDS {
        Err(scan_limit_error())
    } else {
        Ok(())
    }
}

fn resource_not_found() -> SdkError {
    SdkError::new(
        "QUERY_RESOURCE_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested resource was not found.",
    )
}

fn unsupported_query() -> SdkError {
    SdkError::new(
        "QUERY_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The query capability is not configured.",
    )
}

fn unsupported_sort(field: &'static str) -> SdkError {
    SdkError::invalid_argument(
        field,
        "this authoritative query path currently supports updated-at descending order only",
    )
}

fn scan_limit_error() -> SdkError {
    SdkError::new(
        "QUERY_SCAN_LIMIT_EXCEEDED",
        ErrorCategory::Unavailable,
        true,
        "The query could not be completed within the governed scan limit.",
    )
}

fn cursor_error(error: crm_query_runtime::CursorError) -> SdkError {
    let category = match error {
        crm_query_runtime::CursorError::SigningKeyTooShort
        | crm_query_runtime::CursorError::SigningUnavailable
        | crm_query_runtime::CursorError::InvalidPagePolicy => ErrorCategory::Unavailable,
        _ => ErrorCategory::InvalidArgument,
    };
    SdkError::new(error.code(), category, false, error.safe_message())
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The query service configuration is invalid.",
    )
    .with_internal_reference(error.to_string())
}
