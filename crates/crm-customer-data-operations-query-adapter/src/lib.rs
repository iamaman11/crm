#![forbid(unsafe_code)]

mod artifact_download;
pub use artifact_download::*;

mod export_query;
pub use export_query::{
    EXPORT_QUERY_CAPABILITY_IDS, GET_EXPORT_JOB_CAPABILITY, LIST_EXPORT_JOBS_CAPABILITY,
    PartyExportQueryAdapter, export_query_capability_definitions,
};

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::{
    PostgresDataStore, RecordGetQuery, RecordListQuery, RecordQueryContinuation, RecordQuerySort,
    RelatedRecordListQuery,
};
use crm_customer_data_operations::{
    ImportJob, ImportJobStatus, ImportRow, ImportRowStatus, decode_import_row_state,
};
use crm_customer_data_operations_capability_adapter::{
    IMPORT_JOB_RECORD_TYPE, IMPORT_JOB_ROW_RELATIONSHIP_TYPE, IMPORT_ROW_RECORD_TYPE, MODULE_ID,
    import_job_from_snapshot, import_row_persisted_contract, import_row_to_wire, job_to_wire,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordRef, RecordType, RelationshipType, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::customer_data_operations::v1 as wire;
use crm_query_runtime::{
    CursorBinding, CursorCodec, CursorContinuation, PageSizePolicy, QueryExecutionResult,
    QueryExecutor, QueryRequest, QuerySemanticValidator, QueryVisibilityAuthorizer,
    QueryVisibilityDecision, normalized_filter_hash,
};
use prost::Message;
use std::sync::Arc;

pub const GET_IMPORT_JOB_CAPABILITY: &str = "customer_data.import.party.get";
pub const LIST_IMPORT_JOBS_CAPABILITY: &str = "customer_data.import.party.list";
pub const LIST_IMPORT_ROWS_CAPABILITY: &str = "customer_data.import.party.rows.list";

pub const GET_IMPORT_JOB_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.GetPartyImportJobRequest";
pub const GET_IMPORT_JOB_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.GetPartyImportJobResponse";
pub const LIST_IMPORT_JOBS_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.ListPartyImportJobsRequest";
pub const LIST_IMPORT_JOBS_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.ListPartyImportJobsResponse";
pub const LIST_IMPORT_ROWS_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.ListPartyImportRowsRequest";
pub const LIST_IMPORT_ROWS_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.ListPartyImportRowsResponse";

pub const IMPORT_QUERY_CAPABILITY_IDS: [&str; 3] = [
    GET_IMPORT_JOB_CAPABILITY,
    LIST_IMPORT_JOBS_CAPABILITY,
    LIST_IMPORT_ROWS_CAPABILITY,
];

pub const QUERY_CAPABILITY_IDS: [&str; 5] = [
    GET_IMPORT_JOB_CAPABILITY,
    LIST_IMPORT_JOBS_CAPABILITY,
    LIST_IMPORT_ROWS_CAPABILITY,
    GET_EXPORT_JOB_CAPABILITY,
    LIST_EXPORT_JOBS_CAPABILITY,
];

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAXIMUM_PAGE_SIZE: u32 = 200;
const MAXIMUM_VISIBILITY_SCAN_RECORDS: usize = 10_000;
const RELATED_ROW_SORT_ID: &str = "relationship_target_record_id_asc";

#[derive(Clone)]
pub struct CustomerDataOperationsQueryAdapter {
    store: PostgresDataStore,
    cursor_codec: CursorCodec,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
    page_policy: PageSizePolicy,
    export: PartyExportQueryAdapter,
}

impl std::fmt::Debug for CustomerDataOperationsQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CustomerDataOperationsQueryAdapter")
            .field("store", &self.store)
            .field("cursor_codec", &self.cursor_codec)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .field("page_policy", &self.page_policy)
            .finish()
    }
}

impl CustomerDataOperationsQueryAdapter {
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
        let export =
            PartyExportQueryAdapter::new(store.clone(), cursor_codec.clone(), visibility.clone())?;
        Ok(Self {
            store,
            cursor_codec,
            visibility,
            page_policy,
            export,
        })
    }
}

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    let mut definitions = IMPORT_QUERY_CAPABILITY_IDS
        .iter()
        .map(|capability_id| query_capability_definition(capability_id))
        .collect::<Result<Vec<_>, _>>()?;
    definitions.extend(export_query_capability_definitions()?);
    Ok(definitions)
}

