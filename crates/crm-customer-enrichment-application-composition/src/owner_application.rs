use crm_capability_plan_support as support;
use crm_customer_enrichment::{
    PARTY_DISPLAY_NAME_UPDATE_CAPABILITY_ID, PARTY_DISPLAY_NAME_UPDATE_CAPABILITY_VERSION,
    PartyDisplayNameApplicationPort, PartyDisplayNameApplicationRequest,
    PartyDisplayNameApplicationResult, PartySnapshotPort, PartySnapshotRequest,
};
use crm_customer_enrichment_application_adapter::APPLY_PARTY_DISPLAY_NAME_CAPABILITY;
use crm_customer_enrichment_capability_adapter::MODULE_ID as CUSTOMER_ENRICHMENT_MODULE_ID;
use crm_module_sdk::{
    BusinessTransactionId, CapabilityClient, CapabilityId, CapabilityInvocation, CapabilityOutcome,
    CapabilityVersion, CausationId, Clock, CorrelationId, DataClass, ErrorCategory,
    ExecutionContext, IdempotencyKey, ModuleExecutionContext, ModuleId, PayloadEncoding, PortFuture,
    RequestId, SchemaVersion, SdkError, TraceId,
};
use crm_parties_capability_adapter::{
    MODULE_ID as PARTIES_MODULE_ID, RECORD_TYPE as PARTY_RECORD_TYPE, UPDATE_CAPABILITY,
    UPDATE_REQUEST_SCHEMA, UPDATE_RESPONSE_SCHEMA,
};
use crm_proto_contracts::crm::{customer::v1 as customer, parties::v1 as party_wire};
use prost::Message;
use std::fmt;
use std::sync::Arc;

const PARTY_VERSION_CONFLICT_CODE: &str = "PARTIES_PARTY_VERSION_CONFLICT";
const CAPABILITY_PERMISSION_DENIED_CODE: &str = "CAPABILITY_PERMISSION_DENIED";

/// Exact governed owner-capability adapter for the first Customer Enrichment application slice.
///
/// The adapter invokes only `parties.party.update@1.0.0` through the injected `CapabilityClient`.
/// It never accesses Party persistence and therefore preserves the ordinary target authorization,
/// rate-limit, semantic-validation, idempotency, transaction, audit and outbox boundaries.
#[derive(Clone)]
pub struct GatewayPartyDisplayNameApplicationPort {
    capabilities: Arc<dyn CapabilityClient>,
    party_snapshots: Arc<dyn PartySnapshotPort>,
    clock: Arc<dyn Clock>,
}

impl GatewayPartyDisplayNameApplicationPort {
    pub fn new(
        capabilities: Arc<dyn CapabilityClient>,
        party_snapshots: Arc<dyn PartySnapshotPort>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, SdkError> {
        ensure_exact_owner_coordinate()?;
        Ok(Self {
            capabilities,
            party_snapshots,
            clock,
        })
    }

    async fn invoke_owner(
        &self,
        request: &PartyDisplayNameApplicationRequest,
    ) -> Result<CapabilityOutcome, SdkError> {
        let context = owner_call_context(request, self.clock.now_unix_nanos())?;
        let input = support::protobuf_payload(
            PARTIES_MODULE_ID,
            UPDATE_REQUEST_SCHEMA,
            DataClass::Personal,
            &party_wire::UpdatePartyRequest {
                party_ref: Some(customer::PartyRef {
                    party_id: request.party_id.as_str().to_owned(),
                }),
                expected_version: request.expected_party_resource_version,
                display_name: request.reviewed_display_name.clone(),
            },
        )?;
        self.capabilities
            .invoke(
                &context,
                CapabilityInvocation {
                    capability_id: configured(CapabilityId::try_new(
                        PARTY_DISPLAY_NAME_UPDATE_CAPABILITY_ID,
                    ))?,
                    capability_version: configured(CapabilityVersion::try_new(
                        PARTY_DISPLAY_NAME_UPDATE_CAPABILITY_VERSION,
                    ))?,
                    input,
                },
            )
            .await
    }

    async fn resolve_stale_target(
        &self,
        request: &PartyDisplayNameApplicationRequest,
    ) -> Result<PartyDisplayNameApplicationResult, SdkError> {
        let now_unix_nanos = self.clock.now_unix_nanos();
        let requested_at_unix_ms = nonnegative_unix_ms(now_unix_nanos)?;
        let snapshot = self
            .party_snapshots
            .get(PartySnapshotRequest {
                tenant_id: request.tenant_id.clone(),
                actor_id: request.actor_id.clone(),
                request_identity: request.application_attempt_id.as_str().to_owned(),
                party_id: request.party_id.clone(),
                requested_at_unix_ms,
            })
            .await?;
        if snapshot.party_id != request.party_id || snapshot.resource_version <= 0 {
            return Err(owner_response_invalid(
                "stale-target snapshot did not preserve the exact Party identity and version",
            ));
        }
        Ok(PartyDisplayNameApplicationResult::StaleTarget {
            actual_party_resource_version: snapshot.resource_version,
        })
    }
}

impl fmt::Debug for GatewayPartyDisplayNameApplicationPort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GatewayPartyDisplayNameApplicationPort")
            .field("capabilities", &"dyn CapabilityClient")
            .field("party_snapshots", &"dyn PartySnapshotPort")
            .field("clock", &"dyn Clock")
            .finish()
    }
}

