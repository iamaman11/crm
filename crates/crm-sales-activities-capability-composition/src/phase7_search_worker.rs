use crate::{GLOBAL_SEARCH_INDEX_ID, GLOBAL_SEARCH_SCHEMA_VERSION, SearchProjectionGeneration};
use crm_core_data::PostgresDataStore;
use crm_module_sdk::{SdkError, TenantId};
use crm_projection_runtime::ProjectionRunner;
use crm_search_runtime::{
    SearchCatchUpResult, SearchGenerationAction, SearchIndexId, SearchReindexCoordinator,
};
use std::sync::Arc;

pub const INITIAL_GLOBAL_SEARCH_GENERATION_ID: &str = "g1";

#[derive(Debug, Clone)]
pub struct Phase7SearchWorker {
    coordinator: SearchReindexCoordinator,
}

impl Phase7SearchWorker {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_generation_coordinates_are_stable() {
        let generation = SearchProjectionGeneration::new(INITIAL_GLOBAL_SEARCH_GENERATION_ID)
            .expect("valid initial search generation");
        assert_eq!(generation.generation_id, "g1");
        assert_eq!(generation.projection_id.as_str(), "search.global.g1");
    }
}
