#![forbid(unsafe_code)]

//! Cross-domain composition for the rebuildable global CRM search index.
//!
//! This crate owns no authoritative business state and no search persistence.
//! It maps immutable owner-domain snapshot events into generation-scoped search
//! projection documents, while `crm-search-runtime` owns generation mechanics
//! and the public query path repeats live authorization before disclosure.

use crm_core_data::PostgresDataStore;
use crm_core_events::ProjectionDocumentWrite;
use crm_module_sdk::{
    DataClass, ErrorCategory, EventDelivery, EventType, ModuleId, PayloadEncoding, RecordId,
    RecordRef, RecordType, SdkError, TenantId,
};
use crm_projection_runtime::{
    ProjectionDefinition, ProjectionHandler, ProjectionId, ProjectionRegistry, ProjectionRunner,
};
use crm_proto_contracts::{
    crm::{activities::v1 as activities, parties::v1 as parties, sales::v1 as sales},
    message_descriptor_hash,
};
use crm_search_runtime::{
    SearchCatchUpResult, SearchGenerationAction, SearchIndexId, SearchProjectionDocument,
    SearchReindexCoordinator,
};
use prost::Message;
use std::collections::BTreeMap;
use std::sync::Arc;

pub const GLOBAL_SEARCH_INDEX_ID: &str = "crm.global-search";
pub const GLOBAL_SEARCH_SCHEMA_VERSION: &str = "1";
pub const SEARCH_INDEXER_CONSUMER_MODULE_ID: &str = "crm.search-indexer";
pub const INITIAL_GLOBAL_SEARCH_GENERATION_ID: &str = "g2";

const SALES_MODULE_ID: &str = "crm.sales";
const ACTIVITIES_MODULE_ID: &str = "crm.activities";
const PARTIES_MODULE_ID: &str = "crm.parties";
const CONTRACT_VERSION: &str = "1.0.0";

const DEAL_RESOURCE_TYPE: &str = "sales.deal";
const TASK_RESOURCE_TYPE: &str = "activities.task";
const PARTY_RESOURCE_TYPE: &str = "parties.party";

const SALES_CREATED: &str = "sales.deal.created";
const SALES_UPDATED: &str = "sales.deal.updated";
const SALES_CREATED_SCHEMA: &str = "crm.sales.v1.DealCreatedEvent";
const SALES_UPDATED_SCHEMA: &str = "crm.sales.v1.DealUpdatedEvent";

const TASK_CREATED: &str = "activities.task.created";
const TASK_UPDATED: &str = "activities.task.updated";
const TASK_CREATED_SCHEMA: &str = "crm.activities.v1.TaskCreatedEvent";
const TASK_UPDATED_SCHEMA: &str = "crm.activities.v1.TaskUpdatedEvent";

const PARTY_CREATED: &str = "parties.party.created";
const PARTY_UPDATED: &str = "parties.party.updated";
const PARTY_CREATED_SCHEMA: &str = "crm.parties.v1.PartyCreatedEvent";
const PARTY_UPDATED_SCHEMA: &str = "crm.parties.v1.PartyUpdatedEvent";

#[derive(Debug, Clone)]
pub struct SearchProjectionGeneration {
    pub generation_id: String,
    pub projection_id: ProjectionId,
    pub registry: ProjectionRegistry,
}

impl SearchProjectionGeneration {
    pub fn new(generation_id: impl Into<String>) -> Result<Self, SdkError> {
        let generation_id = generation_id.into();
        validate_generation_id(&generation_id)?;
        let projection_id = ProjectionId::try_new(format!("search.global.{generation_id}"))?;
        let definition = ProjectionDefinition::new(
            projection_id.clone(),
            ModuleId::try_new(SEARCH_INDEXER_CONSUMER_MODULE_ID)
                .map_err(|error| search_configuration_invalid(error.to_string()))?,
            configured_event_types(&[
                SALES_CREATED,
                SALES_UPDATED,
                TASK_CREATED,
                TASK_UPDATED,
                PARTY_CREATED,
                PARTY_UPDATED,
            ])?,
            Arc::new(GlobalSearchProjectionHandler {
                generation_id: generation_id.clone(),
            }),
        )?;
        Ok(Self {
            generation_id,
            projection_id,
            registry: ProjectionRegistry::new(vec![definition])?,
        })
    }
}

#[derive(Debug, Clone)]
struct GlobalSearchProjectionHandler {
    generation_id: String,
}