impl PartyDisplayNameApplicationPort for GatewayPartyDisplayNameApplicationPort {
    fn apply<'a>(
        &'a self,
        request: PartyDisplayNameApplicationRequest,
    ) -> PortFuture<'a, Result<PartyDisplayNameApplicationResult, SdkError>> {
        Box::pin(async move {
            validate_application_request(&request)?;
            match self.invoke_owner(&request).await {
                Ok(outcome) => {
                    let resulting_version = validate_owner_outcome(&request, outcome)?;
                    Ok(PartyDisplayNameApplicationResult::Applied {
                        business_transaction_id: request
                            .application_attempt_id
                            .as_str()
                            .to_owned(),
                        resulting_party_resource_version: resulting_version,
                    })
                }
                Err(error)
                    if error.code == CAPABILITY_PERMISSION_DENIED_CODE
                        || error.category == ErrorCategory::Authorization =>
                {
                    Ok(PartyDisplayNameApplicationResult::AuthorizationDenied {
                        decision_id: request.final_authorization_decision_id,
                    })
                }
                Err(error) if error.code == PARTY_VERSION_CONFLICT_CODE => {
                    self.resolve_stale_target(&request).await
                }
                Err(error) if error.retryable => {
                    Ok(PartyDisplayNameApplicationResult::RetryableFailure {
                        safe_code: error.code,
                    })
                }
                Err(error) => Ok(PartyDisplayNameApplicationResult::TerminalFailure {
                    safe_code: error.code,
                }),
            }
        })
    }
}

fn owner_call_context(
    request: &PartyDisplayNameApplicationRequest,
    now_unix_nanos: i64,
) -> Result<ModuleExecutionContext, SdkError> {
    if now_unix_nanos < 0 {
        return Err(owner_configuration_invalid(
            "owner invocation clock returned a negative timestamp",
        ));
    }
    Ok(ModuleExecutionContext {
        module_id: configured(ModuleId::try_new(CUSTOMER_ENRICHMENT_MODULE_ID))?,
        execution: ExecutionContext {
            tenant_id: request.tenant_id.clone(),
            actor_id: request.actor_id.clone(),
            request_id: configured(RequestId::try_new(
                request.application_attempt_id.as_str(),
            ))?,
            correlation_id: configured(CorrelationId::try_new(request.suggestion_id.as_str()))?,
            causation_id: configured(CausationId::try_new(
                request.review_decision_id.as_str(),
            ))?,
            trace_id: configured(TraceId::try_new(request.application_attempt_id.as_str()))?,
            capability_id: configured(CapabilityId::try_new(
                APPLY_PARTY_DISPLAY_NAME_CAPABILITY,
            ))?,
            capability_version: configured(CapabilityVersion::try_new(
                support::CONTRACT_VERSION,
            ))?,
            idempotency_key: configured(IdempotencyKey::try_new(
                request.target_idempotency_key.as_str(),
            ))?,
            business_transaction_id: configured(BusinessTransactionId::try_new(
                request.application_attempt_id.as_str(),
            ))?,
            schema_version: configured(SchemaVersion::try_new(support::CONTRACT_VERSION))?,
            request_started_at_unix_nanos: now_unix_nanos,
        },
    })
}

