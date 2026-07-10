use crate::{
    CapabilityCallEnvelope, CapabilityIngress, CapabilityRoute, IngressMetadata, SafeTransportError,
};
use crm_capability_runtime::{ApprovalEvidence, CapabilityExecutionResult};
use crm_module_sdk::{ErrorCategory, TypedPayload};
use http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use std::str::FromStr;

pub const TENANT_HEADER: &str = "x-tenant-id";
pub const REQUEST_ID_HEADER: &str = "x-request-id";
pub const CORRELATION_ID_HEADER: &str = "x-correlation-id";
pub const CAUSATION_ID_HEADER: &str = "x-causation-id";
pub const TRACE_ID_HEADER: &str = "x-trace-id";
pub const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";
pub const BUSINESS_TRANSACTION_HEADER: &str = "x-business-transaction-id";
pub const TIMEOUT_HEADER: &str = "x-timeout-ms";
pub const RETRY_AFTER_MILLIS_HEADER: &str = "x-retry-after-ms";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpCapabilityRequest {
    pub headers: HeaderMap,
    pub route: CapabilityRoute,
    pub input: TypedPayload,
    pub approval: Option<ApprovalEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpCapabilityBody {
    Success(CapabilityExecutionResult),
    Error(SafeTransportError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpCapabilityResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: HttpCapabilityBody,
}

#[derive(Debug, Clone)]
pub struct HttpCapabilityMiddleware {
    ingress: CapabilityIngress,
}

impl HttpCapabilityMiddleware {
    pub fn new(ingress: CapabilityIngress) -> Self {
        Self { ingress }
    }

    pub async fn handle(&self, request: HttpCapabilityRequest) -> HttpCapabilityResponse {
        let authorization = match header_value(&request.headers, "authorization") {
            Ok(value) => value.unwrap_or_default(),
            Err(error) => return error_response(error),
        };
        let metadata = match metadata_from_headers(&request.headers) {
            Ok(metadata) => metadata,
            Err(error) => return error_response(error),
        };
        let envelope = CapabilityCallEnvelope {
            route: request.route,
            input: request.input,
            approval: request.approval,
            metadata,
        };
        match self.ingress.execute(&authorization, envelope).await {
            Ok(receipt) => {
                let mut headers = HeaderMap::new();
                insert_response_header(&mut headers, REQUEST_ID_HEADER, receipt.request_id.as_str());
                insert_response_header(
                    &mut headers,
                    CORRELATION_ID_HEADER,
                    receipt.correlation_id.as_str(),
                );
                insert_response_header(&mut headers, TRACE_ID_HEADER, receipt.trace_id.as_str());
                HttpCapabilityResponse {
                    status: StatusCode::OK,
                    headers,
                    body: HttpCapabilityBody::Success(receipt.result),
                }
            }
            Err(error) => error_response(error.into()),
        }
    }
}

fn metadata_from_headers(headers: &HeaderMap) -> Result<IngressMetadata, SafeTransportError> {
    Ok(IngressMetadata {
        tenant_id: header_value(headers, TENANT_HEADER)?,
        request_id: header_value(headers, REQUEST_ID_HEADER)?,
        correlation_id: header_value(headers, CORRELATION_ID_HEADER)?,
        causation_id: header_value(headers, CAUSATION_ID_HEADER)?,
        trace_id: header_value(headers, TRACE_ID_HEADER)?,
        idempotency_key: header_value(headers, IDEMPOTENCY_KEY_HEADER)?,
        business_transaction_id: header_value(headers, BUSINESS_TRANSACTION_HEADER)?,
        timeout_millis: optional_u64_header(headers, TIMEOUT_HEADER)?,
    })
}

fn header_value(
    headers: &HeaderMap,
    name: &'static str,
) -> Result<Option<String>, SafeTransportError> {
    let Some(value) = headers.get(name) else {
        return Ok(None);
    };
    value
        .to_str()
        .map(|value| Some(value.to_owned()))
        .map_err(|_| invalid_header_error())
}

fn optional_u64_header(
    headers: &HeaderMap,
    name: &'static str,
) -> Result<Option<u64>, SafeTransportError> {
    let Some(value) = header_value(headers, name)? else {
        return Ok(None);
    };
    value.parse().map(Some).map_err(|_| invalid_header_error())
}

fn invalid_header_error() -> SafeTransportError {
    SafeTransportError {
        code: "TRANSPORT_METADATA_INVALID".to_owned(),
        category: ErrorCategory::InvalidArgument,
        retryable: false,
        safe_message: "The request metadata is invalid.".to_owned(),
        retry_after_millis: None,
    }
}

fn error_response(error: SafeTransportError) -> HttpCapabilityResponse {
    let status = status_for_category(error.category);
    let mut headers = HeaderMap::new();
    if let Some(retry_after_millis) = error.retry_after_millis {
        insert_response_header(
            &mut headers,
            RETRY_AFTER_MILLIS_HEADER,
            &retry_after_millis.to_string(),
        );
        let retry_after_seconds = retry_after_millis.saturating_add(999) / 1_000;
        insert_response_header(
            &mut headers,
            "retry-after",
            &retry_after_seconds.to_string(),
        );
    }
    HttpCapabilityResponse {
        status,
        headers,
        body: HttpCapabilityBody::Error(error),
    }
}

fn status_for_category(category: ErrorCategory) -> StatusCode {
    match category {
        ErrorCategory::InvalidArgument => StatusCode::BAD_REQUEST,
        ErrorCategory::Authentication => StatusCode::UNAUTHORIZED,
        ErrorCategory::Authorization => StatusCode::FORBIDDEN,
        ErrorCategory::Conflict => StatusCode::CONFLICT,
        ErrorCategory::NotFound => StatusCode::NOT_FOUND,
        ErrorCategory::RateLimit => StatusCode::TOO_MANY_REQUESTS,
        ErrorCategory::Dependency | ErrorCategory::Unavailable => {
            StatusCode::SERVICE_UNAVAILABLE
        }
        ErrorCategory::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn insert_response_header(headers: &mut HeaderMap, name: &'static str, value: &str) {
    let Ok(name) = HeaderName::from_str(name) else {
        return;
    };
    let Ok(value) = HeaderValue::from_str(value) else {
        return;
    };
    headers.insert(name, value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_safe_categories_to_http_statuses() {
        assert_eq!(
            status_for_category(ErrorCategory::Authentication),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            status_for_category(ErrorCategory::Authorization),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            status_for_category(ErrorCategory::RateLimit),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            status_for_category(ErrorCategory::Unavailable),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn invalid_timeout_header_is_safe_bad_request() {
        let mut headers = HeaderMap::new();
        headers.insert(TIMEOUT_HEADER, HeaderValue::from_static("not-a-number"));
        let error = metadata_from_headers(&headers).unwrap_err();
        let response = error_response(error);
        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        assert!(matches!(response.body, HttpCapabilityBody::Error(_)));
    }
}