pub fn query_capability_definition(capability_id: &str) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema) = match capability_id {
        GET_IMPORT_JOB_CAPABILITY => (
            GET_IMPORT_JOB_REQUEST_SCHEMA,
            GET_IMPORT_JOB_RESPONSE_SCHEMA,
        ),
        LIST_IMPORT_JOBS_CAPABILITY => (
            LIST_IMPORT_JOBS_REQUEST_SCHEMA,
            LIST_IMPORT_JOBS_RESPONSE_SCHEMA,
        ),
        LIST_IMPORT_ROWS_CAPABILITY => (
            LIST_IMPORT_ROWS_REQUEST_SCHEMA,
            LIST_IMPORT_ROWS_RESPONSE_SCHEMA,
        ),
        _ => return Err(unsupported_query()),
    };

    Ok(CapabilityDefinition {
        capability_id: support::configured_identifier(CapabilityId::try_new(capability_id))?,
        capability_version: support::configured_identifier(CapabilityVersion::try_new(
            support::CONTRACT_VERSION,
        ))?,
        owner_module_id: support::configured_identifier(ModuleId::try_new(MODULE_ID))?,
        input_contract: support::protobuf_contract(
            MODULE_ID,
            input_schema,
            vec![DataClass::Personal],
        )?,
        output_contract: Some(support::protobuf_contract(
            MODULE_ID,
            output_schema,
            vec![DataClass::Personal],
        )?),
        risk: CapabilityRisk::Low,
        mutation: false,
        requires_idempotency: false,
        requires_approval: false,
        authorization_policy_id: capability_id.to_owned(),
        rate_limit_policy_id: None,
    })
}

impl QuerySemanticValidator for CustomerDataOperationsQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            match definition.capability_id.as_str() {
                GET_IMPORT_JOB_CAPABILITY => {
                    let command: wire::GetPartyImportJobRequest =
                        decode_input(request, GET_IMPORT_JOB_REQUEST_SCHEMA)?;
                    let _ = import_job_record_id(command.import_job_ref)?;
                }
                LIST_IMPORT_JOBS_CAPABILITY => {
                    let command: wire::ListPartyImportJobsRequest =
                        decode_input(request, LIST_IMPORT_JOBS_REQUEST_SCHEMA)?;
                    validate_job_status_filter(command.status)?;
                    let page_size = self
                        .page_policy
                        .resolve(command.page_size)
                        .map_err(cursor_error)?;
                    let binding =
                        jobs_cursor_binding(request, job_filter_hash(command.status), page_size)?;
                    let _ = decode_job_after(self, &command.cursor, &binding)?;
                }
                LIST_IMPORT_ROWS_CAPABILITY => {
                    let command: wire::ListPartyImportRowsRequest =
                        decode_input(request, LIST_IMPORT_ROWS_REQUEST_SCHEMA)?;
                    let job_id = import_job_record_id(command.import_job_ref)?;
                    validate_row_status_filter(command.status)?;
                    let page_size = self
                        .page_policy
                        .resolve(command.page_size)
                        .map_err(cursor_error)?;
                    let binding = rows_cursor_binding(
                        request,
                        row_filter_hash(job_id.as_str(), command.status),
                        page_size,
                    )?;
                    let _ = decode_row_after(self, &command.cursor, &binding)?;
                }
                GET_EXPORT_JOB_CAPABILITY | LIST_EXPORT_JOBS_CAPABILITY => {
                    self.export
                        .validate_request(definition.capability_id.as_str(), request)?;
                }
                _ => return Err(unsupported_query()),
            }
            Ok(())
        })
    }
}

impl QueryExecutor for CustomerDataOperationsQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            let output = match definition.capability_id.as_str() {
                GET_IMPORT_JOB_CAPABILITY => self.execute_get_job(&request).await?,
                LIST_IMPORT_JOBS_CAPABILITY => self.execute_list_jobs(&request).await?,
                LIST_IMPORT_ROWS_CAPABILITY => self.execute_list_rows(&request).await?,
                GET_EXPORT_JOB_CAPABILITY | LIST_EXPORT_JOBS_CAPABILITY => {
                    self.export
                        .execute_request(definition.capability_id.as_str(), &request)
                        .await?
                }
                _ => return Err(unsupported_query()),
            };
            Ok(QueryExecutionResult { output })
        })
    }
}