fn validate_application_request(
    request: &PartyDisplayNameApplicationRequest,
) -> Result<(), SdkError> {
    if request.expected_party_resource_version <= 0 {
        return Err(SdkError::invalid_argument(
            "customer_enrichment.expected_party_resource_version",
            "Expected Party resource version must be positive",
        ));
    }
    if request.reviewed_display_name.is_empty()
        || request.reviewed_display_name.trim() != request.reviewed_display_name
        || request.reviewed_display_name.chars().any(char::is_control)
    {
        return Err(SdkError::invalid_argument(
            "customer_enrichment.reviewed_display_name",
            "Reviewed display name must be non-empty and canonical",
        ));
    }
    if request.final_authorization_decision_id.is_empty()
        || request
            .final_authorization_decision_id
            .chars()
            .any(char::is_control)
    {
        return Err(SdkError::invalid_argument(
            "customer_enrichment.final_authorization_decision_id",
            "Final authorization decision identity is required",
        ));
    }
    configured(IdempotencyKey::try_new(
        request.target_idempotency_key.as_str(),
    ))?;
    Ok(())
}

fn validate_owner_outcome(
    request: &PartyDisplayNameApplicationRequest,
    outcome: CapabilityOutcome,
) -> Result<i64, SdkError> {
    let payload = outcome
        .output
        .ok_or_else(|| owner_response_invalid("Party update response payload is missing"))?;
    if payload.owner.as_str() != PARTIES_MODULE_ID
        || payload.schema_id.as_str() != UPDATE_RESPONSE_SCHEMA
        || payload.schema_version.as_str() != support::CONTRACT_VERSION
        || payload.descriptor_hash != support::message_descriptor_hash(UPDATE_RESPONSE_SCHEMA)
        || payload.data_class != DataClass::Personal
        || payload.encoding != PayloadEncoding::Protobuf
        || payload.maximum_size_bytes != support::MAX_PROTOBUF_BYTES
        || payload.validate().is_err()
    {
        return Err(owner_response_invalid(
            "Party update response did not match the exact typed contract",
        ));
    }
    let response = party_wire::UpdatePartyResponse::decode(payload.bytes.as_slice())
        .map_err(|_| owner_response_invalid("Party update response Protobuf is invalid"))?;
    let party = response
        .party
        .ok_or_else(|| owner_response_invalid("Party update response omitted Party state"))?;
    let party_ref = party
        .party_ref
        .ok_or_else(|| owner_response_invalid("Party update response omitted Party identity"))?;
    let version = party
        .resource_version
        .ok_or_else(|| owner_response_invalid("Party update response omitted resource version"))?
        .version;
    let expected_resulting_version = request
        .expected_party_resource_version
        .checked_add(1)
        .ok_or_else(|| owner_response_invalid("Expected Party version cannot be advanced"))?;
    if party_ref.party_id != request.party_id.as_str()
        || party.display_name != request.reviewed_display_name
        || version != expected_resulting_version
        || !outcome.affected_resources.iter().any(|resource| {
            resource.resource_type == PARTY_RECORD_TYPE
                && resource.resource_id == request.party_id.as_str()
                && resource.version == Some(version)
        })
    {
        return Err(owner_response_invalid(
            "Party update response or affected-resource evidence disagreed with the exact request",
        ));
    }
    Ok(version)
}

fn ensure_exact_owner_coordinate() -> Result<(), SdkError> {
    if UPDATE_CAPABILITY != PARTY_DISPLAY_NAME_UPDATE_CAPABILITY_ID
        || PARTY_DISPLAY_NAME_UPDATE_CAPABILITY_VERSION != support::CONTRACT_VERSION
    {
        return Err(owner_configuration_invalid(
            "Party capability adapter coordinate differs from the enrichment owner contract",
        ));
    }
    Ok(())
}

fn nonnegative_unix_ms(now_unix_nanos: i64) -> Result<i64, SdkError> {
    if now_unix_nanos < 0 {
        return Err(owner_configuration_invalid(
            "owner invocation clock returned a negative timestamp",
        ));
    }
    Ok(now_unix_nanos / 1_000_000)
}

fn configured<T>(
    result: Result<T, crm_module_sdk::IdentifierError>,
) -> Result<T, SdkError> {
    result.map_err(|error| owner_configuration_invalid(error.to_string()))
}

fn owner_configuration_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_OWNER_APPLICATION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Customer Enrichment owner-application boundary is not configured safely.",
    )
    .with_internal_reference(reference.into())
}

