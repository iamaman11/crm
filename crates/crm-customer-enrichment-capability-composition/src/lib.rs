#![forbid(unsafe_code)]

//! Production pre-authorization and execution composition for Customer Enrichment requests.
//!
//! Request creation performs separate governed Party and Consent reads, strict immutable
//! provider/mapping validation and a versioned fail-closed policy decision before delegating to the
//! shared transactional aggregate executor. This crate owns no authoritative Party or Consent
//! state and never bypasses their query boundaries.

use crm_capability_plan_support as support;
use crm_capability_runtime::{
    CapabilityDefinition, CapabilityExecutionResult, CapabilityRequest,
    TransactionalCapabilityExecutor,
};
use crm_consents_capability_adapter::MODULE_ID as CONSENTS_MODULE_ID;
use crm_consents_query_adapter::{
    ConsentQueryAdapter, GET_CAPABILITY as CONSENT_GET_CAPABILITY,
    GET_REQUEST_SCHEMA as CONSENT_GET_REQUEST_SCHEMA,
    query_capability_definition as consent_query_definition,
};
use crm_core_data::{PostgresDataStore, RecordGetQuery};
use crm_customer_enrichment::{MappingVersion, ProviderProfileVersion, TargetField};
use crm_customer_enrichment_capability_adapter::{
    CREATE_ENRICHMENT_REQUEST_CAPABILITY, MAPPING_VERSION_RECORD_TYPE, MODULE_ID,
    PROVIDER_PROFILE_VERSION_RECORD_TYPE, enrichment_request_from_create_request,
    enrichment_request_to_wire, mapping_from_snapshot, provider_profile_from_snapshot,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, PortFuture, RecordId,
    RecordType, SchemaVersion, SdkError,
};
use crm_parties_query_adapter::{
    GET_CAPABILITY as PARTY_GET_CAPABILITY, PartyExportExecutionRead, PartyQueryAdapter,
    export_execution_query_request, query_capability_definition as party_query_definition,
};
use crm_proto_contracts::crm::{consents::v1 as consent_wire, customer_enrichment::v1 as wire};
use crm_query_runtime::{
    QueryAuthorizer, QueryExecutionContext, QueryExecutor, QueryRequest, QuerySemanticValidator,
    normalized_filter_hash,
};
use prost::Message;
use std::fmt;
use std::sync::Arc;

pub const REQUEST_POLICY_VERSION: &str = "request-policy-v1";
const CONSENT_LEGAL_BASIS_CODE: &str = "consent";
const LEGITIMATE_INTEREST_LEGAL_BASIS_CODE: &str = "legitimate_interest";
const DISPLAY_NAME_FIELD: &str = "display_name";

#[derive(Clone)]
pub struct CustomerEnrichmentCapabilityExecutor {
    store: PostgresDataStore,
    fallback: Arc<dyn TransactionalCapabilityExecutor>,
    request_create: Arc<dyn TransactionalCapabilityExecutor>,
    party_queries: Arc<PartyQueryAdapter>,
    consent_queries: Arc<ConsentQueryAdapter>,
    query_authorizer: Arc<dyn QueryAuthorizer>,
}

impl CustomerEnrichmentCapabilityExecutor {
    pub fn new(
        store: PostgresDataStore,
        fallback: Arc<dyn TransactionalCapabilityExecutor>,
        request_create: Arc<dyn TransactionalCapabilityExecutor>,
        party_queries: Arc<PartyQueryAdapter>,
        consent_queries: Arc<ConsentQueryAdapter>,
        query_authorizer: Arc<dyn QueryAuthorizer>,
    ) -> Self {
        Self {
            store,
            fallback,
            request_create,
            party_queries,
            consent_queries,
            query_authorizer,
        }
    }

    async fn execute_create(
        &self,
        definition: &CapabilityDefinition,
        request: CapabilityRequest,
    ) -> Result<CapabilityExecutionResult, SdkError> {
        let enrichment_request = enrichment_request_from_create_request(&request)?;
        let public_request = enrichment_request_to_wire(&enrichment_request)?;
        let target = public_request.target.as_ref().ok_or_else(contract_invalid)?;
        let party_ref = target.party_ref.as_ref().ok_or_else(contract_invalid)?;
        let policy = public_request
            .policy_evidence
            .as_ref()
            .ok_or_else(contract_invalid)?;
        let (mapping, profile) = self
            .load_and_validate_definitions(&request, &enrichment_request)
            .await?;
        self.validate_profile_policy(&profile, &mapping, &public_request)?;
        self.validate_party(
            &request,
            &party_ref.party_id,
            target.party_resource_version,
        )
        .await?;
        self.validate_consent(&request, policy, &party_ref.party_id)
            .await?;
        self.request_create.execute(definition, request).await
    }

