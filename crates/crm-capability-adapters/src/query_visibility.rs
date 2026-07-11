use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, Clock, ErrorCategory, ModuleId, PortFuture, RecordId,
    RecordRef, RecordType, SdkError, TenantId,
};
use crm_query_runtime::{QueryRequest, QueryVisibilityAuthorizer, QueryVisibilityDecision};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct VisibilityKey {
    tenant_id: TenantId,
    actor_id: ActorId,
    capability_id: CapabilityId,
    capability_version: CapabilityVersion,
    owner_module_id: ModuleId,
    record_type: RecordType,
    record_id: Option<RecordId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryVisibilityGrant {
    pub tenant_id: TenantId,
    pub actor_id: ActorId,
    pub capability_id: CapabilityId,
    pub capability_version: CapabilityVersion,
    pub owner_module_id: ModuleId,
    pub record_type: RecordType,
    pub record_id: Option<RecordId>,
    pub allowed_fields: BTreeSet<String>,
    pub policy_version: String,
    pub expires_at_unix_nanos: Option<i64>,
}

impl QueryVisibilityGrant {
    fn key(&self) -> VisibilityKey {
        VisibilityKey {
            tenant_id: self.tenant_id.clone(),
            actor_id: self.actor_id.clone(),
            capability_id: self.capability_id.clone(),
            capability_version: self.capability_version.clone(),
            owner_module_id: self.owner_module_id.clone(),
            record_type: self.record_type.clone(),
            record_id: self.record_id.clone(),
        }
    }

    fn validate(&self) -> Result<(), QueryVisibilityStoreError> {
        if self.policy_version.is_empty() {
            return Err(QueryVisibilityStoreError::InvalidGrant(
                "visibility policy version must not be empty",
            ));
        }
        if self
            .expires_at_unix_nanos
            .is_some_and(|expires_at| expires_at <= 0)
        {
            return Err(QueryVisibilityStoreError::InvalidGrant(
                "visibility grant expiry must be positive",
            ));
        }
        if self.allowed_fields.iter().any(|field| field.is_empty()) {
            return Err(QueryVisibilityStoreError::InvalidGrant(
                "visibility field names must not be empty",
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum QueryVisibilityStoreError {
    InvalidGrant(&'static str),
    Poisoned,
}

impl fmt::Display for QueryVisibilityStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGrant(message) => formatter.write_str(message),
            Self::Poisoned => formatter.write_str("query visibility store lock is poisoned"),
        }
    }
}

impl Error for QueryVisibilityStoreError {}

#[derive(Debug, Default)]
struct QueryVisibilityState {
    revision: u64,
    grants: BTreeMap<VisibilityKey, QueryVisibilityGrant>,
}

#[derive(Debug, Clone, Default)]
pub struct LiveQueryVisibilityStore {
    state: Arc<RwLock<QueryVisibilityState>>,
}

impl LiveQueryVisibilityStore {
    pub fn upsert(&self, grant: QueryVisibilityGrant) -> Result<u64, QueryVisibilityStoreError> {
        grant.validate()?;
        let mut state = self
            .state
            .write()
            .map_err(|_| QueryVisibilityStoreError::Poisoned)?;
        state.revision = state.revision.saturating_add(1);
        state.grants.insert(grant.key(), grant);
        Ok(state.revision)
    }

    pub fn revoke(
        &self,
        tenant_id: &TenantId,
        actor_id: &ActorId,
        capability_id: &CapabilityId,
        capability_version: &CapabilityVersion,
        owner_module_id: &ModuleId,
        record_type: &RecordType,
        record_id: Option<&RecordId>,
    ) -> Result<bool, QueryVisibilityStoreError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| QueryVisibilityStoreError::Poisoned)?;
        let removed = state
            .grants
            .remove(&VisibilityKey {
                tenant_id: tenant_id.clone(),
                actor_id: actor_id.clone(),
                capability_id: capability_id.clone(),
                capability_version: capability_version.clone(),
                owner_module_id: owner_module_id.clone(),
                record_type: record_type.clone(),
                record_id: record_id.cloned(),
            })
            .is_some();
        if removed {
            state.revision = state.revision.saturating_add(1);
        }
        Ok(removed)
    }
}

#[derive(Clone)]
pub struct LiveQueryVisibilityAuthorizer {
    store: LiveQueryVisibilityStore,
    clock: Arc<dyn Clock>,
}

impl fmt::Debug for LiveQueryVisibilityAuthorizer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LiveQueryVisibilityAuthorizer")
            .field("store", &self.store)
            .field("clock", &"dyn Clock")
            .finish()
    }
}

impl LiveQueryVisibilityAuthorizer {
    pub fn new(store: LiveQueryVisibilityStore, clock: Arc<dyn Clock>) -> Self {
        Self { store, clock }
    }
}