fn owner_response_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_OWNER_APPLICATION_RESPONSE_INVALID",
        ErrorCategory::Dependency,
        false,
        "The authoritative Party capability returned inconsistent evidence.",
    )
    .with_internal_reference(reference.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_enrichment::{
        ApplicationAttemptId, PartySnapshot, ReviewDecisionId, SuggestionId,
    };
    use crm_module_sdk::testing::{FixedClock, RecordingCapabilityClient};
    use crm_module_sdk::{ActorId, ResourceRef, TenantId};
    use serde::de::DeserializeOwned;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct FixedPartySnapshotPort {
        snapshot: PartySnapshot,
        requests: Mutex<Vec<PartySnapshotRequest>>,
    }

    impl FixedPartySnapshotPort {
        fn new(snapshot: PartySnapshot) -> Self {
            Self {
                snapshot,
                requests: Mutex::new(Vec::new()),
            }
        }

        fn requests(&self) -> Vec<PartySnapshotRequest> {
            self.requests
                .lock()
                .expect("snapshot request mutex poisoned")
                .clone()
        }
    }

    impl PartySnapshotPort for FixedPartySnapshotPort {
        fn get<'a>(
            &'a self,
            request: PartySnapshotRequest,
        ) -> PortFuture<'a, Result<PartySnapshot, SdkError>> {
            Box::pin(async move {
                self.requests
                    .lock()
                    .expect("snapshot request mutex poisoned")
                    .push(request);
                Ok(self.snapshot.clone())
            })
        }
    }

    #[tokio::test]
    async fn invokes_exact_party_update_with_attempt_derived_lineage() {
        let capabilities = Arc::new(RecordingCapabilityClient::default());
        capabilities.push_response(Ok(successful_outcome(8)));
        let snapshots = Arc::new(FixedPartySnapshotPort::new(party_snapshot(8)));
        let port = GatewayPartyDisplayNameApplicationPort::new(
            capabilities.clone(),
            snapshots,
            Arc::new(FixedClock::new(70_000_000)),
        )
        .unwrap();

        let result = port.apply(application_request()).await.unwrap();
        assert_eq!(
            result,
            PartyDisplayNameApplicationResult::Applied {
                business_transaction_id: attempt_id().as_str().to_owned(),
                resulting_party_resource_version: 8,
            }
        );

        let calls = capabilities.calls();
        assert_eq!(calls.len(), 1);
        let call = &calls[0];
        assert_eq!(call.context.module_id.as_str(), CUSTOMER_ENRICHMENT_MODULE_ID);
        assert_eq!(
            call.context.execution.capability_id.as_str(),
            APPLY_PARTY_DISPLAY_NAME_CAPABILITY
        );
        assert_eq!(
            call.context.execution.idempotency_key.as_str(),
            "customer-enrichment-target-application-1"
        );
        assert_eq!(
            call.context.execution.business_transaction_id.as_str(),
            attempt_id().as_str()
        );
        assert_eq!(
            call.request.capability_id.as_str(),
            PARTY_DISPLAY_NAME_UPDATE_CAPABILITY_ID
        );
        assert_eq!(
            call.request.capability_version.as_str(),
            PARTY_DISPLAY_NAME_UPDATE_CAPABILITY_VERSION
        );
        assert_eq!(call.request.input.owner.as_str(), PARTIES_MODULE_ID);
        assert_eq!(call.request.input.schema_id.as_str(), UPDATE_REQUEST_SCHEMA);
        let command = party_wire::UpdatePartyRequest::decode(call.request.input.bytes.as_slice())
            .unwrap();
        assert_eq!(command.party_ref.unwrap().party_id, "party-application-1");
        assert_eq!(command.expected_version, 7);
        assert_eq!(command.display_name, "Applied Company");
    }

    #[tokio::test]
    async fn maps_live_gateway_denial_to_authorization_outcome() {
        let capabilities = Arc::new(RecordingCapabilityClient::default());
        capabilities.push_response(Err(SdkError::new(
            CAPABILITY_PERMISSION_DENIED_CODE,
            ErrorCategory::Authorization,
            false,
            "The capability call is not allowed.",
        )));
        let snapshots = Arc::new(FixedPartySnapshotPort::new(party_snapshot(7)));
        let port = GatewayPartyDisplayNameApplicationPort::new(
            capabilities,
            snapshots.clone(),
            Arc::new(FixedClock::new(70_000_000)),
        )
        .unwrap();

        assert_eq!(
            port.apply(application_request()).await.unwrap(),
            PartyDisplayNameApplicationResult::AuthorizationDenied {
                decision_id: "final-authorization-1".to_owned(),
            }
        );
        assert!(snapshots.requests().is_empty());
    }

    #[tokio::test]
    async fn resolves_version_conflict_through_governed_party_snapshot() {
        let capabilities = Arc::new(RecordingCapabilityClient::default());
        capabilities.push_response(Err(SdkError::new(
            PARTY_VERSION_CONFLICT_CODE,
            ErrorCategory::Conflict,
            false,
            "Party version conflict.",
        )));
        let snapshots = Arc::new(FixedPartySnapshotPort::new(party_snapshot(9)));
        let port = GatewayPartyDisplayNameApplicationPort::new(
            capabilities,
            snapshots.clone(),
            Arc::new(FixedClock::new(71_000_000)),
        )
        .unwrap();

        assert_eq!(
            port.apply(application_request()).await.unwrap(),
            PartyDisplayNameApplicationResult::StaleTarget {
                actual_party_resource_version: 9,
            }
        );
        let requests = snapshots.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].party_id.as_str(), "party-application-1");
        assert_eq!(requests[0].requested_at_unix_ms, 71);
    }

    #[tokio::test]
    async fn rejects_inconsistent_success_evidence() {
        let capabilities = Arc::new(RecordingCapabilityClient::default());
        capabilities.push_response(Ok(successful_outcome(9)));
        let port = GatewayPartyDisplayNameApplicationPort::new(
            capabilities,
            Arc::new(FixedPartySnapshotPort::new(party_snapshot(9))),
            Arc::new(FixedClock::new(70_000_000)),
        )
        .unwrap();

        let error = port.apply(application_request()).await.unwrap_err();
        assert_eq!(
            error.code,
            "CUSTOMER_ENRICHMENT_OWNER_APPLICATION_RESPONSE_INVALID"
        );
    }

    fn application_request() -> PartyDisplayNameApplicationRequest {
        PartyDisplayNameApplicationRequest {
            tenant_id: TenantId::try_new("tenant-application-a").unwrap(),
            actor_id: ActorId::try_new("application-reviewer-a").unwrap(),
            suggestion_id: typed_id("enrichment-suggestion-application-1"),
            review_decision_id: typed_id("enrichment-review-application-1"),
            application_attempt_id: attempt_id(),
            party_id: crm_module_sdk::RecordId::try_new("party-application-1").unwrap(),
            expected_party_resource_version: 7,
            reviewed_display_name: "Applied Company".to_owned(),
            target_idempotency_key: "customer-enrichment-target-application-1".to_owned(),
            final_authorization_decision_id: "final-authorization-1".to_owned(),
        }
    }

    fn attempt_id() -> ApplicationAttemptId {
        typed_id("enrichment-application-application-1")
    }

    fn typed_id<T: DeserializeOwned>(value: &str) -> T {
        serde_json::from_str(&format!("\"{value}\"")).unwrap()
    }

    fn party_snapshot(version: i64) -> PartySnapshot {
        PartySnapshot {
            party_id: crm_module_sdk::RecordId::try_new("party-application-1").unwrap(),
            display_name: "Current Company".to_owned(),
            resource_version: version,
            observed_at_unix_ms: 71,
        }
    }

    fn successful_outcome(version: i64) -> CapabilityOutcome {
        let output = support::protobuf_payload(
            PARTIES_MODULE_ID,
            UPDATE_RESPONSE_SCHEMA,
            DataClass::Personal,
            &party_wire::UpdatePartyResponse {
                party: Some(party_wire::Party {
                    party_ref: Some(customer::PartyRef {
                        party_id: "party-application-1".to_owned(),
                    }),
                    kind: party_wire::PartyKind::Organization as i32,
                    display_name: "Applied Company".to_owned(),
                    resource_version: Some(customer::CustomerResourceVersion {
                        version,
                        created_at: None,
                        updated_at: None,
                    }),
                }),
            },
        )
        .unwrap();
        CapabilityOutcome {
            output: Some(output),
            affected_resources: vec![ResourceRef {
                resource_type: PARTY_RECORD_TYPE.to_owned(),
                resource_id: "party-application-1".to_owned(),
                version: Some(version),
            }],
        }
    }

    #[test]
    fn typed_test_ids_match_domain_types() {
        let _: SuggestionId = typed_id("enrichment-suggestion-application-1");
        let _: ReviewDecisionId = typed_id("enrichment-review-application-1");
        let _: ApplicationAttemptId = attempt_id();
    }
}
