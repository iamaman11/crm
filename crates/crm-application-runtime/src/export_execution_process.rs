use crate::{
    ApplicationConfig, ApplicationRuntimeError, GovernedPartyExportExecutionSource, SystemClock,
    application_query_definitions, bootstrap_export_execution_worker_access,
};
use crm_capability_adapters::{
    LiveAuthorizationStore, LiveCapabilityAuthorizer, LiveQueryVisibilityAuthorizer,
    LiveQueryVisibilityStore,
};
use crm_core_data::{PostgresDataStore, PostgresImmutableFileArtifactStore};
use crm_customer_data_operations_capability_adapter::internal_export_execution_capability_definitions;
use crm_customer_data_operations_execution_composition::{
    EXPORT_EXECUTION_WORKER_ACTOR_ID, PartyExportExecutionWorker,
    PostgresPartyExportExecutionReader, PostgresPartyExportExecutionSink,
    PostgresPartyExportSelectionReader,
};
use crm_module_sdk::{ActorId, Clock, TenantId};
use crm_parties_query_adapter::PartyQueryAdapter;
use crm_query_runtime::CursorCodec;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

const EXPORT_EXECUTION_INTERVAL: Duration = Duration::from_secs(1);

/// Dedicated supervised process lane for Party export execution.
///
/// Selection remains in the primary application background cycle. Execution is isolated here so
/// artifact assembly, immutable file publication and durable per-position checkpointing can fail
/// the process closed without weakening the health semantics of unrelated projection workers.
/// The process supervisor treats an execution-lane error as a process-fatal readiness failure;
/// `crm-api` therefore cannot remain serving as healthy while governed export execution is broken.
#[derive(Clone)]
pub struct PartyExportExecutionProcess {
    worker: Arc<PartyExportExecutionWorker>,
    tenant_ids: BTreeSet<TenantId>,
}

impl std::fmt::Debug for PartyExportExecutionProcess {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PartyExportExecutionProcess")
            .field("worker", &self.worker)
            .field("tenant_count", &self.tenant_ids.len())
            .finish()
    }
}

impl PartyExportExecutionProcess {
    pub fn assemble(
        config: &ApplicationConfig,
        store: PostgresDataStore,
    ) -> Result<Self, ApplicationRuntimeError> {
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let now = clock.now_unix_nanos();
        if now < 0 {
            return Err(ApplicationRuntimeError::Assembly(
                "system clock is before the Unix epoch".to_owned(),
            ));
        }

        let authorization_store = LiveAuthorizationStore::default();
        let visibility_store = LiveQueryVisibilityStore::default();
        let query_definitions = application_query_definitions()
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let internal_definitions = internal_export_execution_capability_definitions()
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let worker_actor_id = ActorId::try_new(EXPORT_EXECUTION_WORKER_ACTOR_ID)
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;

        if config.bootstrap_allow_phase6 {
            bootstrap_export_execution_worker_access(
                config,
                now,
                &authorization_store,
                &visibility_store,
                &query_definitions,
                &internal_definitions,
                &worker_actor_id,
            )?;
        }

        let authorizer = Arc::new(LiveCapabilityAuthorizer::new(
            authorization_store,
            Arc::clone(&clock),
        ));
        let visibility_authorizer = Arc::new(LiveQueryVisibilityAuthorizer::new(
            visibility_store,
            Arc::clone(&clock),
        ));
        let cursor_key: [u8; 32] = config.cursor_signing_key[..32]
            .try_into()
            .map_err(|_| ApplicationRuntimeError::Assembly("cursor key is invalid".to_owned()))?;
        let party_query_adapter = PartyQueryAdapter::new(
            store.clone(),
            CursorCodec::new(cursor_key)
                .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
            visibility_authorizer,
        )
        .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?;
        let source = Arc::new(GovernedPartyExportExecutionSource::new(
            Arc::new(party_query_adapter),
            authorizer.clone(),
        ));
        let selection_reader = Arc::new(PostgresPartyExportSelectionReader::new(store.clone()));
        let execution_reader = Arc::new(PostgresPartyExportExecutionReader::new(store.clone()));
        let execution_sink = Arc::new(PostgresPartyExportExecutionSink::new(
            store.clone(),
            authorizer,
        ));
        let file_store = Arc::new(PostgresImmutableFileArtifactStore::new(store.clone()));
        let worker = Arc::new(
            PartyExportExecutionWorker::new(
                store,
                selection_reader,
                execution_reader,
                execution_sink,
                source,
                file_store,
                clock,
            )
            .map_err(|error| ApplicationRuntimeError::Assembly(error.to_string()))?,
        );

        Ok(Self {
            worker,
            tenant_ids: config.tenant_ids.clone(),
        })
    }

    pub async fn run_until_shutdown(
        self,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), ApplicationRuntimeError> {
        let mut interval = tokio::time::interval(EXPORT_EXECUTION_INTERVAL);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    for tenant_id in &self.tenant_ids {
                        self.worker
                            .run_tenant_cycle(tenant_id.clone())
                            .await
                            .map_err(|error| ApplicationRuntimeError::Server(error.to_string()))?;
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        return Ok(());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn export_execution_process_is_thread_safe() {
        assert_send_sync::<PartyExportExecutionProcess>();
    }
}
