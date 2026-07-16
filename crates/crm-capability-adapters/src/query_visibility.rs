use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, Clock, ErrorCategory, ModuleId, PortFuture, RecordId,
    RecordRef, RecordType, SdkError, TenantId,
};
use crm_query_runtime::{QueryRequest, QueryVisibilityAuthorizer, QueryVisibilityDecision};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, RwLock};

const HIDDEN_FIELDS_ENV: &str = "CRM_QUERY_HIDDEN_FIELDS";
const FIELD_CEILING_POLICY_VERSION: &str = "deployment-field-ceiling/v1";

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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct FieldCeilingKey {
    capability_id: CapabilityId,
    owner_module_id: ModuleId,
    record_type: RecordType,
}

#[derive(Debug, Clone, Default)]
struct FieldVisibilityCeiling {
    hidden_fields: BTreeMap<FieldCeilingKey, BTreeSet<String>>,
}

impl FieldVisibilityCeiling {
    fn from_environment() -> Result<Self, QueryVisibilityStoreError> {
        Self::parse(env::var(HIDDEN_FIELDS_ENV).ok().as_deref())
    }

    fn parse(value: Option<&str>) -> Result<Self, QueryVisibilityStoreError> {
        let Some(value) = value else {
            return Ok(Self::default());
        };
        let mut hidden_fields = BTreeMap::<FieldCeilingKey, BTreeSet<String>>::new();
        for entry in value
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
        {
            let mut parts = entry.split('|').map(str::trim);
            let capability_id = parts.next().ok_or_else(invalid_field_ceiling)?;
            let owner_module_id = parts.next().ok_or_else(invalid_field_ceiling)?;
            let record_type = parts.next().ok_or_else(invalid_field_ceiling)?;
            let field = parts.next().ok_or_else(invalid_field_ceiling)?;
            if parts.next().is_some() || field.is_empty() || field.chars().any(char::is_control) {
                return Err(invalid_field_ceiling());
            }
            let key = FieldCeilingKey {
                capability_id: CapabilityId::try_new(capability_id)
                    .map_err(|_| invalid_field_ceiling())?,
                owner_module_id: ModuleId::try_new(owner_module_id)
                    .map_err(|_| invalid_field_ceiling())?,
                record_type: RecordType::try_new(record_type)
                    .map_err(|_| invalid_field_ceiling())?,
            };
            hidden_fields
                .entry(key)
                .or_default()
                .insert(field.to_owned());
        }
        Ok(Self { hidden_fields })
    }

    fn apply(&self, grant: &mut QueryVisibilityGrant) -> bool {
        let key = FieldCeilingKey {
            capability_id: grant.capability_id.clone(),
            owner_module_id: grant.owner_module_id.clone(),
            record_type: grant.record_type.clone(),
        };
        let Some(hidden_fields) = self.hidden_fields.get(&key) else {
            return false;
        };
        let previous_len = grant.allowed_fields.len();
        grant
            .allowed_fields
            .retain(|field| !hidden_fields.contains(field));
        grant.allowed_fields.len() != previous_len
    }
}

