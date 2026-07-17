#![forbid(unsafe_code)]

//! Production composition for governed Party merge and unmerge.
//!
//! The merge-lineage owner remains pure. This crate performs authoritative
//! cross-owner integrity checks against tenant-scoped Party records and current
//! canonical redirect relationships before delegating to the transactional owner
//! executor. PostgreSQL constraints remain the final race-safe topology guard.

use crm_capability_plan_support as support;
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest,
    CapabilitySemanticValidator, TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, RecordGetQuery, RelatedRecordListQuery};
use crm_identity_resolution::{
    MergeOperation, MergeOperationId, MergeOperationStatus, PartyReference,
};
use crm_identity_resolution_capability_adapter::{
    CANONICAL_REDIRECT_PARTY_RECORD_TYPE, CANONICAL_REDIRECT_RELATIONSHIP_TYPE, MERGE_CAPABILITY,
    MERGE_MUTATION_CAPABILITY_IDS, MERGE_OPERATION_RECORD_TYPE, MODULE_ID, UNMERGE_CAPABILITY,
    UNMERGE_REQUEST_SCHEMA, merge_operation_from_snapshot, merge_reference_scope_from_request,
};
use crm_module_sdk::{
    ErrorCategory, ModuleId, PortFuture, RecordId, RecordRef, RecordType, RelationshipType,
    SdkError, TenantId,
};
use crm_parties_capability_adapter::{
    MODULE_ID as PARTIES_MODULE_ID, RECORD_TYPE as PARTY_RECORD_TYPE,
};
use crm_proto_contracts::crm::identity_resolution::v1 as wire;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

const MAXIMUM_CANONICAL_REDIRECT_HOPS: usize = 64;
const CANONICAL_REDIRECT_QUERY_PAGE_SIZE: u32 = 2;

pub trait MergeLineageReferenceReader: Send + Sync {
    fn current_party_version<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        party_ref: &'a PartyReference,
    ) -> PortFuture<'a, Result<Option<i64>, SdkError>>;

    fn merge_operation<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        operation_id: &'a MergeOperationId,
    ) -> PortFuture<'a, Result<Option<MergeOperation>, SdkError>>;

    fn immediate_redirect_target<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        party_ref: &'a PartyReference,
    ) -> PortFuture<'a, Result<Option<PartyReference>, SdkError>>;
}

#[derive(Debug, Clone)]
pub struct PostgresMergeLineageReferenceReader {
    store: PostgresDataStore,
}

impl PostgresMergeLineageReferenceReader {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }
}

impl MergeLineageReferenceReader for PostgresMergeLineageReferenceReader {
    fn current_party_version<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        party_ref: &'a PartyReference,
    ) -> PortFuture<'a, Result<Option<i64>, SdkError>> {
        Box::pin(async move {
            Ok(self
                .store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id: configured_module_id(PARTIES_MODULE_ID)?,
                    record_type: configured_record_type(PARTY_RECORD_TYPE)?,
                    record_id: configured_record_id(party_ref.as_str())?,
                })
                .await?
                .map(|snapshot| snapshot.version))
        })
    }

    fn merge_operation<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        operation_id: &'a MergeOperationId,
    ) -> PortFuture<'a, Result<Option<MergeOperation>, SdkError>> {
        Box::pin(async move {
            self.store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id: configured_module_id(MODULE_ID)?,
                    record_type: configured_record_type(MERGE_OPERATION_RECORD_TYPE)?,
                    record_id: configured_record_id(operation_id.as_str())?,
                })
                .await?
                .map(|snapshot| merge_operation_from_snapshot(&snapshot))
                .transpose()
        })
    }

    fn immediate_redirect_target<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        party_ref: &'a PartyReference,
    ) -> PortFuture<'a, Result<Option<PartyReference>, SdkError>> {
        Box::pin(async move {
            let page = self
                .store
                .list_related_records_for_query(&RelatedRecordListQuery {
                    tenant_id: tenant_id.clone(),
                    relationship_owner_module_id: configured_module_id(MODULE_ID)?,
                    relationship_type: configured_relationship_type(
                        CANONICAL_REDIRECT_RELATIONSHIP_TYPE,
                    )?,
                    source: RecordRef {
                        record_type: configured_record_type(CANONICAL_REDIRECT_PARTY_RECORD_TYPE)?,
                        record_id: configured_record_id(party_ref.as_str())?,
                    },
                    target_owner_module_id: configured_module_id(PARTIES_MODULE_ID)?,
                    target_record_type: configured_record_type(PARTY_RECORD_TYPE)?,
                    page_size: CANONICAL_REDIRECT_QUERY_PAGE_SIZE,
                    after_record_id: None,
                })
                .await?;
            if page.records.len() > 1 || page.next_record_id.is_some() {
                return Err(canonical_redirect_corrupt(
                    "more than one active canonical redirect exists for one source Party",
                ));
            }
            page.records
                .first()
                .map(|snapshot| PartyReference::try_new(snapshot.reference.record_id.as_str()))
                .transpose()
        })
    }
}