impl CustomerDataOperationsQueryAdapter {
    async fn execute_get_job(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::GetPartyImportJobRequest =
            decode_input(request, GET_IMPORT_JOB_REQUEST_SCHEMA)?;
        let job_id = import_job_record_id(command.import_job_ref)?;
        let snapshot = self.get_visible_job_snapshot(request, job_id).await?;
        let job = import_job_from_snapshot(&snapshot)?;
        let visibility = self
            .visibility
            .authorize_visibility(request, &snapshot.reference)
            .await?;

        support::protobuf_payload(
            MODULE_ID,
            GET_IMPORT_JOB_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::GetPartyImportJobResponse {
                import_job: Some(job_to_wire_with_visibility(&job, &visibility)?),
            },
        )
    }

    async fn execute_list_jobs(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::ListPartyImportJobsRequest =
            decode_input(request, LIST_IMPORT_JOBS_REQUEST_SCHEMA)?;
        validate_job_status_filter(command.status)?;
        let page_size = self
            .page_policy
            .resolve(command.page_size)
            .map_err(cursor_error)?;
        let binding = jobs_cursor_binding(request, job_filter_hash(command.status), page_size)?;
        let after = decode_job_after(self, &command.cursor, &binding)?;
        let (jobs, next) = self
            .collect_jobs(request, page_size, after, command.status)
            .await?;
        let next_cursor = encode_job_next(self, &binding, next.as_ref())?;

        support::protobuf_payload(
            MODULE_ID,
            LIST_IMPORT_JOBS_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::ListPartyImportJobsResponse {
                import_jobs: jobs,
                next_cursor,
            },
        )
    }

    async fn execute_list_rows(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::ListPartyImportRowsRequest =
            decode_input(request, LIST_IMPORT_ROWS_REQUEST_SCHEMA)?;
        let job_id = import_job_record_id(command.import_job_ref)?;
        validate_row_status_filter(command.status)?;
        let job_snapshot = self
            .get_visible_job_snapshot(request, job_id.clone())
            .await?;
        let page_size = self
            .page_policy
            .resolve(command.page_size)
            .map_err(cursor_error)?;
        let binding = rows_cursor_binding(
            request,
            row_filter_hash(job_id.as_str(), command.status),
            page_size,
        )?;
        let after = decode_row_after(self, &command.cursor, &binding)?;
        let (rows, next) = self
            .collect_rows(
                request,
                &job_snapshot.reference,
                page_size,
                after,
                command.status,
            )
            .await?;
        let next_cursor = encode_row_next(self, &binding, next.as_ref())?;

        support::protobuf_payload(
            MODULE_ID,
            LIST_IMPORT_ROWS_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::ListPartyImportRowsResponse {
                import_rows: rows,
                next_cursor,
            },
        )
    }

    async fn get_visible_job_snapshot(
        &self,
        request: &QueryRequest,
        job_id: RecordId,
    ) -> Result<crm_module_sdk::RecordSnapshot, SdkError> {
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: module_id()?,
                record_type: import_job_record_type()?,
                record_id: job_id,
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
        Ok(snapshot)
    }

    async fn collect_jobs(
        &self,
        request: &QueryRequest,
        page_size: u32,
        mut after: Option<RecordQueryContinuation>,
        status: i32,
    ) -> Result<(Vec<wire::ImportJob>, Option<RecordQueryContinuation>), SdkError> {
        let mut output = Vec::with_capacity(page_size as usize);
        let mut scanned = 0_usize;
        loop {
            let remaining = page_size as usize - output.len();
            if remaining == 0 {
                let anchor = after.clone();
                let has_more = self
                    .has_more_visible_job(request, anchor.clone(), status, &mut scanned)
                    .await?;
                return Ok((output, has_more.then_some(anchor).flatten()));
            }

            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: module_id()?,
                    record_type: import_job_record_type()?,
                    page_size: u32::try_from(remaining).map_err(|_| scan_limit_error())?,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(scanned)?;
            for snapshot in &page.records {
                let job = import_job_from_snapshot(snapshot)?;
                if !job_matches_status(&job, status) {
                    continue;
                }
                let visibility = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if visibility.resource_visible {
                    output.push(job_to_wire_with_visibility(&job, &visibility)?);
                }
            }
            after = page.next;
            if after.is_none() {
                return Ok((output, None));
            }
        }
    }