fn invalid_field_ceiling() -> QueryVisibilityStoreError {
    QueryVisibilityStoreError::InvalidConfiguration(
        "query hidden-field entries must be capability|owner|record_type|field",
    )
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
    InvalidConfiguration(&'static str),
    InvalidGrant(&'static str),
    Poisoned,
}

impl fmt::Display for QueryVisibilityStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(message) | Self::InvalidGrant(message) => {
                formatter.write_str(message)
            }
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

#[derive(Debug, Clone)]
pub struct LiveQueryVisibilityStore {
    state: Arc<RwLock<QueryVisibilityState>>,
    field_ceiling: Arc<FieldVisibilityCeiling>,
    configuration_valid: bool,
}

impl Default for LiveQueryVisibilityStore {
    fn default() -> Self {
        match FieldVisibilityCeiling::from_environment() {
            Ok(field_ceiling) => Self {
                state: Arc::new(RwLock::new(QueryVisibilityState::default())),
                field_ceiling: Arc::new(field_ceiling),
                configuration_valid: true,
            },
            Err(_) => Self {
                state: Arc::new(RwLock::new(QueryVisibilityState::default())),
                field_ceiling: Arc::new(FieldVisibilityCeiling::default()),
                configuration_valid: false,
            },
        }
    }
}

impl LiveQueryVisibilityStore {
    pub fn upsert(
        &self,
        mut grant: QueryVisibilityGrant,
    ) -> Result<u64, QueryVisibilityStoreError> {
        if !self.configuration_valid {
            return Err(QueryVisibilityStoreError::InvalidConfiguration(
                "query visibility deployment configuration is invalid",
            ));
        }
        grant.validate()?;
        if self.field_ceiling.apply(&mut grant) {
            grant.policy_version =
                format!("{}+{FIELD_CEILING_POLICY_VERSION}", grant.policy_version);
        }
        let mut state = self
            .state
            .write()
            .map_err(|_| QueryVisibilityStoreError::Poisoned)?;
        state.revision = state.revision.saturating_add(1);
        state.grants.insert(grant.key(), grant);
        Ok(state.revision)
    }

    pub fn revoke(&self, grant: &QueryVisibilityGrant) -> Result<bool, QueryVisibilityStoreError> {
        let mut state = self
            .state
            .write()
            .map_err(|_| QueryVisibilityStoreError::Poisoned)?;
        let removed = state.grants.remove(&grant.key()).is_some();
        if removed {
            state.revision = state.revision.saturating_add(1);
        }
        Ok(removed)
    }

    #[cfg(test)]
    fn with_field_ceiling(field_ceiling: FieldVisibilityCeiling) -> Self {
        Self {
            state: Arc::new(RwLock::new(QueryVisibilityState::default())),
            field_ceiling: Arc::new(field_ceiling),
            configuration_valid: true,
        }
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
        let exact_grant = grant(
            Some("deal-1"),
            BTreeSet::from(["name".to_owned(), "amount".to_owned()]),
        );
        store.upsert(exact_grant.clone()).unwrap();
        let authorizer =
            LiveQueryVisibilityAuthorizer::new(store.clone(), Arc::new(FixedClock::new(100)));

        let exact = authorizer
            .authorize_visibility(&request, &resource)
            .await
            .unwrap();
        assert!(exact.resource_visible);
        assert!(exact.allows_field("amount"));

        store.revoke(&exact_grant).unwrap();
        let fallback = authorizer
            .authorize_visibility(&request, &resource)
            .await
            .unwrap();
        assert!(fallback.resource_visible);
        assert!(!fallback.allows_field("amount"));
    }

    #[tokio::test]
    async fn deployment_ceiling_can_hide_fields_without_hiding_the_resource() {
        let ceiling =
            FieldVisibilityCeiling::parse(Some("sales.deal.get|crm.sales|sales.deal|amount"))
                .unwrap();
        let store = LiveQueryVisibilityStore::with_field_ceiling(ceiling);
        store
            .upsert(grant(
                None,
                BTreeSet::from(["name".to_owned(), "amount".to_owned()]),
            ))
            .unwrap();
        let authorizer = LiveQueryVisibilityAuthorizer::new(store, Arc::new(FixedClock::new(100)));
        let decision = authorizer
            .authorize_visibility(&request(), &resource("deal-1"))
            .await
            .unwrap();
        assert!(decision.resource_visible);
        assert!(decision.allows_field("name"));
        assert!(!decision.allows_field("amount"));
        assert!(
            decision
                .policy_version
                .contains(FIELD_CEILING_POLICY_VERSION)
        );
    }

    #[test]
    fn hidden_field_configuration_is_explicit_and_strict() {
        assert!(FieldVisibilityCeiling::parse(Some("missing-separators")).is_err());
        assert!(
            FieldVisibilityCeiling::parse(Some("sales.deal.get|crm.sales|sales.deal|")).is_err()
        );
        assert!(
            FieldVisibilityCeiling::parse(Some(
                "sales.deal.get|crm.sales|sales.deal|amount|unexpected"
            ))
            .is_err()
        );
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
