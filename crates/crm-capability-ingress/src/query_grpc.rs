use crate::{
    CORRELATION_ID_HEADER, CapabilityRoute, ERROR_CODE_METADATA, ERROR_RETRYABLE_METADATA,
    QueryCallEnvelope, QueryIngress, QueryIngressMetadata, REQUEST_ID_HEADER,
    RETRY_AFTER_MILLIS_HEADER, SafeTransportError, TENANT_HEADER, TIMEOUT_HEADER, TRACE_ID_HEADER,
};
use crm_module_sdk::{ErrorCategory, TypedPayload};
use crm_query_runtime::QueryExecutionResult;
use tonic::metadata::{MetadataMap, MetadataValue};
use tonic::{Code, Request, Response, Status};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrpcQueryMessage {
    pub route: CapabilityRoute,
    pub input: TypedPayload,
}

#[derive(Debug, Clone)]
pub struct GrpcQueryMiddleware {
    ingress: QueryIngress,
}

impl GrpcQueryMiddleware {
    pub fn new(ingress: QueryIngress) -> Self {
        Self { ingress }
    }

    pub async fn handle(
        &self,
        request: Request<GrpcQueryMessage>,
    ) -> Result<Response<QueryExecutionResult>, Status> {
        let authorization = metadata_value(request.metadata(), "authorization")
            .map_err(status_from_safe_error)?
            .unwrap_or_default();
        let ingress_metadata =
            metadata_from_grpc(request.metadata()).map_err(status_from_safe_error)?;
        let message = request.into_inner();
        let envelope = QueryCallEnvelope {
            route: message.route,
            input: message.input,
            metadata: ingress_metadata,
        };
        match self.ingress.execute(&authorization, envelope).await {
            Ok(receipt) => {
                let mut response = Response::new(receipt.result);
                insert_ascii_metadata(
                    response.metadata_mut(),
                    REQUEST_ID_HEADER,
                    receipt.request_id.as_str(),
                );
                insert_ascii_metadata(
                    response.metadata_mut(),
                    CORRELATION_ID_HEADER,
                    receipt.correlation_id.as_str(),
                );
                insert_ascii_metadata(
                    response.metadata_mut(),
                    TRACE_ID_HEADER,
                    receipt.trace_id.as_str(),
                );
                Ok(response)
            }
            Err(error) => Err(status_from_safe_error(error.into())),
        }
    }
}

fn metadata_from_grpc(metadata: &MetadataMap) -> Result<QueryIngressMetadata, SafeTransportError> {
    Ok(QueryIngressMetadata {
        tenant_id: metadata_value(metadata, TENANT_HEADER)?,
        request_id: metadata_value(metadata, REQUEST_ID_HEADER)?,
        correlation_id: metadata_value(metadata, CORRELATION_ID_HEADER)?,
        trace_id: metadata_value(metadata, TRACE_ID_HEADER)?,
        timeout_millis: optional_u64_metadata(metadata, TIMEOUT_HEADER)?,
    })
}

fn metadata_value(
    metadata: &MetadataMap,
    name: &'static str,
) -> Result<Option<String>, SafeTransportError> {
    let Some(value) = metadata.get(name) else {
        return Ok(None);
    };
    value
        .to_str()
        .map(|value| Some(value.to_owned()))
        .map_err(|_| invalid_metadata_error())
}

fn optional_u64_metadata(
    metadata: &MetadataMap,
    name: &'static str,
) -> Result<Option<u64>, SafeTransportError> {
    let Some(value) = metadata_value(metadata, name)? else {
        return Ok(None);
    };
    value
        .parse()
        .map(Some)
        .map_err(|_| invalid_metadata_error())
}

fn invalid_metadata_error() -> SafeTransportError {
    SafeTransportError {
        code: "TRANSPORT_METADATA_INVALID".to_owned(),
        category: ErrorCategory::InvalidArgument,
        retryable: false,
        safe_message: "The request metadata is invalid.".to_owned(),
        retry_after_millis: None,
    }
}

fn status_from_safe_error(error: SafeTransportError) -> Status {
    let mut metadata = MetadataMap::new();
    insert_ascii_metadata(&mut metadata, ERROR_CODE_METADATA, &error.code);
    insert_ascii_metadata(
        &mut metadata,
        ERROR_RETRYABLE_METADATA,
        if error.retryable { "true" } else { "false" },
    );
    if let Some(retry_after_millis) = error.retry_after_millis {
        insert_ascii_metadata(
            &mut metadata,
            RETRY_AFTER_MILLIS_HEADER,
            &retry_after_millis.to_string(),
        );
    }
    Status::with_metadata(
        code_for_category(error.category),
        error.safe_message,
        metadata,
    )
}

fn code_for_category(category: ErrorCategory) -> Code {
    match category {
        ErrorCategory::InvalidArgument => Code::InvalidArgument,
        ErrorCategory::Authentication => Code::Unauthenticated,
        ErrorCategory::Authorization => Code::PermissionDenied,
        ErrorCategory::Conflict => Code::Aborted,
        ErrorCategory::NotFound => Code::NotFound,
        ErrorCategory::RateLimit => Code::ResourceExhausted,
        ErrorCategory::Dependency | ErrorCategory::Unavailable => Code::Unavailable,
        ErrorCategory::Internal => Code::Internal,
    }
}

fn insert_ascii_metadata(metadata: &mut MetadataMap, name: &'static str, value: &str) {
    let Ok(value) = MetadataValue::try_from(value) else {
        return;
    };
    metadata.insert(name, value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BUSINESS_TRANSACTION_HEADER, IDEMPOTENCY_KEY_HEADER};

    #[test]
    fn query_metadata_does_not_depend_on_mutation_only_metadata() {
        let mut metadata = MetadataMap::new();
        insert_ascii_metadata(&mut metadata, TENANT_HEADER, "tenant-1");
        let parsed = metadata_from_grpc(&metadata).unwrap();
        assert_eq!(parsed.tenant_id.as_deref(), Some("tenant-1"));

        insert_ascii_metadata(&mut metadata, IDEMPOTENCY_KEY_HEADER, "ignored-idem");
        insert_ascii_metadata(
            &mut metadata,
            BUSINESS_TRANSACTION_HEADER,
            "ignored-transaction",
        );
        let parsed = metadata_from_grpc(&metadata).unwrap();
        assert_eq!(parsed.tenant_id.as_deref(), Some("tenant-1"));
    }

    #[test]
    fn query_safe_error_code_is_returned_as_metadata() {
        let status = status_from_safe_error(SafeTransportError {
            code: "QUERY_PERMISSION_DENIED".to_owned(),
            category: ErrorCategory::Authorization,
            retryable: false,
            safe_message: "You are not permitted to perform this query.".to_owned(),
            retry_after_millis: None,
        });
        assert_eq!(status.code(), Code::PermissionDenied);
        assert_eq!(
            status
                .metadata()
                .get(ERROR_CODE_METADATA)
                .unwrap()
                .to_str()
                .unwrap(),
            "QUERY_PERMISSION_DENIED"
        );
    }
}