    async fn has_more_visible_job(
        &self,
        request: &QueryRequest,
        mut after: Option<RecordQueryContinuation>,
        status: i32,
        scanned: &mut usize,
    ) -> Result<bool, SdkError> {
        while after.is_some() {
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: module_id()?,
                    record_type: import_job_record_type()?,
                    page_size: MAXIMUM_PAGE_SIZE,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            *scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(*scanned)?;
            for snapshot in &page.records {
                let job = import_job_from_snapshot(snapshot)?;
                if job_matches_status(&job, status)
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

    async fn collect_rows(
        &self,
        request: &QueryRequest,
        job_ref: &RecordRef,
        page_size: u32,
        mut after: Option<RecordId>,
        status: i32,
    ) -> Result<(Vec<wire::ImportRow>, Option<RecordId>), SdkError> {
        let mut output = Vec::with_capacity(page_size as usize);
        let mut scanned = 0_usize;
        loop {
            let remaining = page_size as usize - output.len();
            if remaining == 0 {
                let anchor = after.clone();
                let has_more = self
                    .has_more_visible_row(request, job_ref, anchor.clone(), status, &mut scanned)
                    .await?;
                return Ok((output, has_more.then_some(anchor).flatten()));
            }

            let page = self
                .store
                .list_related_records_for_query(&RelatedRecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    relationship_owner_module_id: module_id()?,
                    relationship_type: import_job_row_relationship_type()?,
                    source: job_ref.clone(),
                    target_owner_module_id: module_id()?,
                    target_record_type: import_row_record_type()?,
                    page_size: u32::try_from(remaining).map_err(|_| scan_limit_error())?,
                    after_record_id: after.clone(),
                })
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(scanned)?;
            for snapshot in &page.records {
                let row = import_row_from_snapshot(snapshot)?;
                if !row_matches_status(&row, status) {
                    continue;
                }
                let visibility = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if visibility.resource_visible {
                    output.push(import_row_to_wire_with_visibility(&row, &visibility)?);
                }
            }
            after = page.next_record_id;
            if after.is_none() {
                return Ok((output, None));
            }
        }
    }

    async fn has_more_visible_row(
        &self,
        request: &QueryRequest,
        job_ref: &RecordRef,
        mut after: Option<RecordId>,
        status: i32,
        scanned: &mut usize,
    ) -> Result<bool, SdkError> {
        while after.is_some() {
            let page = self
                .store
                .list_related_records_for_query(&RelatedRecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    relationship_owner_module_id: module_id()?,
                    relationship_type: import_job_row_relationship_type()?,
                    source: job_ref.clone(),
                    target_owner_module_id: module_id()?,
                    target_record_type: import_row_record_type()?,
                    page_size: MAXIMUM_PAGE_SIZE,
                    after_record_id: after.clone(),
                })
                .await?;
            *scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(*scanned)?;
            for snapshot in &page.records {
                let row = import_row_from_snapshot(snapshot)?;
                if row_matches_status(&row, status)
                    && self
                        .visibility
                        .authorize_visibility(request, &snapshot.reference)
                        .await?
                        .resource_visible
                {
                    return Ok(true);
                }
            }
            after = page.next_record_id;
        }
        Ok(false)
    }
}

fn import_row_from_snapshot(
    snapshot: &crm_module_sdk::RecordSnapshot,
) -> Result<ImportRow, SdkError> {
    let row = decode_import_row_state(support::persisted_json_bytes_with_data_class(
        snapshot,
        import_row_persisted_contract(),
        DataClass::Personal,
    )?)?;
    if row.row_id().as_str() != snapshot.reference.record_id.as_str()
        || row.version() != snapshot.version
    {
        return Err(stored_data_error(
            "CUSTOMER_DATA_IMPORT_PERSISTED_ROW_IDENTITY_INVALID",
        ));
    }
    Ok(row)
}

fn job_to_wire_with_visibility(
    job: &ImportJob,
    visibility: &QueryVisibilityDecision,
) -> Result<wire::ImportJob, SdkError> {
    let mut output = job_to_wire(job)?;
    if !visibility.allows_field("source") {
        output.source = None;
    }
    if !visibility.allows_field("mapping") {
        output.mapping = None;
        output.mapping_version_id.clear();
    }
    if !visibility.allows_field("status") {
        output.status = wire::ImportJobStatus::Unspecified as i32;
    }
    if !visibility.allows_field("counters") {
        output.total_rows = 0;
        output.valid_rows = 0;
        output.invalid_rows = 0;
        output.succeeded_rows = 0;
    }
    if !visibility.allows_field("checkpoint") {
        output.checkpoint_row_position = 0;
    }
    Ok(output)
}

fn import_row_to_wire_with_visibility(
    row: &ImportRow,
    visibility: &QueryVisibilityDecision,
) -> Result<wire::ImportRow, SdkError> {
    let mut output = import_row_to_wire(row)?;
    if !visibility.allows_field("row_position") {
        output.row_position = 0;
    }
    if !visibility.allows_field("source_identity") {
        output.external_row_key_sha256.clear();
        output.source_external_id_sha256.clear();
    }
    if !visibility.allows_field("status") {
        output.status = wire::ImportRowStatus::Unspecified as i32;
    }
    if !visibility.allows_field("prepared_party") {
        output.prepared_party = None;
    }
    if !visibility.allows_field("diagnostics") {
        output.diagnostics.clear();
    }
    if !visibility.allows_field("execution") {
        output.execution_attempts = 0;
        output.last_execution_error_code.clear();
    }
    if !visibility.allows_field("target_party_ref") {
        output.target_party_ref = None;
    }
    Ok(output)
}

fn job_matches_status(job: &ImportJob, status: i32) -> bool {
    match wire::ImportJobStatus::try_from(status).ok() {
        None | Some(wire::ImportJobStatus::Unspecified) => true,
        Some(wire::ImportJobStatus::Created) => job.status() == ImportJobStatus::Created,
        Some(wire::ImportJobStatus::Validated) => job.status() == ImportJobStatus::Validated,
        Some(wire::ImportJobStatus::Executing) => job.status() == ImportJobStatus::Executing,
        Some(wire::ImportJobStatus::Completed) => job.status() == ImportJobStatus::Completed,
        Some(wire::ImportJobStatus::Cancelled) => job.status() == ImportJobStatus::Cancelled,
    }
}

fn row_matches_status(row: &ImportRow, status: i32) -> bool {
    match wire::ImportRowStatus::try_from(status).ok() {
        None | Some(wire::ImportRowStatus::Unspecified) => true,
        Some(wire::ImportRowStatus::Pending) => row.status() == ImportRowStatus::Pending,
        Some(wire::ImportRowStatus::Valid) => row.status() == ImportRowStatus::Valid,
        Some(wire::ImportRowStatus::Invalid) => row.status() == ImportRowStatus::Invalid,
        Some(wire::ImportRowStatus::FailedRetryable) => {
            row.status() == ImportRowStatus::FailedRetryable
        }
        Some(wire::ImportRowStatus::Succeeded) => row.status() == ImportRowStatus::Succeeded,
    }
}

fn validate_job_status_filter(status: i32) -> Result<(), SdkError> {
    if wire::ImportJobStatus::try_from(status).is_err() {
        return Err(SdkError::invalid_argument(
            "customer_data.import.status",
            "Import job status filter is invalid",
        ));
    }
    Ok(())
}

fn validate_row_status_filter(status: i32) -> Result<(), SdkError> {
    if wire::ImportRowStatus::try_from(status).is_err() {
        return Err(SdkError::invalid_argument(
            "customer_data.import.row_status",
            "Import row status filter is invalid",
        ));
    }
    Ok(())
}

fn jobs_cursor_binding(
    request: &QueryRequest,
    filter_hash: [u8; 32],
    page_size: u32,
) -> Result<CursorBinding, SdkError> {
    Ok(CursorBinding {
        tenant_id: request.context.tenant_id.clone(),
        actor_id: Some(request.context.actor_id.clone()),
        capability_id: request.context.capability_id.clone(),
        capability_version: request.context.capability_version.clone(),
        resource_type: import_job_record_type()?,
        normalized_filter_hash: filter_hash,
        sort_id: RecordQuerySort::UpdatedAtDescending.id().to_owned(),
        page_size,
    })
}

fn rows_cursor_binding(
    request: &QueryRequest,
    filter_hash: [u8; 32],
    page_size: u32,
) -> Result<CursorBinding, SdkError> {
    Ok(CursorBinding {
        tenant_id: request.context.tenant_id.clone(),
        actor_id: Some(request.context.actor_id.clone()),
        capability_id: request.context.capability_id.clone(),
        capability_version: request.context.capability_version.clone(),
        resource_type: import_row_record_type()?,
        normalized_filter_hash: filter_hash,
        sort_id: RELATED_ROW_SORT_ID.to_owned(),
        page_size,
    })
}

fn decode_job_after(
    adapter: &CustomerDataOperationsQueryAdapter,
    token: &str,
    binding: &CursorBinding,
) -> Result<Option<RecordQueryContinuation>, SdkError> {
    if token.is_empty() {
        return Ok(None);
    }
    let continuation = adapter
        .cursor_codec
        .decode(token, binding)
        .map_err(cursor_error)?;
    let sort_value = String::from_utf8(continuation.sort_key).map_err(|_| cursor_invalid())?;
    let after = RecordQueryContinuation {
        sort_value,
        record_id: continuation.record_id,
    };
    after.validate()?;
    Ok(Some(after))
}

fn encode_job_next(
    adapter: &CustomerDataOperationsQueryAdapter,
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

fn decode_row_after(
    adapter: &CustomerDataOperationsQueryAdapter,
    token: &str,
    binding: &CursorBinding,
) -> Result<Option<RecordId>, SdkError> {
    if token.is_empty() {
        return Ok(None);
    }
    let continuation = adapter
        .cursor_codec
        .decode(token, binding)
        .map_err(cursor_error)?;
    if !continuation.sort_key.is_empty() {
        return Err(cursor_invalid());
    }
    Ok(Some(continuation.record_id))
}

fn encode_row_next(
    adapter: &CustomerDataOperationsQueryAdapter,
    binding: &CursorBinding,
    next: Option<&RecordId>,
) -> Result<String, SdkError> {
    next.map(|record_id| {
        adapter
            .cursor_codec
            .encode(
                binding,
                &CursorContinuation {
                    sort_key: Vec::new(),
                    record_id: record_id.clone(),
                },
            )
            .map_err(cursor_error)
    })
    .transpose()
    .map(|value| value.unwrap_or_default())
}

fn job_filter_hash(status: i32) -> [u8; 32] {
    let status = status.to_be_bytes();
    normalized_filter_hash([("status", status.as_slice())])
}

fn row_filter_hash(job_id: &str, status: i32) -> [u8; 32] {
    let status = status.to_be_bytes();
    normalized_filter_hash([
        ("import_job_id", job_id.as_bytes()),
        ("status", status.as_slice()),
    ])
}

fn decode_input<M: Message + Default>(
    request: &QueryRequest,
    schema_id: &str,
) -> Result<M, SdkError> {
    let payload = &request.input;
    if payload.owner.as_str() != MODULE_ID
        || payload.schema_id.as_str() != schema_id
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(schema_id)
        || payload.data_class != DataClass::Personal
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes > support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(SdkError::new(
            "CUSTOMER_DATA_IMPORT_QUERY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The customer-data import query input does not match the required contract.",
        ));
    }
    M::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "CUSTOMER_DATA_IMPORT_QUERY_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The customer-data import query input is not valid Protobuf.",
        )
    })
}