impl ProjectionHandler for GlobalSearchProjectionHandler {
    fn project(&self, delivery: &EventDelivery) -> Result<Vec<ProjectionDocumentWrite>, SdkError> {
        let document = match delivery.event_type.as_str() {
            SALES_CREATED => {
                validate_contract(
                    delivery,
                    SALES_MODULE_ID,
                    SALES_CREATED_SCHEMA,
                    DataClass::Confidential,
                )?;
                let event = decode::<sales::DealCreatedEvent>(delivery)?;
                let deal = event
                    .deal
                    .ok_or_else(|| search_event_invalid("Deal created event is missing deal"))?;
                validate_snapshot(delivery, DEAL_RESOURCE_TYPE, &deal.deal_id, deal.version)?;
                single_title_document(
                    &self.generation_id,
                    SALES_MODULE_ID,
                    DEAL_RESOURCE_TYPE,
                    &deal.deal_id,
                    deal.version,
                    "name",
                    deal.name,
                )?
            }
            SALES_UPDATED => {
                validate_contract(
                    delivery,
                    SALES_MODULE_ID,
                    SALES_UPDATED_SCHEMA,
                    DataClass::Confidential,
                )?;
                let event = decode::<sales::DealUpdatedEvent>(delivery)?;
                let deal = event
                    .deal
                    .ok_or_else(|| search_event_invalid("Deal updated event is missing deal"))?;
                validate_snapshot(delivery, DEAL_RESOURCE_TYPE, &deal.deal_id, deal.version)?;
                single_title_document(
                    &self.generation_id,
                    SALES_MODULE_ID,
                    DEAL_RESOURCE_TYPE,
                    &deal.deal_id,
                    deal.version,
                    "name",
                    deal.name,
                )?
            }
            TASK_CREATED => {
                validate_contract(
                    delivery,
                    ACTIVITIES_MODULE_ID,
                    TASK_CREATED_SCHEMA,
                    DataClass::Confidential,
                )?;
                let event = decode::<activities::TaskCreatedEvent>(delivery)?;
                let task = event
                    .task
                    .ok_or_else(|| search_event_invalid("Task created event is missing task"))?;
                validate_snapshot(delivery, TASK_RESOURCE_TYPE, &task.task_id, task.version)?;
                single_title_document(
                    &self.generation_id,
                    ACTIVITIES_MODULE_ID,
                    TASK_RESOURCE_TYPE,
                    &task.task_id,
                    task.version,
                    "subject",
                    task.subject,
                )?
            }
            TASK_UPDATED => {
                validate_contract(
                    delivery,
                    ACTIVITIES_MODULE_ID,
                    TASK_UPDATED_SCHEMA,
                    DataClass::Confidential,
                )?;
                let event = decode::<activities::TaskUpdatedEvent>(delivery)?;
                let task = event
                    .task
                    .ok_or_else(|| search_event_invalid("Task updated event is missing task"))?;
                validate_snapshot(delivery, TASK_RESOURCE_TYPE, &task.task_id, task.version)?;
                single_title_document(
                    &self.generation_id,
                    ACTIVITIES_MODULE_ID,
                    TASK_RESOURCE_TYPE,
                    &task.task_id,
                    task.version,
                    "subject",
                    task.subject,
                )?
            }
            PARTY_CREATED => {
                validate_contract(
                    delivery,
                    PARTIES_MODULE_ID,
                    PARTY_CREATED_SCHEMA,
                    DataClass::Personal,
                )?;
                let event = decode::<parties::PartyCreatedEvent>(delivery)?;
                party_document(
                    &self.generation_id,
                    delivery,
                    event.party.ok_or_else(|| {
                        search_event_invalid("Party created event is missing party")
                    })?,
                )?
            }
            PARTY_UPDATED => {
                validate_contract(
                    delivery,
                    PARTIES_MODULE_ID,
                    PARTY_UPDATED_SCHEMA,
                    DataClass::Personal,
                )?;
                let event = decode::<parties::PartyUpdatedEvent>(delivery)?;
                party_document(
                    &self.generation_id,
                    delivery,
                    event.party.ok_or_else(|| {
                        search_event_invalid("Party updated event is missing party")
                    })?,
                )?
            }
            _ => {
                return Err(search_event_invalid(
                    "Global search projection event type is unsupported",
                ));
            }
        };
        Ok(vec![document])
    }
}

