use crate::ApplicationComponents;
use crm_capability_ingress::{CapabilityRoute, GrpcCapabilityMessage, GrpcQueryMessage};
use crm_capability_runtime::ApprovalEvidence;
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, DataClass, ModuleId, PayloadEncoding,
    RetentionPolicyId, SchemaId, SchemaVersion, TypedPayload,
};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub mod gateway_v1 {
    tonic::include_proto!("crm.gateway.v1");

    // Internal compatibility alias keeps the runtime server wiring stable while
    // the published service follows Buf's required `*Service` naming rule.
    pub mod application_gateway_server {
        pub use super::application_gateway_service_server::ApplicationGatewayServiceServer as ApplicationGatewayServer;
    }
}

use gateway_v1::{
    MutateRequest, MutateResponse, QueryRequest, QueryResponse, ResourceRef as WireResourceRef,
};

#[derive(Clone)]
pub struct ApplicationGatewayService {
    components: Arc<ApplicationComponents>,
}

impl ApplicationGatewayService {
    pub fn new(components: Arc<ApplicationComponents>) -> Self {
        Self { components }
    }
}

#[tonic::async_trait]
impl gateway_v1::application_gateway_service_server::ApplicationGatewayService
    for ApplicationGatewayService
{
    async fn mutate(
        &self,
        request: Request<MutateRequest>,
    ) -> Result<Response<MutateResponse>, Status> {
        let metadata = request.metadata().clone();
        let request = request.into_inner();
        let input = decode_payload(request.input)?;
        let approval = decode_approval(request.approval)?;
        let route = decode_route(
            &request.owner_module_id,
            &request.capability_id,
            &request.capability_version,
            &input,
        )?;
        let mut governed_request = Request::new(GrpcCapabilityMessage {
            route,
            input,
            approval,
        });
        *governed_request.metadata_mut() = metadata;
        let result = self
            .components
            .mutation_grpc
            .handle(governed_request)
            .await?;
        let metadata = result.metadata().clone();
        let mut response = Response::new(encode_mutation_result(result.into_inner())?);
        *response.metadata_mut() = metadata;
        Ok(response)
    }

    async fn query(
        &self,
        request: Request<QueryRequest>,
    ) -> Result<Response<QueryResponse>, Status> {
        let metadata = request.metadata().clone();
        let request = request.into_inner();
        let input = decode_payload(request.input)?;
        let route = decode_route(
            &request.owner_module_id,
            &request.capability_id,
            &request.capability_version,
            &input,
        )?;
        let mut governed_request = Request::new(GrpcQueryMessage { route, input });
        *governed_request.metadata_mut() = metadata;
        let result = self.components.query_grpc.handle(governed_request).await?;
        let metadata = result.metadata().clone();
        let mut response = Response::new(encode_query_result(result.into_inner())?);
        *response.metadata_mut() = metadata;
        Ok(response)
    }
}

fn decode_route(
    owner_module_id: &str,
    capability_id: &str,
    capability_version: &str,
    input: &TypedPayload,
) -> Result<CapabilityRoute, Status> {
    Ok(CapabilityRoute {
        owner_module_id: ModuleId::try_new(owner_module_id.to_owned())
            .map_err(|_| Status::invalid_argument("owner_module_id is invalid"))?,
        capability_id: crm_module_sdk::CapabilityId::try_new(capability_id.to_owned())
            .map_err(|_| Status::invalid_argument("capability_id is invalid"))?,
        capability_version: crm_module_sdk::CapabilityVersion::try_new(
            capability_version.to_owned(),
        )
        .map_err(|_| Status::invalid_argument("capability_version is invalid"))?,
        schema_version: input.schema_version.clone(),
    })
}

fn decode_payload(input: Option<gateway_v1::TypedPayload>) -> Result<TypedPayload, Status> {
    let input = input.ok_or_else(|| Status::invalid_argument("input is required"))?;
    let descriptor_hash: [u8; 32] = input
        .descriptor_hash
        .try_into()
        .map_err(|_| Status::invalid_argument("descriptor_hash must be 32 bytes"))?;
    let payload = TypedPayload {
        owner: ModuleId::try_new(input.owner_module_id)
            .map_err(|_| Status::invalid_argument("input owner_module_id is invalid"))?,
        schema_id: SchemaId::try_new(input.schema_id)
            .map_err(|_| Status::invalid_argument("schema_id is invalid"))?,
        schema_version: SchemaVersion::try_new(input.schema_version)
            .map_err(|_| Status::invalid_argument("schema_version is invalid"))?,
        descriptor_hash,
        data_class: parse_data_class(&input.data_class)?,
        encoding: parse_encoding(&input.encoding)?,
        maximum_size_bytes: input.maximum_size_bytes,
        retention_policy_id: RetentionPolicyId::try_new(input.retention_policy_id)
            .map_err(|_| Status::invalid_argument("retention_policy_id is invalid"))?,
        bytes: input.payload,
    };
    payload
        .validate()
        .map_err(|_| Status::invalid_argument("input payload is invalid"))?;
    Ok(payload)
}