fn import_job_record_id(value: Option<wire::ImportJobRef>) -> Result<RecordId, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_data.import_job_ref",
            "Import job reference is required",
        )
    })?;
    RecordId::try_new(value.import_job_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_data.import_job_ref.import_job_id",
            error.to_string(),
        )
    })
}

fn module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(MODULE_ID).map_err(config_error)
}

fn import_job_record_type() -> Result<RecordType, SdkError> {
    RecordType::try_new(IMPORT_JOB_RECORD_TYPE).map_err(config_error)
}

fn import_row_record_type() -> Result<RecordType, SdkError> {
    RecordType::try_new(IMPORT_ROW_RECORD_TYPE).map_err(config_error)
}

fn import_job_row_relationship_type() -> Result<RelationshipType, SdkError> {
    RelationshipType::try_new(IMPORT_JOB_ROW_RELATIONSHIP_TYPE).map_err(config_error)
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
        "CUSTOMER_DATA_IMPORT_QUERY_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The customer-data import query capability is not configured.",
    )
}

fn cursor_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_IMPORT_QUERY_CURSOR_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The customer-data import page cursor is invalid.",
    )
}

fn cursor_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_IMPORT_QUERY_CURSOR_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The customer-data import page cursor is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn scan_limit_error() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_IMPORT_QUERY_VISIBILITY_SCAN_LIMIT_EXCEEDED",
        ErrorCategory::Unavailable,
        true,
        "The customer-data import list is temporarily unavailable.",
    )
}

