#![forbid(unsafe_code)]

//! Governed read-only adapter for tenant-scoped Admin Studio metadata.
//!
//! The adapter depends on a narrow read port instead of a mutation execution
//! context. Public metadata queries therefore require only the normal
//! `QueryExecutionContext`; they do not invent idempotency keys or business
//! transaction identifiers merely to read tenant-scoped metadata.

use crm_capability_runtime::CapabilityDefinition;
use crm_metadata_api_adapter::{
    ACTIVATION_QUERY_CAPABILITY, ACTIVATION_REQUEST_SCHEMA, ACTIVATION_RESPONSE_SCHEMA,
    CONTRACT_VERSION, IMPACT_QUERY_CAPABILITY, IMPACT_REQUEST_SCHEMA, IMPACT_RESPONSE_SCHEMA,
    MAX_PROTOBUF_BYTES, METADATA_MODULE_ID, METADATA_QUERY_CAPABILITY_IDS,
    REVISION_QUERY_CAPABILITY, REVISION_REQUEST_SCHEMA, REVISION_RESPONSE_SCHEMA,
    activation_state_to_wire, impact_to_wire, metadata_capability_definition, parse_revision_id,
    protobuf_payload, revision_to_wire,
};
use crm_metadata_runtime::{
    MetadataBundleDraft, MetadataImpactReport, MetadataRevisionId, TenantMetadataSnapshot,
};
use crm_module_sdk::{DataClass, ErrorCategory, PortFuture, SdkError, TenantId};
use crm_proto_contracts::{crm::metadata::v1 as wire, message_descriptor_hash};
use crm_query_runtime::{
    QueryExecutionResult, QueryExecutor, QueryRequest, QuerySemanticValidator,
};
use prost::Message;
use std::fmt;
use std::sync::Arc;

pub trait MetadataQueryStore: Send + Sync {
    fn impact_for<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        candidate_revision: &'a MetadataRevisionId,
    ) -> PortFuture<'a, Result<MetadataImpactReport, SdkError>>;

    fn revision<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        revision_id: &'a MetadataRevisionId,
    ) -> PortFuture<'a, Result<Option<MetadataBundleDraft>, SdkError>>;

    fn tenant_state<'a>(
        &'a self,
        tenant_id: &'a TenantId,
    ) -> PortFuture<'a, Result<TenantMetadataSnapshot, SdkError>>;
}

#[derive(Clone)]
pub struct MetadataQueryAdapter {
    store: Arc<dyn MetadataQueryStore>,
}

impl fmt::Debug for MetadataQueryAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MetadataQueryAdapter")
            .field("store", &"dyn MetadataQueryStore")
            .finish()
    }
}

impl MetadataQueryAdapter {
    pub fn new(store: Arc<dyn MetadataQueryStore>) -> Self {
        Self { store }
    }
}

impl QuerySemanticValidator for MetadataQueryAdapter {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            match validated_route(definition, request)? {
                MetadataQueryRoute::Impact => {
                    let command: wire::GetMetadataImpactRequest =
                        decode_query_input(request, IMPACT_REQUEST_SCHEMA)?;
                    parse_revision_id(&command.candidate_revision_id, "candidate_revision_id")?;
                }
                MetadataQueryRoute::Revision => {
                    let command: wire::GetMetadataRevisionRequest =
                        decode_query_input(request, REVISION_REQUEST_SCHEMA)?;
                    parse_revision_id(&command.revision_id, "revision_id")?;
                }
                MetadataQueryRoute::Activation => {
                    let _: wire::GetMetadataActivationRequest =
                        decode_query_input(request, ACTIVATION_REQUEST_SCHEMA)?;
                }
            }
            Ok(())
        })
    }
}

