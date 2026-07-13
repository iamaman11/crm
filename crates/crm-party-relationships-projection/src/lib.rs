#![forbid(unsafe_code)]

//! Rebuildable hierarchy/adjacency projection for authoritative Party
//! Relationship events. This crate owns no Party Relationship business state.

use crm_core_data::PostgresDataStore;
use crm_core_events::ProjectionDocumentWrite;
use crm_module_sdk::{
    DataClass, ErrorCategory, EventDelivery, EventType, ModuleId, PayloadEncoding, SdkError,
    TenantId,
};
use crm_projection_runtime::{
    ProjectionBatchResult, ProjectionDefinition, ProjectionHandler, ProjectionId,
    ProjectionRegistry, ProjectionRunner,
};
use crm_proto_contracts::{crm::party_relationships::v1 as wire, message_descriptor_hash};
use prost::Message;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

pub const PARTY_RELATIONSHIP_HIERARCHY_PROJECTION_ID: &str =
    "customer.party-relationship-hierarchy.v1";
pub const PARTY_RELATIONSHIP_HIERARCHY_CONSUMER_MODULE_ID: &str =
    "crm.party-relationship-hierarchy-projection";
pub const PARTY_RELATIONSHIP_HIERARCHY_RESOURCE_TYPE: &str =
    "party-relationships.hierarchy-adjacency";

const PARTY_RELATIONSHIPS_MODULE_ID: &str = "crm.party-relationships";
const PARTY_RELATIONSHIP_RECORD_TYPE: &str = "party-relationships.party_relationship";
const CONTRACT_VERSION: &str = "1.0.0";
const CREATED_EVENT_TYPE: &str = "party-relationships.party-relationship.created";
const UPDATED_EVENT_TYPE: &str = "party-relationships.party-relationship.updated";
const CREATED_EVENT_SCHEMA: &str = "crm.party_relationships.v1.PartyRelationshipCreatedEvent";
const UPDATED_EVENT_SCHEMA: &str = "crm.party_relationships.v1.PartyRelationshipUpdatedEvent";

#[derive(Debug, Clone, Copy)]
struct PartyRelationshipHierarchyProjectionHandler;

impl ProjectionHandler for PartyRelationshipHierarchyProjectionHandler {
    fn project(&self, delivery: &EventDelivery) -> Result<Vec<ProjectionDocumentWrite>, SdkError> {
        hierarchy_writes(delivery)
    }
}

pub fn party_relationship_hierarchy_projection_registry() -> Result<ProjectionRegistry, SdkError> {
    ProjectionRegistry::new(vec![ProjectionDefinition::new(
        configured_projection_id(PARTY_RELATIONSHIP_HIERARCHY_PROJECTION_ID)?,
        configured_module_id(PARTY_RELATIONSHIP_HIERARCHY_CONSUMER_MODULE_ID)?,
        vec![
            configured_event_type(CREATED_EVENT_TYPE)?,
            configured_event_type(UPDATED_EVENT_TYPE)?,
        ],
        Arc::new(PartyRelationshipHierarchyProjectionHandler),
    )?])
}

#[derive(Debug, Clone)]
pub struct PartyRelationshipHierarchyProjectionWorker {
    runner: ProjectionRunner,
}

impl PartyRelationshipHierarchyProjectionWorker {
    pub fn new(store: PostgresDataStore) -> Result<Self, SdkError> {
        Ok(Self {
            runner: ProjectionRunner::new(
                Arc::new(store),
                party_relationship_hierarchy_projection_registry()?,
            ),
        })
    }

    pub async fn run_batch(
        &self,
        tenant_id: TenantId,
        page_size: u32,
    ) -> Result<ProjectionBatchResult, SdkError> {
        self.runner
            .run_batch(
                tenant_id,
                PARTY_RELATIONSHIP_HIERARCHY_PROJECTION_ID,
                page_size,
            )
            .await
    }

    pub async fn rebuild(&self, tenant_id: TenantId, page_size: u32) -> Result<u64, SdkError> {
        self.runner
            .rebuild(
                tenant_id,
                PARTY_RELATIONSHIP_HIERARCHY_PROJECTION_ID,
                page_size,
            )
            .await
    }

