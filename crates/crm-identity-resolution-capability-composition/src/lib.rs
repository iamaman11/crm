#![forbid(unsafe_code)]

//! Application composition for authoritative Identity Resolution mutations.
//!
//! The pure `crm.identity-resolution` owner never reads Party storage. This
//! composition boundary resolves same-tenant Party references and exact source
//! versions before delegating to the transactional owner executor. Terminal
//! reviewer decisions also re-check the current evidence snapshot against live
//! Party versions, so stale evidence cannot silently become a durable decision.

use crm_capability_plan_support as support;
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest,
    TransactionalCapabilityExecutor,
};
use crm_core_data::{PostgresDataStore, RecordGetQuery};
use crm_identity_resolution::{DuplicateCandidateCase, PartyReference};
use crm_identity_resolution_capability_adapter::{
    CONFIRM_CAPABILITY, CONFIRM_REQUEST_SCHEMA, DISMISS_CAPABILITY, DISMISS_REQUEST_SCHEMA,
    EvidenceReferenceScope, MODULE_ID, MUTATION_CAPABILITY_IDS, RECORD_TYPE, REFRESH_CAPABILITY,
    REGISTER_CAPABILITY, duplicate_candidate_case_from_snapshot,
    evidence_reference_scope_from_request,
};
use crm_module_sdk::{
    DataClass, ErrorCategory, ModuleId, PortFuture, RecordId, RecordType, SdkError, TenantId,
};
use crm_parties_capability_adapter::{
    MODULE_ID as PARTIES_MODULE_ID, RECORD_TYPE as PARTY_RECORD_TYPE,
};
use crm_proto_contracts::crm::identity_resolution::v1 as wire;
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateEvidenceVersions {
    pub case_id: String,
    pub parties: Vec<(PartyReference, i64)>,
}

pub trait IdentityResolutionReferenceReader: Send + Sync {
    fn party_version<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        party_ref: &'a PartyReference,
    ) -> PortFuture<'a, Result<Option<i64>, SdkError>>;

    fn candidate_evidence_versions<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        case_id: &'a str,
    ) -> PortFuture<'a, Result<Option<CandidateEvidenceVersions>, SdkError>>;
}

#[derive(Debug, Clone)]
pub struct PostgresIdentityResolutionReferenceReader {
    store: PostgresDataStore,
}

impl PostgresIdentityResolutionReferenceReader {
    pub fn new(store: PostgresDataStore) -> Self {
        Self { store }
    }
}

impl IdentityResolutionReferenceReader for PostgresIdentityResolutionReferenceReader {
    fn party_version<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        party_ref: &'a PartyReference,
    ) -> PortFuture<'a, Result<Option<i64>, SdkError>> {
        Box::pin(async move {
            let owner_module_id = ModuleId::try_new(PARTIES_MODULE_ID).map_err(config_error)?;
            let record_type = RecordType::try_new(PARTY_RECORD_TYPE).map_err(config_error)?;
            let record_id = RecordId::try_new(party_ref.as_str()).map_err(config_error)?;
            Ok(self
                .store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id,
                    record_type,
                    record_id,
                })
                .await?
                .map(|snapshot| snapshot.version))
        })
    }

    fn candidate_evidence_versions<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        case_id: &'a str,
    ) -> PortFuture<'a, Result<Option<CandidateEvidenceVersions>, SdkError>> {
        Box::pin(async move {
            let owner_module_id = ModuleId::try_new(MODULE_ID).map_err(config_error)?;
            let record_type = RecordType::try_new(RECORD_TYPE).map_err(config_error)?;
            let record_id = RecordId::try_new(case_id).map_err(config_error)?;
            let Some(snapshot) = self
                .store
                .get_record_for_query(&RecordGetQuery {
                    tenant_id: tenant_id.clone(),
                    owner_module_id,
                    record_type,
                    record_id,
                })
                .await?
            else {
                return Ok(None);
            };
            let candidate = duplicate_candidate_case_from_snapshot(&snapshot)?;
            Ok(Some(candidate_evidence_versions(&candidate)))
        })
    }
}

#[derive(Clone)]
pub struct IdentityResolutionCapabilityExecutor {
    references: Arc<dyn IdentityResolutionReferenceReader>,
    inner: Arc<dyn TransactionalCapabilityExecutor>,
}

impl fmt::Debug for IdentityResolutionCapabilityExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("IdentityResolutionCapabilityExecutor")
            .field("references", &"dyn IdentityResolutionReferenceReader")
            .field("inner", &"dyn TransactionalCapabilityExecutor")
            .finish()
    }
}

impl IdentityResolutionCapabilityExecutor {
    pub fn new(
        references: Arc<dyn IdentityResolutionReferenceReader>,
        inner: Arc<dyn TransactionalCapabilityExecutor>,
    ) -> Self {
        Self { references, inner }
    }
}