fn decode_approval(
    approval: Option<gateway_v1::ApprovalEvidence>,
) -> Result<Option<ApprovalEvidence>, Status> {
    approval
        .map(|approval| {
            let input_hash: [u8; 32] = approval
                .input_hash
                .try_into()
                .map_err(|_| Status::invalid_argument("approval input_hash must be 32 bytes"))?;
            Ok(ApprovalEvidence {
                approval_id: approval.approval_id,
                actor_id: ActorId::try_new(approval.actor_id)
                    .map_err(|_| Status::invalid_argument("approval actor_id is invalid"))?,
                capability_id: CapabilityId::try_new(approval.capability_id)
                    .map_err(|_| Status::invalid_argument("approval capability_id is invalid"))?,
                capability_version: CapabilityVersion::try_new(approval.capability_version)
                    .map_err(|_| {
                        Status::invalid_argument("approval capability_version is invalid")
                    })?,
                input_hash,
                policy_version: approval.policy_version,
                expires_at_unix_nanos: approval.expires_at_unix_nanos,
                opaque_proof: approval.opaque_proof,
            })
        })
        .transpose()
}

fn encode_mutation_result(
    result: crm_capability_runtime::CapabilityExecutionResult,
) -> Result<MutateResponse, Status> {
    Ok(MutateResponse {
        output: result.output.map(encode_payload).transpose()?,
        affected_resources: result
            .affected_resources
            .into_iter()
            .map(|resource| WireResourceRef {
                resource_type: resource.resource_type,
                resource_id: resource.resource_id,
                version: resource.version,
            })
            .collect(),
        replayed: result.replayed,
    })
}

fn encode_query_result(
    result: crm_query_runtime::QueryExecutionResult,
) -> Result<QueryResponse, Status> {
    Ok(QueryResponse {
        output: Some(encode_payload(result.output)?),
    })
}

fn encode_payload(payload: TypedPayload) -> Result<gateway_v1::TypedPayload, Status> {
    payload
        .validate()
        .map_err(|_| Status::internal("governed output payload is invalid"))?;
    Ok(gateway_v1::TypedPayload {
        owner_module_id: payload.owner.as_str().to_owned(),
        schema_id: payload.schema_id.as_str().to_owned(),
        schema_version: payload.schema_version.as_str().to_owned(),
        descriptor_hash: payload.descriptor_hash.to_vec(),
        data_class: data_class_name(payload.data_class).to_owned(),
        encoding: encoding_name(payload.encoding).to_owned(),
        maximum_size_bytes: payload.maximum_size_bytes,
        retention_policy_id: payload.retention_policy_id.as_str().to_owned(),
        payload: payload.bytes,
    })
}

fn parse_data_class(value: &str) -> Result<DataClass, Status> {
    match value {
        "public" => Ok(DataClass::Public),
        "internal" => Ok(DataClass::Internal),
        "confidential" => Ok(DataClass::Confidential),
        "restricted" => Ok(DataClass::Restricted),
        "personal" => Ok(DataClass::Personal),
        "sensitive_personal" => Ok(DataClass::SensitivePersonal),
        "biometric" => Ok(DataClass::Biometric),
        "financial" => Ok(DataClass::Financial),
        "credential" => Ok(DataClass::Credential),
        _ => Err(Status::invalid_argument("data_class is invalid")),
    }
}

fn data_class_name(value: DataClass) -> &'static str {
    match value {
        DataClass::Public => "public",
        DataClass::Internal => "internal",
        DataClass::Confidential => "confidential",
        DataClass::Restricted => "restricted",
        DataClass::Personal => "personal",
        DataClass::SensitivePersonal => "sensitive_personal",
        DataClass::Biometric => "biometric",
        DataClass::Financial => "financial",
        DataClass::Credential => "credential",
    }
}

fn parse_encoding(value: &str) -> Result<PayloadEncoding, Status> {
    match value {
        "protobuf" => Ok(PayloadEncoding::Protobuf),
        "json" => Ok(PayloadEncoding::Json),
        "utf8_text" => Ok(PayloadEncoding::Utf8Text),
        "binary" => Ok(PayloadEncoding::Binary),
        _ => Err(Status::invalid_argument("encoding is invalid")),
    }
}

fn encoding_name(value: PayloadEncoding) -> &'static str {
    match value {
        PayloadEncoding::Protobuf => "protobuf",
        PayloadEncoding::Json => "json",
        PayloadEncoding::Utf8Text => "utf8_text",
        PayloadEncoding::Binary => "binary",
    }
}
