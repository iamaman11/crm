#![forbid(unsafe_code)]

use crm_capability_plan_support as support;
use crm_capability_runtime::{CapabilityDefinition, CapabilityRisk};
use crm_core_data::{
    PostgresDataStore, RecordGetQuery, RecordListQuery, RecordQueryContinuation, RecordQuerySort,
};
use crm_customer_accounts::{Account, AccountStatus};
use crm_customer_accounts_capability_adapter::{
    MODULE_ID, RECORD_TYPE, account_from_snapshot, account_to_wire,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PayloadEncoding,
    PortFuture, RecordId, RecordType, SdkError, TypedPayload,
};
use crm_proto_contracts::crm::{accounts::v1 as wire, core::v1 as core};
use crm_query_runtime::{
    CursorBinding, CursorCodec, CursorContinuation, PageSizePolicy, QueryExecutionResult,
    QueryExecutor, QueryRequest, QuerySemanticValidator, QueryVisibilityAuthorizer,
    QueryVisibilityDecision, normalized_filter_hash,
};
use prost::Message;
use std::sync::Arc;

pub const GET_CAPABILITY: &str = "accounts.account.get";
pub const LIST_CAPABILITY: &str = "accounts.account.list";
pub const GET_REQUEST_SCHEMA: &str = "crm.accounts.v1.GetAccountRequest";
pub const GET_RESPONSE_SCHEMA: &str = "crm.accounts.v1.GetAccountResponse";
pub const LIST_REQUEST_SCHEMA: &str = "crm.accounts.v1.ListAccountsRequest";
pub const LIST_RESPONSE_SCHEMA: &str = "crm.accounts.v1.ListAccountsResponse";
pub const QUERY_CAPABILITY_IDS: [&str; 2] = [GET_CAPABILITY, LIST_CAPABILITY];

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAXIMUM_PAGE_SIZE: u32 = 200;
const MAXIMUM_VISIBILITY_SCAN_RECORDS: usize = 10_000;

#[derive(Clone)]
pub struct AccountQueryAdapter {
    store: PostgresDataStore,
    cursor_codec: CursorCodec,
    visibility: Arc<dyn QueryVisibilityAuthorizer>,
    page_policy: PageSizePolicy,
}

impl std::fmt::Debug for AccountQueryAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AccountQueryAdapter")
            .field("store", &self.store)
            .field("cursor_codec", &self.cursor_codec)
            .field("visibility", &"dyn QueryVisibilityAuthorizer")
            .field("page_policy", &self.page_policy)
            .finish()
    }
}

impl AccountQueryAdapter {
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

pub fn query_capability_definitions() -> Result<Vec<CapabilityDefinition>, SdkError> {
    QUERY_CAPABILITY_IDS
        .iter()
        .map(|capability_id| query_capability_definition(capability_id))
        .collect()
}

pub fn query_capability_definition(capability_id: &str) -> Result<CapabilityDefinition, SdkError> {
    let (input_schema, output_schema) = match capability_id {
        GET_CAPABILITY => (GET_REQUEST_SCHEMA, GET_RESPONSE_SCHEMA),
        LIST_CAPABILITY => (LIST_REQUEST_SCHEMA, LIST_RESPONSE_SCHEMA),
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

impl QuerySemanticValidator for AccountQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            match definition.capability_id.as_str() {
                GET_CAPABILITY => {
                    let command: wire::GetAccountRequest =
                        decode_input(request, GET_REQUEST_SCHEMA)?;
                    let account_ref = command.account_ref.ok_or_else(|| {
                        SdkError::invalid_argument(
                            "account.account_ref",
                            "Account reference is required",
                        )
                    })?;
                    validate_record_id(&account_ref.account_id)?;
                }
                LIST_CAPABILITY => {
                    let command: wire::ListAccountsRequest =
                        decode_input(request, LIST_REQUEST_SCHEMA)?;
                    validate_list(self, request, &command)?;
                }
                _ => return Err(unsupported_query()),
            }
            Ok(())
        })
    }
}

impl QueryExecutor for AccountQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            let output = match definition.capability_id.as_str() {
                GET_CAPABILITY => self.execute_get(&request).await?,
                LIST_CAPABILITY => self.execute_list(&request).await?,
                _ => return Err(unsupported_query()),
            };
            Ok(QueryExecutionResult { output })
        })
    }
}

impl AccountQueryAdapter {
    async fn execute_get(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::GetAccountRequest = decode_input(request, GET_REQUEST_SCHEMA)?;
        let account_ref = command.account_ref.ok_or_else(|| {
            SdkError::invalid_argument("account.account_ref", "Account reference is required")
        })?;
        let record_id = validate_record_id(&account_ref.account_id)?;
        let snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.tenant_id.clone(),
                owner_module_id: ModuleId::try_new(MODULE_ID).map_err(config_error)?,
                record_type: account_record_type()?,
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
        let account = account_from_snapshot(&snapshot)?;

        support::protobuf_payload(
            MODULE_ID,
            GET_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::GetAccountResponse {
                account: Some(account_to_wire_with_visibility(&account, &visibility)),
            },
        )
    }