#[derive(Clone)]
pub struct MergeLineageCapabilitySemanticValidator {
    references: Arc<dyn MergeLineageReferenceReader>,
}

impl fmt::Debug for MergeLineageCapabilitySemanticValidator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MergeLineageCapabilitySemanticValidator")
            .field("references", &"MergeLineageReferenceReader")
            .finish()
    }
}

impl MergeLineageCapabilitySemanticValidator {
    pub fn new(references: Arc<dyn MergeLineageReferenceReader>) -> Self {
        Self { references }
    }

    async fn validate_merge(&self, request: &CapabilityRequest) -> Result<(), SdkError> {
        let scope = merge_reference_scope_from_request(request)?;
        let tenant_id = &request.context.execution.tenant_id;

        require_exact_party_version(
            self.references.as_ref(),
            tenant_id,
            &scope.source.party_ref,
            scope.source.expected_version,
        )
        .await?;
        require_exact_party_version(
            self.references.as_ref(),
            tenant_id,
            &scope.survivor.party_ref,
            scope.survivor.expected_version,
        )
        .await?;

        for provenance in &scope.provenance {
            let expected_version = if provenance.party_ref == scope.source.party_ref {
                scope.source.expected_version
            } else if provenance.party_ref == scope.survivor.party_ref {
                scope.survivor.expected_version
            } else {
                return Err(invalid_provenance_party());
            };
            if provenance.expected_version != expected_version {
                return Err(invalid_provenance_version());
            }
        }

        let source_root =
            resolve_canonical_root(self.references.as_ref(), tenant_id, &scope.source.party_ref)
                .await?;
        if source_root != scope.source.party_ref {
            return Err(source_not_canonical());
        }
        let survivor_root = resolve_canonical_root(
            self.references.as_ref(),
            tenant_id,
            &scope.survivor.party_ref,
        )
        .await?;
        if survivor_root != scope.survivor.party_ref {
            return Err(survivor_not_canonical());
        }
        Ok(())
    }

    async fn validate_unmerge(&self, request: &CapabilityRequest) -> Result<(), SdkError> {
        let command: wire::UnmergePartyRequest = support::decode_request_with_data_class(
            request,
            MODULE_ID,
            UNMERGE_REQUEST_SCHEMA,
            crm_module_sdk::DataClass::Personal,
        )?;
        let operation_ref = command.merge_operation_ref.ok_or_else(|| {
            SdkError::invalid_argument(
                "identity_resolution.unmerge.merge_operation_ref",
                "merge operation reference is required",
            )
        })?;
        let operation_id = MergeOperationId::try_new(operation_ref.merge_operation_id)?;
        let tenant_id = &request.context.execution.tenant_id;
        let operation = self
            .references
            .merge_operation(tenant_id, &operation_id)
            .await?
            .ok_or_else(merge_operation_unavailable)?;
        if operation.status() != MergeOperationStatus::Active {
            return Err(merge_operation_not_active());
        }
        require_exact_party_version(
            self.references.as_ref(),
            tenant_id,
            operation.source_party_ref(),
            command.expected_source_party_version,
        )
        .await?;
        require_exact_party_version(
            self.references.as_ref(),
            tenant_id,
            operation.survivor_party_ref(),
            command.expected_survivor_party_version,
        )
        .await?;
        let redirect_target = self
            .references
            .immediate_redirect_target(tenant_id, operation.source_party_ref())
            .await?
            .ok_or_else(|| {
                canonical_redirect_corrupt("active merge operation has no active redirect")
            })?;
        if redirect_target != *operation.survivor_party_ref() {
            return Err(canonical_redirect_corrupt(
                "active merge operation redirect target does not match its survivor Party",
            ));
        }
        Ok(())
    }
}

impl CapabilitySemanticValidator for MergeLineageCapabilitySemanticValidator {
    fn validate<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            match definition.capability_id.as_str() {
                MERGE_CAPABILITY => self.validate_merge(request).await,
                UNMERGE_CAPABILITY => self.validate_unmerge(request).await,
                capability if MERGE_MUTATION_CAPABILITY_IDS.contains(&capability) => Ok(()),
                _ => Err(unsupported_capability()),
            }
        })
    }
}

#[derive(Clone)]
pub struct MergeLineageCapabilityExecutor {
    inner: Arc<dyn TransactionalCapabilityExecutor>,
}

impl fmt::Debug for MergeLineageCapabilityExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MergeLineageCapabilityExecutor")
            .field("inner", &"TransactionalCapabilityExecutor")
            .finish()
    }
}

