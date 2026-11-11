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
use super::{accounts, activity_tasks, bind, contact_points, parties, party_relationships, sales_contract, sales_deals};
use crm_parties::PartyKind as DomainPartyKind;
use crm_customer_accounts::{Account as DomainAccount, AccountPartyRole as DomainAccountPartyRole, AccountStatus as DomainAccountStatus};
use crm_contact_points::{ContactPoint as DomainContactPoint, ContactPointKind as DomainContactPointKind,
    ContactPointStatus as DomainContactPointStatus};
use crm_party_relationships::{
    PartyRelationship as DomainRelationship,
    RelationshipDirectionality as DomainDirectionality,
    PartyRelationshipStatus as DomainRelationshipStatus,
};
use crm_activities::{
    Task as DomainTask, TaskPriority as DomainTaskPriority,
    TaskStatus as DomainTaskStatus,
};
use buffa_types::google::protobuf::Timestamp;
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
        pub mod contact_points {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.contact_points.v2.mod.rs"));
            }
        }
        pub mod party_relationships {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.party_relationships.v2.mod.rs"));
            }
        }
        pub mod activities {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.activities.v2.mod.rs"));
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
use proto::crm::contact_points::v2 as contact_wire;
use proto::crm::party_relationships::v2 as rel_wire;
use proto::crm::activities::v2 as task_wire;

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

// Connect wire for ORIGINAL crm-contact-points (no browser verification claim).
struct ContactPointV2Service{
    env:Arc<Env>,
    tenant:String,
    request_id:String,
}
impl ContactPointV2Service{
    fn d1(&self)->std::result::Result<D1Database,ConnectError>{
        self.env.d1("DB").map_err(|_|ConnectError::internal("D1 unavailable"))
    }
    fn valid_id(id:&str)->std::result::Result<String,ConnectError>{
        if id!=id.trim()||id.contains('/') {
            return Err(ConnectError::invalid_argument("CONTACT_POINTS_ID_INVALID"));
        }
        let value=crm_contact_points::ContactPointId::try_new(id)
            .map_err(|_|ConnectError::invalid_argument("CONTACT_POINTS_ID_INVALID"))?;
        Ok(value.as_str().to_owned())
    }
    fn nanos(ts:&Timestamp)->std::result::Result<i64,ConnectError>{
        if !(0..1_000_000_000).contains(&ts.nanos){
            return Err(ConnectError::invalid_argument("invalid timestamp nanos"));
        }
        ts.seconds.checked_mul(1_000_000_000)
            .and_then(|s|s.checked_add(i64::from(ts.nanos)))
            .ok_or_else(||ConnectError::invalid_argument("timestamp outside domain range"))
    }
    fn timestamp(nanos:i64)->Timestamp{
        Timestamp{
            seconds:nanos.div_euclid(1_000_000_000),
            nanos:nanos.rem_euclid(1_000_000_000) as i32,
            ..Default::default()
        }
    }
    fn time_patch(patch:Option<&core_v2::TimestampPatch>)
        ->std::result::Result<contact_points::TimestampChange,ConnectError>{
        use contact_points::TimestampChange;
        let Some(patch)=patch else{return Ok(TimestampChange::Keep)};
        match patch.operation.as_ref(){
            Some(core_v2::timestamp_patch::Operation::Set(ts))=>
                Self::nanos(ts).map(TimestampChange::Set),
            Some(core_v2::timestamp_patch::Operation::Clear(true))=>
                Ok(TimestampChange::Clear),
            _=>Err(ConnectError::invalid_argument("invalid timestamp patch")),
        }
    }
    fn convert(point:&DomainContactPoint)
        ->std::result::Result<contact_wire::ContactPoint,ConnectError>{
        let kind=match point.kind(){
            DomainContactPointKind::Email=>contact_wire::ContactPointKind::CONTACT_POINT_KIND_EMAIL,
            DomainContactPointKind::Phone=>contact_wire::ContactPointKind::CONTACT_POINT_KIND_PHONE,
            DomainContactPointKind::Postal=>contact_wire::ContactPointKind::CONTACT_POINT_KIND_POSTAL,
            DomainContactPointKind::Web=>contact_wire::ContactPointKind::CONTACT_POINT_KIND_WEB,
            DomainContactPointKind::Messaging=>contact_wire::ContactPointKind::CONTACT_POINT_KIND_MESSAGING,
        };
        let status=match point.status(){
            DomainContactPointStatus::Active=>contact_wire::ContactPointStatus::CONTACT_POINT_STATUS_ACTIVE,
            DomainContactPointStatus::Inactive=>contact_wire::ContactPointStatus::CONTACT_POINT_STATUS_INACTIVE,
        };
        Ok(contact_wire::ContactPoint{
            contact_point_id:point.contact_point_id().as_str().to_owned(),
            party_id:point.party_ref().as_str().to_owned(),kind:kind.into(),
            display_value:point.display_value().to_owned(),
            normalized_value:point.normalized_value().to_owned(),
            preferred:point.preferred(),status:status.into(),
            verified:point.verification().is_verified(),
            valid_from:point.valid_from_unix_nanos().map(Self::timestamp).into(),
            valid_until:point.valid_until_unix_nanos().map(Self::timestamp).into(),
            version:i64::try_from(point.version())
                .map_err(|_|ConnectError::internal("invalid ContactPoint version"))?,
            ..Default::default()
        })
    }
    fn receipt(&self,outcome:&contact_points::MutationOutcome)
        ->std::result::Result<core_v2::MutationReceipt,ConnectError>{
        Ok(core_v2::MutationReceipt{
            request_id:self.request_id.clone(),
            resource_version:i64::try_from(outcome.contact_point.version())
                .map_err(|_|ConnectError::internal("invalid ContactPoint version"))?,
            replayed:outcome.replayed,..Default::default()
        })
    }
    fn failed(f:contact_points::MutationFailure)->ConnectError{
        match f.status{
            404=>ConnectError::not_found(f.code),
            409=>ConnectError::aborted(f.code),
            422=>ConnectError::invalid_argument(f.code),
            _=>ConnectError::internal(f.code),
        }
    }
    fn precondition(pre:Option<&core_v2::MutationPrecondition>,create:bool)
        ->std::result::Result<(String,Option<i64>),ConnectError>{
        AccountV2Service::precondition(pre,create)
    }
}
impl contact_wire::ContactPointService for ContactPointV2Service{
    #[worker::send]
    async fn create_contact_point(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,contact_wire::CreateContactPointRequest>,
    )->ServiceResult<contact_wire::CreateContactPointResponse>{
        let input=req.to_owned_message();
        let id=Self::valid_id(&input.contact_point_id)?;
        let kind=match input.kind{
            x if x==contact_wire::ContactPointKind::CONTACT_POINT_KIND_EMAIL=>
                contact_points::KindInput::Email,
            x if x==contact_wire::ContactPointKind::CONTACT_POINT_KIND_PHONE=>
                contact_points::KindInput::Phone,
            x if x==contact_wire::ContactPointKind::CONTACT_POINT_KIND_POSTAL=>
                contact_points::KindInput::Postal,
            x if x==contact_wire::ContactPointKind::CONTACT_POINT_KIND_WEB=>
                contact_points::KindInput::Web,
            x if x==contact_wire::ContactPointKind::CONTACT_POINT_KIND_MESSAGING=>
                contact_points::KindInput::Messaging,
            _=>return Err(ConnectError::invalid_argument("invalid ContactPointKind")),
        };
        let (key,_)=Self::precondition(input.precondition.as_option(),true)?;
        let from=input.valid_from.as_option().map(Self::nanos).transpose()?;
        let until=input.valid_until.as_option().map(Self::nanos).transpose()?;
        let saved=contact_points::create_typed(contact_points::CreateInput{
            id,party_id:input.party_id,kind,value:input.value,
            preferred:input.preferred,valid_from_unix_nanos:from,
            valid_until_unix_nanos:until,
        },&key,&self.d1()?,&self.tenant,&self.request_id).await
            .map_err(|_|ConnectError::internal("ContactPoint D1 create failed"))?
            .map_err(Self::failed)?;
        RpcResponse::ok(contact_wire::CreateContactPointResponse{
            contact_point:Some(Self::convert(&saved.contact_point)?).into(),
            receipt:Some(self.receipt(&saved)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn update_contact_point(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,contact_wire::UpdateContactPointRequest>,
    )->ServiceResult<contact_wire::UpdateContactPointResponse>{
        let input=req.to_owned_message();
        let id=Self::valid_id(&input.contact_point_id)?;
        let (key,version)=Self::precondition(input.precondition.as_option(),false)?;
        let status=match input.status{
            x if x==contact_wire::ContactPointStatus::CONTACT_POINT_STATUS_ACTIVE=>
                contact_points::StatusInput::Active,
            x if x==contact_wire::ContactPointStatus::CONTACT_POINT_STATUS_INACTIVE=>
                contact_points::StatusInput::Inactive,
            _=>return Err(ConnectError::invalid_argument("invalid ContactPointStatus")),
        };
        let db=self.d1()?;
        // Hash the unchanged wire patch intent before reading current D1 state;
        // the existing owner checks its receipt first, even after later updates.
        let from=Self::time_patch(input.valid_from.as_option())?;
        let until=Self::time_patch(input.valid_until.as_option())?;
        let saved=contact_points::update_typed(contact_points::UpdateInput{
            expected_version:version.unwrap_or_default(),value:input.value,
            status,preferred:input.preferred,valid_from_unix_nanos:from,
            valid_until_unix_nanos:until,
        },&key,&db,&self.tenant,&id,&self.request_id).await
            .map_err(|_|ConnectError::internal("ContactPoint D1 update failed"))?
            .map_err(Self::failed)?;
        RpcResponse::ok(contact_wire::UpdateContactPointResponse{
            contact_point:Some(Self::convert(&saved.contact_point)?).into(),
            receipt:Some(self.receipt(&saved)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn get_contact_point(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,contact_wire::GetContactPointRequest>,
    )->ServiceResult<contact_wire::GetContactPointResponse>{
        let id=Self::valid_id(&req.contact_point_id)?;
        let point=contact_points::load(&self.d1()?,&self.tenant,&id).await
            .map_err(|_|ConnectError::internal("ContactPoint D1 read failed"))?
            .ok_or_else(||ConnectError::not_found("CONTACT_POINTS_NOT_FOUND"))?;
        RpcResponse::ok(contact_wire::GetContactPointResponse{
            contact_point:Some(Self::convert(&point)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn list_contact_points(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,contact_wire::ListContactPointsRequest>,
    )->ServiceResult<contact_wire::ListContactPointsResponse>{
        let size=if req.page.page_size==0{25}else{req.page.page_size};
        if size>100{return Err(ConnectError::invalid_argument("page_size exceeds 100"));}
        let after=&req.page.page_token;
        if !after.is_empty(){Self::valid_id(after)?;}
        let filter=req.party_id.as_deref();
        if let Some(party)=filter{
            crm_contact_points::PartyReference::try_new(party)
                .map_err(|_|ConnectError::invalid_argument("invalid party_id filter"))?;
        }
        let (list,next)=contact_points::list_page_typed(&self.d1()?,&self.tenant,
            after,size as usize,filter).await
            .map_err(|_|ConnectError::internal("ContactPoint D1 list failed"))?;
        let mut results=Vec::new();
        for point in &list{results.push(Self::convert(point)?);}
        RpcResponse::ok(contact_wire::ListContactPointsResponse{
            contact_points:results,
            page_info:Some(core_v2::PageInfo{
                next_page_token:next.unwrap_or_default(),..Default::default()
            }).into(),..Default::default()
        })
    }
}

// Generated Connect wire for ORIGINAL crm-party-relationships. No second
// persistence path: party_relationships.rs owns D1, domain invariants and
// receipts; this service only checks wire fields and maps typed commands.
struct RelationshipV2Service {
    env:Arc<Env>,tenant:String,request_id:String,
}
impl RelationshipV2Service {
    fn d1(&self)->std::result::Result<D1Database,ConnectError>{
        self.env.d1("DB").map_err(|_|ConnectError::internal("D1 unavailable"))
    }
    fn valid_id(raw:&str)->std::result::Result<String,ConnectError>{
        if raw!=raw.trim()||raw.contains('/') {
            return Err(ConnectError::invalid_argument("PARTY_RELATIONSHIPS_RELATIONSHIP_ID_INVALID"));
        }
        crm_party_relationships::PartyRelationshipId::try_new(raw.to_owned())
            .map(|x|x.as_str().to_owned())
            .map_err(|_|ConnectError::invalid_argument("PARTY_RELATIONSHIPS_RELATIONSHIP_ID_INVALID"))
    }
    fn patch(patch:Option<&core_v2::TimestampPatch>)
        ->std::result::Result<party_relationships::TimestampChange,ConnectError>{
        use party_relationships::TimestampChange;
        let Some(patch)=patch else{return Ok(TimestampChange::Keep)};
        match patch.operation.as_ref(){
            Some(core_v2::timestamp_patch::Operation::Set(ts))=>
                ContactPointV2Service::nanos(ts).map(TimestampChange::Set),
            Some(core_v2::timestamp_patch::Operation::Clear(true))=>
                Ok(TimestampChange::Clear),
            _=>Err(ConnectError::invalid_argument("invalid timestamp patch")),
        }
    }
    fn convert(relation:&DomainRelationship)
        ->std::result::Result<rel_wire::PartyRelationship,ConnectError>{
        let rtype=relation.relationship_type();
        let directionality=match rtype.directionality(){
            DomainDirectionality::Directional=>
                rel_wire::RelationshipDirectionality::RELATIONSHIP_DIRECTIONALITY_DIRECTIONAL,
            DomainDirectionality::Reciprocal=>
                rel_wire::RelationshipDirectionality::RELATIONSHIP_DIRECTIONALITY_RECIPROCAL,
        };
        let status=match relation.status(){
            DomainRelationshipStatus::Active=>
                rel_wire::RelationshipStatus::RELATIONSHIP_STATUS_ACTIVE,
            DomainRelationshipStatus::Inactive=>
                rel_wire::RelationshipStatus::RELATIONSHIP_STATUS_INACTIVE,
        };
        Ok(rel_wire::PartyRelationship{
            relationship_id:relation.party_relationship_id().as_str().to_owned(),
            from_party_id:relation.from_party_ref().as_str().to_owned(),
            to_party_id:relation.to_party_ref().as_str().to_owned(),
            r#type:Some(rel_wire::RelationshipType{
                code:rtype.code().to_owned(),directionality:directionality.into(),
                from_role:rtype.from_role().to_owned(),to_role:rtype.to_role().to_owned(),
                ..Default::default()
            }).into(),
            status:status.into(),
            valid_from:relation.valid_from_unix_nanos()
                .map(ContactPointV2Service::timestamp).into(),
            valid_until:relation.valid_until_unix_nanos()
                .map(ContactPointV2Service::timestamp).into(),
            version:relation.version(),..Default::default()
        })
    }
    fn receipt(&self,out:&party_relationships::MutationOutcome)->core_v2::MutationReceipt{
        core_v2::MutationReceipt{
            request_id:self.request_id.clone(),resource_version:out.relationship.version(),
            replayed:out.replayed,..Default::default()
        }
    }
    fn failed(f:party_relationships::MutationFailure)->ConnectError{
        match f.status{
            404=>ConnectError::not_found(f.code),
            409=>ConnectError::aborted(f.code),
            422=>ConnectError::invalid_argument(f.code),
            _=>ConnectError::internal(f.code),
        }
    }
}
impl rel_wire::PartyRelationshipService for RelationshipV2Service {
    #[worker::send]
    async fn create_party_relationship(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,rel_wire::CreatePartyRelationshipRequest>,
    )->ServiceResult<rel_wire::CreatePartyRelationshipResponse>{
        let input=req.to_owned_message();
        let id=Self::valid_id(&input.relationship_id)?;
        let (key,_)=AccountV2Service::precondition(input.precondition.as_option(),true)?;
        let typ=input.r#type.as_option()
            .ok_or_else(||ConnectError::invalid_argument("relationship type required"))?;
        let directionality=match typ.directionality {
            x if x==rel_wire::RelationshipDirectionality::RELATIONSHIP_DIRECTIONALITY_DIRECTIONAL=>
                party_relationships::DirectionalityInput::Directional,
            x if x==rel_wire::RelationshipDirectionality::RELATIONSHIP_DIRECTIONALITY_RECIPROCAL=>
                party_relationships::DirectionalityInput::Reciprocal,
            _=>return Err(ConnectError::invalid_argument("invalid RelationshipDirectionality")),
        };
        let from=input.valid_from.as_option()
            .map(ContactPointV2Service::nanos).transpose()?;
        let until=input.valid_until.as_option()
            .map(ContactPointV2Service::nanos).transpose()?;
        let saved=party_relationships::create_typed(party_relationships::CreateInput{
            id,from_party_id:input.from_party_id,to_party_id:input.to_party_id,
            code:typ.code.clone(),directionality,from_role:typ.from_role.clone(),
            to_role:typ.to_role.clone(),valid_from_unix_nanos:from,
            valid_until_unix_nanos:until,
        },&key,&self.d1()?,&self.tenant,&self.request_id).await
            .map_err(|_|ConnectError::internal("Relationship D1 create failed"))?
            .map_err(Self::failed)?;
        RpcResponse::ok(rel_wire::CreatePartyRelationshipResponse{
            relationship:Some(Self::convert(&saved.relationship)?).into(),
            receipt:Some(self.receipt(&saved)).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn update_party_relationship(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,rel_wire::UpdatePartyRelationshipRequest>,
    )->ServiceResult<rel_wire::UpdatePartyRelationshipResponse>{
        let input=req.to_owned_message();
        let id=Self::valid_id(&input.relationship_id)?;
        let (key,version)=AccountV2Service::precondition(input.precondition.as_option(),false)?;
        let status=match input.status{
            x if x==rel_wire::RelationshipStatus::RELATIONSHIP_STATUS_ACTIVE=>
                party_relationships::StatusInput::Active,
            x if x==rel_wire::RelationshipStatus::RELATIONSHIP_STATUS_INACTIVE=>
                party_relationships::StatusInput::Inactive,
            _=>return Err(ConnectError::invalid_argument("invalid RelationshipStatus")),
        };
        let from=Self::patch(input.valid_from.as_option())?;
        let until=Self::patch(input.valid_until.as_option())?;
        let saved=party_relationships::update_typed(party_relationships::UpdateCommand{
            expected_version:version.unwrap_or_default(),status,
            valid_from:from,valid_until:until,
        },&key,&self.d1()?,&self.tenant,&id,&self.request_id).await
            .map_err(|_|ConnectError::internal("Relationship D1 update failed"))?
            .map_err(Self::failed)?;
        RpcResponse::ok(rel_wire::UpdatePartyRelationshipResponse{
            relationship:Some(Self::convert(&saved.relationship)?).into(),
            receipt:Some(self.receipt(&saved)).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn get_party_relationship(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,rel_wire::GetPartyRelationshipRequest>,
    )->ServiceResult<rel_wire::GetPartyRelationshipResponse>{
        let id=Self::valid_id(&req.relationship_id)?;
        let relationship=party_relationships::load(&self.d1()?,&self.tenant,&id).await
            .map_err(|_|ConnectError::internal("Relationship D1 read failed"))?
            .ok_or_else(||ConnectError::not_found("PARTY_RELATIONSHIPS_NOT_FOUND"))?;
        RpcResponse::ok(rel_wire::GetPartyRelationshipResponse{
            relationship:Some(Self::convert(&relationship)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn list_party_relationships(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,rel_wire::ListPartyRelationshipsRequest>,
    )->ServiceResult<rel_wire::ListPartyRelationshipsResponse>{
        let size=if req.page.page_size==0{25}else{req.page.page_size};
        if size>100{return Err(ConnectError::invalid_argument("page_size exceeds 100"));}
        let after=&req.page.page_token;
        if !after.is_empty(){Self::valid_id(after)?;}
        let party=req.party_id.as_deref();
        if let Some(id)=party{
            crm_party_relationships::PartyReference::try_new(id.to_owned())
                .map_err(|_|ConnectError::invalid_argument("invalid Party filter"))?;
        }
        let (list,next)=party_relationships::list_page_typed(
            &self.d1()?,&self.tenant,after,size as usize,party).await
            .map_err(|_|ConnectError::internal("Relationship D1 list failed"))?;
        let mut results=Vec::with_capacity(list.len());
        for relationship in &list{results.push(Self::convert(relationship)?);}
        RpcResponse::ok(rel_wire::ListPartyRelationshipsResponse{
            relationships:results,
            page_info:Some(core_v2::PageInfo{
                next_page_token:next.unwrap_or_default(),..Default::default()
            }).into(),..Default::default()
        })
    }
}

// Native generated Connect service for the ORIGINAL crm-activities Task.
// Same Task codec, D1 table, CAS and receipts as the blocked historical HTTP.
struct TaskV2Service {
    env:Arc<Env>,tenant:String,actor:String,request_id:String,
}
impl TaskV2Service {
    fn d1(&self)->std::result::Result<D1Database,ConnectError>{
        self.env.d1("DB").map_err(|_|ConnectError::internal("D1 unavailable"))
    }
    fn valid_id(raw:&str)->std::result::Result<String,ConnectError>{
        if raw!=raw.trim()||raw.contains('/') {
            return Err(ConnectError::invalid_argument("ACTIVITIES_TASK_ID_INVALID"));
        }
        crm_module_sdk::RecordId::try_new(raw.to_owned())
            .map(|x|x.as_str().to_owned())
            .map_err(|_|ConnectError::invalid_argument("ACTIVITIES_TASK_ID_INVALID"))
    }
    fn convert(task:&DomainTask)->std::result::Result<task_wire::Task,ConnectError>{
        let priority=match task.priority(){
            DomainTaskPriority::Low=>task_wire::TaskPriority::TASK_PRIORITY_LOW,
            DomainTaskPriority::Normal=>task_wire::TaskPriority::TASK_PRIORITY_NORMAL,
            DomainTaskPriority::High=>task_wire::TaskPriority::TASK_PRIORITY_HIGH,
            DomainTaskPriority::Urgent=>task_wire::TaskPriority::TASK_PRIORITY_URGENT,
        };
        let status=match task.status(){
            DomainTaskStatus::Open=>task_wire::TaskStatus::TASK_STATUS_OPEN,
            DomainTaskStatus::Completed=>task_wire::TaskStatus::TASK_STATUS_COMPLETED,
        };
        let account_id=task.related_resources().iter().find(
            |item|item.resource_type=="crm.customer-accounts")
            .ok_or_else(||ConnectError::internal("original Task missing Account reference"))?
            .resource_id.clone();
        Ok(task_wire::Task{
            task_id:task.task_id().as_str().to_owned(),
            subject:task.subject().to_owned(),
            description:task.description().map(str::to_owned),
            account_id,priority:priority.into(),status:status.into(),
            due_at:task.due_at_unix_nanos()
                .map(ContactPointV2Service::timestamp).into(),
            reminder_at:task.reminder_at_unix_nanos()
                .map(ContactPointV2Service::timestamp).into(),
            completed_at:task.completed_at_unix_nanos()
                .map(ContactPointV2Service::timestamp).into(),
            version:task.version(),..Default::default()
        })
    }
    fn receipt(&self,out:&activity_tasks::MutationOutcome)->core_v2::MutationReceipt{
        core_v2::MutationReceipt{
            request_id:self.request_id.clone(),resource_version:out.task.version(),
            replayed:out.replayed,..Default::default()
        }
    }
    fn failed(f:activity_tasks::MutationFailure)->ConnectError{
        match f.status{
            404=>ConnectError::not_found(f.code),
            409=>ConnectError::aborted(f.code),
            422=>ConnectError::invalid_argument(f.code),
            _=>ConnectError::internal(f.code),
        }
    }
}
impl task_wire::TaskService for TaskV2Service {
    #[worker::send]
    async fn create_task(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,task_wire::CreateTaskRequest>,
    )->ServiceResult<task_wire::CreateTaskResponse>{
        let input=req.to_owned_message();
        let id=Self::valid_id(&input.task_id)?;
        let (key,_)=AccountV2Service::precondition(input.precondition.as_option(),true)?;
        let priority=match input.priority{
            x if x==task_wire::TaskPriority::TASK_PRIORITY_LOW=>
                activity_tasks::Priority::Low,
            x if x==task_wire::TaskPriority::TASK_PRIORITY_NORMAL=>
                activity_tasks::Priority::Normal,
            x if x==task_wire::TaskPriority::TASK_PRIORITY_HIGH=>
                activity_tasks::Priority::High,
            x if x==task_wire::TaskPriority::TASK_PRIORITY_URGENT=>
                activity_tasks::Priority::Urgent,
            _=>return Err(ConnectError::invalid_argument("invalid TaskPriority")),
        };
        let due=input.due_at.as_option()
            .map(ContactPointV2Service::nanos).transpose()?;
        let reminder=input.reminder_at.as_option()
            .map(ContactPointV2Service::nanos).transpose()?;
        let saved=activity_tasks::create_typed(activity_tasks::CreateInput{
            id,subject:input.subject,description:input.description,
            account_id:input.account_id,priority,
            due_at_unix_nanos:due,reminder_at_unix_nanos:reminder,
        },&key,&self.d1()?,&self.tenant,&self.actor,&self.request_id).await
            .map_err(|_|ConnectError::internal("Task D1 create failed"))?
            .map_err(Self::failed)?;
        RpcResponse::ok(task_wire::CreateTaskResponse{
            task:Some(Self::convert(&saved.task)?).into(),
            receipt:Some(self.receipt(&saved)).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn complete_task(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,task_wire::CompleteTaskRequest>,
    )->ServiceResult<task_wire::CompleteTaskResponse>{
        let input=req.to_owned_message();
        let id=Self::valid_id(&input.task_id)?;
        let (key,version)=AccountV2Service::precondition(input.precondition.as_option(),false)?;
        let saved=activity_tasks::complete_typed(activity_tasks::CompleteInput{
            expected_version:version.unwrap_or_default(),
        },&key,&self.d1()?,&self.tenant,&id,&self.request_id).await
            .map_err(|_|ConnectError::internal("Task D1 complete failed"))?
            .map_err(Self::failed)?;
        RpcResponse::ok(task_wire::CompleteTaskResponse{
            task:Some(Self::convert(&saved.task)?).into(),
            changed:saved.changed,
            receipt:if saved.changed||saved.replayed{
                Some(self.receipt(&saved)).into()
            }else{None.into()},
            ..Default::default()
        })
    }
    #[worker::send]
    async fn get_task(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,task_wire::GetTaskRequest>,
    )->ServiceResult<task_wire::GetTaskResponse>{
        let id=Self::valid_id(&req.task_id)?;
        let task=activity_tasks::stored(&self.d1()?,&self.tenant,&id).await
            .map_err(|_|ConnectError::internal("Task D1 get failed"))?
            .ok_or_else(||ConnectError::not_found("ACTIVITIES_TASK_NOT_FOUND"))?;
        RpcResponse::ok(task_wire::GetTaskResponse{
            task:Some(Self::convert(&task)?).into(),..Default::default()
        })
    }
    #[worker::send]
    async fn list_tasks(&self,_ctx:RequestContext,
        req:ServiceRequest<'_,task_wire::ListTasksRequest>,
    )->ServiceResult<task_wire::ListTasksResponse>{
        let size=if req.page.page_size==0{25}else{req.page.page_size};
        if size>100{return Err(ConnectError::invalid_argument("page_size exceeds 100"));}
        let after=&req.page.page_token;
        if !after.is_empty(){Self::valid_id(after)?;}
        let status=match req.status{
            None=>None,
            Some(x) if x==task_wire::TaskStatus::TASK_STATUS_OPEN=>
                Some(DomainTaskStatus::Open),
            Some(x) if x==task_wire::TaskStatus::TASK_STATUS_COMPLETED=>
                Some(DomainTaskStatus::Completed),
            _=>return Err(ConnectError::invalid_argument("invalid TaskStatus filter")),
        };
        let account=req.account_id.as_deref();
        if let Some(id)=account {
            crm_module_sdk::RecordId::try_new(id.to_owned())
                .map_err(|_|ConnectError::invalid_argument("invalid Account filter"))?;
        }
        let (list,next)=activity_tasks::list_page_typed(
            &self.d1()?,&self.tenant,after,size as usize,status,account).await
            .map_err(|_|ConnectError::internal("Task D1 list failed"))?;
        let mut tasks=Vec::with_capacity(list.len());
        for task in &list{tasks.push(Self::convert(task)?);}
        RpcResponse::ok(task_wire::ListTasksResponse{
            tasks,page_info:Some(core_v2::PageInfo{
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
        .add_service(Arc::new(ContactPointV2Service {
            env: Arc::clone(&env), tenant: tenant.to_owned(),
            request_id:request_id.to_owned(),
        }))
        .add_service(Arc::new(RelationshipV2Service {
            env: Arc::clone(&env), tenant: tenant.to_owned(),
            request_id:request_id.to_owned(),
        }))
        .add_service(Arc::new(TaskV2Service {
            env: Arc::clone(&env),tenant:tenant.to_owned(),
            actor:actor.to_owned(),request_id:request_id.to_owned(),
        }))
        .add_service(Arc::new(SalesService {
            env,tenant:tenant.to_owned(),
            actor:actor.to_owned(),request_id:request_id.to_owned(),idempotency_key,
        }))
        .into_axum_router();
    let http=HttpRequest::try_from(req)?;
    Ok(Response::try_from(app.call(http).await?)?)
}