    async fn execute_list(&self, request: &QueryRequest) -> Result<TypedPayload, SdkError> {
        let command: wire::ListAccountsRequest = decode_input(request, LIST_REQUEST_SCHEMA)?;
        let page_size = resolve_page_size(self.page_policy, command.page.as_ref())?;
        let filter_hash = account_filter_hash(&command);
        let binding = cursor_binding(
            request,
            account_record_type()?,
            filter_hash,
            RecordQuerySort::UpdatedAtDescending,
            page_size,
        );
        let after = decode_after(self, command.page.as_ref(), &binding)?;
        let (accounts, next) = self
            .collect_accounts(request, page_size, after, command.status)
            .await?;
        let next_page_token = encode_next(self, &binding, next.as_ref())?;

        support::protobuf_payload(
            MODULE_ID,
            LIST_RESPONSE_SCHEMA,
            DataClass::Personal,
            &wire::ListAccountsResponse {
                accounts,
                page: Some(core::PageInfo {
                    next_page_token,
                    total_size: 0,
                }),
            },
        )
    }

    async fn collect_accounts(
        &self,
        request: &QueryRequest,
        page_size: u32,
        mut after: Option<RecordQueryContinuation>,
        status: Option<i32>,
    ) -> Result<(Vec<wire::Account>, Option<RecordQueryContinuation>), SdkError> {
        let mut output = Vec::with_capacity(page_size as usize);
        let mut scanned = 0_usize;
        loop {
            let remaining = page_size as usize - output.len();
            if remaining == 0 {
                let anchor = after.clone();
                let has_more = self
                    .has_more_visible_account(request, anchor.clone(), status, &mut scanned)
                    .await?;
                return Ok((output, has_more.then_some(anchor).flatten()));
            }

            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: ModuleId::try_new(MODULE_ID).map_err(config_error)?,
                    record_type: account_record_type()?,
                    page_size: u32::try_from(remaining).map_err(|_| scan_limit_error())?,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(scanned)?;
            for snapshot in &page.records {
                let account = account_from_snapshot(snapshot)?;
                if !account_matches_status(&account, status) {
                    continue;
                }
                let visibility = self
                    .visibility
                    .authorize_visibility(request, &snapshot.reference)
                    .await?;
                if visibility.resource_visible {
                    output.push(account_to_wire_with_visibility(&account, &visibility));
                }
            }
            after = page.next;
            if after.is_none() {
                return Ok((output, None));
            }
        }
    }

    async fn has_more_visible_account(
        &self,
        request: &QueryRequest,
        mut after: Option<RecordQueryContinuation>,
        status: Option<i32>,
        scanned: &mut usize,
    ) -> Result<bool, SdkError> {
        while after.is_some() {
            let page = self
                .store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: ModuleId::try_new(MODULE_ID).map_err(config_error)?,
                    record_type: account_record_type()?,
                    page_size: MAXIMUM_PAGE_SIZE,
                    sort: RecordQuerySort::UpdatedAtDescending,
                    after: after.clone(),
                })
                .await?;
            *scanned = scanned.saturating_add(page.records.len());
            enforce_scan_limit(*scanned)?;
            for snapshot in &page.records {
                let account = account_from_snapshot(snapshot)?;
                if account_matches_status(&account, status)
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

fn validate_list(
    adapter: &AccountQueryAdapter,
    request: &QueryRequest,
    command: &wire::ListAccountsRequest,
) -> Result<(), SdkError> {
    if let Some(status) = command.status {
        match wire::AccountStatus::try_from(status).ok() {
            Some(wire::AccountStatus::Active | wire::AccountStatus::Inactive) => {}
            _ => {
                return Err(SdkError::invalid_argument(
                    "account.status",
                    "Account status filter must be ACTIVE or INACTIVE",
                ));
            }
        }
    }
    match wire::AccountSort::try_from(command.sort).ok() {
        Some(wire::AccountSort::Unspecified | wire::AccountSort::UpdatedAtDescending) => {}
        None => {
            return Err(SdkError::invalid_argument(
                "account.sort",
                "Account sort is invalid",
            ));
        }
    }

    let page_size = resolve_page_size(adapter.page_policy, command.page.as_ref())?;
    let binding = cursor_binding(
        request,
        account_record_type()?,
        account_filter_hash(command),
        RecordQuerySort::UpdatedAtDescending,
        page_size,
    );
    let _ = decode_after(adapter, command.page.as_ref(), &binding)?;
    Ok(())
}

fn account_matches_status(account: &Account, status: Option<i32>) -> bool {
    match status.and_then(|value| wire::AccountStatus::try_from(value).ok()) {
        None => true,
        Some(wire::AccountStatus::Active) => account.status() == AccountStatus::Active,
        Some(wire::AccountStatus::Inactive) => account.status() == AccountStatus::Inactive,
        Some(wire::AccountStatus::Unspecified) => false,
    }
}

fn account_to_wire_with_visibility(
    account: &Account,
    visibility: &QueryVisibilityDecision,
) -> wire::Account {
    let mut output = account_to_wire(account);
    if !visibility.allows_field("name") {
        output.name.clear();
    }
    if !visibility.allows_field("status") {
        output.status = wire::AccountStatus::Unspecified as i32;
    }
    if !visibility.allows_field("party_associations") {
        output.party_associations.clear();
    }
    output
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
    resource_type: RecordType,
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
    adapter: &AccountQueryAdapter,
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
            "CUSTOMER_ACCOUNTS_QUERY_CURSOR_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Account page cursor is invalid.",
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
    adapter: &AccountQueryAdapter,
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

fn account_filter_hash(command: &wire::ListAccountsRequest) -> [u8; 32] {
    let status = command.status.unwrap_or_default().to_be_bytes();
    normalized_filter_hash([("status", status.as_slice())])
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
            "CUSTOMER_ACCOUNTS_QUERY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The Account query input does not match the required contract.",
        ));
    }
    M::decode(payload.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "CUSTOMER_ACCOUNTS_QUERY_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The Account query input is not valid Protobuf.",
        )
    })
}

