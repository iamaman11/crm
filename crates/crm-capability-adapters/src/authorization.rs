use crm_capability_runtime::{
    AuthorizationDecision, CapabilityAuthorizer, CapabilityDefinition, CapabilityRequest,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, Clock, ErrorCategory, ModuleId, PortFuture, SdkError,
    TenantId,
};
use crm_query_runtime::{QueryAuthorizer, QueryRequest};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AuthorizationKey {
    tenant_id: TenantId,
    actor_id: ActorId,
    policy_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationGrant {
    pub tenant_id: TenantId,
    pub actor_id: ActorId,
    pub policy_id: String,
    pub capability_id: CapabilityId,
    pub capability_version: CapabilityVersion,
    pub owner_module_id: ModuleId,
    pub policy_version: String,
    pub expires_at_unix_nanos: Option<i64>,
}

impl AuthorizationGrant {
    fn key(&self) -> AuthorizationKey {
        AuthorizationKey {
            tenant_id: self.tenant_id.clone(),
            actor_id: self.actor_id.clone(),
            policy_id: self.policy_id.clone(),
        }
    }

    fn validate(&self) -> Result<(), AuthorizationStoreError> {
        if self.policy_id.is_empty() || self.policy_version.is_empty() {
            return Err(AuthorizationStoreError::InvalidGrant(
                "policy ID and version must not be empty",
            ));
        }
        if self.expires_at_unix_nanos.is_some_and(|value| value <= 0) {
            return Err(AuthorizationStoreError::InvalidGrant(
                "grant expiry must not be zero",
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum AuthorizationStoreError {
    InvalidGrant(&'static str),
    Poisoned,
}

impl fmt::Display for AuthorizationStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGrant(message) => formatter.write_str(message),
            Self::Poisoned => formatter.write_str("authorization store lock is poisoned"),
        }
    }
}

impl Error for AuthorizationStoreError {}

#[derive(Debug, Default)]
struct AuthorizationState {
    revision: u64,
    grants: BTreeMap<AuthorizationKey, AuthorizationGrant>,
}

#[derive(Debug, Clone, Default)]
pub struct LiveAuthorizationStore {
    state: Arc<RwLock<AuthorizationState>>,
}

impl LiveAuthorizationStore {
    pub fn upsert(&self, grant: AuthorizationGrant) -> Result<u64, AuthorizationStoreError> {
        grant.validate()?;
        let mut state = self
            .state
            .write()
            .map_err(|_| AuthorizationStoreError::Poisoned)?;
        state.revision = state.revision.saturating_add(1);
        state.grants.insert(grant.key(), grant);
        Ok(state.revision)
    }

    pub fn revoke(
        &self,
        tenant_id: &TenantId,
        actor_id: &ActorId,
        policy_id: &str,
    ) -> Result<bool, AuthorizationStoreError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| AuthorizationStoreError::Poisoned)?;
        let removed = state
            .grants
            .remove(&AuthorizationKey {
                tenant_id: tenant_id.clone(),
                actor_id: actor_id.clone(),
                policy_id: policy_id.to_owned(),
            })
            .is_some();
        if removed {
            state.revision = state.revision.saturating_add(1);
        }
        Ok(removed)
    }

    pub fn revision(&self) -> Result<u64, AuthorizationStoreError> {
        self.state
            .read()
            .map(|state| state.revision)
            .map_err(|_| AuthorizationStoreError::Poisoned)
    }
}

#[derive(Clone)]
pub struct LiveCapabilityAuthorizer {
    store: LiveAuthorizationStore,
    clock: Arc<dyn Clock>,
}

impl fmt::Debug for LiveCapabilityAuthorizer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LiveCapabilityAuthorizer")
            .field("store", &self.store)
            .field("clock", &"dyn Clock")
            .finish()
    }
}

impl LiveCapabilityAuthorizer {
    pub fn new(store: LiveAuthorizationStore, clock: Arc<dyn Clock>) -> Self {
        Self { store, clock }
    }

    fn authorize_binding(
        &self,
        definition: &CapabilityDefinition,
        tenant_id: &TenantId,
        actor_id: &ActorId,
    ) -> Result<AuthorizationDecision, SdkError> {
        let state = self.store.state.read().map_err(|_| {
            SdkError::new(
                "AUTHORIZATION_STORE_UNAVAILABLE",
                ErrorCategory::Unavailable,
                true,
                "Authorization is temporarily unavailable.",
            )
        })?;
        let key = AuthorizationKey {
            tenant_id: tenant_id.clone(),
            actor_id: actor_id.clone(),
            policy_id: definition.authorization_policy_id.clone(),
        };
        let decision_id = format!(
            "authorization:{}:{}:{}:{}",
            state.revision, key.tenant_id, key.actor_id, key.policy_id
        );
        let Some(grant) = state.grants.get(&key) else {
            return Ok(AuthorizationDecision {
                allowed: false,
                decision_id,
                reason_code: "grant_missing".to_owned(),
                policy_version: "none".to_owned(),
            });
        };
        if grant.capability_id != definition.capability_id
            || grant.capability_version != definition.capability_version
            || grant.owner_module_id != definition.owner_module_id
        {
            return Ok(AuthorizationDecision {
                allowed: false,
                decision_id,
                reason_code: "grant_binding_mismatch".to_owned(),
                policy_version: grant.policy_version.clone(),
            });
        }
        if grant
            .expires_at_unix_nanos
            .is_some_and(|expires_at| expires_at <= self.clock.now_unix_nanos())
        {
            return Ok(AuthorizationDecision {
                allowed: false,
                decision_id,
                reason_code: "grant_expired".to_owned(),
                policy_version: grant.policy_version.clone(),
            });
        }
        Ok(AuthorizationDecision {
            allowed: true,
            decision_id,
            reason_code: "grant_active".to_owned(),
            policy_version: grant.policy_version.clone(),
        })
    }
}

