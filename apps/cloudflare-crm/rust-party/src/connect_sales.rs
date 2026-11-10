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
use super::{bind, parties, sales_contract, sales_deals};
use crm_parties::PartyKind as DomainPartyKind;
use crm_proto_contracts::crm::{core::v1 as core_contract, sales::v1 as sales_contract_proto};

// The published crm.core.v1 package shadows Rust's 'core' crate name.
// Mount the UNMODIFIED generated Buffa package stitchers and expose the
// standard library fmt at the enclosing package. No runtime adapter/codegen
// rewrite is required; all message and service code remains generated.
pub mod proto {
    pub mod crm {
        pub mod core {
            use super::*;
            pub use ::core::fmt;
            pub mod v1 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.core.v1.mod.rs"));
            }
        }
        pub mod sales {
            use super::*;
            pub mod v1 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.sales.v1.mod.rs"));
            }
        }
        pub mod parties {
            use super::*;
            pub mod v2 {
                use super::*;
                include!(concat!(env!("OUT_DIR"), "/crm.parties.v2.mod.rs"));
            }
        }
    }
}
use proto::crm::sales::v1 as sales;
use proto::crm::parties::v2 as party_wire;

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
        .add_service(Arc::new(SalesService {
            env,tenant:tenant.to_owned(),
            actor:actor.to_owned(),request_id:request_id.to_owned(),idempotency_key,
        }))
        .into_axum_router();
    let http=HttpRequest::try_from(req)?;
    Ok(Response::try_from(app.call(http).await?)?)
}
