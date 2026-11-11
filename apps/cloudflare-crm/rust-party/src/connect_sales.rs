//! One native Connect transport for the published crm.sales.v1.DealService.
//! Cloudflare Access and tenant selection happen BEFORE routing here.
//! Domain rules and storage belong exclusively to existing crm-sales and D1.
use std::sync::Arc;
use axum::Router;
use connectrpc::{
    ConnectError, RequestContext, Response as RpcResponse, Router as ConnectRouter,
    ServiceRequest, ServiceResult,
};
use tower_service::Service;
use worker::{D1Database, Env, HttpRequest, Request, Response, Result};
use buffa::Message as BuffaMessage;
use prost::Message as ProstMessage;
use serde::Deserialize;
use wasm_bindgen::JsValue;
use super::{accounts, bind, parties, sales_contract, sales_deals};
use crm_parties::PartyKind as DomainPartyKind;
use crm_customer_accounts::{Account as DomainAccount, AccountPartyRole as DomainAccountPartyRole, AccountStatus as DomainAccountStatus};
use crm_proto_contracts::crm::{core::v1 as core_contract, sales::v1 as sales_contract_proto};

// The published crm.core.v1 package shadows Rust's 'core' crate name.
// Mount the UNMODIFIED generated Buffa package stitchers and expose the
// standard library fmt at the enclosing package. No runtime adapter/codegen
// rewrite is required; all message and service code remains generated.
pub mod proto {
    #[allow(unused_imports)]
    pub mod crm {
        use super::*;
        pub mod core {
            use super::*;
            pub use ::core::fmt;
            pub mod v1 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.core.v1.mod.rs"));
            }
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.core.v2.mod.rs"));
            }
        }
                pub mod sales {
            use super::*;
            pub mod v1 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.sales.v1.mod.rs"));
            }
        }
        pub mod accounts {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.accounts.v2.mod.rs"));
            }
        }
        pub mod parties {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.parties.v2.mod.rs"));
            }
            pub mod v3 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.parties.v3.mod.rs"));
            }
        }
    }
}
use proto::crm::sales::v1 as sales;
use proto::crm::parties::v2 as party_wire;
use proto::crm::parties::v3 as party_v3;
use proto::crm::core::v2 as core_v2;
use proto::crm::accounts::v2 as account_wire;

#[derive(Deserialize)]
struct Row { state: String }

struct SalesService {
    env: Arc<Env>,
    tenant: String,
    actor: String,
    request_id: String,
    idempotency_key: Option<String>,
}