fn party_document(
    generation_id: &str,
    delivery: &EventDelivery,
    party: parties::Party,
) -> Result<ProjectionDocumentWrite, SdkError> {
    let party_ref = party
        .party_ref
        .ok_or_else(|| search_event_invalid("Party search snapshot is missing party reference"))?;
    let resource_version = party
        .resource_version
        .ok_or_else(|| search_event_invalid("Party search snapshot is missing resource version"))?;
    validate_snapshot(
        delivery,
        PARTY_RESOURCE_TYPE,
        &party_ref.party_id,
        resource_version.version,
    )?;

    let kind = match parties::PartyKind::try_from(party.kind) {
        Ok(parties::PartyKind::Person) => "person",
        Ok(parties::PartyKind::Organization) => "organization",
        Ok(parties::PartyKind::Unspecified) | Err(_) => {
            return Err(search_event_invalid(
                "Party search snapshot contains an invalid Party kind",
            ));
        }
    };
    let display_name = party.display_name.trim().to_owned();
    if display_name.is_empty() {
        return Err(search_event_invalid(
            "Party search snapshot display name must not be empty",
        ));
    }

    search_document(
        generation_id,
        PARTIES_MODULE_ID,
        PARTY_RESOURCE_TYPE,
        &party_ref.party_id,
        resource_version.version,
        BTreeMap::from([("display_name".to_owned(), display_name.clone())]),
        BTreeMap::from([
            ("display_name".to_owned(), display_name),
            ("kind".to_owned(), kind.to_owned()),
        ]),
    )
}

fn single_title_document(
    generation_id: &str,
    owner_module_id: &str,
    resource_type: &str,
    resource_id: &str,
    source_version: i64,
    title_field: &str,
    title: String,
) -> Result<ProjectionDocumentWrite, SdkError> {
    if title.trim().is_empty() {
        return Err(search_event_invalid("Search title must not be empty"));
    }
    search_document(
        generation_id,
        owner_module_id,
        resource_type,
        resource_id,
        source_version,
        BTreeMap::from([(title_field.to_owned(), title.clone())]),
        BTreeMap::from([(title_field.to_owned(), title)]),
    )
}

fn search_document(
    generation_id: &str,
    owner_module_id: &str,
    resource_type: &str,
    resource_id: &str,
    source_version: i64,
    searchable_fields: BTreeMap<String, String>,
    display_fields: BTreeMap<String, String>,
) -> Result<ProjectionDocumentWrite, SdkError> {
    SearchProjectionDocument {
        index_id: SearchIndexId::try_new(GLOBAL_SEARCH_INDEX_ID)?,
        generation_id: generation_id.to_owned(),
        schema_version: GLOBAL_SEARCH_SCHEMA_VERSION.to_owned(),
        owner_module_id: ModuleId::try_new(owner_module_id)
            .map_err(|error| search_configuration_invalid(error.to_string()))?,
        resource: RecordRef {
            record_type: RecordType::try_new(resource_type)
                .map_err(|error| search_configuration_invalid(error.to_string()))?,
            record_id: RecordId::try_new(resource_id)
                .map_err(|error| search_event_invalid(error.to_string()))?,
        },
        source_version,
        searchable_fields,
        display_fields,
    }
    .into_projection_write()
}

fn validate_snapshot(
    delivery: &EventDelivery,
    expected_resource_type: &str,
    resource_id: &str,
    version: i64,
) -> Result<(), SdkError> {
    if delivery.aggregate.record_type.as_str() != expected_resource_type
        || delivery.aggregate.record_id.as_str() != resource_id
        || delivery.aggregate_version != version
    {
        return Err(search_event_invalid(
            "Global search source event snapshot identity is inconsistent",
        ));
    }
    Ok(())
}

fn validate_contract(
    delivery: &EventDelivery,
    owner_module_id: &str,
    schema_id: &str,
    data_class: DataClass,
) -> Result<(), SdkError> {
    if delivery.source_module_id.as_str() != owner_module_id
        || delivery.payload.owner.as_str() != owner_module_id
        || delivery.event_version.as_str() != CONTRACT_VERSION
        || delivery.payload.schema_id.as_str() != schema_id
        || delivery.payload.schema_version.as_str() != CONTRACT_VERSION
        || delivery.payload.descriptor_hash != message_descriptor_hash(schema_id)
        || delivery.payload.data_class != data_class
        || delivery.payload.encoding != PayloadEncoding::Protobuf
    {
        return Err(search_event_invalid(
            "Global search source event contract identity is invalid",
        ));
    }
    Ok(())
}

fn decode<M>(delivery: &EventDelivery) -> Result<M, SdkError>
where
    M: Message + Default,
{
    M::decode(delivery.payload.bytes.as_slice())
        .map_err(|error| search_event_invalid(error.to_string()))
}

fn configured_event_types(values: &[&str]) -> Result<Vec<EventType>, SdkError> {
    values
        .iter()
        .map(|value| {
            EventType::try_new(*value)
                .map_err(|error| search_configuration_invalid(error.to_string()))
        })
        .collect()
}

