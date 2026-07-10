#![forbid(unsafe_code)]

//! Governed ports available to Ultimate CRM business modules.
//!
//! The SDK deliberately exposes no database pool, message-bus client, object
//! storage client, arbitrary HTTP client, secret-store client or model-provider
//! client. Host services bind these ports to a tenant, actor and execution
//! context before invoking business-module code.

pub mod ports;
pub mod testing;
pub mod types;

use ports::{
    CapabilityClient, Clock, EventPublisher, FileClient, ModuleStateStore,
    ObservabilityContext, RandomSource, RecordClient, RelationshipClient, WorkflowClient,
};
use std::sync::Arc;

pub use ports::*;
pub use types::*;

/// Object-safe service bundle injected by the platform host into a module.
///
/// All side-effecting methods on the contained ports require an explicit
/// [`ModuleExecutionContext`]. The bundle owns only governed interfaces, never
/// infrastructure clients.
#[derive(Clone)]
pub struct ModuleServices {
    pub capabilities: Arc<dyn CapabilityClient>,
    pub records: Arc<dyn RecordClient>,
    pub relationships: Arc<dyn RelationshipClient>,
    pub events: Arc<dyn EventPublisher>,
    pub state: Arc<dyn ModuleStateStore>,
    pub workflows: Arc<dyn WorkflowClient>,
    pub files: Arc<dyn FileClient>,
    pub clock: Arc<dyn Clock>,
    pub random: Arc<dyn RandomSource>,
    pub observability: Arc<dyn ObservabilityContext>,
}

impl std::fmt::Debug for ModuleServices {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ModuleServices")
            .field("capabilities", &"dyn CapabilityClient")
            .field("records", &"dyn RecordClient")
            .field("relationships", &"dyn RelationshipClient")
            .field("events", &"dyn EventPublisher")
            .field("state", &"dyn ModuleStateStore")
            .field("workflows", &"dyn WorkflowClient")
            .field("files", &"dyn FileClient")
            .field("clock", &"dyn Clock")
            .field("random", &"dyn RandomSource")
            .field("observability", &"dyn ObservabilityContext")
            .finish()
    }
}