    async fn load_and_validate_definitions(
        &self,
        request: &CapabilityRequest,
        enrichment_request: &crm_customer_enrichment::EnrichmentRequest,
    ) -> Result<(MappingVersion, ProviderProfileVersion), SdkError> {
        let mapping_snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.execution.tenant_id.clone(),
                owner_module_id: module_id(MODULE_ID)?,
                record_type: record_type(MAPPING_VERSION_RECORD_TYPE)?,
                record_id: RecordId::try_new(enrichment_request.mapping_version_id().as_str())
                    .map_err(configuration_error)?,
            })
            .await?
            .ok_or_else(definition_unavailable)?;
        let mapping = mapping_from_snapshot(&mapping_snapshot)?;
        if mapping.version_id() != enrichment_request.mapping_version_id()
            || mapping.target_field() != enrichment_request.target().target_field
            || mapping.provider_profile_version_id()
                != enrichment_request.provider_profile_version_id()
        {
            return Err(definition_invalid(
                "request, mapping and provider-profile identities are inconsistent",
            ));
        }

        let profile_snapshot = self
            .store
            .get_record_for_query(&RecordGetQuery {
                tenant_id: request.context.execution.tenant_id.clone(),
                owner_module_id: module_id(MODULE_ID)?,
                record_type: record_type(PROVIDER_PROFILE_VERSION_RECORD_TYPE)?,
                record_id: RecordId::try_new(
                    enrichment_request.provider_profile_version_id().as_str(),
                )
                .map_err(configuration_error)?,
            })
            .await?
            .ok_or_else(definition_unavailable)?;
        let profile = provider_profile_from_snapshot(&profile_snapshot)?;
        if profile.version_id() != enrichment_request.provider_profile_version_id()
            || !profile
                .supported_target_fields()
                .contains(&mapping.target_field())
        {
            return Err(definition_invalid(
                "provider-profile identity or supported target fields are inconsistent",
            ));
        }
        Ok((mapping, profile))
    }

    fn validate_profile_policy(
        &self,
        profile: &ProviderProfileVersion,
        mapping: &MappingVersion,
        request: &wire::EnrichmentRequest,
    ) -> Result<(), SdkError> {
        let policy = request.policy_evidence.as_ref().ok_or_else(contract_invalid)?;
        if policy.policy_version != REQUEST_POLICY_VERSION {
            return Err(policy_denied(
                "policy_version_mismatch",
                "The supplied enrichment policy version is not active.",
            ));
        }
        if !matches!(
            policy.legal_basis_code.as_str(),
            CONSENT_LEGAL_BASIS_CODE | LEGITIMATE_INTEREST_LEGAL_BASIS_CODE
        ) {
            return Err(policy_denied(
                "legal_basis_not_permitted",
                "The supplied legal basis is not permitted by the active enrichment policy.",
            ));
        }
        if !profile
            .purpose_codes()
            .iter()
            .any(|value| value == &policy.purpose_code)
        {
            return Err(policy_denied(
                "purpose_not_permitted",
                "The provider profile does not permit the requested purpose.",
            ));
        }
        if mapping.target_field() != TargetField::PartyDisplayName
            || request.requested_fields
                != vec![wire::EnrichmentTargetField::PartyDisplayName as i32]
        {
            return Err(definition_invalid(
                "request fields do not match the immutable mapping target",
            ));
        }
        let created_at = nonnegative_u64(request.created_at_unix_ms)?;
        if !profile.is_effective_at(created_at) {
            return Err(policy_denied(
                "provider_profile_not_effective",
                "The provider profile is not effective for new enrichment requests.",
            ));
        }
        Ok(())
    }

    async fn validate_party(
        &self,
        request: &CapabilityRequest,
        party_id: &str,
        expected_resource_version: i64,
    ) -> Result<(), SdkError> {
        if expected_resource_version <= 0 {
            return Err(stale_target());
        }
        let party_id = RecordId::try_new(party_id).map_err(|_| target_unavailable())?;
        let source_identity = format!(
            "enrichment-party-{}",
            request.context.execution.request_id.as_str()
        );
        let query_request = export_execution_query_request(
            &request.context.execution.tenant_id,
            &request.context.execution.actor_id,
            &source_identity,
            &party_id,
            request.context.execution.request_started_at_unix_nanos,
        )?;
        let definition = party_query_definition(PARTY_GET_CAPABILITY)?;
        authorize_query(
            self.query_authorizer.as_ref(),
            &definition,
            &query_request,
            "CUSTOMER_ENRICHMENT_PARTY_PERMISSION_DENIED",
        )
        .await?;
        self.party_queries.validate(&definition, &query_request).await?;
        match self
            .party_queries
            .get_for_export_execution(&query_request, &party_id, expected_resource_version)
            .await?
        {
            PartyExportExecutionRead::Visible { allowed_fields, .. }
                if allowed_fields.contains(DISPLAY_NAME_FIELD) =>
            {
                Ok(())
            }
            PartyExportExecutionRead::VersionChanged => Err(stale_target()),
            PartyExportExecutionRead::NotVisible
            | PartyExportExecutionRead::Unavailable
            | PartyExportExecutionRead::Visible { .. } => Err(target_unavailable()),
        }
    }

    async fn validate_consent(
        &self,
        request: &CapabilityRequest,
        policy: &wire::EnrichmentRequestPolicyEvidence,
        party_id: &str,
    ) -> Result<(), SdkError> {
        let consent_required = policy.legal_basis_code == CONSENT_LEGAL_BASIS_CODE;
        let Some(authorization_id) = policy.consent_evidence_reference.as_deref() else {
            return if consent_required {
                Err(consent_denied("consent_evidence_required"))
            } else {
                Ok(())
            };
        };
        let query_request = consent_get_query_request(request, authorization_id)?;
        let definition = consent_query_definition(CONSENT_GET_CAPABILITY)?;
        authorize_query(
            self.query_authorizer.as_ref(),
            &definition,
            &query_request,
            "CUSTOMER_ENRICHMENT_CONSENT_PERMISSION_DENIED",
        )
        .await?;
        self.consent_queries
            .validate(&definition, &query_request)
            .await?;
        let result = self
            .consent_queries
            .execute(&definition, query_request)
            .await?;
        let response = consent_wire::GetConsentAuthorizationResponse::decode(
            result.output.bytes.as_slice(),
        )
        .map_err(|error| contract_invalid().with_internal_reference(error.to_string()))?;
        let authorization = response
            .authorization
            .ok_or_else(|| consent_denied("consent_authorization_missing"))?;
        validate_consent_authorization(
            &authorization,
            authorization_id,
            party_id,
            policy,
            request.context.execution.request_started_at_unix_nanos,
        )
    }
}