impl QueryExecutor for MetadataQueryAdapter {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: QueryRequest,
    ) -> PortFuture<'a, Result<QueryExecutionResult, SdkError>> {
        Box::pin(async move {
            let route = validated_route(definition, &request)?;
            let tenant_id = &request.context.tenant_id;
            let output = match route {
                MetadataQueryRoute::Impact => {
                    let command: wire::GetMetadataImpactRequest =
                        decode_query_input(&request, IMPACT_REQUEST_SCHEMA)?;
                    let revision_id =
                        parse_revision_id(&command.candidate_revision_id, "candidate_revision_id")?;
                    let impact = self.store.impact_for(tenant_id, &revision_id).await?;
                    protobuf_payload(
                        METADATA_MODULE_ID,
                        IMPACT_RESPONSE_SCHEMA,
                        DataClass::Confidential,
                        &wire::GetMetadataImpactResponse {
                            impact: Some(impact_to_wire(&impact)),
                        },
                    )?
                }
                MetadataQueryRoute::Revision => {
                    let command: wire::GetMetadataRevisionRequest =
                        decode_query_input(&request, REVISION_REQUEST_SCHEMA)?;
                    let revision_id = parse_revision_id(&command.revision_id, "revision_id")?;
                    let bundle = self
                        .store
                        .revision(tenant_id, &revision_id)
                        .await?
                        .ok_or_else(revision_not_found)?;
                    protobuf_payload(
                        METADATA_MODULE_ID,
                        REVISION_RESPONSE_SCHEMA,
                        DataClass::Confidential,
                        &wire::GetMetadataRevisionResponse {
                            revision: Some(revision_to_wire(&revision_id, &bundle)),
                        },
                    )?
                }
                MetadataQueryRoute::Activation => {
                    let _: wire::GetMetadataActivationRequest =
                        decode_query_input(&request, ACTIVATION_REQUEST_SCHEMA)?;
                    let state = self.store.tenant_state(tenant_id).await?;
                    protobuf_payload(
                        METADATA_MODULE_ID,
                        ACTIVATION_RESPONSE_SCHEMA,
                        DataClass::Confidential,
                        &wire::GetMetadataActivationResponse {
                            state: Some(activation_state_to_wire(&state)),
                        },
                    )?
                }
            };
            Ok(QueryExecutionResult { output })
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetadataQueryRoute {
    Impact,
    Revision,
    Activation,
}

fn validated_route(
    definition: &CapabilityDefinition,
    request: &QueryRequest,
) -> Result<MetadataQueryRoute, SdkError> {
    let capability_id = definition.capability_id.as_str();
    if !METADATA_QUERY_CAPABILITY_IDS.contains(&capability_id) {
        return Err(configuration_error(
            "METADATA_QUERY_ROUTE_UNSUPPORTED",
            "The metadata query route is unsupported.",
        ));
    }
    let expected = metadata_capability_definition(capability_id)?;
    if definition != &expected {
        return Err(configuration_error(
            "METADATA_QUERY_DEFINITION_MISMATCH",
            "The metadata query definition binding is invalid.",
        ));
    }
    if request.owner_module_id != expected.owner_module_id
        || request.context.capability_id != expected.capability_id
        || request.context.capability_version != expected.capability_version
    {
        return Err(configuration_error(
            "METADATA_QUERY_REQUEST_BINDING_MISMATCH",
            "The metadata query request binding is invalid.",
        ));
    }
    if !expected.input_contract.matches(&request.input) {
        return Err(SdkError::new(
            "METADATA_QUERY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The metadata query input does not match the required contract.",
        ));
    }

    match capability_id {
        IMPACT_QUERY_CAPABILITY => Ok(MetadataQueryRoute::Impact),
        REVISION_QUERY_CAPABILITY => Ok(MetadataQueryRoute::Revision),
        ACTIVATION_QUERY_CAPABILITY => Ok(MetadataQueryRoute::Activation),
        _ => Err(configuration_error(
            "METADATA_QUERY_ROUTE_UNSUPPORTED",
            "The metadata query route is unsupported.",
        )),
    }
}

fn decode_query_input<M>(request: &QueryRequest, schema_id: &'static str) -> Result<M, SdkError>
where
    M: Message + Default,
{
    if request.input.owner.as_str() != METADATA_MODULE_ID
        || request.input.schema_id.as_str() != schema_id
        || request.input.schema_version.as_str() != CONTRACT_VERSION
        || request.input.descriptor_hash != message_descriptor_hash(schema_id)
        || request.input.data_class != DataClass::Confidential
        || request.input.encoding != crm_module_sdk::PayloadEncoding::Protobuf
        || request.input.maximum_size_bytes != MAX_PROTOBUF_BYTES
        || request.input.validate().is_err()
    {
        return Err(SdkError::new(
            "METADATA_QUERY_INPUT_CONTRACT_MISMATCH",
            ErrorCategory::InvalidArgument,
            false,
            "The metadata query input does not match the required contract.",
        ));
    }

    M::decode(request.input.bytes.as_slice()).map_err(|_| {
        SdkError::new(
            "METADATA_QUERY_INPUT_PROTOBUF_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "The metadata query input is not valid Protobuf.",
        )
    })
}

fn revision_not_found() -> SdkError {
    SdkError::new(
        "METADATA_REVISION_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested metadata revision does not exist.",
    )
}

fn configuration_error(code: &'static str, safe_message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::Internal, false, safe_message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_metadata_runtime::{
        MetadataChange, MetadataChangeType, MetadataDocument, MetadataId, MetadataImpactSeverity,
        MetadataKey, MetadataKind,
    };
    use crm_module_sdk::{
        ActorId, CapabilityId, CapabilityVersion, CorrelationId, ModuleId, RequestId,
        SchemaVersion, TraceId,
    };
    use crm_query_runtime::QueryExecutionContext;

    #[derive(Debug, Clone)]
    struct FakeStore {
        revision_id: MetadataRevisionId,
        bundle: MetadataBundleDraft,
        impact: MetadataImpactReport,
        state: TenantMetadataSnapshot,
    }

    impl MetadataQueryStore for FakeStore {
        fn impact_for<'a>(
            &'a self,
            tenant_id: &'a TenantId,
            candidate_revision: &'a MetadataRevisionId,
        ) -> PortFuture<'a, Result<MetadataImpactReport, SdkError>> {
            Box::pin(async move {
                assert_eq!(tenant_id.as_str(), "tenant-a");
                assert_eq!(candidate_revision, &self.revision_id);
                Ok(self.impact.clone())
            })
        }

        fn revision<'a>(
            &'a self,
            tenant_id: &'a TenantId,
            revision_id: &'a MetadataRevisionId,
        ) -> PortFuture<'a, Result<Option<MetadataBundleDraft>, SdkError>> {
            Box::pin(async move {
                assert_eq!(tenant_id.as_str(), "tenant-a");
                assert_eq!(revision_id, &self.revision_id);
                Ok(Some(self.bundle.clone()))
            })
        }

        fn tenant_state<'a>(
            &'a self,
            tenant_id: &'a TenantId,
        ) -> PortFuture<'a, Result<TenantMetadataSnapshot, SdkError>> {
            Box::pin(async move {
                assert_eq!(tenant_id.as_str(), "tenant-a");
                Ok(self.state.clone())
            })
        }
    }

    fn fixture() -> FakeStore {
        let document = MetadataDocument::new(
            MetadataKey::new(
                MetadataKind::Object,
                MetadataId::try_new("crm.sales.deal").unwrap(),
            ),
            "crm.metadata.definition/v1",
            br#"{"kind":"object"}"#.to_vec(),
            [],
        )
        .unwrap();
        let bundle = MetadataBundleDraft::new([document]).unwrap();
        let revision_id = bundle.revision_id();
        FakeStore {
            revision_id: revision_id.clone(),
            bundle,
            impact: MetadataImpactReport {
                current_revision: None,
                candidate_revision: revision_id.clone(),
                changes: vec![MetadataChange {
                    key: MetadataKey::new(
                        MetadataKind::Object,
                        MetadataId::try_new("crm.sales.deal").unwrap(),
                    ),
                    change_type: MetadataChangeType::Added,
                    severity: MetadataImpactSeverity::Informational,
                }],
            },
            state: TenantMetadataSnapshot {
                generation: 3,
                active_revision: Some(revision_id),
                rollback_depth: 2,
            },
        }
    }

    fn request<M>(capability_id: &str, schema_id: &str, message: &M) -> QueryRequest
    where
        M: Message,
    {
        QueryRequest {
            owner_module_id: ModuleId::try_new(METADATA_MODULE_ID).unwrap(),
            context: QueryExecutionContext {
                tenant_id: TenantId::try_new("tenant-a").unwrap(),
                actor_id: ActorId::try_new("actor-a").unwrap(),
                request_id: RequestId::try_new("request-a").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
                trace_id: TraceId::try_new("trace-a").unwrap(),
                capability_id: CapabilityId::try_new(capability_id).unwrap(),
                capability_version: CapabilityVersion::try_new(CONTRACT_VERSION).unwrap(),
                schema_version: SchemaVersion::try_new(CONTRACT_VERSION).unwrap(),
                request_started_at_unix_nanos: 1,
            },
            input: protobuf_payload(
                METADATA_MODULE_ID,
                schema_id,
                DataClass::Confidential,
                message,
            )
            .unwrap(),
            input_hash: [1; 32],
        }
    }

    #[tokio::test]
    async fn revision_query_executes_from_query_context_without_mutation_identifiers() {
        let store = fixture();
        let revision_id = store.revision_id.to_hex();
        let adapter = MetadataQueryAdapter::new(Arc::new(store));
        let definition = metadata_capability_definition(REVISION_QUERY_CAPABILITY).unwrap();
        let request = request(
            REVISION_QUERY_CAPABILITY,
            REVISION_REQUEST_SCHEMA,
            &wire::GetMetadataRevisionRequest {
                revision_id: revision_id.clone(),
            },
        );

        adapter.validate(&definition, &request).await.unwrap();
        let result = adapter.execute(&definition, request).await.unwrap();
        let response =
            wire::GetMetadataRevisionResponse::decode(result.output.bytes.as_slice()).unwrap();

        assert_eq!(response.revision.unwrap().revision_id, revision_id);
    }

    #[tokio::test]
    async fn activation_query_returns_tenant_scoped_snapshot() {
        let adapter = MetadataQueryAdapter::new(Arc::new(fixture()));
        let definition = metadata_capability_definition(ACTIVATION_QUERY_CAPABILITY).unwrap();
        let request = request(
            ACTIVATION_QUERY_CAPABILITY,
            ACTIVATION_REQUEST_SCHEMA,
            &wire::GetMetadataActivationRequest {},
        );

        let result = adapter.execute(&definition, request).await.unwrap();
        let response =
            wire::GetMetadataActivationResponse::decode(result.output.bytes.as_slice()).unwrap();
        let state = response.state.unwrap();

        assert_eq!(state.generation, 3);
        assert_eq!(state.rollback_depth, 2);
        assert!(!state.active_revision_id.is_empty());
    }

    #[test]
    fn unknown_or_mutation_coordinate_cannot_enter_metadata_query_router() {
        let definition = metadata_capability_definition(IMPACT_QUERY_CAPABILITY).unwrap();
        let mut request = request(
            IMPACT_QUERY_CAPABILITY,
            IMPACT_REQUEST_SCHEMA,
            &wire::GetMetadataImpactRequest {
                candidate_revision_id: "ab".repeat(32),
            },
        );
        request.context.capability_id = CapabilityId::try_new("metadata.bundle.publish").unwrap();

        assert_eq!(
            validated_route(&definition, &request).unwrap_err().code,
            "METADATA_QUERY_REQUEST_BINDING_MISMATCH"
        );
    }
}
