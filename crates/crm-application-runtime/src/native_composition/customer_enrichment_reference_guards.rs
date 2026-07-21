use crm_capability_plan_support as support;
use crm_capability_runtime::CapabilityRequest;
use crm_core_data::TransactionalAggregateGuard;
use crm_customer_enrichment::decode_provider_profile_version_state;
use crm_customer_enrichment_capability_adapter::{
    CREATE_ENRICHMENT_REQUEST_CAPABILITY, MODULE_ID, PROVIDER_PROFILE_VERSION_RECORD_TYPE,
    PUBLISH_MAPPING_CAPABILITY, PUBLISH_MAPPING_REQUEST_SCHEMA, REQUEST_PARTY_SOURCE_RECORD_TYPE,
    enrichment_request_from_create_request, mapping_from_definition,
};
use crm_module_sdk::{DataClass, ErrorCategory, PortFuture, SdkError};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use sqlx::{Postgres, Row, Transaction};

const PARTIES_MODULE_ID: &str = "crm.parties";

#[derive(Debug, Default, Clone, Copy)]
pub struct PostgresCustomerEnrichmentMappingReferenceGuard;

impl TransactionalAggregateGuard for PostgresCustomerEnrichmentMappingReferenceGuard {
    fn check<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            if request.context.execution.capability_id.as_str() != PUBLISH_MAPPING_CAPABILITY {
                return Ok(());
            }
            let command: wire::PublishMappingVersionRequest =
                support::decode_request_with_data_class(
                    request,
                    MODULE_ID,
                    PUBLISH_MAPPING_REQUEST_SCHEMA,
                    DataClass::Confidential,
                )?;
            let mapping = mapping_from_definition(command.definition)?;
            let row = sqlx::query(
                r#"
                SELECT record_id, payload_bytes
                FROM crm.records
                WHERE tenant_id = $1
                  AND owner_module_id = $2
                  AND record_type = $3
                  AND record_id = $4
                  AND deleted_at IS NULL
                FOR SHARE
                "#,
            )
            .bind(request.context.execution.tenant_id.as_str())
            .bind(MODULE_ID)
            .bind(PROVIDER_PROFILE_VERSION_RECORD_TYPE)
            .bind(mapping.provider_profile_version_id().as_str())
            .fetch_optional(&mut **transaction)
            .await
            .map_err(mapping_store_unavailable)?
            .ok_or_else(mapping_reference_unavailable)?;

            let record_id: String = row.try_get("record_id").map_err(mapping_store_unavailable)?;
            let payload: Vec<u8> = row
                .try_get("payload_bytes")
                .map_err(mapping_store_unavailable)?;
            let profile = decode_provider_profile_version_state(&payload)
                .map_err(mapping_state_invalid)?;
            if record_id != profile.version_id().as_str()
                || profile.version_id() != mapping.provider_profile_version_id()
            {
                return Err(mapping_state_invalid(
                    "provider-profile row identity differs from canonical state",
                ));
            }
            if !profile
                .supported_target_fields()
                .contains(&mapping.target_field())
            {
                return Err(mapping_target_unsupported());
            }
            Ok(())
        })
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PostgresCustomerEnrichmentRequestPartyGuard;

impl TransactionalAggregateGuard for PostgresCustomerEnrichmentRequestPartyGuard {
    fn check<'a>(
        &'a self,
        transaction: &'a mut Transaction<'_, Postgres>,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            if request.context.execution.capability_id.as_str()
                != CREATE_ENRICHMENT_REQUEST_CAPABILITY
            {
                return Ok(());
            }
            let enrichment_request = enrichment_request_from_create_request(request)?;
            let expected_version = i64::try_from(enrichment_request.target().resource_version)
                .map_err(|_| {
                    request_target_stale(
                        "requested Party resource version exceeds the storage range",
                    )
                })?;
            let row = sqlx::query(
                r#"
                SELECT record_id, version
                FROM crm.records
                WHERE tenant_id = $1
                  AND owner_module_id = $2
                  AND record_type = $3
                  AND record_id = $4
                  AND deleted_at IS NULL
                FOR SHARE
                "#,
            )
            .bind(request.context.execution.tenant_id.as_str())
            .bind(PARTIES_MODULE_ID)
            .bind(REQUEST_PARTY_SOURCE_RECORD_TYPE)
            .bind(enrichment_request.target().resource_id.as_str())
            .fetch_optional(&mut **transaction)
            .await
            .map_err(request_store_unavailable)?
            .ok_or_else(request_target_unavailable)?;

            let record_id: String = row.try_get("record_id").map_err(request_store_unavailable)?;
            let version: i64 = row.try_get("version").map_err(request_store_unavailable)?;
            if record_id != enrichment_request.target().resource_id.as_str()
                || version != expected_version
            {
                return Err(request_target_stale(
                    "locked Party snapshot differs from the exact request target version",
                ));
            }
            Ok(())
        })
    }
}

fn mapping_reference_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_MAPPING_PROVIDER_PROFILE_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "The referenced provider-profile version is unavailable.",
    )
}

fn mapping_target_unsupported() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_MAPPING_TARGET_FIELD_UNSUPPORTED",
        ErrorCategory::InvalidArgument,
        false,
        "The referenced provider profile does not support the mapping target field.",
    )
}

fn mapping_store_unavailable(reference: impl ToString) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_MAPPING_REFERENCE_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The provider-profile reference could not be verified atomically.",
    )
    .with_internal_reference(reference.to_string())
}

fn mapping_state_invalid(reference: impl ToString) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_MAPPING_PROVIDER_PROFILE_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The referenced provider-profile state is invalid.",
    )
    .with_internal_reference(reference.to_string())
}

fn request_target_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_TARGET_UNAVAILABLE",
        ErrorCategory::NotFound,
        false,
        "The Party target is unavailable.",
    )
}

fn request_target_stale(reference: impl ToString) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_TARGET_STALE",
        ErrorCategory::Conflict,
        false,
        "The Party resource version changed before the enrichment request was committed.",
    )
    .with_internal_reference(reference.to_string())
}

fn request_store_unavailable(reference: impl ToString) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_TARGET_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "The Party target could not be verified atomically.",
    )
    .with_internal_reference(reference.to_string())
}
