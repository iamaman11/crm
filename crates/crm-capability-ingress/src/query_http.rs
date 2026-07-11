use crate::{
    CORRELATION_ID_HEADER, CapabilityRoute, QueryCallEnvelope, QueryIngress, QueryIngressMetadata,
    REQUEST_ID_HEADER, RETRY_AFTER_MILLIS_HEADER, SafeTransportError, TENANT_HEADER, TIMEOUT_HEADER,
    TRACE_ID_HEADER,
};
use ::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use crm_module_sdk::{ErrorCategory, TypedPayload};
use crm_query_runtime::QueryExecutionResult;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpQueryRequest {
    pub headers: HeaderMap,
    pub route: CapabilityRoute,
    pub input: TypedPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpQueryBody {
    Success(QueryExecutionResult),
    Error(SafeTransportError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpQueryResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: HttpQueryBody,
}

#[derive(Debug, Clone)]
pub struct HttpQueryMiddleware {
    ingress: QueryIngress,
}

impl HttpQueryMiddleware {
    pub fn new(ingress: QueryIngress) -> Self {
        Self { ingress }
    }

    pub async fn handle(&self, request: HttpQueryRequest) -> HttpQueryResponse {
        let authorization = match header_value(&request.headers, "authorization") {
            Ok(value) => value.unwrap_or_default(),
            Err(error) => return error_response(error),
        };
        let metadata = match metadata_from_headers(&request.headers) {
            Ok(metadata) => metadata,
            Err(error) => return error_response(error),
        };
        let envelope = QueryCallEnvelope {
            route: request.route,
            input: request.input,
            metadata,
        };
        match self.ingress.execute(&authorization, envelope).await {
            Ok(receipt) => {
                let mut headers = HeaderMap::new();
                insert_response_header(
                    &mut headers,
                    REQUEST_ID_HEADER,
                    receipt.request_id.as_str(),
                );
                insert_response_header(
                    &mut headers,
                    CORRELATION_ID_HEADER,
                    receipt.correlation_id.as_str(),
                );
                insert_response_header(&mut headers, TRACE_ID_HEADER, receipt.trace_id.as_str());
                HttpQueryResponse {
                    status: StatusCode::OK,
                    headers,
                    body: HttpQueryBody::Success(receipt.result),
                }
            }
            Err(error) => error_response(error.into()),
        }
    }
}

fn metadata_from_headers(headers: &HeaderMap) -> Result<QueryIngressMetadata, SafeTransportError> {
    Ok(QueryIngressMetadata {
        tenant_id: header_value(headers, TENANT_HEADER)?,
        request_id: header_value(headers, REQUEST_ID_HEADER)?,
        correlation_id: header_value(headers, CORRELATION_ID_HEADER)?,
        trace_id: header_value(headers, TRACE_ID_HEADER)?,
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

fn error_response(error: SafeTransportError) -> HttpQueryResponse {
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
    HttpQueryResponse {
        status,
        headers,
        body: HttpQueryBody::Error(error),
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
        ErrorCategory::Dependency | ErrorCategory::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
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
    use crate::{BUSINESS_TRANSACTION_HEADER, IDEMPOTENCY_KEY_HEADER};

    #[test]
    fn query_metadata_does_not_depend_on_mutation_only_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(TENANT_HEADER, HeaderValue::from_static("tenant-1"));
        let metadata = metadata_from_headers(&headers).unwrap();
        assert_eq!(metadata.tenant_id.as_deref(), Some("tenant-1"));

        headers.insert(IDEMPOTENCY_KEY_HEADER, HeaderValue::from_static("ignored-idem"));
        headers.insert(
            BUSINESS_TRANSACTION_HEADER,
            HeaderValue::from_static("ignored-transaction"),
        );
        let metadata = metadata_from_headers(&headers).unwrap();
        assert_eq!(metadata.tenant_id.as_deref(), Some("tenant-1"));
    }
}