impl fmt::Debug for CustomerEnrichmentCapabilityExecutor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CustomerEnrichmentCapabilityExecutor")
            .field("store", &self.store)
            .field("fallback", &"dyn TransactionalCapabilityExecutor")
            .field("request_create", &"dyn TransactionalCapabilityExecutor")
            .field("party_queries", &"PartyQueryAdapter")
            .field("consent_queries", &"ConsentQueryAdapter")
            .field("query_authorizer", &"dyn QueryAuthorizer")
            .finish()
    }
}

impl TransactionalCapabilityExecutor for CustomerEnrichmentCapabilityExecutor {
    fn execute<'a>(
        &'a self,
        definition: &'a CapabilityDefinition,
        request: CapabilityRequest,
    ) -> PortFuture<'a, Result<CapabilityExecutionResult, SdkError>> {
        if definition.capability_id.as_str() == CREATE_ENRICHMENT_REQUEST_CAPABILITY {
            Box::pin(async move { self.execute_create(definition, request).await })
        } else {
            self.fallback.execute(definition, request)
        }
    }
}

async fn authorize_query(
    authorizer: &dyn QueryAuthorizer,
    definition: &CapabilityDefinition,
    request: &QueryRequest,
    error_code: &'static str,
) -> Result<(), SdkError> {
    let decision = authorizer.authorize(definition, request).await?;
    if decision.allowed {
        return Ok(());
    }
    Err(SdkError::new(
        error_code,
        ErrorCategory::Authorization,
        false,
        "The actor is not authorized to inspect required enrichment policy evidence.",
    )
    .with_internal_reference(format!(
        "decision_id={} reason_code={} policy_version={}",
        decision.decision_id, decision.reason_code, decision.policy_version
    )))
}

fn consent_get_query_request(
    request: &CapabilityRequest,
    authorization_id: &str,
) -> Result<QueryRequest, SdkError> {
    let authorization_id = RecordId::try_new(authorization_id)
        .map_err(|_| consent_denied("consent_reference_invalid"))?;
    let command = consent_wire::GetConsentAuthorizationRequest {
        authorization_ref: Some(consent_wire::ConsentAuthorizationRef {
            authorization_id: authorization_id.as_str().to_owned(),
        }),
    };
    let input = support::protobuf_payload(
        CONSENTS_MODULE_ID,
        CONSENT_GET_REQUEST_SCHEMA,
        DataClass::Personal,
        &command,
    )?;
    Ok(QueryRequest {
        owner_module_id: module_id(CONSENTS_MODULE_ID)?,
        context: QueryExecutionContext {
            tenant_id: request.context.execution.tenant_id.clone(),
            actor_id: request.context.execution.actor_id.clone(),
            request_id: request.context.execution.request_id.clone(),
            correlation_id: request.context.execution.correlation_id.clone(),
            trace_id: request.context.execution.trace_id.clone(),
            capability_id: CapabilityId::try_new(CONSENT_GET_CAPABILITY)
                .map_err(configuration_error)?,
            capability_version: CapabilityVersion::try_new(support::CONTRACT_VERSION)
                .map_err(configuration_error)?,
            schema_version: SchemaVersion::try_new(support::CONTRACT_VERSION)
                .map_err(configuration_error)?,
            request_started_at_unix_nanos: request
                .context
                .execution
                .request_started_at_unix_nanos,
        },
        input,
        input_hash: normalized_filter_hash([(
            "authorization_id",
            authorization_id.as_str().as_bytes(),
        )]),
    })
}