impl QueryVisibilityAuthorizer for LiveQueryVisibilityAuthorizer {
    fn authorize_visibility<'a>(
        &'a self,
        request: &'a QueryRequest,
        resource: &'a RecordRef,
    ) -> PortFuture<'a, Result<QueryVisibilityDecision, SdkError>> {
        Box::pin(async move {
            let state = self.store.state.read().map_err(|_| {
                SdkError::new(
                    "QUERY_VISIBILITY_UNAVAILABLE",
                    ErrorCategory::Unavailable,
                    true,
                    "Resource visibility is temporarily unavailable.",
                )
            })?;
            let exact = VisibilityKey {
                tenant_id: request.context.tenant_id.clone(),
                actor_id: request.context.actor_id.clone(),
                capability_id: request.context.capability_id.clone(),
                capability_version: request.context.capability_version.clone(),
                owner_module_id: request.owner_module_id.clone(),
                record_type: resource.record_type.clone(),
                record_id: Some(resource.record_id.clone()),
            };
            let type_wide = VisibilityKey {
                record_id: None,
                ..exact.clone()
            };
            let decision_id = format!(
                "query-visibility:{}:{}:{}:{}:{}:{}",
                state.revision,
                request.context.tenant_id,
                request.context.actor_id,
                request.context.capability_id,
                resource.record_type,
                resource.record_id,
            );
            let Some(grant) = state
                .grants
                .get(&exact)
                .or_else(|| state.grants.get(&type_wide))
            else {
                return Ok(QueryVisibilityDecision::denied(decision_id, "none"));
            };
            if grant
                .expires_at_unix_nanos
                .is_some_and(|expires_at| expires_at <= self.clock.now_unix_nanos())
            {
                return Ok(QueryVisibilityDecision::denied(
                    decision_id,
                    grant.policy_version.clone(),
                ));
            }
            Ok(QueryVisibilityDecision {
                resource_visible: true,
                allowed_fields: grant.allowed_fields.clone(),
                decision_id,
                policy_version: grant.policy_version.clone(),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::testing::FixedClock;
    use crm_module_sdk::{
        CorrelationId, DataClass, PayloadEncoding, RequestId, RetentionPolicyId, SchemaId,
        SchemaVersion, TraceId, TypedPayload,
    };
    use crm_query_runtime::QueryExecutionContext;

    #[tokio::test]
    async fn exact_resource_grant_overrides_type_wide_fields_and_revoke_is_live() {
        let store = LiveQueryVisibilityStore::default();
        let request = request();
        let resource = resource("deal-1");
        store
            .upsert(grant(None, BTreeSet::from(["name".to_owned()])))
            .unwrap();
        store
            .upsert(grant(
                Some("deal-1"),
                BTreeSet::from(["name".to_owned(), "amount".to_owned()]),
            ))
            .unwrap();
        let authorizer =
            LiveQueryVisibilityAuthorizer::new(store.clone(), Arc::new(FixedClock::new(100)));

        let exact = authorizer
            .authorize_visibility(&request, &resource)
            .await
            .unwrap();
        assert!(exact.resource_visible);
        assert!(exact.allows_field("amount"));

        store
            .revoke(
                &request.context.tenant_id,
                &request.context.actor_id,
                &request.context.capability_id,
                &request.context.capability_version,
                &request.owner_module_id,
                &resource.record_type,
                Some(&resource.record_id),
            )
            .unwrap();
        let fallback = authorizer
            .authorize_visibility(&request, &resource)
            .await
            .unwrap();
        assert!(fallback.resource_visible);
        assert!(!fallback.allows_field("amount"));
    }

    fn grant(record_id: Option<&str>, allowed_fields: BTreeSet<String>) -> QueryVisibilityGrant {
        QueryVisibilityGrant {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            actor_id: ActorId::try_new("actor-a").unwrap(),
            capability_id: CapabilityId::try_new("sales.deal.get").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
            record_type: RecordType::try_new("sales.deal").unwrap(),
            record_id: record_id.map(|value| RecordId::try_new(value).unwrap()),
            allowed_fields,
            policy_version: "visibility-1".to_owned(),
            expires_at_unix_nanos: Some(1_000),
        }
    }

    fn resource(record_id: &str) -> RecordRef {
        RecordRef {
            record_type: RecordType::try_new("sales.deal").unwrap(),
            record_id: RecordId::try_new(record_id).unwrap(),
        }
    }

    fn request() -> QueryRequest {
        QueryRequest {
            owner_module_id: ModuleId::try_new("crm.sales").unwrap(),
            context: QueryExecutionContext {
                tenant_id: TenantId::try_new("tenant-a").unwrap(),
                actor_id: ActorId::try_new("actor-a").unwrap(),
                request_id: RequestId::try_new("request-a").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
                trace_id: TraceId::try_new("trace-a").unwrap(),
                capability_id: CapabilityId::try_new("sales.deal.get").unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 100,
            },
            input: TypedPayload {
                owner: ModuleId::try_new("crm.sales").unwrap(),
                schema_id: SchemaId::try_new("crm.sales.v1.GetDealRequest").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                descriptor_hash: [1; 32],
                data_class: DataClass::Confidential,
                encoding: PayloadEncoding::Protobuf,
                maximum_size_bytes: 1024,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes: vec![1],
            },
            input_hash: [2; 32],
        }
    }
}
