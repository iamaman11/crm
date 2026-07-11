#![forbid(unsafe_code)]

use crm_module_sdk::{EventDelivery, EventId, EventType, ModuleId, SdkError, TenantId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

pub const DEFAULT_EVENT_HISTORY_PAGE_SIZE: u32 = 100;
pub const MAX_EVENT_HISTORY_PAGE_SIZE: u32 = 500;
pub const MAX_PROJECTION_WRITES_PER_EVENT: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventHistoryCursor {
    pub occurred_at_unix_nanos: i64,
    pub event_id: EventId,
}

impl EventHistoryCursor {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.occurred_at_unix_nanos < 0 {
            return Err("event history cursor time must not be negative");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventHistoryRequest {
    pub tenant_id: TenantId,
    pub consumer_module_id: ModuleId,
    pub event_types: Vec<EventType>,
    pub after: Option<EventHistoryCursor>,
    pub page_size: u32,
}

impl EventHistoryRequest {
    pub fn effective_page_size(&self) -> Result<u32, &'static str> {
        if self.event_types.is_empty() {
            return Err("event history request requires at least one event type");
        }
        if self.page_size > MAX_EVENT_HISTORY_PAGE_SIZE {
            return Err("event history page size exceeds the maximum");
        }
        if let Some(cursor) = &self.after {
            cursor.validate()?;
        }
        Ok(if self.page_size == 0 {
            DEFAULT_EVENT_HISTORY_PAGE_SIZE
        } else {
            self.page_size
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventHistoryPage {
    pub deliveries: Vec<EventDelivery>,
    pub next_cursor: Option<EventHistoryCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionDocumentWrite {
    pub resource_type: String,
    pub resource_id: String,
    pub source_version: i64,
    pub document: Value,
}

impl ProjectionDocumentWrite {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.resource_type.is_empty()
            || self.resource_type.len() > 180
            || self.resource_id.is_empty()
            || self.resource_id.len() > 360
            || self.source_version <= 0
            || !self.document.is_object()
        {
            return Err("projection document write is invalid");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionEventApplication {
    pub projection_id: String,
    pub delivery: EventDelivery,
    pub writes: Vec<ProjectionDocumentWrite>,
}

impl ProjectionEventApplication {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.projection_id.is_empty() || self.projection_id.len() > 180 {
            return Err("projection id is invalid");
        }
        self.delivery
            .validate()
            .map_err(|_| "projection event delivery is invalid")?;
        if self.writes.len() > MAX_PROJECTION_WRITES_PER_EVENT {
            return Err("projection event produces too many writes");
        }
        for write in &self.writes {
            write.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionApplyResult {
    pub replayed: bool,
    pub documents_written: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionCheckpoint {
    pub tenant_id: TenantId,
    pub projection_id: String,
    pub cursor: EventHistoryCursor,
    pub applied_event_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionFailure {
    pub tenant_id: TenantId,
    pub projection_id: String,
    pub event_id: EventId,
    pub occurred_at_unix_nanos: i64,
    pub failure_code: String,
}

impl ProjectionFailure {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.projection_id.is_empty() || self.projection_id.len() > 180 {
            return Err("projection id is invalid");
        }
        if self.failure_code.is_empty() || self.failure_code.len() > 180 {
            return Err("projection failure code is invalid");
        }
        if self.occurred_at_unix_nanos < 0 {
            return Err("projection failure occurrence time must not be negative");
        }
        Ok(())
    }
}

pub type ProjectionStoreFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, SdkError>> + Send + 'a>>;

/// Durable history/state boundary required by the generic projection runtime.
///
/// Implementations may use PostgreSQL or another platform-owned store, but
/// projection handlers and orchestration depend only on this port.
pub trait ProjectionStore: Send + Sync {
    fn projection_checkpoint(
        &self,
        tenant_id: TenantId,
        projection_id: String,
    ) -> ProjectionStoreFuture<'_, Option<ProjectionCheckpoint>>;

    fn list_event_history(
        &self,
        request: EventHistoryRequest,
    ) -> ProjectionStoreFuture<'_, EventHistoryPage>;

    fn apply_projection_event(
        &self,
        application: ProjectionEventApplication,
    ) -> ProjectionStoreFuture<'_, ProjectionApplyResult>;

    fn mark_projection_failed(&self, failure: ProjectionFailure) -> ProjectionStoreFuture<'_, ()>;

    fn reset_projection(
        &self,
        tenant_id: TenantId,
        projection_id: String,
    ) -> ProjectionStoreFuture<'_, ()>;
}

/// Architecture marker for `crm-core-events`.
pub const CRATE_NAME: &str = "crm-core-events";