    pub fn runner(&self) -> &ProjectionRunner {
        &self.runner
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HierarchyAdjacencyDocument {
    pub relationship_id: String,
    pub party_id: String,
    pub related_party_id: String,
    pub relationship_type_code: String,
    pub directionality: String,
    pub relationship_direction: String,
    pub role: String,
    pub related_role: String,
    pub status: String,
    pub valid_from_unix_nanos: Option<i64>,
    pub valid_until_unix_nanos: Option<i64>,
    pub version: i64,
}

impl HierarchyAdjacencyDocument {
    pub fn from_json(value: &Value) -> Result<Self, SdkError> {
        serde_json::from_value(value.clone())
            .map_err(|error| hierarchy_document_invalid(error.to_string()))
    }

    pub fn is_active_at(&self, unix_nanos: i64) -> bool {
        self.status == "active"
            && self
                .valid_from_unix_nanos
                .is_none_or(|value| unix_nanos >= value)
            && self
                .valid_until_unix_nanos
                .is_none_or(|value| unix_nanos < value)
    }
}

/// Deterministically traverses a materialized adjacency set without treating
/// the projection as authoritative state. Each Party is returned once at its
/// minimum discovered depth. `maximum_depth == 0` returns only the start Party.
pub fn traverse_projected_hierarchy(
    documents: &[HierarchyAdjacencyDocument],
    start_party_id: &str,
    maximum_depth: u32,
    effective_at_unix_nanos: i64,
) -> BTreeMap<String, u32> {
    let mut adjacency: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for document in documents
        .iter()
        .filter(|document| document.is_active_at(effective_at_unix_nanos))
    {
        adjacency
            .entry(document.party_id.as_str())
            .or_default()
            .insert(document.related_party_id.as_str());
    }

    let mut depths = BTreeMap::from([(start_party_id.to_owned(), 0_u32)]);
    let mut queue = VecDeque::from([(start_party_id.to_owned(), 0_u32)]);
    while let Some((party_id, depth)) = queue.pop_front() {
        if depth >= maximum_depth {
            continue;
        }
        let Some(neighbors) = adjacency.get(party_id.as_str()) else {
            continue;
        };
        for neighbor in neighbors {
            if depths.contains_key(*neighbor) {
                continue;
            }
            let next_depth = depth + 1;
            depths.insert((*neighbor).to_owned(), next_depth);
            queue.push_back(((*neighbor).to_owned(), next_depth));
        }
    }
    depths
}

fn hierarchy_writes(delivery: &EventDelivery) -> Result<Vec<ProjectionDocumentWrite>, SdkError> {
    if delivery.source_module_id.as_str() != PARTY_RELATIONSHIPS_MODULE_ID
        || delivery.aggregate.record_type.as_str() != PARTY_RELATIONSHIP_RECORD_TYPE
    {
        return Err(projection_event_invalid(
            "Party Relationship hierarchy event ownership is invalid",
        ));
    }

    let relationship = match delivery.event_type.as_str() {
        CREATED_EVENT_TYPE => {
            validate_contract(delivery, CREATED_EVENT_SCHEMA)?;
            decode::<wire::PartyRelationshipCreatedEvent>(delivery)?
                .party_relationship
                .ok_or_else(|| projection_event_invalid("created event is missing relationship"))?
        }
        UPDATED_EVENT_TYPE => {
            validate_contract(delivery, UPDATED_EVENT_SCHEMA)?;
            decode::<wire::PartyRelationshipUpdatedEvent>(delivery)?
                .party_relationship
                .ok_or_else(|| projection_event_invalid("updated event is missing relationship"))?
        }
        _ => {
            return Err(projection_event_invalid(
                "Party Relationship hierarchy event type is unsupported",
            ));
        }
    };

    let relationship_ref = relationship
        .party_relationship_ref
        .as_ref()
        .ok_or_else(|| projection_event_invalid("relationship reference is missing"))?;
    if relationship_ref.party_relationship_id != delivery.aggregate.record_id.as_str() {
        return Err(projection_event_invalid(
            "relationship event identity is inconsistent",
        ));
    }
    let resource_version = relationship
        .resource_version
        .as_ref()
        .ok_or_else(|| projection_event_invalid("resource version is missing"))?;
    if resource_version.version != delivery.aggregate_version {
        return Err(projection_event_invalid(
            "relationship event version is inconsistent",
        ));
    }
    let from_party = relationship
        .from_party_ref
        .as_ref()
        .ok_or_else(|| projection_event_invalid("from Party reference is missing"))?;
    let to_party = relationship
        .to_party_ref
        .as_ref()
        .ok_or_else(|| projection_event_invalid("to Party reference is missing"))?;
    if from_party.party_id.is_empty()
        || to_party.party_id.is_empty()
        || from_party.party_id == to_party.party_id
    {
        return Err(projection_event_invalid(
            "relationship Party endpoints are invalid",
        ));
    }
    let relationship_type = relationship
        .relationship_type
        .as_ref()
        .ok_or_else(|| projection_event_invalid("relationship type is missing"))?;
    let directionality = directionality_name(relationship_type.directionality)?;
    let status = status_name(relationship.status)?;
    let valid_from_unix_nanos = relationship.valid_from.map(|value| value.unix_nanos);
    let valid_until_unix_nanos = relationship.valid_until.map(|value| value.unix_nanos);

    let from_direction = if directionality == "reciprocal" {
        "reciprocal"
    } else {
        "outbound"
    };
    let to_direction = if directionality == "reciprocal" {
        "reciprocal"
    } else {
        "inbound"
    };

    Ok(vec![
        adjacency_write(
            delivery,
            relationship_ref.party_relationship_id.as_str(),
            from_party.party_id.as_str(),
            to_party.party_id.as_str(),
            relationship_type,
            directionality,
            from_direction,
            relationship_type.from_role.as_str(),
            relationship_type.to_role.as_str(),
            status,
            valid_from_unix_nanos,
            valid_until_unix_nanos,
        ),
        adjacency_write(
            delivery,
            relationship_ref.party_relationship_id.as_str(),
            to_party.party_id.as_str(),
            from_party.party_id.as_str(),
            relationship_type,
            directionality,
            to_direction,
            relationship_type.to_role.as_str(),
            relationship_type.from_role.as_str(),
            status,
            valid_from_unix_nanos,
            valid_until_unix_nanos,
        ),
    ])
}

#[allow(clippy::too_many_arguments)]
fn adjacency_write(
    delivery: &EventDelivery,
    relationship_id: &str,
    party_id: &str,
    related_party_id: &str,
    relationship_type: &wire::PartyRelationshipType,
    directionality: &str,
    relationship_direction: &str,
    role: &str,
    related_role: &str,
    status: &str,
    valid_from_unix_nanos: Option<i64>,
    valid_until_unix_nanos: Option<i64>,
) -> ProjectionDocumentWrite {
    ProjectionDocumentWrite {
        resource_type: PARTY_RELATIONSHIP_HIERARCHY_RESOURCE_TYPE.to_owned(),
        resource_id: format!("{party_id}:{relationship_id}"),
        source_version: delivery.aggregate_version,
        document: json!({
            "relationship_id": relationship_id,
            "party_id": party_id,
            "related_party_id": related_party_id,
            "relationship_type_code": relationship_type.code,
            "directionality": directionality,
            "relationship_direction": relationship_direction,
            "role": role,
            "related_role": related_role,
            "status": status,
            "valid_from_unix_nanos": valid_from_unix_nanos,
            "valid_until_unix_nanos": valid_until_unix_nanos,
            "version": delivery.aggregate_version,
        }),
    }
}

fn validate_contract(delivery: &EventDelivery, schema_id: &str) -> Result<(), SdkError> {
    if delivery.payload.owner.as_str() != PARTY_RELATIONSHIPS_MODULE_ID
        || delivery.event_version.as_str() != CONTRACT_VERSION
        || delivery.payload.schema_id.as_str() != schema_id
        || delivery.payload.schema_version.as_str() != CONTRACT_VERSION
        || delivery.payload.descriptor_hash != message_descriptor_hash(schema_id)
        || delivery.payload.data_class != DataClass::Personal
        || delivery.payload.encoding != PayloadEncoding::Protobuf
    {
        return Err(projection_event_invalid(
            "Party Relationship hierarchy event contract identity is invalid",
        ));
    }
    Ok(())
}

fn decode<M>(delivery: &EventDelivery) -> Result<M, SdkError>
where
    M: Message + Default,
{
    M::decode(delivery.payload.bytes.as_slice())
        .map_err(|error| projection_event_invalid(error.to_string()))
}

fn directionality_name(value: i32) -> Result<&'static str, SdkError> {
    match wire::PartyRelationshipDirectionality::try_from(value) {
        Ok(wire::PartyRelationshipDirectionality::Directional) => Ok("directional"),
        Ok(wire::PartyRelationshipDirectionality::Reciprocal) => Ok("reciprocal"),
        Ok(wire::PartyRelationshipDirectionality::Unspecified) | Err(_) => Err(
            projection_event_invalid("relationship directionality is invalid"),
        ),
    }
}

fn status_name(value: i32) -> Result<&'static str, SdkError> {
    match wire::PartyRelationshipStatus::try_from(value) {
        Ok(wire::PartyRelationshipStatus::Active) => Ok("active"),
        Ok(wire::PartyRelationshipStatus::Inactive) => Ok("inactive"),
        Ok(wire::PartyRelationshipStatus::Unspecified) | Err(_) => {
            Err(projection_event_invalid("relationship status is invalid"))
        }
    }
}