fn validate_generation_id(value: &str) -> Result<(), SdkError> {
    if value.is_empty()
        || value.len() > 120
        || value.chars().any(char::is_control)
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
    {
        return Err(search_configuration_invalid(
            "search generation id is invalid",
        ));
    }
    Ok(())
}

fn search_configuration_invalid(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "GLOBAL_SEARCH_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The global search projection configuration is invalid.",
    )
    .with_internal_reference(internal)
}

fn search_event_invalid(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "GLOBAL_SEARCH_EVENT_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The global search source event is invalid.",
    )
    .with_internal_reference(internal)
}

#[derive(Debug, Clone)]
pub struct GlobalSearchWorker {
    coordinator: SearchReindexCoordinator,
}

impl GlobalSearchWorker {
    pub fn new(store: PostgresDataStore) -> Result<Self, SdkError> {
        Self::for_generation(store, INITIAL_GLOBAL_SEARCH_GENERATION_ID)
    }

    pub fn for_generation(
        store: PostgresDataStore,
        generation_id: impl Into<String>,
    ) -> Result<Self, SdkError> {
        let generation = SearchProjectionGeneration::new(generation_id)?;
        let projection_id = generation.projection_id.as_str().to_owned();
        let runner = ProjectionRunner::new(Arc::new(store.clone()), generation.registry);
        let coordinator = SearchReindexCoordinator::new(
            runner,
            Arc::new(store),
            SearchIndexId::try_new(GLOBAL_SEARCH_INDEX_ID)?,
            generation.generation_id,
            projection_id,
            GLOBAL_SEARCH_SCHEMA_VERSION,
        )?;
        Ok(Self { coordinator })
    }

    pub fn generation_id(&self) -> &str {
        self.coordinator.generation_id()
    }

    pub fn projection_id(&self) -> &str {
        self.coordinator.projection_id()
    }

    pub async fn ensure_ready(
        &self,
        tenant_id: TenantId,
        page_size: u32,
    ) -> Result<SearchGenerationAction, SdkError> {
        self.coordinator.ensure_ready(tenant_id, page_size).await
    }

    pub async fn catch_up(
        &self,
        tenant_id: TenantId,
        page_size: u32,
    ) -> Result<SearchCatchUpResult, SdkError> {
        self.coordinator.catch_up(tenant_id, page_size).await
    }

    pub async fn reindex(
        &self,
        tenant_id: TenantId,
        page_size: u32,
    ) -> Result<SearchGenerationAction, SdkError> {
        self.coordinator.reindex(tenant_id, page_size).await
    }
}

/// Compatibility alias for downstream code migrating from the Phase 7 name.
pub type Phase7SearchWorker = GlobalSearchWorker;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_registry_subscribes_to_six_authoritative_snapshot_events() {
        let generation = SearchProjectionGeneration::new("g2-test").unwrap();
        assert_eq!(generation.projection_id.as_str(), "search.global.g2-test");
        assert_eq!(generation.registry.len(), 1);
        let definition = generation.registry.get("search.global.g2-test").unwrap();
        assert_eq!(
            definition.consumer_module_id().as_str(),
            SEARCH_INDEXER_CONSUMER_MODULE_ID
        );
        let event_types = definition
            .event_types()
            .iter()
            .map(|event_type| event_type.as_str())
            .collect::<Vec<_>>();
        assert_eq!(event_types.len(), 6);
        assert!(event_types.contains(&SALES_CREATED));
        assert!(event_types.contains(&SALES_UPDATED));
        assert!(event_types.contains(&TASK_CREATED));
        assert!(event_types.contains(&TASK_UPDATED));
        assert!(event_types.contains(&PARTY_CREATED));
        assert!(event_types.contains(&PARTY_UPDATED));
        assert!(!event_types.contains(&"sales.deal.stage_changed"));
        assert!(!event_types.contains(&"activities.task.completed"));
    }

    #[test]
    fn production_generation_advances_to_g2_for_subscription_rebuild() {
        assert_eq!(INITIAL_GLOBAL_SEARCH_GENERATION_ID, "g2");
        let generation = SearchProjectionGeneration::new(INITIAL_GLOBAL_SEARCH_GENERATION_ID)
            .expect("valid production search generation");
        assert_eq!(generation.projection_id.as_str(), "search.global.g2");
    }

    #[test]
    fn generation_id_rejects_path_or_whitespace_characters() {
        let error = SearchProjectionGeneration::new("g2/../../bad generation").unwrap_err();
        assert_eq!(error.code, "GLOBAL_SEARCH_CONFIGURATION_INVALID");
    }
}
