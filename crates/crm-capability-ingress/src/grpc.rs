use crate::{
    BUSINESS_TRANSACTION_HEADER, CAUSATION_ID_HEADER, CORRELATION_ID_HEADER,
    CapabilityCallEnvelope, CapabilityIngress, CapabilityRoute, IDEMPOTENCY_KEY_HEADER,
    IngressMetadata, REQUEST_ID_HEADER, RETRY_AFTER_MILLIS_HEADER, SafeTransportError,
    TENANT_HEADER, TIMEOUT_HEADER, TRACE_ID_HEADER,
};
use crm_capability_runtime::{ApprovalEvidence, CapabilityExecutionResult};
use crm_module_sdk::{ErrorCategory, TypedPayload};
use tonic::metadata::{MetadataMap, MetadataValue};
use tonic::{Code, Request, Response, Status};

pub const ERROR_CODE_METADATA: &str = "x-error-code";
pub const ERROR_RETRYABLE_METADATA: &str = "x-error-retryable";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrpcCapabilityMessage {
    pub route: CapabilityRoute,
    pub input: TypedPayload,
    pub approval: Option<ApprovalEvidence>,
}

#[derive(Debug, Clone)]
pub struct GrpcCapabilityMiddleware {
    ingress: CapabilityIngress,
}

impl GrpcCapabilityMiddleware {
    pub fn new(ingress: CapabilityIngress) -> Self {
        Self { ingress }
    }

    pub async fn handle(
        &self,
        request: Request<GrpcCapabilityMessage>,
    ) -> Result<Response<CapabilityExecutionResult>, Status> {
        let authorization = metadata_value(request.metadata(), "authorization")
            .map_err(status_from_safe_error)?
            .unwrap_or_default();
        let ingress_metadata = metadata_from_grpc(request.metadata())
            .map_err(status_from_safe_error)?;
        let message = request.into_inner();
        let envelope = CapabilityCallEnvelope {
            route: message.route,
            input: message.input,
            approval: message.approval,
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

fn metadata_from_grpc(metadata: &MetadataMap) -> Result<IngressMetadata, SafeTransportError> {
    Ok(IngressMetadata {
        tenant_id: metadata_value(metadata, TENANT_HEADER)?,
        request_id: metadata_value(metadata, REQUEST_ID_HEADER)?,
        correlation_id: metadata_value(metadata, CORRELATION_ID_HEADER)?,
        causation_id: metadata_value(metadata, CAUSATION_ID_HEADER)?,
        trace_id: metadata_value(metadata, TRACE_ID_HEADER)?,
        idempotency_key: metadata_value(metadata, IDEMPOTENCY_KEY_HEADER)?,
        business_transaction_id: metadata_value(metadata, BUSINESS_TRANSACTION_HEADER)?,
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
    value.parse().map(Some).map_err(|_| invalid_metadata_error())
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

    #[test]
    fn maps_safe_categories_to_grpc_codes() {
        assert_eq!(
            code_for_category(ErrorCategory::Authentication),
            Code::Unauthenticated
        );
        assert_eq!(
            code_for_category(ErrorCategory::Authorization),
            Code::PermissionDenied
        );
        assert_eq!(
            code_for_category(ErrorCategory::RateLimit),
            Code::ResourceExhausted
        );
        assert_eq!(
            code_for_category(ErrorCategory::Unavailable),
            Code::Unavailable
        );
    }

    #[test]
    fn typed_error_code_is_returned_as_metadata() {
        let status = status_from_safe_error(SafeTransportError {
            code: "CAPABILITY_PERMISSION_DENIED".to_owned(),
            category: ErrorCategory::Authorization,
            retryable: false,
            safe_message: "You are not permitted to perform this action.".to_owned(),
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
            "CAPABILITY_PERMISSION_DENIED"
        );
    }
}