fn stored_data_error(code: &'static str) -> SdkError {
    SdkError::new(
        code,
        ErrorCategory::Unavailable,
        true,
        "Stored customer-data import state is temporarily unavailable.",
    )
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_IMPORT_QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The customer-data import query adapter is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_data_operations::{
        CreateImportJob, CreateImportRow, ImportJobId, ImportParserProfile, PartialExecutionPolicy,
        PartyImportMapping, SourceDescriptor, SourceSystemId,
    };
    use std::collections::BTreeSet;

    fn job() -> ImportJob {
        ImportJob::create(CreateImportJob {
            job_id: ImportJobId::try_new("import-job-visible-1").unwrap(),
            source: SourceDescriptor::try_new(
                "customers.csv",
                "11".repeat(32),
                2,
                SourceSystemId::try_new("legacy-crm").unwrap(),
                ImportParserProfile::csv_v1(b',', b'"').unwrap(),
            )
            .unwrap(),
            mapping: PartyImportMapping::try_new(
                None,
                "kind",
                "display_name",
                Some("legacy_customer_id".to_owned()),
                Some("row_key".to_owned()),
            )
            .unwrap(),
            partial_execution_policy: PartialExecutionPolicy::AllValidRows,
            occurred_at_unix_nanos: 10,
        })
        .unwrap()
    }

    #[test]
    fn publishes_five_personal_read_only_queries() {
        let definitions = query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 5);
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            QUERY_CAPABILITY_IDS
        );
        assert!(definitions.iter().all(|definition| !definition.mutation));
        assert!(definitions.iter().all(|definition| {
            !definition.requires_idempotency
                && definition.input_contract.allowed_data_classes == vec![DataClass::Personal]
        }));
    }

    #[test]
    fn job_visibility_redacts_source_mapping_and_operational_fields() {
        let decision = QueryVisibilityDecision {
            resource_visible: true,
            allowed_fields: BTreeSet::new(),
            decision_id: "decision-1".to_owned(),
            policy_version: "policy-1".to_owned(),
        };
        let output = job_to_wire_with_visibility(&job(), &decision).unwrap();
        assert!(output.import_job_ref.is_some());
        assert!(output.source.is_none());
        assert!(output.mapping.is_none());
        assert!(output.mapping_version_id.is_empty());
        assert_eq!(output.status, wire::ImportJobStatus::Unspecified as i32);
        assert_eq!(output.total_rows, 0);
        assert_eq!(output.checkpoint_row_position, 0);
        assert!(output.resource_version.is_some());
    }

    #[test]
    fn row_visibility_redacts_source_identifier_evidence() {
        let row = ImportRow::create(CreateImportRow {
            job_id: ImportJobId::try_new("import-job-visible-1").unwrap(),
            row_position: 1,
            external_row_key: Some("row-1".to_owned()),
            source_external_id: Some("legacy-customer-42".to_owned()),
            occurred_at_unix_nanos: 10,
        })
        .unwrap();
        let decision = QueryVisibilityDecision {
            resource_visible: true,
            allowed_fields: BTreeSet::from(["status".to_owned()]),
            decision_id: "decision-1".to_owned(),
            policy_version: "policy-1".to_owned(),
        };
        let output = import_row_to_wire_with_visibility(&row, &decision).unwrap();
        assert!(output.import_row_ref.is_some());
        assert!(output.external_row_key_sha256.is_empty());
        assert!(output.source_external_id_sha256.is_empty());
        assert_eq!(output.row_position, 0);
        assert_eq!(output.status, wire::ImportRowStatus::Pending as i32);
        assert!(output.resource_version.is_some());
    }

    #[test]
    fn row_cursor_filter_is_bound_to_job_and_status() {
        assert_ne!(
            row_filter_hash("job-a", wire::ImportRowStatus::Valid as i32),
            row_filter_hash("job-b", wire::ImportRowStatus::Valid as i32)
        );
        assert_ne!(
            row_filter_hash("job-a", wire::ImportRowStatus::Valid as i32),
            row_filter_hash("job-a", wire::ImportRowStatus::Invalid as i32)
        );
    }
}