impl TransactionalCapabilityExecutor for IdentityResolutionCapabilityExecutor {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        if !MUTATION_CAPABILITY_IDS.contains(&definition.capability_id.as_str()) {
            return Box::pin(async { Err(unsupported_capability()) });
        }
        Box::pin(async move {
            let capability_id = definition.capability_id.as_str();
            let tenant_id = &request.context.execution.tenant_id;
            if let Some(scope) = evidence_reference_scope_from_request(capability_id, &request)? {
                validate_evidence_scope(self.references.as_ref(), tenant_id, &scope).await?;
            }
            if matches!(capability_id, DISMISS_CAPABILITY | CONFIRM_CAPABILITY) {
                let case_id = candidate_case_id_from_request(capability_id, &request)?;
                validate_current_candidate_evidence(
                    self.references.as_ref(),
                    tenant_id,
                    case_id.as_str(),
                )
                .await?;
            }
            self.inner.execute(definition, request).await
        })
    }
}

pub async fn validate_evidence_scope(
    reader: &dyn IdentityResolutionReferenceReader,
    tenant_id: &TenantId,
    scope: &EvidenceReferenceScope,
) -> Result<(), SdkError> {
    if scope.parties.len() != 2 || scope.parties[0].party_ref == scope.parties[1].party_ref {
        return Err(reference_unavailable());
    }
    for expectation in &scope.parties {
        let Some(actual_version) = reader
            .party_version(tenant_id, &expectation.party_ref)
            .await?
        else {
            return Err(reference_unavailable());
        };
        if actual_version != expectation.expected_version {
            return Err(evidence_stale());
        }
    }
    Ok(())
}

pub async fn validate_current_candidate_evidence(
    reader: &dyn IdentityResolutionReferenceReader,
    tenant_id: &TenantId,
    case_id: &str,
) -> Result<(), SdkError> {
    let Some(current) = reader
        .candidate_evidence_versions(tenant_id, case_id)
        .await?
    else {
        return Err(reference_unavailable());
    };
    if current.case_id != case_id || current.parties.len() != 2 {
        return Err(reference_unavailable());
    }
    let scope = EvidenceReferenceScope {
        parties: current
            .parties
            .into_iter()
            .map(|(party_ref, expected_version)| {
                crm_identity_resolution_capability_adapter::PartyVersionExpectation {
                    party_ref,
                    expected_version,
                }
            })
            .collect(),
    };
    validate_evidence_scope(reader, tenant_id, &scope).await
}

fn candidate_evidence_versions(candidate: &DuplicateCandidateCase) -> CandidateEvidenceVersions {
    let evidence = candidate.current_evidence();
    CandidateEvidenceVersions {
        case_id: candidate.case_id().as_str().to_owned(),
        parties: vec![
            (
                evidence.pair().left().clone(),
                evidence.left_party_version(),
            ),
            (
                evidence.pair().right().clone(),
                evidence.right_party_version(),
            ),
        ],
    }
}

fn candidate_case_id_from_request(
    capability_id: &str,
    request: &CapabilityRequest,
) -> Result<String, SdkError> {
    let case_ref = match capability_id {
        DISMISS_CAPABILITY => {
            let command: wire::DismissDuplicateCandidateRequest =
                support::decode_request_with_data_class(
                    request,
                    MODULE_ID,
                    DISMISS_REQUEST_SCHEMA,
                    DataClass::Personal,
                )?;
            command.case_ref
        }
        CONFIRM_CAPABILITY => {
            let command: wire::ConfirmDuplicateCandidateRequest =
                support::decode_request_with_data_class(
                    request,
                    MODULE_ID,
                    CONFIRM_REQUEST_SCHEMA,
                    DataClass::Personal,
                )?;
            command.case_ref
        }
        REGISTER_CAPABILITY | REFRESH_CAPABILITY => return Err(unsupported_capability()),
        _ => return Err(unsupported_capability()),
    };
    let case_ref = case_ref.ok_or_else(|| {
        SdkError::invalid_argument(
            "identity_resolution.candidate.case_ref",
            "candidate case ref is required",
        )
    })?;
    RecordId::try_new(case_ref.case_id)
        .map(|value| value.into_inner())
        .map_err(|error| {
            SdkError::invalid_argument(
                "identity_resolution.candidate.case_ref.case_id",
                error.to_string(),
            )
        })
}

fn reference_unavailable() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_REFERENCE_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "One or more referenced Identity Resolution resources are unavailable.",
    )
}

fn evidence_stale() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_EVIDENCE_STALE",
        ErrorCategory::Conflict,
        false,
        "The candidate evidence no longer matches the current Party versions.",
    )
}

