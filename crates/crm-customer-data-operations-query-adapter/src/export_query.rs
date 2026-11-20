use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::{
    PostgresDataStore, RecordGetQuery, RecordListQuery, RecordQueryContinuation, RecordQuerySort,
};
use crm_customer_data_operations::{PartyExportJob, PartyExportJobStatus};
use crm_customer_data_operations_capability_adapter::{
    EXPORT_JOB_RECORD_TYPE, MODULE_ID, export_job_from_snapshot, export_job_to_wire,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding, RecordId,
    RecordType, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::customer_data_operations::v1 as wire;
use crm_query_runtime::{
    CursorBinding, CursorCodec, CursorContinuation, PageSizePolicy, QueryRequest,
    QueryVisibilityAuthorizer, QueryVisibilityDecision, normalized_filter_hash,
};
use prost::Message;
use std::sync::Arc;

pub const GET_EXPORT_JOB_CAPABILITY: &str = "customer_data.export.party.get";
pub const LIST_EXPORT_JOBS_CAPABILITY: &str = "customer_data.export.party.list";

pub const GET_EXPORT_JOB_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.GetPartyExportJobRequest";
pub const GET_EXPORT_JOB_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.GetPartyExportJobResponse";
pub const LIST_EXPORT_JOBS_REQUEST_SCHEMA: &str =
    "crm.customer_data_operations.v1.ListPartyExportJobsRequest";
pub const LIST_EXPORT_JOBS_RESPONSE_SCHEMA: &str =
    "crm.customer_data_operations.v1.ListPartyExportJobsResponse";

pub const EXPORT_QUERY_CAPABILITY_IDS: [&str; 2] =
    [GET_EXPORT_JOB_CAPABILITY, LIST_EXPORT_JOBS_CAPABILITY];

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAXIMUM_PAGE_SIZE: u32 = 200;
const MAXIMUM_VISIBILITY_SCAN_RECORDS: usize = 10_000;

#[derive(Clone)]
pub struct PartyExportQueryAdapter {
    store: PostgresDataStore,
    cursor_codec: CursorCodec,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
    page_policy: PageSizePolicy,
}

impl std::fmt::Debug for PartyExportQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PartyExportQueryAdapter")
            .field("store", &self.store)
            .field("cursor_codec", &self.cursor_codec)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .field("page_policy", &self.page_policy)
            .finish()
    }
}

impl PartyExportQueryAdapter {
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

    pub fn validate_request(
        &self,
        capability_id: &str,
        request: &QueryRequest,
    ) -> Result<(), SdkError> {
        match capability_id {
            GET_EXPORT_JOB_CAPABILITY => {
                let command: wire::GetPartyExportJobRequest =
                    decode_input(request, GET_EXPORT_JOB_REQUEST_SCHEMA)?;
                let _ = export_job_record_id(command.export_job_ref)?;
            }
            LIST_EXPORT_JOBS_CAPABILITY => {
                let command: wire::ListPartyExportJobsRequest =
                    decode_input(request, LIST_EXPORT_JOBS_REQUEST_SCHEMA)?;
                validate_status_filter(command.status)?;
                let page_size = self
                    .page_policy
                    .resolve(command.page_size)
                    .map_err(cursor_error)?;
                let binding = cursor_binding(request, filter_hash(command.status), page_size)?;
                let _ = decode_after(self, &command.cursor, &binding)?;
            }
            _ => return Err(unsupported_query()),
        }
        Ok(())
    }

    pub async fn execute_request(
        &self,
        capability_id: &str,
        request: &QueryRequest,
    ) -> Result<TypedPayload, SdkError> {
        match capability_id {
            GET_EXPORT_JOB_CAPABILITY => self.execute_get_job(request).await,
            LIST_EXPORT_JOBS_CAPABILITY => self.execute_list_jobs(request).await,
            _ => Err(unsupported_query()),
        }
    }