impl SalesService {
    async fn load(&self,id:&str)->std::result::Result<Option<crm_sales::Deal>, ConnectError> {
        let db=self.env.d1("DB").map_err(|_|ConnectError::internal("D1 binding unavailable"))?;
        let row=bind(&db, "SELECT state FROM crm_sales_deal_records WHERE tenant=? AND id=?",
            &[JsValue::from_str(&self.tenant),JsValue::from_str(id)])
            .map_err(|_|ConnectError::internal("D1 binding failed"))?
            .first::<Row>(None).await.map_err(|_|ConnectError::internal("D1 query failed"))?;
        row.map(|x|sales_deals::decode(&x.state)
            .map_err(|_|ConnectError::internal("invalid persisted Deal"))).transpose()
    }
    fn protobuf(&self,deal:&crm_sales::Deal)->std::result::Result<sales::Deal, ConnectError> {
        let existing=sales_contract::deal_view(deal,&self.tenant);
        let bytes=existing.encode_to_vec();
        sales::Deal::decode_from_slice(&bytes)
            .map_err(|_|ConnectError::internal("generated Deal contract mismatch"))
    }
    // Only the canonical existing request fields are supported. Do not silently
    // discard extra RPC data until corresponding owner commands are exposed.
    fn create_input(&self,wire:sales::CreateDealRequest)
        -> std::result::Result<sales_deals::CreateInput, ConnectError> {
        // A wire-compatible conversion between the two existing, version-pinned
        // Rust protobuf generators. This does not create a JSON domain model.
        let encoded=wire.encode_to_vec();
        let request=sales_contract_proto::CreateDealRequest::decode(encoded.as_slice())
            .map_err(|_|ConnectError::invalid_argument("invalid CreateDeal protobuf"))?;
        if let Some(owner)=request.owner {
            match owner.owner {
                Some(core_contract::actor_or_team_owner::Owner::ActorId(id))
                    if id==self.actor=>{},
                _=>return Err(ConnectError::permission_denied("verified actor required")),
            }
        }
        let account=request.account
            .ok_or_else(||ConnectError::invalid_argument("account required"))?;
        if account.tenant_id!=self.tenant ||
            account.resource_type!="crm.customer-accounts" ||
            account.version.is_some() {
            return Err(ConnectError::invalid_argument("scoped Account reference required"));
        }
        if request.primary_contact.is_some()||request.expected_close_date.is_some() {
            return Err(ConnectError::unimplemented("contact and close date not connected"));
        }
        let stage=request.stage
            .ok_or_else(||ConnectError::invalid_argument("stage required"))?;
        let ordinal=u16::try_from(stage.ordinal)
            .map_err(|_|ConnectError::invalid_argument("stage ordinal out of range"))?;
        let probability_bps=u16::try_from(request.probability_basis_points)
            .map_err(|_|ConnectError::invalid_argument("probability out of range"))?;
        let (amount_minor_units,currency)=match request.amount {
            None=>(None,None),
            Some(money)=>{
                let units=money.minor_units.parse::<i64>()
                    .map_err(|_|ConnectError::invalid_argument("money minor units out of range"))?;
                (Some(units),Some(money.currency_code))
            }
        };
        Ok(sales_deals::CreateInput {
            id:request.deal_id,name:request.name,account_id:account.resource_id,
            pipeline_id:stage.pipeline_id,stage_id:stage.stage_id,ordinal,
            probability_bps,amount_minor_units,currency,
        })
    }

}

