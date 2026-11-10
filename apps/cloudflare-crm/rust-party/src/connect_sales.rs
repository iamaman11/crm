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
use super::{bind, sales_contract, sales_deals};

pub mod proto {
    connectrpc::include_generated!();
}
use proto::crm::sales::v1 as sales;

#[derive(Deserialize)]
struct Row { state: String }

struct SalesService {
    env: Arc<Env>,
    tenant: String,
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
        &self,_ctx:RequestContext,_req:ServiceRequest<'_, sales::ListDealsRequest>,
    )->ServiceResult<sales::ListDealsResponse> {
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
        &self,_ctx:RequestContext,_req:ServiceRequest<'_,sales::CreateDealRequest>,
    )->ServiceResult<sales::CreateDealResponse> {
        Err(ConnectError::unimplemented("CreateDeal pending native typed mutation gate"))
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

// Called by existing native-only dispatch AFTER signed Access authentication
// and role decision. No second fetch endpoint, tenant header, or JS route.
pub(super) async fn route(
    req:Request,env:Env,tenant:&str,
)->Result<Response> {
    let mut app:Router=ConnectRouter::new()
        .add_service(Arc::new(SalesService {
            env:Arc::new(env),tenant:tenant.to_owned(),
        }))
        .into_axum_router();
    let http=HttpRequest::try_from(req)?;
    Ok(Response::try_from(app.call(http).await?)?)
}