fn validate_consent_authorization(
    authorization: &consent_wire::ConsentAuthorization,
    expected_authorization_id: &str,
    expected_party_id: &str,
    policy: &wire::EnrichmentRequestPolicyEvidence,
    evaluated_at_unix_nanos: i64,
) -> Result<(), SdkError> {
    let identity_matches = authorization
        .authorization_ref
        .as_ref()
        .is_some_and(|value| value.authorization_id == expected_authorization_id);
    let party_matches = authorization
        .party_ref
        .as_ref()
        .is_some_and(|value| value.party_id == expected_party_id);
    let effect = consent_wire::ConsentEffect::try_from(authorization.effect).ok();
    let status = consent_wire::ConsentAuthorizationStatus::try_from(authorization.status).ok();
    let effective_from = authorization
        .effective_from
        .as_ref()
        .map(|value| value.unix_nanos)
        .ok_or_else(|| consent_denied("consent_effective_time_missing"))?;
    let not_expired = authorization
        .expires_at
        .as_ref()
        .is_none_or(|value| evaluated_at_unix_nanos < value.unix_nanos);
    if !identity_matches
        || !party_matches
        || authorization.purpose != policy.purpose_code
        || authorization.legal_basis != policy.legal_basis_code
        || effect != Some(consent_wire::ConsentEffect::Grant)
        || status != Some(consent_wire::ConsentAuthorizationStatus::Active)
        || effective_from > evaluated_at_unix_nanos
        || !not_expired
        || authorization.evidence_ref.is_empty()
    {
        return Err(consent_denied("consent_evidence_not_applicable"));
    }
    Ok(())
}

fn module_id(value: &str) -> Result<ModuleId, SdkError> {
    ModuleId::try_new(value).map_err(configuration_error)
}

fn record_type(value: &str) -> Result<RecordType, SdkError> {
    RecordType::try_new(value).map_err(configuration_error)
}

fn nonnegative_u64(value: i64) -> Result<u64, SdkError> {
    u64::try_from(value).map_err(|_| {
        SdkError::invalid_argument(
            "customer_enrichment.request.created_at_unix_ms",
            "timestamp must not be negative",
        )
    })
}

fn definition_unavailable() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_DEFINITION_UNAVAILABLE",
        ErrorCategory::InvalidArgument,
        false,
        "The referenced enrichment definition is unavailable.",
    )
}

fn definition_invalid(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_DEFINITION_INVALID",
        ErrorCategory::InvalidArgument,
        false,
        "The referenced enrichment definition is invalid.",
    )
    .with_internal_reference(reference.into())
}

fn policy_denied(reason: &'static str, safe_message: &'static str) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_POLICY_DENIED",
        ErrorCategory::Authorization,
        false,
        safe_message,
    )
    .with_internal_reference(reason)
}

fn consent_denied(reason: &'static str) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_CONSENT_DENIED",
        ErrorCategory::Authorization,
        false,
        "The required Consent evidence is unavailable or does not permit this enrichment request.",
    )
    .with_internal_reference(reason)
}

fn target_unavailable() -> SdkError {
    SdkError::new(
        "QUERY_RESOURCE_NOT_FOUND",
        ErrorCategory::NotFound,
        false,
        "The requested resource was not found.",
    )
}

fn stale_target() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_TARGET_STALE",
        ErrorCategory::Conflict,
        false,
        "The Party resource version changed before the enrichment request was authorized.",
    )
}

fn contract_invalid() -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_CONTRACT_INVALID",
        ErrorCategory::Internal,
        false,
        "The enrichment request contract could not be interpreted safely.",
    )
}

fn configuration_error(error: impl fmt::Display) -> SdkError {
    SdkError::new(
        "CUSTOMER_ENRICHMENT_REQUEST_COMPOSITION_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        "The enrichment request production composition is not configured safely.",
    )
    .with_internal_reference(error.to_string())
}

pub const CRATE_NAME: &str = "crm-customer-enrichment-capability-composition";
