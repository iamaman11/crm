#[path = "import_execution_types.rs"]
mod import_execution_types;
pub use import_execution_types::*;
#[path = "import_execution_coordinator.rs"]
mod import_execution_coordinator;
pub use import_execution_coordinator::*;

use crm_module_sdk::{ActorId, PortFuture, RecordId, SdkError, TenantId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyExportExecutionSourceKind {
    Person,
    Organization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartyExportExecutionSourceResult {
    NotVisible,
    VersionChanged,
    Unavailable,
    Visible {
        party_id: RecordId,
        kind: Option<PartyExportExecutionSourceKind>,
        display_name: Option<String>,
        resource_version: i64,
    },
}

#[derive(Debug, Clone)]
pub struct PartyExportExecutionSourceRequest<'a> {
    pub tenant_id: &'a TenantId,
    pub actor_id: &'a ActorId,
    pub job_id: &'a str,
    pub party_id: &'a RecordId,
    pub expected_resource_version: i64,
    pub request_started_at_unix_nanos: i64,
}

/// Worker-private governed exact Party read used by deterministic export execution.
///
/// Production composition must perform top-level Party GET authorization before the authoritative
/// read and must repeat live per-resource/field visibility before returning visible values.
pub trait PartyExportExecutionSource: Send + Sync {
    fn get<'a>(
        &'a self,
        request: PartyExportExecutionSourceRequest<'a>,
    ) -> PortFuture<'a, Result<PartyExportExecutionSourceResult, SdkError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_result_has_exact_bounded_terminal_categories() {
        assert_ne!(
            PartyExportExecutionSourceResult::NotVisible,
            PartyExportExecutionSourceResult::Unavailable
        );
        assert_ne!(
            PartyExportExecutionSourceResult::VersionChanged,
            PartyExportExecutionSourceResult::Unavailable
        );
    }
}