impl MergeLineageCapabilityExecutor {
    pub fn new(inner: Arc<dyn TransactionalCapabilityExecutor>) -> Self {
        Self { inner }
    }
}

impl TransactionalCapabilityExecutor for MergeLineageCapabilityExecutor {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        if !MERGE_MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            return Box::pin(async { Err(unsupported_capability()) });
        }
        self.inner.execute(definition, request)
    }
}

async fn require_exact_party_version(
    references: &dyn MergeLineageReferenceReader,
    tenant_id: &TenantId,
    party_ref: &PartyReference,
    expected_version: i64,
) -> Result<(), SdkError> {
    if expected_version <= 0 {
        return Err(SdkError::invalid_argument(
            "identity_resolution.merge.party_version",
            "Party version must be positive",
        ));
    }
    let actual = references
        .current_party_version(tenant_id, party_ref)
        .await?
        .ok_or_else(party_reference_unavailable)?;
    if actual != expected_version {
        return Err(stale_party_version());
    }
    Ok(())
}

async fn resolve_canonical_root(
    references: &dyn MergeLineageReferenceReader,
    tenant_id: &TenantId,
    party_ref: &PartyReference,
) -> Result<PartyReference, SdkError> {
    let mut current = party_ref.clone();
    let mut visited = BTreeSet::from([current.clone()]);
    for _ in 0..MAXIMUM_CANONICAL_REDIRECT_HOPS {
        let Some(next) = references
            .immediate_redirect_target(tenant_id, &current)
            .await?
        else {
            return Ok(current);
        };
        if !visited.insert(next.clone()) {
            return Err(canonical_redirect_corrupt(
                "canonical redirect topology contains a cycle",
            ));
        }
        current = next;
    }
    Err(canonical_redirect_corrupt(
        "canonical redirect topology exceeds the supported hop bound",
    ))
}

fn configured_module_id(value: &str) -> Result<ModuleId, SdkError> {
    ModuleId::try_new(value).map_err(config_error)
}

fn configured_record_type(value: &str) -> Result<RecordType, SdkError> {
    RecordType::try_new(value).map_err(config_error)
}

fn configured_record_id(value: &str) -> Result<RecordId, SdkError> {
    RecordId::try_new(value).map_err(config_error)
}

fn configured_relationship_type(value: &str) -> Result<RelationshipType, SdkError> {
    RelationshipType::try_new(value).map_err(config_error)
}

fn party_reference_unavailable() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MERGE_PARTY_REFERENCE_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "One or more referenced Parties are unavailable.",
    )
}

fn stale_party_version() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MERGE_PARTY_VERSION_STALE",
        ErrorCategory::Conflict,
        true,
        "One or more referenced Party versions are stale.",
    )
}

fn invalid_provenance_party() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_SURVIVORSHIP_PROVENANCE_PARTY_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "Survivorship provenance must belong to the source or survivor Party lineage.",
    )
}

fn invalid_provenance_version() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_SURVIVORSHIP_PROVENANCE_VERSION_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "Survivorship provenance must use the exact governed Party version.",
    )
}

fn source_not_canonical() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MERGE_SOURCE_ALREADY_REDIRECTED",
        ErrorCategory::Conflict,
        false,
        "The proposed merge source is not a current canonical Party root.",
    )
}

fn survivor_not_canonical() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MERGE_SURVIVOR_NOT_CANONICAL",
        ErrorCategory::Conflict,
        false,
        "The proposed survivor is not a current canonical Party root.",
    )
}

fn merge_operation_unavailable() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MERGE_OPERATION_UNAVAILABLE",
        ErrorCategory::NotFound,
        false,
        "The requested merge operation was not found.",
    )
}

fn merge_operation_not_active() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MERGE_OPERATION_NOT_ACTIVE",
        ErrorCategory::Conflict,
        false,
        "The requested merge operation is not active.",
    )
}

fn canonical_redirect_corrupt(internal: impl Into<String>) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_CANONICAL_REDIRECT_INVALID",
        ErrorCategory::Internal,
        false,
        "The canonical Party redirect topology is temporarily unavailable.",
    )
    .with_internal_reference(internal)
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MERGE_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Identity Resolution merge capability is unsupported.",
    )
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_MERGE_COMPOSITION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Identity Resolution merge composition is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_capability_coordinates_are_separate_from_candidate_mutations() {
        assert_eq!(
            MERGE_MUTATION_CAPABILITY_IDS,
            [MERGE_CAPABILITY, UNMERGE_CAPABILITY]
        );
        assert_eq!(
            CANONICAL_REDIRECT_RELATIONSHIP_TYPE,
            "identity_resolution.canonical_redirect"
        );
    }
}
