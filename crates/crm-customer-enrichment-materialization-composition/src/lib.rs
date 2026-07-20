#![forbid(unsafe_code)]

//! PostgreSQL composition for worker-only Customer Enrichment suggestion materialization.

mod candidate_evidence;

pub use candidate_evidence::*;

use crm_capability_plan_support::{self as support, PersistedPayloadContract};
use crm_capability_runtime::{
    CapabilityExecutionResult, CapabilityRequest, TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, PostgresTransactionalAggregateExecutor};
use crm_customer_enrichment::{
    LIFECYCLE_STATE_RETENTION_POLICY_ID, LIFECYCLE_STATE_SCHEMA_VERSION,
    MAPPING_VERSION_RECORD_TYPE, PROVIDER_PROFILE_VERSION_RECORD_TYPE,
    PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE, PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES,
    PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID, ProviderResponseReceipt,
    decode_provider_response_receipt_state, provider_response_receipt_state_descriptor_hash,
};
use crm_customer_enrichment_capability_adapter::{
    MODULE_ID, mapping_from_snapshot, provider_profile_from_snapshot,
};
use crm_customer_enrichment_materialization_adapter::{
    CustomerEnrichmentSuggestionMaterializationPlanner, MATERIALIZE_SUGGESTIONS_REQUEST_SCHEMA,
    suggestion_materialization_capability_definition,
};
use crm_module_sdk::{DataClass, ErrorCategory, RecordId, RecordRef, SdkError};
use crm_proto_contracts::crm::customer_enrichment::v1 as wire;
use serde::Deserialize;
use std::sync::Arc;

/// Stable crate identity for architecture tooling.
pub const CRATE_NAME: &str = "crm-customer-enrichment-materialization-composition";

/// Durable non-runtime materialization coordinator over exact immutable dependency snapshots.
#[derive(Debug, Clone)]
pub struct PostgresCustomerEnrichmentSuggestionMaterializationWorker {
    store: PostgresDataStore,
}

impl PostgresCustomerEnrichmentSuggestionMaterializationWorker {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }

    pub async fn execute(
        &self,
        request: CapabilityRequest,
    ) -> Result<CapabilityExecutionResult, SdkError> {
        let definition = suggestion_materialization_capability_definition()?;
        let command: wire::MaterializeSuggestionsRequest = support::decode_request_with_data_class(
            &request,
            MODULE_ID,
            MATERIALIZE_SUGGESTIONS_REQUEST_SCHEMA,
            DataClass::Personal,
        )?;
        let request_ref = command.enrichment_request_ref.as_ref().ok_or_else(|| {
            SdkError::invalid_argument(
                "customer_enrichment.enrichment_request_ref",
                "Enrichment-request reference is required",
            )
        })?;
        let receipt_ref = command
            .provider_response_receipt_ref
            .as_ref()
            .ok_or_else(|| {
                SdkError::invalid_argument(
                    "customer_enrichment.provider_response_receipt_ref",
                    "Provider-response receipt reference is required",
                )
            })?;

        let receipt_snapshot = self
            .store
            .get_record(
                &request.context,
                &record_ref(
                    PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE,
                    &receipt_ref.provider_response_receipt_id,
                    "customer_enrichment.provider_response_receipt_ref.provider_response_receipt_id",
                )?,
            )
            .await
            .map_err(|error| store_read_failed(error.to_string()))?
            .ok_or_else(receipt_not_found)?;
        let ReceiptDependency {
            receipt,
            provider_profile_version_id,
            mapping_version_id,
        } = receipt_from_snapshot(&receipt_snapshot)?;
        if receipt.request_id().as_str() != request_ref.enrichment_request_id {
            return Err(dependency_conflict(
                "CUSTOMER_ENRICHMENT_MATERIALIZATION_REQUEST_RECEIPT_CONFLICT",
                "The response receipt does not belong to the requested enrichment request.",
            ));
        }

        let profile_snapshot = self
            .store
            .get_record(
                &request.context,
                &record_ref(
                    PROVIDER_PROFILE_VERSION_RECORD_TYPE,
                    &provider_profile_version_id,
                    "customer_enrichment.provider_profile_version_ref.provider_profile_version_id",
                )?,
            )
            .await
            .map_err(|error| store_read_failed(error.to_string()))?
            .ok_or_else(profile_not_found)?;
        let mapping_snapshot = self
            .store
            .get_record(
                &request.context,
                &record_ref(
                    MAPPING_VERSION_RECORD_TYPE,
                    &mapping_version_id,
                    "customer_enrichment.mapping_version_ref.mapping_version_id",
                )?,
            )
            .await
            .map_err(|error| store_read_failed(error.to_string()))?
            .ok_or_else(mapping_not_found)?;

        let profile = provider_profile_from_snapshot(&profile_snapshot)?;
        let mapping = mapping_from_snapshot(&mapping_snapshot)?;
        let planner =
            CustomerEnrichmentSuggestionMaterializationPlanner::new(receipt, profile, mapping);
        let executor =
            PostgresTransactionalAggregateExecutor::new(self.store.clone(), Arc::new(planner));
        executor.execute(&definition, request).await
    }
}