fn unsupported_capability() -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_COMPOSITION_CAPABILITY_UNSUPPORTED",
        ErrorCategory::Internal,
        false,
        "The Identity Resolution mutation capability is not configured for this composition boundary.",
    )
}

fn config_error(error: crm_module_sdk::IdentifierError) -> SdkError {
    SdkError::new(
        "IDENTITY_RESOLUTION_REFERENCE_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The Identity Resolution reference validation boundary is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_identity_resolution_capability_adapter::PartyVersionExpectation;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct FakeReader {
        versions: BTreeMap<String, i64>,
        candidate: Option<CandidateEvidenceVersions>,
        seen_tenants: Mutex<Vec<String>>,
    }

    impl IdentityResolutionReferenceReader for FakeReader {
        fn party_version<'a>(
            &'a self,
            tenant_id: &'a TenantId,
            party_ref: &'a PartyReference,
        ) -> PortFuture<'a, Result<Option<i64>, SdkError>> {
            Box::pin(async move {
                self.seen_tenants
                    .lock()
                    .unwrap()
                    .push(tenant_id.as_str().to_owned());
                Ok(self.versions.get(party_ref.as_str()).copied())
            })
        }

        fn candidate_evidence_versions<'a>(
            &'a self,
            tenant_id: &'a TenantId,
            _case_id: &'a str,
        ) -> PortFuture<'a, Result<Option<CandidateEvidenceVersions>, SdkError>> {
            Box::pin(async move {
                self.seen_tenants
                    .lock()
                    .unwrap()
                    .push(tenant_id.as_str().to_owned());
                Ok(self.candidate.clone())
            })
        }
    }

    fn tenant() -> TenantId {
        TenantId::try_new("tenant-1").unwrap()
    }

    fn party(value: &str) -> PartyReference {
        PartyReference::try_new(value).unwrap()
    }

    fn reader() -> FakeReader {
        FakeReader {
            versions: BTreeMap::from([("party-a".to_owned(), 3), ("party-b".to_owned(), 7)]),
            candidate: Some(CandidateEvidenceVersions {
                case_id: "case-1".to_owned(),
                parties: vec![(party("party-a"), 3), (party("party-b"), 7)],
            }),
            seen_tenants: Mutex::new(Vec::new()),
        }
    }

    #[tokio::test]
    async fn exact_same_tenant_party_versions_are_required() {
        let reader = reader();
        let scope = EvidenceReferenceScope {
            parties: vec![
                PartyVersionExpectation {
                    party_ref: party("party-a"),
                    expected_version: 3,
                },
                PartyVersionExpectation {
                    party_ref: party("party-b"),
                    expected_version: 7,
                },
            ],
        };
        validate_evidence_scope(&reader, &tenant(), &scope)
            .await
            .unwrap();
        assert_eq!(reader.seen_tenants.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn missing_party_fails_closed_and_version_drift_is_a_conflict() {
        let reader = reader();
        let missing = EvidenceReferenceScope {
            parties: vec![
                PartyVersionExpectation {
                    party_ref: party("party-a"),
                    expected_version: 3,
                },
                PartyVersionExpectation {
                    party_ref: party("party-missing"),
                    expected_version: 1,
                },
            ],
        };
        assert_eq!(
            validate_evidence_scope(&reader, &tenant(), &missing)
                .await
                .unwrap_err()
                .code,
            "IDENTITY_RESOLUTION_REFERENCE_UNAVAILABLE"
        );

        let stale = EvidenceReferenceScope {
            parties: vec![
                PartyVersionExpectation {
                    party_ref: party("party-a"),
                    expected_version: 2,
                },
                PartyVersionExpectation {
                    party_ref: party("party-b"),
                    expected_version: 7,
                },
            ],
        };
        assert_eq!(
            validate_evidence_scope(&reader, &tenant(), &stale)
                .await
                .unwrap_err()
                .code,
            "IDENTITY_RESOLUTION_EVIDENCE_STALE"
        );
    }

    #[tokio::test]
    async fn terminal_decision_rechecks_current_candidate_evidence_versions() {
        let reader = reader();
        validate_current_candidate_evidence(&reader, &tenant(), "case-1")
            .await
            .unwrap();

        let stale_reader = FakeReader {
            versions: BTreeMap::from([("party-a".to_owned(), 4), ("party-b".to_owned(), 7)]),
            candidate: reader.candidate.clone(),
            seen_tenants: Mutex::new(Vec::new()),
        };
        assert_eq!(
            validate_current_candidate_evidence(&stale_reader, &tenant(), "case-1")
                .await
                .unwrap_err()
                .code,
            "IDENTITY_RESOLUTION_EVIDENCE_STALE"
        );
    }
}
