use crm_core_events::ProjectionDocumentWrite;
use crm_module_sdk::{
    DataClass, ErrorCategory, EventDelivery, EventType, ModuleId, PayloadEncoding, RecordId,
    RecordRef, RecordType, SdkError,
};
use crm_projection_runtime::{
    ProjectionDefinition, ProjectionHandler, ProjectionId, ProjectionRegistry,
};
use crm_proto_contracts::{
    crm::{activities::v1 as activities, sales::v1 as sales},
    message_descriptor_hash,
};
use crm_search_runtime::{SearchIndexId, SearchProjectionDocument};
use prost::Message;
use std::collections::BTreeMap;
use std::sync::Arc;

pub const GLOBAL_SEARCH_INDEX_ID: &str = "crm.global-search";
pub const GLOBAL_SEARCH_SCHEMA_VERSION: &str = "1";
pub const SEARCH_INDEXER_CONSUMER_MODULE_ID: &str = "crm.search-indexer";

const SALES_MODULE_ID: &str = "crm.sales";
const ACTIVITIES_MODULE_ID: &str = "crm.activities";
const CONTRACT_VERSION: &str = "1.0.0";
const DEAL_RESOURCE_TYPE: &str = "sales.deal";
const TASK_RESOURCE_TYPE: &str = "activities.task";

const SALES_CREATED: &str = "sales.deal.created";
const SALES_UPDATED: &str = "sales.deal.updated";
const SALES_CREATED_SCHEMA: &str = "crm.sales.v1.DealCreatedEvent";
const SALES_UPDATED_SCHEMA: &str = "crm.sales.v1.DealUpdatedEvent";

const TASK_CREATED: &str = "activities.task.created";
const TASK_UPDATED: &str = "activities.task.updated";
const TASK_CREATED_SCHEMA: &str = "crm.activities.v1.TaskCreatedEvent";
const TASK_UPDATED_SCHEMA: &str = "crm.activities.v1.TaskUpdatedEvent";

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
            configured_event_types(&[SALES_CREATED, SALES_UPDATED, TASK_CREATED, TASK_UPDATED])?,
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
                validate_contract(delivery, SALES_MODULE_ID, SALES_CREATED_SCHEMA)?;
                let event = decode::<sales::DealCreatedEvent>(delivery)?;
                let deal = event
                    .deal
                    .ok_or_else(|| search_event_invalid("Deal created event is missing deal"))?;
                validate_snapshot(delivery, DEAL_RESOURCE_TYPE, &deal.deal_id, deal.version)?;
                search_document(
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
                validate_contract(delivery, SALES_MODULE_ID, SALES_UPDATED_SCHEMA)?;
                let event = decode::<sales::DealUpdatedEvent>(delivery)?;
                let deal = event
                    .deal
                    .ok_or_else(|| search_event_invalid("Deal updated event is missing deal"))?;
                validate_snapshot(delivery, DEAL_RESOURCE_TYPE, &deal.deal_id, deal.version)?;
                search_document(
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
                validate_contract(delivery, ACTIVITIES_MODULE_ID, TASK_CREATED_SCHEMA)?;
                let event = decode::<activities::TaskCreatedEvent>(delivery)?;
                let task = event
                    .task
                    .ok_or_else(|| search_event_invalid("Task created event is missing task"))?;
                validate_snapshot(delivery, TASK_RESOURCE_TYPE, &task.task_id, task.version)?;
                search_document(
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
                validate_contract(delivery, ACTIVITIES_MODULE_ID, TASK_UPDATED_SCHEMA)?;
                let event = decode::<activities::TaskUpdatedEvent>(delivery)?;
                let task = event
                    .task
                    .ok_or_else(|| search_event_invalid("Task updated event is missing task"))?;
                validate_snapshot(delivery, TASK_RESOURCE_TYPE, &task.task_id, task.version)?;
                search_document(
                    &self.generation_id,
                    ACTIVITIES_MODULE_ID,
                    TASK_RESOURCE_TYPE,
                    &task.task_id,
                    task.version,
                    "subject",
                    task.subject,
                )?
            }
            _ => {
                return Err(search_event_invalid(
                    "Search projection event type is unsupported",
                ));
            }
        };
        Ok(vec![document])
    }
}

fn search_document(
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
        searchable_fields: BTreeMap::from([(title_field.to_owned(), title.clone())]),
        display_fields: BTreeMap::from([(title_field.to_owned(), title)]),
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
            "Search source event snapshot identity is inconsistent",
        ));
    }
    Ok(())
}

fn validate_contract(
    delivery: &EventDelivery,
    owner_module_id: &str,
    schema_id: &str,
) -> Result<(), SdkError> {
    if delivery.source_module_id.as_str() != owner_module_id
        || delivery.payload.owner.as_str() != owner_module_id
        || delivery.event_version.as_str() != CONTRACT_VERSION
        || delivery.payload.schema_id.as_str() != schema_id
        || delivery.payload.schema_version.as_str() != CONTRACT_VERSION
        || delivery.payload.descriptor_hash != message_descriptor_hash(schema_id)
        || delivery.payload.data_class != DataClass::Confidential
        || delivery.payload.encoding != PayloadEncoding::Protobuf
    {
        return Err(search_event_invalid(
            "Search source event contract identity is invalid",
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
        "PHASE7_SEARCH_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Phase 7 search projection configuration is invalid.",
    )
    .with_internal_reference(internal)
}

fn search_event_invalid(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "PHASE7_SEARCH_EVENT_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The search source event is invalid.",
    )
    .with_internal_reference(internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_registry_has_one_projection_and_four_snapshot_event_types() {
        let generation = SearchProjectionGeneration::new("g1").unwrap();
        assert_eq!(generation.projection_id.as_str(), "search.global.g1");
        assert_eq!(generation.registry.len(), 1);
        let definition = generation.registry.get("search.global.g1").unwrap();
        assert_eq!(
            definition.consumer_module_id().as_str(),
            SEARCH_INDEXER_CONSUMER_MODULE_ID
        );
        assert_eq!(definition.event_types().len(), 4);
        assert!(
            !definition
                .event_types()
                .iter()
                .any(|event_type| event_type.as_str() == "sales.deal.stage_changed")
        );
        assert!(
            !definition
                .event_types()
                .iter()
                .any(|event_type| event_type.as_str() == "activities.task.completed")
        );
    }

    #[test]
    fn generation_id_rejects_path_or_whitespace_characters() {
        let error = SearchProjectionGeneration::new("g1/../../bad generation").unwrap_err();
        assert_eq!(error.code, "PHASE7_SEARCH_CONFIGURATION_INVALID");
    }
}
