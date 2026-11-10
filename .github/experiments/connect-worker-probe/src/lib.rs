//! One-time experiment: exact same generated ConnectRPC route with and
//! without the native workers-rs D1 await inside the service handler.
use std::sync::Arc;
use axum::Router;
use connectrpc::{
    RequestContext, Response, Router as ConnectRouter, ServiceRequest, ServiceResult,
};
#[cfg(feature = "with_d1")]
use serde::Deserialize;
use tower_service::Service;
use worker::{event, Context, Env, HttpRequest, Result};

pub mod proto {
    connectrpc::include_generated!();
}
use proto::crm::compatproof::v1::*;

#[cfg(feature = "with_d1")]
#[derive(Deserialize)]
struct D1Row { result: i64 }

struct Proof {
    #[cfg(feature = "with_d1")]
    env: Arc<Env>,
}

impl ProofService for Proof {
    async fn ping(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, PingRequest>,
    ) -> ServiceResult<PingResponse> {
        #[cfg(feature = "with_d1")]
        let d1_result = {
            let db = self.env.d1("DB").expect("proof requires native D1");
            let row = db.prepare("SELECT 1 AS result")
                .first::<D1Row>(None).await.expect("D1 readback").expect("D1 row");
            row.result
        };
        #[cfg(not(feature = "with_d1"))]
        let d1_result = 1;
        Response::ok(PingResponse {
            marker: req.marker.to_string(),
            d1_readback: d1_result,
            ..Default::default()
        })
    }
}

fn router(env: Env) -> Router {
    #[cfg(feature = "with_d1")]
    let service = Proof { env: Arc::new(env) };
    #[cfg(not(feature = "with_d1"))]
    let service = { let _ = env; Proof {} };
    ConnectRouter::new()
        .add_service(Arc::new(service))
        .into_axum_router()
}

#[event(fetch)]
async fn main(req: HttpRequest, env: Env, _ctx: Context)
    -> Result<axum::http::Response<axum::body::Body>>
{
    Ok(router(env).call(req).await?)
}
