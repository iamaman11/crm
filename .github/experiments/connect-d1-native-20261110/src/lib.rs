//! No custom transport/async adapter: only workers-rs' official #[worker::send].
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
    #[worker::send]
    async fn ping(
        &self,
        _ctx: RequestContext,
        req: ServiceRequest<'_, PingRequest>,
    ) -> ServiceResult<PingResponse> {
        // Existing workers-rs D1 binding, not REST D1 API, no second Worker.
        let db = self.env.d1("DB").expect("native D1 binding required");
        db.exec("CREATE TABLE IF NOT EXISTS crm_connect_native_proof (marker TEXT PRIMARY KEY)")
            .await.expect("D1 create probe table");
        let marker = req.marker.to_string();
        let written = db.prepare("INSERT OR IGNORE INTO crm_connect_native_proof(marker) VALUES(?)")
            .bind(&[worker::wasm_bindgen::JsValue::from_str(&marker)])
            .expect("bind marker").run().await.expect("D1 insert");
        let _ = written;
        let row = db.prepare("SELECT COUNT(*) AS result FROM crm_connect_native_proof WHERE marker = ?")
            .bind(&[worker::wasm_bindgen::JsValue::from_str(&marker)])
            .expect("bind query").first::<D1Row>(None).await
            .expect("D1 readback").expect("D1 row");
        Response::ok(PingResponse {
            marker,
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
