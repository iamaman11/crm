//! One-time integration experiment: real Connect RPC service, Cloudflare WASM
//! entrypoint and actual native D1 read in the same async handler.
use std::sync::Arc;
use axum::Router;
use connectrpc::{
    RequestContext, Response, Router as ConnectRouter, ServiceRequest, ServiceResult,
};
use serde::Deserialize;
use tower_service::Service;
use worker::{event, Context, Env, HttpRequest, Result};

pub mod proto {
    connectrpc::include_generated!();
}
use proto::crm::compatproof::v1::*;

#[derive(Deserialize)]
struct D1Row { result: i64 }

struct Proof { env: Arc<Env> }

impl ProofService for Proof {
    async fn ping(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, PingRequest>,
    ) -> ServiceResult<PingResponse> {
        let db = self.env.d1("DB").expect("proof requires native D1");
        let row = db.prepare("SELECT 1 AS result")
            .first::<D1Row>(None).await.expect("D1 readback").expect("D1 row");
        Response::ok(PingResponse {
            marker: req.marker.to_string(),
            d1_readback: row.result,
            ..Default::default()
        })
    }
}

fn router(env: Env) -> Router {
    ConnectRouter::new()
        .add_service(Arc::new(Proof { env: Arc::new(env) }))
        .into_axum_router()
}

#[event(fetch)]
async fn main(req: HttpRequest, env: Env, _ctx: Context)
    -> Result<axum::http::Response<axum::body::Body>>
{
    Ok(router(env).call(req).await?)
}
