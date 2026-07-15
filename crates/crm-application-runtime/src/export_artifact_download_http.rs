use crate::{
    PartyExportArtifactDownloadRequest, PartyExportArtifactDownloadResult,
    PartyExportArtifactDownloadService,
};
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use crm_module_sdk::{ErrorCategory, SdkError};
use serde_json::json;
use std::sync::Arc;

pub(crate) fn export_artifact_download_router(
    service: Arc<PartyExportArtifactDownloadService>,
) -> Router {
    Router::new()
        .route(
            "/v1/customer-data/exports/{export_job_id}/artifact",
            get(download_export_artifact),
        )
        .with_state(service)
}

async fn download_export_artifact(
    State(service): State<Arc<PartyExportArtifactDownloadService>>,
    Path(export_job_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let authorization = match optional_header(&headers, "authorization") {
        Ok(value) => value.unwrap_or_default(),
        Err(response) => return response,
    };
    let tenant_id = match optional_header(&headers, "x-tenant-id") {
        Ok(value) => value,
        Err(response) => return response,
    };
    let request_id = match optional_header(&headers, "x-request-id") {
        Ok(value) => value,
        Err(response) => return response,
    };
    let correlation_id = match optional_header(&headers, "x-correlation-id") {
        Ok(value) => value,
        Err(response) => return response,
    };
    let trace_id = match optional_header(&headers, "x-trace-id") {
        Ok(value) => value,
        Err(response) => return response,
    };
    let timeout_millis = match optional_header(&headers, "x-timeout-ms") {
        Ok(Some(value)) => match value.parse::<u64>() {
            Ok(value) => Some(value),
            Err(_) => return invalid_request(),
        },
        Ok(None) => None,
        Err(response) => return response,
    };

    match service
        .download(PartyExportArtifactDownloadRequest {
            authorization,
            tenant_id,
            request_id,
            correlation_id,
            trace_id,
            timeout_millis,
            export_job_id,
        })
        .await
    {
        Ok(result) => success_response(result),
        Err(error) => error_response(error),
    }
}

fn success_response(result: PartyExportArtifactDownloadResult) -> Response {
    let content_length = result.bytes.len().to_string();
    let content_sha256 = hex(&result.content_sha256);
    let mut response = Response::new(Body::from(result.bytes));
    *response.status_mut() = StatusCode::OK;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&result.media_type)
            .expect("validated export media type must be a valid HTTP header"),
    );
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&content_length).expect("content length must be a valid HTTP header"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"party-export.csv\""),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    headers.insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"sha256-{content_sha256}\""))
            .expect("SHA-256 ETag must be a valid HTTP header"),
    );
    headers.insert(
        "x-content-sha256",
        HeaderValue::from_str(&content_sha256).expect("SHA-256 digest must be a valid HTTP header"),
    );
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn error_response(error: SdkError) -> Response {
    let status = match error.category {
        ErrorCategory::InvalidArgument => StatusCode::BAD_REQUEST,
        ErrorCategory::Authentication => StatusCode::UNAUTHORIZED,
        ErrorCategory::Authorization => StatusCode::FORBIDDEN,
        ErrorCategory::Conflict => StatusCode::CONFLICT,
        ErrorCategory::NotFound => StatusCode::NOT_FOUND,
        ErrorCategory::RateLimit => StatusCode::TOO_MANY_REQUESTS,
        ErrorCategory::Dependency | ErrorCategory::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        ErrorCategory::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(json!({"error": "request_failed"}))).into_response()
}

fn optional_header(headers: &HeaderMap, name: &'static str) -> Result<Option<String>, Response> {
    headers
        .get(name)
        .map(|value| {
            value
                .to_str()
                .map(str::to_owned)
                .map_err(|_| invalid_request())
        })
        .transpose()
}

fn invalid_request() -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": "invalid_request"})),
    )
        .into_response()
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_timeout_is_rejected_before_disclosure() {
        let mut headers = HeaderMap::new();
        headers.insert("x-timeout-ms", HeaderValue::from_static("invalid"));
        let value = optional_header(&headers, "x-timeout-ms").unwrap().unwrap();
        assert!(value.parse::<u64>().is_err());
    }
}
