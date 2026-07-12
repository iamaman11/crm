use crate::SearchIndexId;
use crm_module_sdk::{PortFuture, SdkError, TenantId};
use crm_projection_runtime::{ProjectionBatchResult, ProjectionRunner};
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchGenerationStatus {
    Building,
    Active,
    Retired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchIndexGeneration {
    pub tenant_id: TenantId,
    pub index_id: SearchIndexId,
    pub generation_id: String,
    pub projection_id: String,
    pub schema_version: String,
    pub status: SearchGenerationStatus,
}

pub trait SearchGenerationStore: Send + Sync {
    fn active_generation<'a>(
        &'a self,
        tenant_id: TenantId,
        index_id: SearchIndexId,
    ) -> PortFuture<'a, Result<Option<SearchIndexGeneration>, SdkError>>;

    fn register_building_generation<'a>(
        &'a self,
        generation: SearchIndexGeneration,
    ) -> PortFuture<'a, Result<(), SdkError>>;

    fn activate_generation<'a>(
        &'a self,
        tenant_id: TenantId,
        index_id: SearchIndexId,
        generation_id: String,
    ) -> PortFuture<'a, Result<(), SdkError>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchCatchUpResult {
    pub events_seen: u64,
    pub events_applied: u64,
    pub replayed_events: u64,
}

impl SearchCatchUpResult {
    fn add_batch(&mut self, batch: &ProjectionBatchResult) {
        self.events_seen = self.events_seen.saturating_add(u64::from(batch.events_seen));
        self.events_applied = self
            .events_applied
            .saturating_add(u64::from(batch.events_applied));
        self.replayed_events = self
            .replayed_events
            .saturating_add(u64::from(batch.replayed_events));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchGenerationAction {
    RebuiltAndActivated { applied_events: u64 },
    CaughtUp(SearchCatchUpResult),
}

#[derive(Clone)]
pub struct SearchReindexCoordinator {
    runner: ProjectionRunner,
    generations: Arc<dyn SearchGenerationStore>,
    index_id: SearchIndexId,
    generation_id: String,
    projection_id: String,
    schema_version: String,
}

impl fmt::Debug for SearchReindexCoordinator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SearchReindexCoordinator")
            .field("runner", &self.runner)
            .field("generations", &"dyn SearchGenerationStore")
            .field("index_id", &self.index_id)
            .field("generation_id", &self.generation_id)
            .field("projection_id", &self.projection_id)
            .field("schema_version", &self.schema_version)
            .finish()
    }
}

impl SearchReindexCoordinator {
    pub fn new(
        runner: ProjectionRunner,
        generations: Arc<dyn SearchGenerationStore>,
        index_id: SearchIndexId,
        generation_id: impl Into<String>,
        projection_id: impl Into<String>,
        schema_version: impl Into<String>,
    ) -> Result<Self, SdkError> {
        let generation_id = generation_id.into();
        let projection_id = projection_id.into();
        let schema_version = schema_version.into();
        validate_coordinate(&generation_id)?;
        validate_coordinate(&projection_id)?;
        validate_coordinate(&schema_version)?;
        runner.registry().get(&projection_id)?;
        Ok(Self {
            runner,
            generations,
            index_id,
            generation_id,
            projection_id,
            schema_version,
        })
    }

    pub fn index_id(&self) -> &SearchIndexId {
        &self.index_id
    }

    pub fn generation_id(&self) -> &str {
        &self.generation_id
    }

    pub fn projection_id(&self) -> &str {
        &self.projection_id
    }

    pub async fn ensure_ready(
        &self,
        tenant_id: TenantId,
        page_size: u32,
    ) -> Result<SearchGenerationAction, SdkError> {
        let active = self
            .generations
            .active_generation(tenant_id.clone(), self.index_id.clone())
            .await?;
        if active.as_ref().is_some_and(|generation| {
            generation.generation_id == self.generation_id
                && generation.projection_id == self.projection_id
                && generation.schema_version == self.schema_version
                && generation.status == SearchGenerationStatus::Active
        }) {
            return self
                .catch_up(tenant_id, page_size)
                .await
                .map(SearchGenerationAction::CaughtUp);
        }
        self.reindex(tenant_id, page_size).await
    }

    pub async fn reindex(
        &self,
        tenant_id: TenantId,
        page_size: u32,
    ) -> Result<SearchGenerationAction, SdkError> {
        self.generations
            .register_building_generation(SearchIndexGeneration {
                tenant_id: tenant_id.clone(),
                index_id: self.index_id.clone(),
                generation_id: self.generation_id.clone(),
                projection_id: self.projection_id.clone(),
                schema_version: self.schema_version.clone(),
                status: SearchGenerationStatus::Building,
            })
            .await?;
        let applied_events = self
            .runner
            .rebuild(tenant_id.clone(), &self.projection_id, page_size)
            .await?;
        self.generations
            .activate_generation(
                tenant_id,
                self.index_id.clone(),
                self.generation_id.clone(),
            )
            .await?;
        Ok(SearchGenerationAction::RebuiltAndActivated { applied_events })
    }

    pub async fn catch_up(
        &self,
        tenant_id: TenantId,
        page_size: u32,
    ) -> Result<SearchCatchUpResult, SdkError> {
        let mut result = SearchCatchUpResult {
            events_seen: 0,
            events_applied: 0,
            replayed_events: 0,
        };
        loop {
            let batch = self
                .runner
                .run_batch(tenant_id.clone(), &self.projection_id, page_size)
                .await?;
            result.add_batch(&batch);
            if !batch.has_more {
                return Ok(result);
            }
        }
    }
}

fn validate_coordinate(value: &str) -> Result<(), SdkError> {
    if value.is_empty() || value.len() > 180 || value.chars().any(char::is_control) {
        return Err(crm_module_sdk::SdkError::new(
            "SEARCH_GENERATION_COORDINATE_INVALID",
            crm_module_sdk::ErrorCategory::InvalidArgument,
            false,
            "The search generation coordinate is invalid.",
        ));
    }
    Ok(())
}