fn configured_projection_id(value: &str) -> Result<ProjectionId, SdkError> {
    ProjectionId::try_new(value)
        .map_err(|error| projection_configuration_invalid(error.to_string()))
}

fn configured_module_id(value: &str) -> Result<ModuleId, SdkError> {
    ModuleId::try_new(value).map_err(|error| projection_configuration_invalid(error.to_string()))
}

fn configured_event_type(value: &str) -> Result<EventType, SdkError> {
    EventType::try_new(value).map_err(|error| projection_configuration_invalid(error.to_string()))
}

fn projection_configuration_invalid(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIP_HIERARCHY_PROJECTION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Party Relationship hierarchy projection is misconfigured.",
    )
    .with_internal_reference(internal)
}

fn projection_event_invalid(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIP_HIERARCHY_PROJECTION_EVENT_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The Party Relationship hierarchy projection source event is invalid.",
    )
    .with_internal_reference(internal)
}

fn hierarchy_document_invalid(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "PARTY_RELATIONSHIP_HIERARCHY_DOCUMENT_INVALID",
        ErrorCategory::Unavailable,
        true,
        "The Party Relationship hierarchy projection is temporarily unavailable.",
    )
    .with_internal_reference(internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::{
        ActorId, CorrelationId, DeliveryId, EventId, EventVersion, RecordId, RecordRef, RecordType,
        RetentionPolicyId, SchemaId, SchemaVersion, TraceId, TypedPayload,
    };
    use crm_proto_contracts::crm::{core::v1 as core, customer::v1 as customer};

    #[test]
    fn registry_is_stable_and_consumes_only_authoritative_relationship_events() {
        let registry = party_relationship_hierarchy_projection_registry().unwrap();
        assert_eq!(registry.len(), 1);
        let definition = registry
            .get(PARTY_RELATIONSHIP_HIERARCHY_PROJECTION_ID)
            .unwrap();
        assert_eq!(definition.event_types().len(), 2);
    }

    #[test]
    fn directional_event_produces_forward_and_reverse_adjacency_documents() {
        let delivery = created_delivery(
            "relationship-1",
            "party-parent",
            "party-child",
            wire::PartyRelationshipType {
                code: "parent_subsidiary".to_owned(),
                directionality: wire::PartyRelationshipDirectionality::Directional as i32,
                from_role: "parent".to_owned(),
                to_role: "subsidiary".to_owned(),
            },
        );
        let writes = hierarchy_writes(&delivery).unwrap();
        assert_eq!(writes.len(), 2);
        let forward = HierarchyAdjacencyDocument::from_json(&writes[0].document).unwrap();
        let reverse = HierarchyAdjacencyDocument::from_json(&writes[1].document).unwrap();
        assert_eq!(forward.party_id, "party-parent");
        assert_eq!(forward.related_party_id, "party-child");
        assert_eq!(forward.relationship_direction, "outbound");
        assert_eq!(forward.role, "parent");
        assert_eq!(reverse.party_id, "party-child");
        assert_eq!(reverse.related_party_id, "party-parent");
        assert_eq!(reverse.relationship_direction, "inbound");
        assert_eq!(reverse.role, "subsidiary");
    }

    #[test]
    fn bounded_traversal_uses_only_active_effective_projected_edges() {
        let documents = vec![
            edge("r1", "party-a", "party-b", "active", None, None),
            edge("r1", "party-b", "party-a", "active", None, None),
            edge("r2", "party-b", "party-c", "active", Some(10), Some(100)),
            edge("r2", "party-c", "party-b", "active", Some(10), Some(100)),
            edge("r3", "party-c", "party-d", "inactive", None, None),
        ];

        assert_eq!(
            traverse_projected_hierarchy(&documents, "party-a", 1, 50),
            BTreeMap::from([("party-a".to_owned(), 0), ("party-b".to_owned(), 1)])
        );
        assert_eq!(
            traverse_projected_hierarchy(&documents, "party-a", 3, 50),
            BTreeMap::from([
                ("party-a".to_owned(), 0),
                ("party-b".to_owned(), 1),
                ("party-c".to_owned(), 2),
            ])
        );
        assert_eq!(
            traverse_projected_hierarchy(&documents, "party-a", 3, 100),
            BTreeMap::from([("party-a".to_owned(), 0), ("party-b".to_owned(), 1)])
        );
    }

    fn edge(
        relationship_id: &str,
        party_id: &str,
        related_party_id: &str,
        status: &str,
        valid_from_unix_nanos: Option<i64>,
        valid_until_unix_nanos: Option<i64>,
    ) -> HierarchyAdjacencyDocument {
        HierarchyAdjacencyDocument {
            relationship_id: relationship_id.to_owned(),
            party_id: party_id.to_owned(),
            related_party_id: related_party_id.to_owned(),
            relationship_type_code: "parent_subsidiary".to_owned(),
            directionality: "directional".to_owned(),
            relationship_direction: "outbound".to_owned(),
            role: "parent".to_owned(),
            related_role: "subsidiary".to_owned(),
            status: status.to_owned(),
            valid_from_unix_nanos,
            valid_until_unix_nanos,
            version: 1,
        }
    }

    fn created_delivery(
        relationship_id: &str,
        from_party_id: &str,
        to_party_id: &str,
        relationship_type: wire::PartyRelationshipType,
    ) -> EventDelivery {
        let relationship = wire::PartyRelationship {
            party_relationship_ref: Some(customer::PartyRelationshipRef {
                party_relationship_id: relationship_id.to_owned(),
            }),
            from_party_ref: Some(customer::PartyRef {
                party_id: from_party_id.to_owned(),
            }),
            to_party_ref: Some(customer::PartyRef {
                party_id: to_party_id.to_owned(),
            }),
            relationship_type: Some(relationship_type),
            status: wire::PartyRelationshipStatus::Active as i32,
            valid_from: Some(core::UnixTime { unix_nanos: 10 }),
            valid_until: Some(core::UnixTime { unix_nanos: 1_000 }),
            resource_version: Some(customer::CustomerResourceVersion {
                version: 1,
                created_at: Some(core::UnixTime { unix_nanos: 10 }),
                updated_at: Some(core::UnixTime { unix_nanos: 10 }),
            }),
        };
        let event = wire::PartyRelationshipCreatedEvent {
            party_relationship: Some(relationship),
        };
        EventDelivery {
            delivery_id: DeliveryId::try_new("delivery-relationship-created-1").unwrap(),
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            event_id: EventId::try_new("event-relationship-created-1").unwrap(),
            source_module_id: ModuleId::try_new(PARTY_RELATIONSHIPS_MODULE_ID).unwrap(),
            consumer_module_id: ModuleId::try_new(PARTY_RELATIONSHIP_HIERARCHY_CONSUMER_MODULE_ID)
                .unwrap(),
            source_actor_id: ActorId::try_new("actor-a").unwrap(),
            event_type: EventType::try_new(CREATED_EVENT_TYPE).unwrap(),
            event_version: EventVersion::try_new(CONTRACT_VERSION).unwrap(),
            aggregate: RecordRef {
                record_type: RecordType::try_new(PARTY_RELATIONSHIP_RECORD_TYPE).unwrap(),
                record_id: RecordId::try_new(relationship_id).unwrap(),
            },
            aggregate_version: 1,
            occurred_at_unix_nanos: 10,
            payload: TypedPayload {
                owner: ModuleId::try_new(PARTY_RELATIONSHIPS_MODULE_ID).unwrap(),
                schema_id: SchemaId::try_new(CREATED_EVENT_SCHEMA).unwrap(),
                schema_version: SchemaVersion::try_new(CONTRACT_VERSION).unwrap(),
                descriptor_hash: message_descriptor_hash(CREATED_EVENT_SCHEMA),
                data_class: DataClass::Personal,
                encoding: PayloadEncoding::Protobuf,
                maximum_size_bytes: 1024 * 1024,
                retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
                bytes: event.encode_to_vec(),
            },
            correlation_id: CorrelationId::try_new("correlation-relationship-created-1").unwrap(),
            trace_id: TraceId::try_new("trace-relationship-created-1").unwrap(),
        }
    }
}