impl sales::DealService for SalesService {
    #[worker::send]
    async fn get_deal(
        &self,_ctx:RequestContext,req:ServiceRequest<'_, sales::GetDealRequest>,
    )->ServiceResult<sales::GetDealResponse> {
        let id=req.deal_id.trim();
        if id.is_empty()||id.len()>180||id.contains('/') {
            return Err(ConnectError::invalid_argument("invalid deal_id"));
        }
        let deal=self.load(id).await?
            .ok_or_else(||ConnectError::not_found("deal not found"))?;
        RpcResponse::ok(sales::GetDealResponse{
            deal:Some(self.protobuf(&deal)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn list_deals(
        &self,_ctx:RequestContext,req:ServiceRequest<'_, sales::ListDealsRequest>,
    )->ServiceResult<sales::ListDealsResponse> {
        // An implementation of filters/pagination is required before accepting
        // those arguments: never silently return an unfiltered result.
        if req.to_owned_message()!=sales::ListDealsRequest::default() {
            return Err(ConnectError::unimplemented("filtered or paged list is not connected"));
        }
        // The existing native Sales list has a hard limit of 300, identical
        // tenant filter and ordering. Advanced filtering is not exposed yet.
        let db=self.env.d1("DB")
            .map_err(|_|ConnectError::internal("D1 binding unavailable"))?;
        let rows=bind(&db,
            "SELECT state FROM crm_sales_deal_records WHERE tenant=? ORDER BY id LIMIT 300",
            &[JsValue::from_str(&self.tenant)])
            .map_err(|_|ConnectError::internal("D1 binding failed"))?
            .all().await.map_err(|_|ConnectError::internal("D1 query failed"))?
            .results::<Row>().map_err(|_|ConnectError::internal("D1 row decode failed"))?;
        let mut deals=Vec::with_capacity(rows.len());
        for row in &rows {
            let deal=sales_deals::decode(&row.state)
                .map_err(|_|ConnectError::internal("invalid persisted Deal"))?;
            deals.push(self.protobuf(&deal)?);
        }
        RpcResponse::ok(sales::ListDealsResponse{
            deals,..Default::default()
        })
    }
    #[worker::send]
    async fn change_stage(
        &self,_ctx:RequestContext,_req:ServiceRequest<'_,sales::ChangeDealStageRequest>,
    )->ServiceResult<sales::ChangeDealStageResponse> {
        Err(ConnectError::unimplemented("Use AdvanceStage with optimistic version"))
    }
    #[worker::send]
    async fn create_deal(
        &self,_ctx:RequestContext,req:ServiceRequest<'_,sales::CreateDealRequest>,
    )->ServiceResult<sales::CreateDealResponse> {
        let key=self.idempotency_key.as_deref()
            .filter(|key|(8..=120).contains(&key.len()))
            .ok_or_else(||ConnectError::invalid_argument("IDEMPOTENCY_KEY_REQUIRED"))?;
        let input=self.create_input(req.to_owned_message())?;
        let db=self.env.d1("DB")
            .map_err(|_|ConnectError::internal("D1 binding unavailable"))?;
        let saved=sales_deals::create_typed(input,key,&db,&self.tenant,&self.actor,
            &self.request_id).await
            .map_err(|_|ConnectError::internal("Sales create D1 unavailable"))?
            .map_err(|failure|match failure.status {
                409=>ConnectError::aborted(failure.code),
                422=>ConnectError::invalid_argument(failure.code),
                _=>ConnectError::internal(failure.code),
            })?;
        RpcResponse::ok(sales::CreateDealResponse{
            deal:Some(self.protobuf(&saved.deal)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn update_deal(
        &self,_ctx:RequestContext,_req:ServiceRequest<'_,sales::UpdateDealRequest>,
    )->ServiceResult<sales::UpdateDealResponse> {
        Err(ConnectError::unimplemented("UpdateDeal pending native typed mutation gate"))
    }
    #[worker::send]
    async fn advance_stage(
        &self,_ctx:RequestContext,_req:ServiceRequest<'_,sales::AdvanceStageRequest>,
    )->ServiceResult<sales::AdvanceStageResponse> {
        Err(ConnectError::unimplemented("AdvanceStage pending linked D1 transaction gate"))
    }
}


struct PartyService {
    env: Arc<Env>,
    tenant: String,
}

// Generated Connect-Rust + Buffa response, original Party owner + one D1.
impl party_wire::PartyService for PartyService {
    #[worker::send]
    async fn get_party(
        &self, _ctx: RequestContext, req: ServiceRequest<'_, party_wire::GetPartyRequest>,
    ) -> ServiceResult<party_wire::GetPartyResponse> {
        let raw = req.party_id.trim();
        if raw.is_empty() || raw.len() > 180 || raw != req.party_id || raw.contains('/') {
            return Err(ConnectError::invalid_argument("PARTIES_PARTY_ID_INVALID"));
        }
        let id = crm_parties::PartyId::try_new(raw.to_owned())
            .map_err(|_| ConnectError::invalid_argument("PARTIES_PARTY_ID_INVALID"))?;
        let db = self.env.d1("DB")
            .map_err(|_| ConnectError::internal("D1 binding unavailable"))?;
        let party = parties::stored(&db, &self.tenant, id.as_str()).await
            .map_err(|_| ConnectError::internal("Party D1 read failed"))?
            .ok_or_else(|| ConnectError::not_found("PARTIES_NOT_FOUND"))?;
        let version = i64::try_from(party.version())
            .map_err(|_| ConnectError::internal("invalid Party version"))?;
        let kind = match party.kind() {
            DomainPartyKind::Person => party_wire::PartyKind::PARTY_KIND_PERSON,
            DomainPartyKind::Organization => party_wire::PartyKind::PARTY_KIND_ORGANIZATION,
        };
        RpcResponse::ok(party_wire::GetPartyResponse {
            party: Some(party_wire::Party {
                party_id: party.party_id().as_str().to_owned(),
                kind: kind.into(),
                display_name: party.display_name().to_owned(),
                version,
                ..Default::default()
            }).into(),
            ..Default::default()
        })
    }
}

// Cutover Party v3: generated Connect wire only. The historical Party owner
// in parties.rs remains the SOLE state codec, D1 mutation and receipt owner.
// This service is exposed to signed native-ci workerd only until full cutover.
struct PartyV3Service {
    env: Arc<Env>,
    tenant: String,
    request_id: String,
}
impl PartyV3Service {
    fn d1(&self) -> std::result::Result<D1Database, ConnectError> {
        self.env.d1("DB").map_err(|_|ConnectError::internal("D1 binding unavailable"))
    }
    fn valid_id(raw: &str) -> std::result::Result<crm_parties::PartyId, ConnectError> {
        if raw != raw.trim() || raw.contains('/') {
            return Err(ConnectError::invalid_argument("PARTIES_PARTY_ID_INVALID"));
        }
        crm_parties::PartyId::try_new(raw.to_owned())
            .map_err(|_|ConnectError::invalid_argument("PARTIES_PARTY_ID_INVALID"))
    }
    fn convert(party: &crm_parties::Party)
        -> std::result::Result<party_v3::Party,ConnectError> {
        let version=i64::try_from(party.version())
            .map_err(|_|ConnectError::internal("invalid Party version"))?;
        let kind=match party.kind(){
            DomainPartyKind::Person=>party_v3::PartyKind::PARTY_KIND_PERSON,
            DomainPartyKind::Organization=>party_v3::PartyKind::PARTY_KIND_ORGANIZATION,
        };
        Ok(party_v3::Party {
            party_id:party.party_id().as_str().to_owned(),
            kind:kind.into(),display_name:party.display_name().to_owned(),
            version,..Default::default()
        })
    }
    fn receipt(&self, outcome:&parties::MutationOutcome)
        -> std::result::Result<core_v2::MutationReceipt,ConnectError> {
        Ok(core_v2::MutationReceipt {
            request_id:self.request_id.clone(),
            resource_version:i64::try_from(outcome.party.version())
                .map_err(|_|ConnectError::internal("invalid Party version"))?,
            replayed:outcome.replayed,..Default::default()
        })
    }
    fn mutation_error(f: parties::MutationFailure) -> ConnectError {
        match f.status {
            404=>ConnectError::not_found(f.code),
            409=>ConnectError::aborted(f.code),
            422=>ConnectError::invalid_argument(f.code),
            _=>ConnectError::internal(f.code),
        }
    }
}
impl party_v3::PartyService for PartyV3Service {
    #[worker::send]
    async fn create_party(
        &self,_ctx:RequestContext,req:ServiceRequest<'_,party_v3::CreatePartyRequest>,
    )->ServiceResult<party_v3::CreatePartyResponse>{
        let input=req.to_owned_message();
        let id=Self::valid_id(&input.party_id)?;
        let kind=match input.kind {
            x if x==party_v3::PartyKind::PARTY_KIND_PERSON=>DomainPartyKind::Person,
            x if x==party_v3::PartyKind::PARTY_KIND_ORGANIZATION=>DomainPartyKind::Organization,
            _=>return Err(ConnectError::invalid_argument("INVALID_KIND")),
        };
        let pre=input.precondition.as_option()
            .ok_or_else(||ConnectError::invalid_argument("mutation precondition required"))?;
        if pre.expected_version.is_some() {
            return Err(ConnectError::invalid_argument("create must not specify expected_version"));
        }
        let saved=parties::create_typed(&self.d1()?,&self.tenant,
            id.as_str().to_owned(),kind,input.display_name,
            &pre.idempotency_key,&self.request_id).await
            .map_err(|_|ConnectError::internal("Party D1 create failed"))?
            .map_err(Self::mutation_error)?;
        RpcResponse::ok(party_v3::CreatePartyResponse {
            party:Some(Self::convert(&saved.party)?).into(),
            receipt:Some(self.receipt(&saved)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn update_party(
        &self,_ctx:RequestContext,req:ServiceRequest<'_,party_v3::UpdatePartyRequest>,
    )->ServiceResult<party_v3::UpdatePartyResponse>{
        let input=req.to_owned_message();
        let id=Self::valid_id(&input.party_id)?;
        let pre=input.precondition.as_option()
            .ok_or_else(||ConnectError::invalid_argument("mutation precondition required"))?;
        let version=pre.expected_version
            .ok_or_else(||ConnectError::invalid_argument("expected_version required"))?;
        if version<1 {return Err(ConnectError::invalid_argument("invalid expected_version"));}
        let saved=parties::update_typed(&self.d1()?,&self.tenant,
            id.as_str(),input.display_name,version,&pre.idempotency_key,
            &self.request_id).await
            .map_err(|_|ConnectError::internal("Party D1 update failed"))?
            .map_err(Self::mutation_error)?;
        RpcResponse::ok(party_v3::UpdatePartyResponse {
            party:Some(Self::convert(&saved.party)?).into(),
            receipt:Some(self.receipt(&saved)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn get_party(
        &self,_ctx:RequestContext,req:ServiceRequest<'_,party_v3::GetPartyRequest>,
    )->ServiceResult<party_v3::GetPartyResponse>{
        let id=Self::valid_id(&req.party_id)?;
        let party=parties::stored(&self.d1()?,&self.tenant,id.as_str()).await
            .map_err(|_|ConnectError::internal("Party D1 get failed"))?
            .ok_or_else(||ConnectError::not_found("PARTIES_NOT_FOUND"))?;
        RpcResponse::ok(party_v3::GetPartyResponse {
            party:Some(Self::convert(&party)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn list_parties(
        &self,_ctx:RequestContext,req:ServiceRequest<'_,party_v3::ListPartiesRequest>,
    )->ServiceResult<party_v3::ListPartiesResponse>{
        let size=if req.page.page_size==0 {25} else {req.page.page_size};
        if size>100 {return Err(ConnectError::invalid_argument("page_size exceeds 100"));}
        let after=&req.page.page_token;
        if !after.is_empty(){Self::valid_id(after)?;}
        let filter=match req.kind {
            None=>None,
            Some(x) if x==party_v3::PartyKind::PARTY_KIND_PERSON=>Some(DomainPartyKind::Person),
            Some(x) if x==party_v3::PartyKind::PARTY_KIND_ORGANIZATION=>
                Some(DomainPartyKind::Organization),
            _=>return Err(ConnectError::invalid_argument("invalid PartyKind filter")),
        };
        let (parties,next)=parties::list_page_typed(&self.d1()?,&self.tenant,
            after,size as usize,filter).await
            .map_err(|_|ConnectError::internal("Party D1 list failed"))?;
        let mut items=Vec::with_capacity(parties.len());
        for party in &parties {items.push(Self::convert(party)?);}
        RpcResponse::ok(party_v3::ListPartiesResponse {
            parties:items,
            page_info:Some(core_v2::PageInfo {
                next_page_token:next.unwrap_or_default(),..Default::default()
            }).into(),
            ..Default::default()
        })
    }
}

// One new Connect surface for the EXISTING customer-accounts Rust owner.
// All commands and D1 writes reside in accounts.rs; no parallel JSON handler.
struct AccountV2Service {
    env: Arc<Env>,
    tenant: String,
    request_id: String,
}
impl AccountV2Service {
    fn d1(&self)->std::result::Result<D1Database,ConnectError>{
        self.env.d1("DB").map_err(|_|ConnectError::internal("D1 unavailable"))
    }
    fn valid_id(id:&str)->std::result::Result<String,ConnectError>{
        if id!=id.trim()||id.contains('/') {
            return Err(ConnectError::invalid_argument("CUSTOMER_ACCOUNTS_ID_INVALID"));
        }
        let value=crm_customer_accounts::AccountId::try_new(id)
            .map_err(|_|ConnectError::invalid_argument("CUSTOMER_ACCOUNTS_ID_INVALID"))?;
        Ok(value.as_str().to_owned())
    }
    fn convert(account:&DomainAccount)
        ->std::result::Result<account_wire::Account,ConnectError>{
        let status=match account.status(){
            DomainAccountStatus::Active=>account_wire::AccountStatus::ACCOUNT_STATUS_ACTIVE,
            DomainAccountStatus::Inactive=>account_wire::AccountStatus::ACCOUNT_STATUS_INACTIVE,
        };
        let parties=account.party_associations().iter().map(|association|{
            let role=match association.role(){
                DomainAccountPartyRole::Primary=>account_wire::AccountPartyRole::ACCOUNT_PARTY_ROLE_PRIMARY,
                DomainAccountPartyRole::Member=>account_wire::AccountPartyRole::ACCOUNT_PARTY_ROLE_MEMBER,
            };
            account_wire::AccountPartyAssociation{
                party_id:association.party().as_str().to_owned(),role:role.into(),
                ..Default::default()
            }
        }).collect();
        Ok(account_wire::Account {
            account_id:account.account_id().as_str().to_owned(),
            name:account.name().to_owned(),status:status.into(),parties,
            version:i64::try_from(account.version())
                .map_err(|_|ConnectError::internal("invalid Account version"))?,
            ..Default::default()
        })
    }
    fn receipt(&self,outcome:&accounts::MutationOutcome)
        ->std::result::Result<core_v2::MutationReceipt,ConnectError>{
        Ok(core_v2::MutationReceipt {
            request_id:self.request_id.clone(),
            resource_version:i64::try_from(outcome.account.version())
                .map_err(|_|ConnectError::internal("invalid Account version"))?,
            replayed:outcome.replayed,..Default::default()
        })
    }
    fn failed(f:accounts::MutationFailure)->ConnectError{
        match f.status{
            404=>ConnectError::not_found(f.code),
            409=>ConnectError::aborted(f.code),
            422=>ConnectError::invalid_argument(f.code),
            _=>ConnectError::internal(f.code),
        }
    }
    fn precondition(
        pre:Option<&core_v2::MutationPrecondition>,create:bool
    )->std::result::Result<(String,Option<i64>),ConnectError>{
        let pre=pre.ok_or_else(||ConnectError::invalid_argument("precondition required"))?;
        if !(8..=120).contains(&pre.idempotency_key.len()){
            return Err(ConnectError::invalid_argument("IDEMPOTENCY_KEY_REQUIRED"));
        }
        match (create,pre.expected_version){
            (true,None)=>Ok((pre.idempotency_key.clone(),None)),
            (false,Some(version)) if version>0=>Ok((pre.idempotency_key.clone(),Some(version))),
            _=>Err(ConnectError::invalid_argument("invalid mutation version")),
        }
    }
}
impl account_wire::AccountService for AccountV2Service{
    #[worker::send]
    async fn create_account(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,account_wire::CreateAccountRequest>,
    )->ServiceResult<account_wire::CreateAccountResponse>{
        let input=req.to_owned_message();
        let id=Self::valid_id(&input.account_id)?;
        let (key,_)=Self::precondition(input.precondition.as_option(),true)?;
        let saved=accounts::create_typed(accounts::CreateInput{
            id,name:input.name,party_id:input.primary_party_id,
        },&key,&self.d1()?,&self.tenant,&self.request_id).await
            .map_err(|_|ConnectError::internal("Account D1 create failed"))?
            .map_err(Self::failed)?;
        RpcResponse::ok(account_wire::CreateAccountResponse{
            account:Some(Self::convert(&saved.account)?).into(),
            receipt:Some(self.receipt(&saved)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn update_account(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,account_wire::UpdateAccountRequest>,
    )->ServiceResult<account_wire::UpdateAccountResponse>{
        let input=req.to_owned_message();
        let id=Self::valid_id(&input.account_id)?;
        let (key,version)=Self::precondition(input.precondition.as_option(),false)?;
        let status=match input.status{
            x if x==account_wire::AccountStatus::ACCOUNT_STATUS_ACTIVE=>
                accounts::AccountStatusInput::Active,
            x if x==account_wire::AccountStatus::ACCOUNT_STATUS_INACTIVE=>
                accounts::AccountStatusInput::Inactive,
            _=>return Err(ConnectError::invalid_argument("invalid AccountStatus")),
        };
        let saved=accounts::update_typed(accounts::UpdateInput{
            name:input.name,status,expected_version:version.unwrap_or_default(),
        },&key,&self.d1()?,&self.tenant,&id,&self.request_id).await
            .map_err(|_|ConnectError::internal("Account D1 update failed"))?
            .map_err(Self::failed)?;
        RpcResponse::ok(account_wire::UpdateAccountResponse{
            account:Some(Self::convert(&saved.account)?).into(),
            receipt:Some(self.receipt(&saved)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn get_account(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,account_wire::GetAccountRequest>,
    )->ServiceResult<account_wire::GetAccountResponse>{
        let id=Self::valid_id(&req.account_id)?;
        let account=accounts::load(&self.d1()?,&self.tenant,&id).await
            .map_err(|_|ConnectError::internal("Account D1 read failed"))?
            .ok_or_else(||ConnectError::not_found("CUSTOMER_ACCOUNTS_NOT_FOUND"))?;
        RpcResponse::ok(account_wire::GetAccountResponse{
            account:Some(Self::convert(&account)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn list_accounts(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,account_wire::ListAccountsRequest>,
    )->ServiceResult<account_wire::ListAccountsResponse>{
        let size=if req.page.page_size==0{25}else{req.page.page_size};
        if size>100{return Err(ConnectError::invalid_argument("page_size exceeds 100"));}
        let after=&req.page.page_token;
        if !after.is_empty(){Self::valid_id(after)?;}
        let status=match req.status{
            None=>None,
            Some(x) if x==account_wire::AccountStatus::ACCOUNT_STATUS_ACTIVE=>
                Some(DomainAccountStatus::Active),
            Some(x) if x==account_wire::AccountStatus::ACCOUNT_STATUS_INACTIVE=>
                Some(DomainAccountStatus::Inactive),
            _=>return Err(ConnectError::invalid_argument("invalid AccountStatus filter")),
        };
        let (list,next)=accounts::list_page_typed(&self.d1()?,&self.tenant,
            after,size as usize,status).await
            .map_err(|_|ConnectError::internal("Account D1 list failed"))?;
        let mut result=Vec::new();
        for account in &list{result.push(Self::convert(account)?);}
        RpcResponse::ok(account_wire::ListAccountsResponse{
            accounts:result,
            page_info:Some(core_v2::PageInfo{
                next_page_token:next.unwrap_or_default(),..Default::default()
            }).into(),..Default::default()
        })
    }
}

// Called by existing native-only dispatch AFTER signed Access authentication
// and role decision. No second fetch endpoint, tenant header, or JS route.
pub(super) async fn route(
    req:Request,env:Env,tenant:&str,actor:&str,request_id:&str,
)->Result<Response> {
    let idempotency_key=req.headers().get("idempotency-key")?;
    let env = Arc::new(env);
    let mut app:Router=ConnectRouter::new()
        .add_service(Arc::new(PartyService {
            env: Arc::clone(&env), tenant: tenant.to_owned(),
        }))
        .add_service(Arc::new(PartyV3Service {
            env: Arc::clone(&env), tenant: tenant.to_owned(),
            request_id:request_id.to_owned(),
        }))
        .add_service(Arc::new(AccountV2Service {
            env: Arc::clone(&env), tenant: tenant.to_owned(),
            request_id:request_id.to_owned(),
        }))
        .add_service(Arc::new(SalesService {
            env,tenant:tenant.to_owned(),
            actor:actor.to_owned(),request_id:request_id.to_owned(),idempotency_key,
        }))
        .into_axum_router();
    let http=HttpRequest::try_from(req)?;
    Ok(Response::try_from(app.call(http).await?)?)
}