impl CapabilityAuthorizer for LiveCapabilityAuthorizer {
    fn authorize<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<AuthorizationDecision, SdkError>> {
        Box::pin(async move {
            self.authorize_binding(
                definition,
                &request.context.execution.tenant_id,
                &request.context.execution.actor_id,
            )
        })
    }
}

impl QueryAuthorizer for LiveCapabilityAuthorizer {
    fn authorize<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a QueryRequest,
    ) -> PortFuture<'a, Result<AuthorizationDecision, SdkError>> {
        Box::pin(async move {
            self.authorize_binding(
                definition,
                &request.context.tenant_id,
                &request.context.actor_id,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_capability_runtime::{CapabilityRisk, PayloadContract};
    use crm_module_sdk::testing::FixedClock;
    use crm_module_sdk::{
        BusinessTransactionId, CausationId, CorrelationId, DataClass, ExecutionContext,
        IdempotencyKey, ModuleExecutionContext, PayloadEncoding, RequestId, RetentionPolicyId,
        SchemaId, SchemaVersion, TraceId, TypedPayload,
    };

    #[tokio::test]
    async fn revocation_is_visible_to_the_next_live_decision() {
        let clock = Arc::new(FixedClock::new(100));
        let store = LiveAuthorizationStore::default();
        let grant = grant();
        store.upsert(grant.clone()).unwrap();
        let authorizer = LiveCapabilityAuthorizer::new(store.clone(), clock);
        let definition = definition();
        let request = request();

        assert!(
            CapabilityAuthorizer::authorize(&authorizer, &definition, &request)
                .await
                .unwrap()
                .allowed
        );
        store
            .revoke(&grant.tenant_id, &grant.actor_id, &grant.policy_id)
            .unwrap();
        let denied = CapabilityAuthorizer::authorize(&authorizer, &definition, &request)
            .await
            .unwrap();
        assert!(!denied.allowed);
        assert_eq!(denied.reason_code, "grant_missing");
    }

    #[tokio::test]
    async fn query_revocation_uses_the_same_live_authorization_store() {
        let clock = Arc::new(FixedClock::new(100));
        let store = LiveAuthorizationStore::default();
        let definition = query_definition();
        let grant = AuthorizationGrant {
            tenant_id: TenantId::try_new("tenant-1").unwrap(),
            actor_id: ActorId::try_new("actor-1").unwrap(),
            policy_id: definition.authorization_policy_id.clone(),
            capability_id: definition.capability_id.clone(),
            capability_version: definition.capability_version.clone(),
            owner_module_id: definition.owner_module_id.clone(),
            policy_version: "policy-query-1".to_owned(),
            expires_at_unix_nanos: Some(1_000),
        };
        store.upsert(grant.clone()).unwrap();
        let authorizer = LiveCapabilityAuthorizer::new(store.clone(), clock);
        let request = query_request(&definition);

        assert!(
            QueryAuthorizer::authorize(&authorizer, &definition, &request)
                .await
                .unwrap()
                .allowed
        );
        store
            .revoke(&grant.tenant_id, &grant.actor_id, &grant.policy_id)
            .unwrap();
        let denied = QueryAuthorizer::authorize(&authorizer, &definition, &request)
            .await
            .unwrap();
        assert!(!denied.allowed);
        assert_eq!(denied.reason_code, "grant_missing");
    }

    #[tokio::test]
    async fn exact_capability_binding_is_enforced() {
        let store = LiveAuthorizationStore::default();
        let mut grant = grant();
        grant.capability_version = CapabilityVersion::try_new("2.0.0").unwrap();
        store.upsert(grant).unwrap();
        let authorizer = LiveCapabilityAuthorizer::new(store, Arc::new(FixedClock::new(100)));

        let denied = CapabilityAuthorizer::authorize(&authorizer, &definition(), &request())
            .await
            .unwrap();
        assert_eq!(denied.reason_code, "grant_binding_mismatch");
    }

    fn query_definition() -> CapabilityDefinition {
        CapabilityDefinition {
            capability_id: CapabilityId::try_new("crm.sales.deal.get").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
            input_contract: PayloadContract {
                owner: ModuleId::try_new("crm.sales").unwrap(),
                schema_id: SchemaId::try_new("crm.sales.deal.get").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [3; 32],
                allowed_data_classes: vec![DataClass::Internal],
                allowed_encodings: vec![PayloadEncoding::Protobuf],
                maximum_size_bytes: 4096,
            },
            output_contract: Some(PayloadContract {
                owner: ModuleId::try_new("crm.sales").unwrap(),
                schema_id: SchemaId::try_new("crm.sales.deal.get.response").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [4; 32],
                allowed_data_classes: vec![DataClass::Internal],
                allowed_encodings: vec![PayloadEncoding::Protobuf],
                maximum_size_bytes: 16_384,
            }),
            risk: CapabilityRisk::Low,
            mutation: false,
            requires_idempotency: false,
            requires_approval: false,
            authorization_policy_id: "sales.deal.get".to_owned(),
            rate_limit_policy_id: None,
        }
    }

    fn query_request(definition: &CapabilityDefinition) -> QueryRequest {
        QueryRequest {
            owner_module_id: definition.owner_module_id.clone(),
            context: crm_query_runtime::QueryExecutionContext {
                tenant_id: TenantId::try_new("tenant-1").unwrap(),
                actor_id: ActorId::try_new("actor-1").unwrap(),
                request_id: RequestId::try_new("query-request-1").unwrap(),
                correlation_id: CorrelationId::try_new("query-correlation-1").unwrap(),
                trace_id: TraceId::try_new("query-trace-1").unwrap(),
                capability_id: definition.capability_id.clone(),
                capability_version: definition.capability_version.clone(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 100,
            },
            input: TypedPayload {
                owner: definition.input_contract.owner.clone(),
                schema_id: definition.input_contract.schema_id.clone(),
                schema_version: definition.input_contract.schema_version.clone(),
                descriptor_hash: definition.input_contract.descriptor_hash,
                data_class: DataClass::Internal,
                encoding: PayloadEncoding::Protobuf,
                maximum_size_bytes: definition.input_contract.maximum_size_bytes,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes: vec![1],
            },
            input_hash: [5; 32],
        }
    }

    fn grant() -> AuthorizationGrant {
        AuthorizationGrant {
            tenant_id: TenantId::try_new("tenant-1").unwrap(),
            actor_id: ActorId::try_new("actor-1").unwrap(),
            policy_id: "sales.deal.create".to_owned(),
            capability_id: CapabilityId::try_new("crm.sales.deal.create").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
            policy_version: "policy-7".to_owned(),
            expires_at_unix_nanos: Some(1_000),
        }
    }

    fn definition() -> CapabilityDefinition {
        CapabilityDefinition {
            capability_id: CapabilityId::try_new("crm.sales.deal.create").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
            input_contract: PayloadContract {
                owner: ModuleId::try_new("crm.sales").unwrap(),
                schema_id: SchemaId::try_new("crm.sales.deal.create").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [1; 32],
                allowed_data_classes: vec![DataClass::Internal],
                allowed_encodings: vec![PayloadEncoding::Json],
                maximum_size_bytes: 4096,
            },
            output_contract: None,
            risk: CapabilityRisk::Medium,
            mutation: true,
            requires_idempotency: true,
            requires_approval: false,
            authorization_policy_id: "sales.deal.create".to_owned(),
            rate_limit_policy_id: None,
        }
    }

    fn request() -> CapabilityRequest {
        CapabilityRequest {
            context: ModuleExecutionContext {
                module_id: ModuleId::try_new("crm.sales").unwrap(),
                execution: ExecutionContext {
                    tenant_id: TenantId::try_new("tenant-1").unwrap(),
                    actor_id: ActorId::try_new("actor-1").unwrap(),
                    request_id: RequestId::try_new("request-1").unwrap(),
                    correlation_id: CorrelationId::try_new("correlation-1").unwrap(),
                    causation_id: CausationId::try_new("causation-1").unwrap(),
                    trace_id: TraceId::try_new("trace-1").unwrap(),
                    capability_id: CapabilityId::try_new("crm.sales.deal.create").unwrap(),
                    capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                    idempotency_key: IdempotencyKey::try_new("idem-1").unwrap(),
                    business_transaction_id: BusinessTransactionId::try_new("txn-1").unwrap(),
                    schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                    request_started_at_unix_nanos: 100,
                },
            },
            input: TypedPayload {
                owner: ModuleId::try_new("crm.sales").unwrap(),
                schema_id: SchemaId::try_new("crm.sales.deal.create").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [1; 32],
                data_class: DataClass::Internal,
                encoding: PayloadEncoding::Json,
                maximum_size_bytes: 4096,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes: b"{}".to_vec(),
            },
            input_hash: [2; 32],
            approval: None,
        }
    }
}
