use super::{GET_CAPABILITY, GET_REQUEST_SCHEMA, PartyQueryAdapter, party_record_type};
use crm_capability_plan_support as support;
use crm_core_data::{RecordGetQuery, RecordId};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, DataClass, ErrorCategory, ModuleId,
    RequestId, SchemaVersion, SdkError, TenantId, TraceId,
};
use crm_parties::PartyKind;
use crm_parties_capability_adapter::{MODULE_ID, party_from_snapshot};
use crm_proto_contracts::crm::{customer::v1 as customer, parties::v1 as wire};
use crm_query_runtime::{
    QueryExecutionContext, QueryRequest, QueryVisibilityAuthorizer, normalized_filter_hash,
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartyExportExecutionRead {
    NotVisible,
    VersionChanged,
    Unavailable,
    Visible {
        party_id: RecordId,
        kind: PartyKind,
        display_name: String,
        resource_version: i64,
        allowed_fields: BTreeSet<String>,
    },
}

pub fn export_execution_query_request(
    tenant_id: &TenantId,
    actor_id: &ActorId,
    request_identity: &str,
    party_id: &RecordId,
    request_started_at_unix_nanos: i64,
) -> Result<QueryRequest, SdkError> {
    let command = wire::GetPartyRequest {
        party_ref: Some(customer::PartyRef {
            party_id: party_id.as_str().to_owned(),
        }),
    };
    let input = support::protobuf_payload(
        MODULE_ID,
        GET_REQUEST_SCHEMA,
        DataClass::Personal,
        &command,
    )?;
    let input_hash = normalized_filter_hash([("party_id", party_id.as_str().as_bytes())]);
    Ok(QueryRequest {
        owner_module_id: ModuleId::try_new(MODULE_ID).map_err(config_error)?,
        context: QueryExecutionContext {
            tenant_id: tenant_id.clone(),
            actor_id: actor_id.clone(),
            request_id: RequestId::try_new(request_identity).map_err(config_error)?,
            correlation_id: CorrelationId::try_new(request_identity).map_err(config_error)?,
            trace_id: TraceId::try_new(request_identity).map_err(config_error)?,
            capability_id: CapabilityId::try_new(GET_CAPABILITY).map_err(config_error)?,
            capability_version: CapabilityVersion::try_new(support::CONTRACT_VERSION)
                .map_err(config_error)?,
            schema_version: SchemaVersion::try_new(support::CONTRACT_VERSION)
                .map_err(config_error)?,
            request_started_at_unix_nanos,
        },
        input,
        input_hash,
    })
}

impl PartyQueryAdapter {
    /// Worker-private exact Party read for deterministic export execution.
    ///
    /// The caller must perform the top-level Party GET authorization first. This method then performs
    /// the tenant/RLS authoritative read, live per-resource/field visibility decision and exact
    /// resource-version comparison without exposing any public bulk or storage bypass.
    pub fn get_for_export_execution<'a>(
        &'a self,
        request: &'a QueryRequest,
        party_id: &'a RecordId,
        expected_resource_version: i64,
    ) -> crm_module_sdk::PortFuture<'a, Result<PartyExportExecutionRead, SdkError>> {
        Box::pin(async move {
            request.context.validate()?;
            if expected_resource_version <= 0 {
                return Err(SdkError::invalid_argument(
                    "customer_data.export.expected_resource_version",
                    "expected Party resource version must be positive",
                ));
            }
            let snapshot = self
                .store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: request.context.tenant_id.clone(),
                    owner_module_id: ModuleId::try_new(MODULE_ID).map_err(config_error)?,
                    record_type: party_record_type()?,
                    record_id: party_id.clone(),
                })
                .await?;
            let Some(snapshot) = snapshot else {
                return Ok(PartyExportExecutionRead::Unavailable);
            };
            let visibility = self
                .visibility
                .authorize_visibility(request, &snapshot.reference)
                .await?;
            if !visibility.resource_visible {
                return Ok(PartyExportExecutionRead::NotVisible);
            }
            if snapshot.version != expected_resource_version {
                return Ok(PartyExportExecutionRead::VersionChanged);
            }
            let party = party_from_snapshot(&snapshot)?;
            Ok(PartyExportExecutionRead::Visible {
                party_id: snapshot.reference.record_id,
                kind: party.kind(),
                display_name: party.display_name().to_owned(),
                resource_version: snapshot.version,
                allowed_fields: visibility.allowed_fields,
            })
        })
    }
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "PARTIES_EXPORT_EXECUTION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The governed Party export execution read is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_get_request_is_bound_to_party_identity() {
        let tenant_id = TenantId::try_new("tenant-export-execution-request").unwrap();
        let actor_id = ActorId::try_new("export-execution-worker").unwrap();
        let party_id = RecordId::try_new("party-export-execution-request").unwrap();
        let request = export_execution_query_request(
            &tenant_id,
            &actor_id,
            "export-execution-request",
            &party_id,
            100,
        )
        .unwrap();
        assert_eq!(request.context.tenant_id, tenant_id);
        assert_eq!(request.context.actor_id, actor_id);
        assert_eq!(request.context.capability_id.as_str(), GET_CAPABILITY);
    }
}