struct ReceiptDependency {
    receipt: ProviderResponseReceipt,
    provider_profile_version_id: String,
    mapping_version_id: String,
}

#[derive(Deserialize)]
struct ReceiptLineageView {
    provider_profile_version_id: String,
    mapping_version_id: String,
}

fn receipt_from_snapshot(
    snapshot: &crm_module_sdk::RecordSnapshot,
) -> Result<ReceiptDependency, SdkError> {
    if snapshot.reference.record_type.as_str() != PROVIDER_RESPONSE_RECEIPT_RECORD_TYPE
        || snapshot.version != 1
    {
        return Err(invalid_receipt_snapshot(
            "record type or immutable version is invalid",
        ));
    }
    let bytes = support::persisted_json_bytes_with_data_class(
        snapshot,
        PersistedPayloadContract {
            owner: MODULE_ID,
            schema_id: PROVIDER_RESPONSE_RECEIPT_STATE_SCHEMA_ID,
            schema_version: LIFECYCLE_STATE_SCHEMA_VERSION,
            descriptor_hash: provider_response_receipt_state_descriptor_hash(),
            maximum_size_bytes: PROVIDER_RESPONSE_RECEIPT_STATE_MAXIMUM_BYTES,
            retention_policy_id: LIFECYCLE_STATE_RETENTION_POLICY_ID,
        },
        DataClass::Personal,
    )?;
    let receipt = decode_provider_response_receipt_state(bytes)?;
    if snapshot.reference.record_id.as_str() != receipt.receipt_id().as_str() {
        return Err(invalid_receipt_snapshot(
            "record identity differs from the content-derived receipt identity",
        ));
    }
    let lineage: ReceiptLineageView = serde_json::from_slice(bytes).map_err(|error| {
        invalid_receipt_snapshot(format!("receipt lineage decode failed: {error}"))
    })?;
    Ok(ReceiptDependency {
        receipt,
        provider_profile_version_id: lineage.provider_profile_version_id,
        mapping_version_id: lineage.mapping_version_id,
    })
}

fn record_ref(
    record_type: &str,
    record_id: &str,
    field: &'static str,
) -> Result<RecordRef, SdkError> {
    support::record_ref(
        record_type,
        RecordId::try_new(record_id.to_owned())
            .map_err(|error| SdkError::invalid_argument(field, error.to_string()))?
            .as_str(),
        field,
    )
}

fn receipt_not_found() -> SdkError {
    dependency_not_found(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_RECEIPT_NOT_FOUND",
        "The provider response receipt was not found.",
    )
}

fn profile_not_found() -> SdkError {
    dependency_not_found(
        "CUSTOMER_ENRICHMENT_PROVIDER_PROFILE_NOT_FOUND",
        "The provider profile version was not found.",
    )
}

fn mapping_not_found() -> SdkError {
    dependency_not_found(
        "CUSTOMER_ENRICHMENT_MAPPING_NOT_FOUND",
        "The mapping version was not found.",
    )
}

fn dependency_not_found(code: &'static str, message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::NotFound, false, message)
}

fn dependency_conflict(code: &'static str, message: &'static str) -> SdkError {
    SdkError::new(code, ErrorCategory::Conflict, false, message)
}

fn store_read_failed(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_MATERIALIZATION_STORE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Customer Enrichment materialization dependencies could not be loaded.",
    )
    .with_internal_reference(reference.into())
}

fn invalid_receipt_snapshot(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_PROVIDER_RESPONSE_RECEIPT_PERSISTED_STATE_INVALID",
        ErrorCategory::Internal,
        false,
        "The persisted provider response receipt is invalid.",
    )
    .with_internal_reference(reference.into())
}
