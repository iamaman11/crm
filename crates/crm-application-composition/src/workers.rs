use crate::{BackgroundWorkerRegistry, TenantBackgroundWorker};
use crm_module_sdk::ModuleId;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackgroundCompositionError {
    UndeclaredModule(String),
    DuplicateWorker { module_id: String, worker_id: String },
    InvalidWorkerId { module_id: String, worker_id: String },
}

impl std::fmt::Display for BackgroundCompositionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UndeclaredModule(module_id) => {
                write!(formatter, "background worker owner {module_id} is not in application composition")
            }
            Self::DuplicateWorker { module_id, worker_id } => {
                write!(formatter, "duplicate background worker {module_id}/{worker_id}")
            }
            Self::InvalidWorkerId { module_id, worker_id } => {
                write!(formatter, "invalid background worker id {module_id}/{worker_id}")
            }
        }
    }
}

impl std::error::Error for BackgroundCompositionError {}

#[derive(Debug)]
pub struct BackgroundWorkerRegistryBuilder {
    declared_modules: BTreeSet<String>,
    workers: BTreeMap<(String, String), Arc<dyn TenantBackgroundWorker>>,
}

impl BackgroundWorkerRegistryBuilder {
    pub fn new(declared_modules: impl IntoIterator<Item = String>) -> Self {
        Self {
            declared_modules: declared_modules.into_iter().collect(),
            workers: BTreeMap::new(),
        }
    }

    pub fn add(
        &mut self,
        module_id: ModuleId,
        worker_id: impl Into<String>,
        worker: Arc<dyn TenantBackgroundWorker>,
    ) -> Result<&mut Self, BackgroundCompositionError> {
        let module_id = module_id.as_str().to_owned();
        let worker_id = worker_id.into();
        if !self.declared_modules.contains(&module_id) {
            return Err(BackgroundCompositionError::UndeclaredModule(module_id));
        }
        if !valid_worker_id(&worker_id) {
            return Err(BackgroundCompositionError::InvalidWorkerId {
                module_id,
                worker_id,
            });
        }
        let key = (module_id.clone(), worker_id.clone());
        if self.workers.insert(key, worker).is_some() {
            return Err(BackgroundCompositionError::DuplicateWorker {
                module_id,
                worker_id,
            });
        }
        Ok(self)
    }

    pub fn build(self) -> BackgroundWorkerRegistry {
        BackgroundWorkerRegistry::from_routes(self.workers)
    }
}

fn valid_worker_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 180
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-'))
}