    async fn execute_get_job(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::GetPartyExportJobRequest =
            decode_input(request, GET_EXPORT_JOB_REQUEST_SCHEMA)?;
        let job_id = export_job_record_id(command.export_job_ref)?;
        let snapshot = self.get_visible_job_snapshot(request, job_id).await?;
        let job = export_job_from_snapshot(&snapshot)?;
        let visibility = self
            .visibility
            .authorize_visibility(request, &snapshot.reference)
            .await?;
        support::protobuf_payload(
            MODULE_ID,
            GET_EXPORT_JOB_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::GetPartyExportJobResponse {
                export_job: Some(export_job_to_wire_with_visibility(&job, &visibility)?),
            },
        )
    }

    async fn execute_list_jobs(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::ListPartyExportJobsRequest =
            decode_input(request, LIST_EXPORT_JOBS_REQUEST_SCHEMA)?;
        validate_status_filter(command.status)?;
        let page_size = self
            .page_policy
            .resolve(command.page_size)
            .map_err(cursor_error)?;
        let binding = cursor_binding(request, filter_hash(command.status), page_size)?;
        let after = decode_after(self, &command.cursor, &binding)?;
        let (jobs, next) = self
            .collect_jobs(request, page_size, after, command.status)
            .await?;
        let next_cursor = encode_next(self, &binding, next.as_ref())?;
        support::protobuf_payload(
            MODULE_ID,
            LIST_EXPORT_JOBS_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::ListPartyExportJobsResponse {
                export_jobs: jobs,
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
                record_type: export_job_record_type()?,
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
    ) -> Result<(Vec<wire::PartyExportJob>, Option<RecordQueryContinuation>), SdkError> {
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
                    record_type: export_job_record_type()?,
                    page_size: u32::try_from(remaining).map_err(|_| scan_limit_error())?,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(scanned)?;
            for snapshot in &page.records {
                let job = export_job_from_snapshot(snapshot)?;
                if !job_matches_status(&job, status) {
                    continue;
                }
                let visibility = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if visibility.resource_visible {
                    output.push(export_job_to_wire_with_visibility(&job, &visibility)?);
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
                    record_type: export_job_record_type()?,
                    page_size: MAXIMUM_PAGE_SIZE,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            *scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(*scanned)?;
            for snapshot in &page.records {
                let job = export_job_from_snapshot(snapshot)?;
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
}

pub fn export_query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    EXPORT_QUERY_CAPABILITY_IDS
        .iter()
        .map(|capability_id| export_query_capability_definition(capability_id))
        .collect()
}

pub fn export_query_capability_definition(
    capability_id: &str,
) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema) = match capability_id {
        GET_EXPORT_JOB_CAPABILITY => (
            GET_EXPORT_JOB_REQUEST_SCHEMA,
            GET_EXPORT_JOB_RESPONSE_SCHEMA,
        ),
        LIST_EXPORT_JOBS_CAPABILITY => (
            LIST_EXPORT_JOBS_REQUEST_SCHEMA,
            LIST_EXPORT_JOBS_RESPONSE_SCHEMA,
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

fn export_job_to_wire_with_visibility(
    job: &PartyExportJob,
    visibility: &QueryVisibilityDecision,
) -> Result<wire::PartyExportJob, SdkError> {
    let mut output = export_job_to_wire(job)?;
    if !visibility.allows_field("specification") {
        output.specification = None;
        output.export_specification_version_id.clear();
    }
    if !visibility.allows_field("status") {
        output.status = wire::PartyExportJobStatus::Unspecified as i32;
    }
    if !visibility.allows_field("selection") {
        output.selection = None;
    }
    if !visibility.allows_field("checkpoint") {
        output.checkpoint_manifest_position = 0;
    }
    if !visibility.allows_field("execution") {
        output.execution_attempts = 0;
        output.last_execution_error_code.clear();
    }
    if !visibility.allows_field("artifact") {
        output.artifact = None;
    }
    if !visibility.allows_field("reconciliation") {
        output.reconciliation = None;
    }
    Ok(output)
}

fn job_matches_status(job: &PartyExportJob, status: i32) -> bool {
    match wire::PartyExportJobStatus::try_from(status).ok() {
        None | Some(wire::PartyExportJobStatus::Unspecified) => true,
        Some(wire::PartyExportJobStatus::Created) => job.status() == PartyExportJobStatus::Created,
        Some(wire::PartyExportJobStatus::Selecting) => {
            job.status() == PartyExportJobStatus::Selecting
        }
        Some(wire::PartyExportJobStatus::Ready) => job.status() == PartyExportJobStatus::Ready,
        Some(wire::PartyExportJobStatus::Executing) => {
            job.status() == PartyExportJobStatus::Executing
        }
        Some(wire::PartyExportJobStatus::Completed) => {
            job.status() == PartyExportJobStatus::Completed
        }
        Some(wire::PartyExportJobStatus::FailedRetryable) => {
            job.status() == PartyExportJobStatus::FailedRetryable
        }
        Some(wire::PartyExportJobStatus::Cancelled) => {
            job.status() == PartyExportJobStatus::Cancelled
        }
    }
}

fn validate_status_filter(status: i32) -> Result<(), SdkError> {
    if wire::PartyExportJobStatus::try_from(status).is_err() {
        return Err(SdkError::invalid_argument(
            "customer_data.export.status",
            "Export job status filter is invalid",
        ));
    }
    Ok(())
}

fn export_job_record_id(value: Option<wire::ExportJobRef>) -> Result<RecordId, SdkError> {
    let value = value.ok_or_else(|| {
        SdkError::invalid_argument(
            "customer_data.export_job_ref",
            "Export job reference is required",
        )
    })?;
    RecordId::try_new(value.export_job_id).map_err(|error| {
        SdkError::invalid_argument(
            "customer_data.export_job_ref.export_job_id",
            error.to_string(),
        )
    })
}

fn cursor_binding(
    request: &QueryRequest,
    filter_hash: [u8; 32],
    page_size: u32,
) -> Result<CursorBinding, SdkError> {
    Ok(CursorBinding {
        tenant_id: request.context.tenant_id.clone(),
        actor_id: Some(request.context.actor_id.clone()),
        capability_id: request.context.capability_id.clone(),
        capability_version: request.context.capability_version.clone(),
        resource_type: export_job_record_type()?,
        normalized_filter_hash: filter_hash,
        sort_id: RecordQuerySort::UpdatedAtDescending.id().to_owned(),
        page_size,
    })
}

fn filter_hash(status: i32) -> [u8; 32] {
    let status = status.to_be_bytes();
    normalized_filter_hash([("status", status.as_slice())])
}

fn decode_after(
    adapter: &PartyExportQueryAdapter,
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

fn encode_next(
    adapter: &PartyExportQueryAdapter,
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
            "CUSTOMER_DATA_EXPORT_QUERY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The customer-data export query input does not match the required contract.",
        ));
    }
    M::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "CUSTOMER_DATA_EXPORT_QUERY_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The customer-data export query input is not valid Protobuf.",
        )
    })
}

fn module_id() -> Result<ModuleId, SdkError> {
    ModuleId::try_new(MODULE_ID).map_err(config_error)
}

fn export_job_record_type() -> Result<RecordType, SdkError> {
    RecordType::try_new(EXPORT_JOB_RECORD_TYPE).map_err(config_error)
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
        "CUSTOMER_DATA_EXPORT_QUERY_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The customer-data export query capability is not configured.",
    )
}

fn cursor_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_QUERY_CURSOR_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The customer-data export page cursor is invalid.",
    )
}

fn cursor_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_QUERY_CURSOR_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The customer-data export page cursor is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn scan_limit_error() -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_QUERY_VISIBILITY_SCAN_LIMIT_EXCEEDED",
        ErrorCategory::Unavailable,
        true,
        "The customer-data export list is temporarily unavailable.",
    )
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CUSTOMER_DATA_EXPORT_QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The customer-data export query adapter is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_two_personal_read_only_export_queries() {
        let definitions = export_query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
        assert_eq!(
            definitions
                .iter()
                .map(|definition| definition.capability_id.as_str())
                .collect::<Vec<_>>(),
            EXPORT_QUERY_CAPABILITY_IDS
        );
        assert!(definitions.iter().all(|definition| !definition.mutation));
        assert!(
            definitions
                .iter()
                .all(|definition| definition.input_contract.allowed_data_classes
                    == vec![DataClass::Personal])
        );
    }
}
