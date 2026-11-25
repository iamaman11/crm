//! Generated Connect trait for the existing AccountV2Service Rust domain owner.
//! Access, tenant, and the single native router are defined by the parent.
use super::*;
use crate::diagnostics;

impl account_wire::AccountService for AccountV2Service {
    #[worker::send]
    async fn create_account(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, account_wire::CreateAccountRequest>,
    ) -> ServiceResult<account_wire::CreateAccountResponse> {
        let input = req.to_owned_message();
        let id = Self::valid_id(&input.account_id)?;
        let (key, _) = wire_precondition(input.precondition.as_option(), true)?;
        let saved = accounts::create_typed(
            accounts::CreateInput {
                id,
                name: input.name,
                party_id: input.primary_party_id,
            },
            &key,
            &self.d1()?,
            &self.actor,
            &self.request_id,
        )
        .await
        .map_err(|failure| {
            diagnostics::mutation_error("CreateAccount", &self.request_id, failure)
        })?;
        RpcResponse::ok(account_wire::CreateAccountResponse {
            account: Some(Self::convert(&saved.account)?).into(),
            receipt: Some(self.receipt(&saved)?).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn update_account(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, account_wire::UpdateAccountRequest>,
    ) -> ServiceResult<account_wire::UpdateAccountResponse> {
        let input = req.to_owned_message();
        let id = Self::valid_id(&input.account_id)?;
        let (key, version) = wire_precondition(input.precondition.as_option(), false)?;
        let status = match input.status {
            x if x == account_wire::AccountStatus::ACCOUNT_STATUS_ACTIVE => {
                accounts::AccountStatusInput::Active
            }
            x if x == account_wire::AccountStatus::ACCOUNT_STATUS_INACTIVE => {
                accounts::AccountStatusInput::Inactive
            }
            _ => return Err(ConnectError::invalid_argument("invalid AccountStatus")),
        };
        let saved = accounts::update_typed(
            accounts::UpdateInput {
                name: input.name,
                status,
                expected_version: version.unwrap_or_default(),
            },
            &key,
            &self.d1()?,
            &self.actor,
            &id,
            &self.request_id,
        )
        .await
        .map_err(|failure| {
            diagnostics::mutation_error("UpdateAccount", &self.request_id, failure)
        })?;
        RpcResponse::ok(account_wire::UpdateAccountResponse {
            account: Some(Self::convert(&saved.account)?).into(),
            receipt: Some(self.receipt(&saved)?).into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn get_account(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, account_wire::GetAccountRequest>,
    ) -> ServiceResult<account_wire::GetAccountResponse> {
        let input = req.to_owned_message();
        let id = Self::valid_id(&input.account_id)?;
        let db = self.d1()?;
        let item = accounts::load(&db, &self.actor.tenant, &id)
            .await
            .map_err(|error| crate::diagnostics::read_error(&self.request_id, &error))?
            .ok_or_else(|| ConnectError::not_found("CUSTOMER_ACCOUNTS_NOT_FOUND"))?;
        let confirmed_mutation = match receipt_key(input.receipt_idempotency_key.as_deref())? {
            None => None,
            Some(key) => {
                let stored = crud_receipt(&db, &self.actor.tenant, key, CrudReceiptOwner::Account)
                    .await
                    .map_err(|_| ConnectError::unavailable("ACCOUNT_RECEIPT_READ_FAILED"))?;
                crud_commit_evidence(stored, &id, &self.request_id, |state| {
                    crm_customer_accounts::decode_account_state(state.as_bytes())
                        .map(|row| (row.account_id().as_str().to_owned(), row.version()))
                        .map_err(|e| worker::Error::RustError(e.code))
                })
                .map_err(|_| ConnectError::unavailable("ACCOUNT_RECEIPT_DECODE_FAILED"))?
            }
        };
        RpcResponse::ok(account_wire::GetAccountResponse {
            account: Some(Self::convert(&item)?).into(),
            confirmed_mutation: confirmed_mutation.into(),
            ..Default::default()
        })
    }
    #[worker::send]
    async fn list_accounts(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, account_wire::ListAccountsRequest>,
    ) -> ServiceResult<account_wire::ListAccountsResponse> {
        let size = if req.page.page_size == 0 {
            25
        } else {
            req.page.page_size
        };
        if size > 100 {
            return Err(ConnectError::invalid_argument("page_size exceeds 100"));
        }
        let after = &req.page.page_token;
        if !after.is_empty() {
            Self::valid_id(after)?;
        }
        let status = match req.status {
            None => None,
            Some(x) if x == account_wire::AccountStatus::ACCOUNT_STATUS_ACTIVE => {
                Some(DomainAccountStatus::Active)
            }
            Some(x) if x == account_wire::AccountStatus::ACCOUNT_STATUS_INACTIVE => {
                Some(DomainAccountStatus::Inactive)
            }
            _ => {
                return Err(ConnectError::invalid_argument(
                    "invalid AccountStatus filter",
                ));
            }
        };
        let (list, next) = accounts::list_page_typed(
            &self.d1()?,
            &self.actor.tenant,
            after,
            size as usize,
            status,
            wire_search(req.search)?,
        )
        .await
        .map_err(|error| crate::diagnostics::read_error(&self.request_id, &error))?;
        let mut result = Vec::new();
        for account in &list {
            result.push(Self::convert(account)?);
        }
        RpcResponse::ok(account_wire::ListAccountsResponse {
            accounts: result,
            page_info: Some(core_v2::PageInfo {
                next_page_token: next.unwrap_or_default(),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        })
    }
}