fn validate_record_id(value: &str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value.to_owned()).map_err(|error| {
        SdkError::invalid_argument("account.account_ref.account_id", error.to_string())
    })
}

fn account_record_type() -> Result<RecordType, SdkError> {
    RecordType::try_new(RECORD_TYPE).map_err(config_error)
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
        "CUSTOMER_ACCOUNTS_QUERY_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Account query capability is not configured.",
    )
}

fn cursor_error(error: impl std::fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_ACCOUNTS_QUERY_CURSOR_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Account page cursor is invalid.",
    )
    .with_internal_reference(error.to_string())
}

fn scan_limit_error() -> SdkError {
    SdkError::new(
        "CUSTOMER_ACCOUNTS_QUERY_VISIBILITY_SCAN_LIMIT_EXCEEDED",
        ErrorCategory::Unavailable,
        true,
        "The Account list is temporarily unavailable.",
    )
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "CUSTOMER_ACCOUNTS_QUERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Account query adapter is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_accounts::{
        AccountId, AccountPartyAssociation, AccountPartyRole, CreateAccount, PartyReference,
    };
    use std::collections::BTreeSet;

    fn account() -> Account {
        Account::create(CreateAccount {
            account_id: AccountId::try_new("account-visible-1").unwrap(),
            name: "Northwind".to_owned(),
            party_associations: vec![AccountPartyAssociation::new(
                PartyReference::try_new("party-org-northwind").unwrap(),
                AccountPartyRole::Primary,
            )],
            occurred_at_unix_nanos: 10,
        })
        .unwrap()
    }

    #[test]
    fn publishes_get_and_list_as_personal_read_only_queries() {
        let definitions = query_capability_definitions().unwrap();
        assert_eq!(definitions.len(), 2);
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
    fn field_visibility_redacts_relationship_data_without_hiding_resource_identity() {
        let value = account();
        let decision = QueryVisibilityDecision {
            resource_visible: true,
            allowed_fields: BTreeSet::from(["name".to_owned()]),
            decision_id: "decision-1".to_owned(),
            policy_version: "policy-1".to_owned(),
        };

        let output = account_to_wire_with_visibility(&value, &decision);
        assert_eq!(output.account_ref.unwrap().account_id, "account-visible-1");
        assert_eq!(output.name, "Northwind");
        assert_eq!(output.status, wire::AccountStatus::Unspecified as i32);
        assert!(output.party_associations.is_empty());
        assert_eq!(output.resource_version.unwrap().version, 1);
    }

    #[test]
    fn status_filter_is_exact_and_cursor_bound() {
        let value = account();
        assert!(account_matches_status(
            &value,
            Some(wire::AccountStatus::Active as i32)
        ));
        assert!(!account_matches_status(
            &value,
            Some(wire::AccountStatus::Inactive as i32)
        ));

        let active_request = wire::ListAccountsRequest {
            page: None,
            status: Some(wire::AccountStatus::Active as i32),
            sort: wire::AccountSort::UpdatedAtDescending as i32,
        };
        let inactive_request = wire::ListAccountsRequest {
            status: Some(wire::AccountStatus::Inactive as i32),
            ..active_request.clone()
        };
        assert_ne!(
            account_filter_hash(&active_request),
            account_filter_hash(&inactive_request)
        );
    }
}
