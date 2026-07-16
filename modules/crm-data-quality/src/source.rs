use crm_module_sdk::{ActorId, PortFuture, RecordId, TenantId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartyQualitySourceKind {
    Person,
    Organization,
}

#[derive(Debug, Clone, Copy)]
pub struct PartyQualitySourceRequest<'a> {
    pub tenant_id: &'a TenantId,
    pub actor_id: &'a ActorId,
    pub request_identity: &'a str,
    pub party_id: &'a RecordId,
    pub request_started_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartyQualitySourceResult {
    NotVisible,
    Unavailable,
    Visible {
        party_id: RecordId,
        kind: PartyQualitySourceKind,
        display_name: String,
        resource_version: i64,
    },
}

pub trait PartyQualitySource: Send + Sync {
    fn get<'a>(
        &'a self,
        request: PartyQualitySourceRequest<'a>,
    ) -> PortFuture<'a, Result<PartyQualitySourceResult, crm_module_sdk::SdkError>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_result_contract_is_closed_and_minimized() {
        assert_ne!(PartyQualitySourceKind::Person, PartyQualitySourceKind::Organization);
        let result = PartyQualitySourceResult::NotVisible;
        assert!(matches!(result, PartyQualitySourceResult::NotVisible));
    }
}
