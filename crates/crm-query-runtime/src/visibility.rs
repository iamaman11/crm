use crate::QueryRequest;
use crm_module_sdk::{PortFuture, RecordRef, SdkError};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryVisibilityDecision {
    pub resource_visible: bool,
    pub allowed_fields: BTreeSet<String>,
    pub decision_id: String,
    pub policy_version: String,
}

impl QueryVisibilityDecision {
    pub fn denied(decision_id: impl Into<String>, policy_version: impl Into<String>) -> Self {
        Self {
            resource_visible: false,
            allowed_fields: BTreeSet::new(),
            decision_id: decision_id.into(),
            policy_version: policy_version.into(),
        }
    }

    pub fn allows_field(&self, field: &str) -> bool {
        self.resource_visible && self.allowed_fields.contains(field)
    }
}

pub trait QueryVisibilityAuthorizer: Send + Sync {
    fn authorize_visibility<'a>(
        &'a self,
        request: &'a QueryRequest,
        resource: &'a RecordRef,
    ) -> PortFuture<'a, Result<QueryVisibilityDecision, SdkError>>;
}
