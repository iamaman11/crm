use crm_application_composition::ModuleActivationPort;
use crm_capability_runtime::CapabilityDefinition;
use crm_customer_privacy::{
    ContributionCompletenessProof, DiscoveryOwnerScopeContribution, DiscoveryScopeSnapshot,
    EvidenceClass, OwnerScopeContract, OwnerScopeContribution, OwnerScopeRegistry,
    ScopeDiscoveryLineage, ScopeResource, discovery_lineage_digest, discovery_sha256,
};
use crm_module_sdk::{
    ActorId, CapabilityId, CapabilityVersion, CorrelationId, DataClass, ErrorCategory, ModuleId,
    PortFuture, RecordId, RequestId, RetentionPolicyId, SdkError, TenantId, TraceId, TypedPayload,
};
use crm_proto_contracts::crm::{customer::v1::PartyRef, customer_privacy::v1 as privacy};
use crm_query_runtime::{QueryExecutionContext, QueryExecutor, QueryRequest};
use prost::Message;
use std::collections::BTreeMap;
use std::sync::Arc;

pub const DISCOVERY_PHASE: u16 = 260;
pub const DEFAULT_DISCOVERY_PAGE_SIZE: u32 = 64;
pub const MAXIMUM_DISCOVERY_PAGE_SIZE: u32 = 128;
pub const MAXIMUM_DISCOVERY_CURSOR_BYTES: usize = 2_048;
pub const EXPECTED_DISCOVERY_OWNER_COUNT: usize = 9;
const INTERNAL_REQUEST_RETENTION: &str = "crm.customer_privacy.discovery.owner_request";
const SNAPSHOT_READ_FIELD: &str = "discovery_scope_snapshot";

#[derive(Debug, Clone)]
pub struct DiscoveryInvocation {
    pub lineage: ScopeDiscoveryLineage,
    pub actor_id: ActorId,
    pub request_id: RequestId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub request_started_at_unix_nanos: i64,
    pub proposed_captured_at_unix_nanos: i64,
    pub trusted_internal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryAttempt {
    pub attempt_digest: [u8; 32],
    pub captured_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryPageReceipt {
    pub owner_module_id: ModuleId,
    pub capability_id: CapabilityId,
    pub capability_version: CapabilityVersion,
    pub lineage_digest: [u8; 32],
    pub page_number: u32,
    pub request_cursor_digest: [u8; 32],
    pub response_cursor_digest: [u8; 32],
    pub owner_cursor_digest: [u8; 32],
    pub page_digest: [u8; 32],
    pub scanned_resource_count: u64,
    pub emitted_resource_count: u64,
    pub terminal_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedDiscoveryPage {
    pub receipt: DiscoveryPageReceipt,
    /// Exact accepted owner response. It contains reference-only projection and no owner payload.
    pub response_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryAuditEvent {
    DiscoveryStarted,
    OwnerPageAccepted,
    OwnerTerminalComplete,
    DiscoveryFailed,
    SnapshotFinalized,
    SnapshotReadAllowed,
    SnapshotReadDenied,
}

impl DiscoveryAuditEvent {
    pub const fn label(self) -> &'static str {
        match self {
            Self::DiscoveryStarted => "discovery_started",
            Self::OwnerPageAccepted => "owner_page_accepted",
            Self::OwnerTerminalComplete => "owner_terminal_complete",
            Self::DiscoveryFailed => "discovery_failed",
            Self::SnapshotFinalized => "snapshot_finalized",
            Self::SnapshotReadAllowed => "snapshot_read_allowed",
            Self::SnapshotReadDenied => "snapshot_read_denied",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryAuditRecord {
    pub event: DiscoveryAuditEvent,
    pub tenant_id: TenantId,
    pub privacy_case_id: RecordId,
    pub attempt_digest: [u8; 32],
    pub owner_module_id: Option<ModuleId>,
    pub page_number: Option<u32>,
    pub snapshot_id: Option<RecordId>,
    pub count: Option<u64>,
    pub policy_reference: Option<String>,
    pub occurred_at_unix_nanos: i64,
}

pub trait DiscoveryPersistencePort: Send + Sync {
    fn begin_attempt<'a>(
        &'a self,
        invocation: &'a DiscoveryInvocation,
        expected_attempt_digest: [u8; 32],
    ) -> PortFuture<'a, Result<DiscoveryAttempt, SdkError>>;

    fn load_owner_pages<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        attempt_digest: [u8; 32],
        owner_module_id: &'a ModuleId,
    ) -> PortFuture<'a, Result<Vec<PersistedDiscoveryPage>, SdkError>>;

    fn accept_owner_page<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        attempt_digest: [u8; 32],
        page: &'a PersistedDiscoveryPage,
    ) -> PortFuture<'a, Result<(), SdkError>>;

    fn advance_checkpoint<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        attempt_digest: [u8; 32],
        owner_module_id: &'a ModuleId,
        contiguous_page_number: u32,
        terminal_complete: bool,
    ) -> PortFuture<'a, Result<(), SdkError>>;

    fn finalize_snapshot<'a>(
        &'a self,
        attempt: &'a DiscoveryAttempt,
        snapshot: &'a DiscoveryScopeSnapshot,
    ) -> PortFuture<'a, Result<DiscoveryScopeSnapshot, SdkError>>;

    fn load_snapshot<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        snapshot_id: &'a RecordId,
    ) -> PortFuture<'a, Result<Option<DiscoveryScopeSnapshot>, SdkError>>;

    fn record_audit<'a>(
        &'a self,
        record: &'a DiscoveryAuditRecord,
    ) -> PortFuture<'a, Result<(), SdkError>>;
}

#[derive(Clone)]
pub struct OwnerContributionEndpoint {
    pub definition: CapabilityDefinition,
    pub executor: Arc<dyn QueryExecutor>,
}

impl std::fmt::Debug for OwnerContributionEndpoint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OwnerContributionEndpoint")
            .field("definition", &self.definition)
            .field("executor", &"dyn QueryExecutor")
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct OwnerContributionEndpoints {
    by_owner: BTreeMap<ModuleId, OwnerContributionEndpoint>,
}

impl OwnerContributionEndpoints {
    pub fn exact_canonical(
        endpoints: impl IntoIterator<Item = OwnerContributionEndpoint>,
    ) -> Result<Self, SdkError> {
        let registry = OwnerScopeRegistry::canonical_v1().map_err(scope_error)?;
        let mut by_owner = BTreeMap::new();
        for endpoint in endpoints {
            let owner = endpoint.definition.owner_module_id.clone();
            if by_owner.insert(owner.clone(), endpoint).is_some() {
                return Err(configuration_error(format!(
                    "duplicate discovery endpoint for {owner}"
                )));
            }
        }
        if by_owner.len() != EXPECTED_DISCOVERY_OWNER_COUNT
            || registry.contracts().len() != EXPECTED_DISCOVERY_OWNER_COUNT
        {
            return Err(configuration_error(
                "discovery endpoint inventory must contain exactly nine owners",
            ));
        }
        for contract in registry.contracts() {
            let endpoint = by_owner.get(contract.owner_module_id()).ok_or_else(|| {
                configuration_error(format!(
                    "missing discovery endpoint for {}",
                    contract.owner_module_id()
                ))
            })?;
            validate_endpoint(contract, &endpoint.definition)?;
        }
        Ok(Self { by_owner })
    }

    fn get(&self, owner: &ModuleId) -> Result<&OwnerContributionEndpoint, SdkError> {
        self.by_owner.get(owner).ok_or_else(|| {
            configuration_error(format!("discovery endpoint unavailable for {owner}"))
        })
    }

    pub fn len(&self) -> usize {
        self.by_owner.len()
    }
}

#[derive(Clone)]
pub struct ScopeDiscoveryService {
    activation: Arc<dyn ModuleActivationPort>,
    persistence: Arc<dyn DiscoveryPersistencePort>,
    endpoints: OwnerContributionEndpoints,
}

impl std::fmt::Debug for ScopeDiscoveryService {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ScopeDiscoveryService")
            .field("activation", &"dyn ModuleActivationPort")
            .field("persistence", &"dyn DiscoveryPersistencePort")
            .field("endpoint_count", &self.endpoints.len())
            .finish()
    }
}

impl ScopeDiscoveryService {
    pub fn new(
        activation: Arc<dyn ModuleActivationPort>,
        persistence: Arc<dyn DiscoveryPersistencePort>,
        endpoints: OwnerContributionEndpoints,
    ) -> Self {
        Self {
            activation,
            persistence,
            endpoints,
        }
    }

    pub async fn discover(
        &self,
        invocation: DiscoveryInvocation,
    ) -> Result<DiscoveryScopeSnapshot, SdkError> {
        if !invocation.trusted_internal {
            return Err(discovery_error(
                "CUSTOMER_PRIVACY_DISCOVERY_TRUST_REQUIRED",
                ErrorCategory::Authorization,
                false,
                "discovery invocation is not trusted internal",
            ));
        }
        if invocation.request_started_at_unix_nanos <= 0
            || invocation.proposed_captured_at_unix_nanos < invocation.request_started_at_unix_nanos
        {
            return Err(discovery_error(
                "CUSTOMER_PRIVACY_DISCOVERY_TIME_INVALID",
                ErrorCategory::InvalidArgument,
                false,
                "discovery timestamps are invalid",
            ));
        }

        let customer_privacy_module = module_id("crm.customer-privacy")?;
        ensure_active(
            self.activation.as_ref(),
            invocation.lineage.tenant_id(),
            &customer_privacy_module,
        )
        .await?;

        let registry = OwnerScopeRegistry::canonical_v1().map_err(scope_error)?;
        if invocation.lineage.registry_version() != registry.registry_version()
            || invocation.lineage.registry_digest() != registry.digest()
        {
            return Err(discovery_error(
                "CUSTOMER_PRIVACY_DISCOVERY_REGISTRY_DRIFT",
                ErrorCategory::Conflict,
                false,
                "discovery lineage does not match the active registry",
            ));
        }

        let expected_attempt_digest = discovery_attempt_digest(&invocation.lineage);
        let attempt = self
            .persistence
            .begin_attempt(&invocation, expected_attempt_digest)
            .await?;
        if attempt.attempt_digest != expected_attempt_digest
            || attempt.captured_at_unix_nanos < invocation.request_started_at_unix_nanos
        {
            return Err(corrupt_evidence(
                "persisted discovery attempt identity is invalid",
            ));
        }
        self.audit(
            &invocation,
            &attempt,
            DiscoveryAuditEvent::DiscoveryStarted,
            None,
            None,
            None,
            None,
            None,
        )
        .await?;

        let result = self
            .discover_all_owners(&invocation, &attempt, registry.clone())
            .await;
        let contributions = match result {
            Ok(contributions) => contributions,
            Err(error) => {
                let _ = self
                    .audit(
                        &invocation,
                        &attempt,
                        DiscoveryAuditEvent::DiscoveryFailed,
                        None,
                        None,
                        None,
                        None,
                        Some(error.code.clone()),
                    )
                    .await;
                return Err(error);
            }
        };

        let snapshot = DiscoveryScopeSnapshot::finalize(
            invocation.lineage.clone(),
            registry,
            attempt.captured_at_unix_nanos,
            contributions,
        )
        .map_err(scope_error)?;
        let snapshot = self
            .persistence
            .finalize_snapshot(&attempt, &snapshot)
            .await?;
        self.audit(
            &invocation,
            &attempt,
            DiscoveryAuditEvent::SnapshotFinalized,
            None,
            None,
            Some(snapshot.snapshot_id().clone()),
            Some(snapshot.aggregation().resources().len() as u64),
            None,
        )
        .await?;
        Ok(snapshot)
    }

    async fn discover_all_owners(
        &self,
        invocation: &DiscoveryInvocation,
        attempt: &DiscoveryAttempt,
        registry: OwnerScopeRegistry,
    ) -> Result<Vec<DiscoveryOwnerScopeContribution>, SdkError> {
        let mut contributions = Vec::with_capacity(EXPECTED_DISCOVERY_OWNER_COUNT);
        for contract in registry.contracts() {
            ensure_active(
                self.activation.as_ref(),
                invocation.lineage.tenant_id(),
                contract.owner_module_id(),
            )
            .await?;
            let endpoint = self.endpoints.get(contract.owner_module_id())?;
            validate_endpoint(contract, &endpoint.definition)?;
            let pages = self
                .discover_owner(invocation, attempt, contract, endpoint)
                .await?;
            contributions.push(build_owner_contribution(
                invocation.lineage.clone(),
                contract.clone(),
                pages,
            )?);
        }
        Ok(contributions)
    }

    async fn discover_owner(
        &self,
        invocation: &DiscoveryInvocation,
        attempt: &DiscoveryAttempt,
        contract: &OwnerScopeContract,
        endpoint: &OwnerContributionEndpoint,
    ) -> Result<Vec<PersistedDiscoveryPage>, SdkError> {
        let mut persisted = self
            .persistence
            .load_owner_pages(
                invocation.lineage.tenant_id(),
                attempt.attempt_digest,
                contract.owner_module_id(),
            )
            .await?;
        persisted.sort_by_key(|page| page.receipt.page_number);

        let mut cursor = String::new();
        let mut expected_page = 1_u32;
        let mut accepted = Vec::new();
        for page in persisted {
            let validated = validate_response_page(
                &invocation.lineage,
                contract,
                expected_page,
                &cursor,
                &page.response_bytes,
            )?;
            if validated.receipt != page.receipt {
                return Err(replay_conflict(
                    "persisted owner page receipt does not match response",
                ));
            }
            cursor = validated.next_cursor;
            let terminal = validated.receipt.terminal_complete;
            accepted.push(page);
            self.persistence
                .advance_checkpoint(
                    invocation.lineage.tenant_id(),
                    attempt.attempt_digest,
                    contract.owner_module_id(),
                    expected_page,
                    terminal,
                )
                .await?;
            if terminal {
                if cursor.is_empty() {
                    return Ok(accepted);
                }
                return Err(corrupt_evidence(
                    "terminal persisted page retained a cursor",
                ));
            }
            expected_page = expected_page
                .checked_add(1)
                .ok_or_else(|| corrupt_evidence("owner page sequence overflowed"))?;
        }

        loop {
            let request = owner_query_request(
                invocation,
                contract,
                &endpoint.definition,
                DEFAULT_DISCOVERY_PAGE_SIZE,
                cursor.clone(),
            )?;
            let result = endpoint
                .executor
                .execute(&endpoint.definition, request)
                .await
                .map_err(map_owner_error)?;
            let output_contract = endpoint
                .definition
                .output_contract
                .as_ref()
                .ok_or_else(|| incompatible_owner("owner output contract is missing"))?;
            result.output.validate()?;
            if !output_contract.matches(&result.output) {
                return Err(incompatible_owner(
                    "owner output descriptor is incompatible",
                ));
            }
            let validated = validate_response_page(
                &invocation.lineage,
                contract,
                expected_page,
                &cursor,
                &result.output.bytes,
            )?;
            let page = PersistedDiscoveryPage {
                receipt: validated.receipt.clone(),
                response_bytes: result.output.bytes,
            };
            self.persistence
                .accept_owner_page(
                    invocation.lineage.tenant_id(),
                    attempt.attempt_digest,
                    &page,
                )
                .await?;
            self.persistence
                .advance_checkpoint(
                    invocation.lineage.tenant_id(),
                    attempt.attempt_digest,
                    contract.owner_module_id(),
                    expected_page,
                    page.receipt.terminal_complete,
                )
                .await?;
            self.audit(
                invocation,
                attempt,
                DiscoveryAuditEvent::OwnerPageAccepted,
                Some(contract.owner_module_id().clone()),
                Some(expected_page),
                None,
                Some(page.receipt.emitted_resource_count),
                None,
            )
            .await?;
            let terminal = page.receipt.terminal_complete;
            cursor = validated.next_cursor;
            accepted.push(page);
            if terminal {
                self.audit(
                    invocation,
                    attempt,
                    DiscoveryAuditEvent::OwnerTerminalComplete,
                    Some(contract.owner_module_id().clone()),
                    Some(expected_page),
                    None,
                    Some(accepted.len() as u64),
                    None,
                )
                .await?;
                return Ok(accepted);
            }
            if cursor.is_empty() {
                return Err(incompatible_owner(
                    "nonterminal owner page returned an empty next cursor",
                ));
            }
            expected_page = expected_page
                .checked_add(1)
                .ok_or_else(|| corrupt_evidence("owner page sequence overflowed"))?;
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn audit(
        &self,
        invocation: &DiscoveryInvocation,
        attempt: &DiscoveryAttempt,
        event: DiscoveryAuditEvent,
        owner_module_id: Option<ModuleId>,
        page_number: Option<u32>,
        snapshot_id: Option<RecordId>,
        count: Option<u64>,
        policy_reference: Option<String>,
    ) -> Result<(), SdkError> {
        self.persistence
            .record_audit(&DiscoveryAuditRecord {
                event,
                tenant_id: invocation.lineage.tenant_id().clone(),
                privacy_case_id: invocation.lineage.privacy_case_id().clone(),
                attempt_digest: attempt.attempt_digest,
                owner_module_id,
                page_number,
                snapshot_id,
                count,
                policy_reference,
                occurred_at_unix_nanos: invocation.request_started_at_unix_nanos,
            })
            .await
    }
}

#[derive(Debug, Clone)]
pub struct SnapshotReadContext {
    pub tenant_id: TenantId,
    pub actor_id: ActorId,
    pub request_id: RequestId,
    pub correlation_id: CorrelationId,
    pub trace_id: TraceId,
    pub request_started_at_unix_nanos: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotVisibilityDecision {
    pub allowed: bool,
    pub decision_id: String,
    pub policy_version: String,
}

pub trait DiscoverySnapshotVisibilityPort: Send + Sync {
    fn authorize<'a>(
        &'a self,
        context: &'a SnapshotReadContext,
        snapshot_id: &'a RecordId,
        required_field: &'a str,
    ) -> PortFuture<'a, Result<SnapshotVisibilityDecision, SdkError>>;
}

#[derive(Clone)]
pub struct DiscoverySnapshotReader {
    persistence: Arc<dyn DiscoveryPersistencePort>,
    visibility: Arc<dyn DiscoverySnapshotVisibilityPort>,
}

impl DiscoverySnapshotReader {
    pub fn new(
        persistence: Arc<dyn DiscoveryPersistencePort>,
        visibility: Arc<dyn DiscoverySnapshotVisibilityPort>,
    ) -> Self {
        Self {
            persistence,
            visibility,
        }
    }

    pub async fn read(
        &self,
        context: SnapshotReadContext,
        snapshot_id: RecordId,
    ) -> Result<DiscoveryScopeSnapshot, SdkError> {
        let decision = self
            .visibility
            .authorize(&context, &snapshot_id, SNAPSHOT_READ_FIELD)
            .await?;
        let synthetic_attempt = discovery_sha256(snapshot_id.as_str().as_bytes());
        let event = if decision.allowed {
            DiscoveryAuditEvent::SnapshotReadAllowed
        } else {
            DiscoveryAuditEvent::SnapshotReadDenied
        };
        self.persistence
            .record_audit(&DiscoveryAuditRecord {
                event,
                tenant_id: context.tenant_id.clone(),
                privacy_case_id: snapshot_id.clone(),
                attempt_digest: synthetic_attempt,
                owner_module_id: None,
                page_number: None,
                snapshot_id: Some(snapshot_id.clone()),
                count: None,
                policy_reference: Some(format!(
                    "{}:{}",
                    decision.policy_version, decision.decision_id
                )),
                occurred_at_unix_nanos: context.request_started_at_unix_nanos,
            })
            .await?;
        if !decision.allowed {
            return Err(discovery_error(
                "CUSTOMER_PRIVACY_DISCOVERY_SNAPSHOT_READ_DENIED",
                ErrorCategory::Authorization,
                false,
                "live snapshot visibility denied the read",
            ));
        }
        let snapshot = self
            .persistence
            .load_snapshot(&context.tenant_id, &snapshot_id)
            .await?
            .ok_or_else(|| {
                discovery_error(
                    "CUSTOMER_PRIVACY_DISCOVERY_SNAPSHOT_NOT_FOUND",
                    ErrorCategory::NotFound,
                    false,
                    "discovery snapshot was not found",
                )
            })?;
        if snapshot.lineage().tenant_id() != &context.tenant_id
            || snapshot.snapshot_id() != &snapshot_id
        {
            return Err(corrupt_evidence(
                "snapshot read returned mismatched identity",
            ));
        }
        Ok(snapshot)
    }
}

#[derive(Debug)]
struct ValidatedPage {
    receipt: DiscoveryPageReceipt,
    next_cursor: String,
    resources: Vec<ScopeResource>,
}

fn validate_endpoint(
    contract: &OwnerScopeContract,
    definition: &CapabilityDefinition,
) -> Result<(), SdkError> {
    if definition.owner_module_id != *contract.owner_module_id()
        || definition.capability_id != *contract.capability_id()
        || definition.capability_version != *contract.capability_version()
        || definition.mutation
        || definition.requires_idempotency
        || definition.requires_approval
        || definition.output_contract.is_none()
        || definition
            .input_contract
            .descriptor_hash
            .iter()
            .all(|byte| *byte == 0)
        || definition
            .output_contract
            .as_ref()
            .is_some_and(|contract| contract.descriptor_hash.iter().all(|byte| *byte == 0))
    {
        return Err(incompatible_owner(
            "owner capability definition does not match the frozen registry",
        ));
    }
    Ok(())
}

fn owner_query_request(
    invocation: &DiscoveryInvocation,
    contract: &OwnerScopeContract,
    definition: &CapabilityDefinition,
    page_size: u32,
    cursor: String,
) -> Result<QueryRequest, SdkError> {
    if page_size == 0 || page_size > MAXIMUM_DISCOVERY_PAGE_SIZE {
        return Err(discovery_error(
            "CUSTOMER_PRIVACY_DISCOVERY_PAGE_SIZE_INVALID",
            ErrorCategory::InvalidArgument,
            false,
            "discovery page size is outside frozen bounds",
        ));
    }
    if cursor.len() > MAXIMUM_DISCOVERY_CURSOR_BYTES {
        return Err(discovery_error(
            "CUSTOMER_PRIVACY_DISCOVERY_CURSOR_TOO_LARGE",
            ErrorCategory::InvalidArgument,
            false,
            "owner cursor exceeds the frozen maximum",
        ));
    }
    let envelope = privacy::PrivacyScopeContributionRequestEnvelope {
        lineage: Some(proto_lineage(&invocation.lineage)),
        page_size,
        cursor,
    };
    let bytes = encode_owner_request(contract.capability_id().as_str(), envelope)?;
    let data_class = *definition
        .input_contract
        .allowed_data_classes
        .first()
        .ok_or_else(|| incompatible_owner("owner input data class is missing"))?;
    let encoding = *definition
        .input_contract
        .allowed_encodings
        .first()
        .ok_or_else(|| incompatible_owner("owner input encoding is missing"))?;
    let input = TypedPayload {
        owner: definition.input_contract.owner.clone(),
        schema_id: definition.input_contract.schema_id.clone(),
        schema_version: definition.input_contract.schema_version.clone(),
        descriptor_hash: definition.input_contract.descriptor_hash,
        data_class,
        encoding,
        maximum_size_bytes: definition.input_contract.maximum_size_bytes,
        retention_policy_id: retention(INTERNAL_REQUEST_RETENTION)?,
        bytes,
    };
    input.validate()?;
    let input_hash = discovery_sha256(&input.bytes);
    Ok(QueryRequest {
        owner_module_id: contract.owner_module_id().clone(),
        context: QueryExecutionContext {
            tenant_id: invocation.lineage.tenant_id().clone(),
            actor_id: invocation.actor_id.clone(),
            request_id: invocation.request_id.clone(),
            correlation_id: invocation.correlation_id.clone(),
            trace_id: invocation.trace_id.clone(),
            capability_id: contract.capability_id().clone(),
            capability_version: contract.capability_version().clone(),
            schema_version: definition.input_contract.schema_version.clone(),
            request_started_at_unix_nanos: invocation.request_started_at_unix_nanos,
        },
        input,
        input_hash,
    })
}

fn validate_response_page(
    lineage: &ScopeDiscoveryLineage,
    contract: &OwnerScopeContract,
    expected_page_number: u32,
    request_cursor: &str,
    bytes: &[u8],
) -> Result<ValidatedPage, SdkError> {
    let envelope = decode_owner_response(contract.capability_id().as_str(), bytes)?;
    if envelope.owner_module_id != contract.owner_module_id().as_str()
        || envelope.capability_id != contract.capability_id().as_str()
        || envelope.capability_version != contract.capability_version().as_str()
        || envelope.lineage.as_ref() != Some(&proto_lineage(lineage))
    {
        return Err(incompatible_owner(
            "owner response descriptor or immutable lineage mismatched",
        ));
    }
    let evidence = envelope
        .page_evidence
        .ok_or_else(|| incompatible_owner("owner page evidence is missing"))?;
    if evidence.page_number != expected_page_number || expected_page_number == 0 {
        return Err(replay_conflict("owner page sequence is not contiguous"));
    }
    if evidence.next_cursor.len() > MAXIMUM_DISCOVERY_CURSOR_BYTES {
        return Err(incompatible_owner(
            "owner response cursor exceeds frozen maximum",
        ));
    }
    if evidence.terminal_complete != evidence.next_cursor.is_empty() {
        return Err(incompatible_owner(
            "terminal completeness and next cursor are inconsistent",
        ));
    }
    if !evidence.terminal_complete && envelope.resources.is_empty() {
        return Err(incompatible_owner(
            "nonterminal owner page emitted no resources",
        ));
    }
    if evidence.emitted_resource_count != envelope.resources.len() as u64 {
        return Err(incompatible_owner(
            "owner emitted count does not match resources",
        ));
    }
    let owner_cursor_digest = exact_digest(&evidence.cursor_digest_sha256, "owner cursor digest")?;
    let page_digest = exact_digest(&evidence.page_digest_sha256, "owner page digest")?;
    let resources = envelope
        .resources
        .into_iter()
        .map(scope_resource)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ValidatedPage {
        receipt: DiscoveryPageReceipt {
            owner_module_id: contract.owner_module_id().clone(),
            capability_id: contract.capability_id().clone(),
            capability_version: contract.capability_version().clone(),
            lineage_digest: discovery_lineage_digest(lineage),
            page_number: evidence.page_number,
            request_cursor_digest: discovery_sha256(request_cursor.as_bytes()),
            response_cursor_digest: discovery_sha256(evidence.next_cursor.as_bytes()),
            owner_cursor_digest,
            page_digest,
            scanned_resource_count: evidence.scanned_resource_count,
            emitted_resource_count: evidence.emitted_resource_count,
            terminal_complete: evidence.terminal_complete,
        },
        next_cursor: evidence.next_cursor,
        resources,
    })
}

fn build_owner_contribution(
    lineage: ScopeDiscoveryLineage,
    contract: OwnerScopeContract,
    pages: Vec<PersistedDiscoveryPage>,
) -> Result<DiscoveryOwnerScopeContribution, SdkError> {
    if pages.is_empty()
        || !pages
            .last()
            .is_some_and(|page| page.receipt.terminal_complete)
    {
        return Err(discovery_error(
            "CUSTOMER_PRIVACY_DISCOVERY_OWNER_INCOMPLETE",
            ErrorCategory::Conflict,
            true,
            "owner did not prove terminal completeness",
        ));
    }
    let mut unique = BTreeMap::<(String, String), ScopeResource>::new();
    let mut scanned = 0_u64;
    let mut terminal_cursor_digest = None;
    for (index, page) in pages.iter().enumerate() {
        if page.receipt.page_number != index as u32 + 1
            || page.receipt.owner_module_id != *contract.owner_module_id()
        {
            return Err(replay_conflict(
                "owner durable page prefix is not contiguous",
            ));
        }
        scanned = scanned
            .checked_add(page.receipt.scanned_resource_count)
            .ok_or_else(|| corrupt_evidence("owner scanned count overflowed"))?;
        let request_cursor = if index == 0 {
            String::new()
        } else {
            decode_owner_response(
                contract.capability_id().as_str(),
                &pages[index - 1].response_bytes,
            )?
            .page_evidence
            .map(|evidence| evidence.next_cursor)
            .ok_or_else(|| corrupt_evidence("previous owner page evidence is missing"))?
        };
        let validated = validate_response_page(
            &lineage,
            &contract,
            page.receipt.page_number,
            &request_cursor,
            &page.response_bytes,
        )?;
        if validated.receipt != page.receipt {
            return Err(replay_conflict(
                "owner page changed after durable acceptance",
            ));
        }
        for resource in validated.resources {
            let key = (
                resource.resource_type().to_owned(),
                resource.resource_id().as_str().to_owned(),
            );
            if let Some(existing) = unique.get(&key) {
                if existing != &resource {
                    return Err(discovery_error(
                        "CUSTOMER_PRIVACY_DISCOVERY_RESOURCE_CONFLICT",
                        ErrorCategory::Conflict,
                        false,
                        "one resource identity has conflicting classification",
                    ));
                }
            } else {
                unique.insert(key, resource);
            }
        }
        if page.receipt.terminal_complete {
            terminal_cursor_digest = Some(page.receipt.owner_cursor_digest);
        }
    }
    let emitted = unique.len() as u64;
    let completeness = ContributionCompletenessProof::new(
        true,
        u32::try_from(pages.len()).map_err(|_| corrupt_evidence("owner page count overflowed"))?,
        scanned,
        emitted,
        terminal_cursor_digest.ok_or_else(|| corrupt_evidence("terminal cursor digest missing"))?,
    )
    .map_err(scope_error)?;
    let contribution = OwnerScopeContribution::new(
        contract,
        lineage.tenant_id().clone(),
        lineage.canonical_party_id().clone(),
        lineage.identity_resolution_generation(),
        unique.into_values(),
        completeness,
    )
    .map_err(scope_error)?;
    DiscoveryOwnerScopeContribution::new(lineage, contribution).map_err(scope_error)
}

fn proto_lineage(lineage: &ScopeDiscoveryLineage) -> privacy::PrivacyScopeContributionLineage {
    privacy::PrivacyScopeContributionLineage {
        privacy_case_id: lineage.privacy_case_id().as_str().to_owned(),
        tenant_id: lineage.tenant_id().as_str().to_owned(),
        canonical_party_ref: Some(PartyRef {
            party_id: lineage.canonical_party_id().as_str().to_owned(),
        }),
        identity_resolution_generation: lineage.identity_resolution_generation(),
        registry_version: lineage.registry_version().as_str().to_owned(),
        registry_digest_sha256: lineage.registry_digest().to_vec(),
        purpose_code: lineage.purpose_code().to_owned(),
        effective_request_at_unix_ms: lineage.effective_request_at_unix_ms(),
    }
}

fn encode_owner_request(
    capability_id: &str,
    envelope: privacy::PrivacyScopeContributionRequestEnvelope,
) -> Result<Vec<u8>, SdkError> {
    let bytes = match capability_id {
        "consents.privacy.scope.contribute" => privacy::ConsentsPrivacyScopeContributionRequest {
            contribution: Some(envelope),
        }
        .encode_to_vec(),
        "contact_points.privacy.scope.contribute" => {
            privacy::ContactPointsPrivacyScopeContributionRequest {
                contribution: Some(envelope),
            }
            .encode_to_vec()
        }
        "customer_accounts.privacy.scope.contribute" => {
            privacy::CustomerAccountsPrivacyScopeContributionRequest {
                contribution: Some(envelope),
            }
            .encode_to_vec()
        }
        "customer_data.privacy.scope.contribute" => {
            privacy::CustomerDataPrivacyScopeContributionRequest {
                contribution: Some(envelope),
            }
            .encode_to_vec()
        }
        "customer_enrichment.privacy.scope.contribute" => {
            privacy::CustomerEnrichmentPrivacyScopeContributionRequest {
                contribution: Some(envelope),
            }
            .encode_to_vec()
        }
        "data_quality.privacy.scope.contribute" => {
            privacy::DataQualityPrivacyScopeContributionRequest {
                contribution: Some(envelope),
            }
            .encode_to_vec()
        }
        "identity_resolution.privacy.scope.contribute" => {
            privacy::IdentityResolutionPrivacyScopeContributionRequest {
                contribution: Some(envelope),
            }
            .encode_to_vec()
        }
        "parties.privacy.scope.contribute" => privacy::PartiesPrivacyScopeContributionRequest {
            contribution: Some(envelope),
        }
        .encode_to_vec(),
        "party_relationships.privacy.scope.contribute" => {
            privacy::PartyRelationshipsPrivacyScopeContributionRequest {
                contribution: Some(envelope),
            }
            .encode_to_vec()
        }
        _ => return Err(incompatible_owner("unknown owner contribution coordinate")),
    };
    Ok(bytes)
}

fn decode_owner_response(
    capability_id: &str,
    bytes: &[u8],
) -> Result<privacy::PrivacyScopeContributionResponseEnvelope, SdkError> {
    let envelope = match capability_id {
        "consents.privacy.scope.contribute" => {
            privacy::ConsentsPrivacyScopeContributionResponse::decode(bytes)
                .map_err(decode_error)?
                .contribution
        }
        "contact_points.privacy.scope.contribute" => {
            privacy::ContactPointsPrivacyScopeContributionResponse::decode(bytes)
                .map_err(decode_error)?
                .contribution
        }
        "customer_accounts.privacy.scope.contribute" => {
            privacy::CustomerAccountsPrivacyScopeContributionResponse::decode(bytes)
                .map_err(decode_error)?
                .contribution
        }
        "customer_data.privacy.scope.contribute" => {
            privacy::CustomerDataPrivacyScopeContributionResponse::decode(bytes)
                .map_err(decode_error)?
                .contribution
        }
        "customer_enrichment.privacy.scope.contribute" => {
            privacy::CustomerEnrichmentPrivacyScopeContributionResponse::decode(bytes)
                .map_err(decode_error)?
                .contribution
        }
        "data_quality.privacy.scope.contribute" => {
            privacy::DataQualityPrivacyScopeContributionResponse::decode(bytes)
                .map_err(decode_error)?
                .contribution
        }
        "identity_resolution.privacy.scope.contribute" => {
            privacy::IdentityResolutionPrivacyScopeContributionResponse::decode(bytes)
                .map_err(decode_error)?
                .contribution
        }
        "parties.privacy.scope.contribute" => {
            privacy::PartiesPrivacyScopeContributionResponse::decode(bytes)
                .map_err(decode_error)?
                .contribution
        }
        "party_relationships.privacy.scope.contribute" => {
            privacy::PartyRelationshipsPrivacyScopeContributionResponse::decode(bytes)
                .map_err(decode_error)?
                .contribution
        }
        _ => return Err(incompatible_owner("unknown owner contribution coordinate")),
    };
    envelope.ok_or_else(|| incompatible_owner("owner response envelope is missing"))
}

fn scope_resource(
    resource: privacy::PrivacyScopeResourceReference,
) -> Result<ScopeResource, SdkError> {
    let data_class = match resource.data_class {
        1 => DataClass::Public,
        2 => DataClass::Internal,
        3 => DataClass::Confidential,
        4 => DataClass::Personal,
        5 => DataClass::SensitivePersonal,
        6 => DataClass::Biometric,
        7 => DataClass::Financial,
        8 => DataClass::Credential,
        9 => DataClass::Restricted,
        _ => return Err(incompatible_owner("owner resource data class is invalid")),
    };
    let evidence_class = match resource.evidence_class {
        1 => EvidenceClass::DestroyableSubjectData,
        2 => EvidenceClass::RetainMinimizedEvidence,
        3 => EvidenceClass::ImmutableRequiredEvidence,
        4 => EvidenceClass::DerivedRebuildableState,
        5 => EvidenceClass::CryptoShreddableData,
        _ => {
            return Err(incompatible_owner(
                "owner resource evidence class is invalid",
            ));
        }
    };
    ScopeResource::new(
        resource.resource_type,
        record_id(resource.resource_id)?,
        resource.resource_version,
        data_class,
        evidence_class,
        retention(resource.retention_policy_id)?,
    )
    .map_err(scope_error)
}

pub fn discovery_attempt_digest(lineage: &ScopeDiscoveryLineage) -> [u8; 32] {
    let mut bytes = Vec::new();
    append_frame(&mut bytes, b"crm.customer-privacy.discovery-attempt/v1");
    append_frame(&mut bytes, lineage.tenant_id().as_str().as_bytes());
    append_frame(&mut bytes, lineage.privacy_case_id().as_str().as_bytes());
    append_frame(&mut bytes, lineage.canonical_party_id().as_str().as_bytes());
    append_frame(
        &mut bytes,
        lineage
            .identity_resolution_generation()
            .to_string()
            .as_bytes(),
    );
    append_frame(&mut bytes, lineage.registry_digest());
    append_frame(&mut bytes, lineage.purpose_code().as_bytes());
    append_frame(
        &mut bytes,
        &lineage.effective_request_at_unix_ms().to_be_bytes(),
    );
    discovery_sha256(&bytes)
}

pub fn discovery_page_key_digest(
    attempt_digest: &[u8; 32],
    receipt: &DiscoveryPageReceipt,
) -> [u8; 32] {
    let mut bytes = Vec::new();
    append_frame(&mut bytes, b"crm.customer-privacy.discovery-page-key/v1");
    append_frame(&mut bytes, attempt_digest);
    append_frame(&mut bytes, receipt.owner_module_id.as_str().as_bytes());
    append_frame(&mut bytes, &receipt.page_number.to_be_bytes());
    append_frame(&mut bytes, &receipt.request_cursor_digest);
    discovery_sha256(&bytes)
}

fn append_frame(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value);
}

async fn ensure_active(
    activation: &dyn ModuleActivationPort,
    tenant_id: &TenantId,
    module_id: &ModuleId,
) -> Result<(), SdkError> {
    if activation.is_active(tenant_id, module_id).await? {
        Ok(())
    } else {
        Err(discovery_error(
            "CUSTOMER_PRIVACY_DISCOVERY_OWNER_DISABLED",
            ErrorCategory::Conflict,
            true,
            format!("module {module_id} is disabled"),
        ))
    }
}

fn exact_digest(bytes: &[u8], name: &str) -> Result<[u8; 32], SdkError> {
    bytes
        .try_into()
        .map_err(|_| incompatible_owner(format!("{name} must contain exactly 32 bytes")))
        .and_then(|digest: [u8; 32]| {
            if digest.iter().all(|byte| *byte == 0) {
                Err(incompatible_owner(format!("{name} must not be all zeroes")))
            } else {
                Ok(digest)
            }
        })
}

fn decode_error(error: prost::DecodeError) -> SdkError {
    incompatible_owner(format!("owner response Protobuf is invalid: {error}"))
}

fn map_owner_error(error: SdkError) -> SdkError {
    if error.retryable
        || matches!(
            error.category,
            ErrorCategory::Unavailable | ErrorCategory::Dependency
        )
    {
        discovery_error(
            "CUSTOMER_PRIVACY_DISCOVERY_OWNER_UNAVAILABLE",
            ErrorCategory::Unavailable,
            true,
            error.code,
        )
    } else {
        discovery_error(
            "CUSTOMER_PRIVACY_DISCOVERY_OWNER_FAILED",
            error.category,
            false,
            error.code,
        )
    }
}

fn incompatible_owner(reference: impl Into<String>) -> SdkError {
    discovery_error(
        "CUSTOMER_PRIVACY_DISCOVERY_OWNER_INCOMPATIBLE",
        ErrorCategory::Conflict,
        false,
        reference,
    )
}

fn replay_conflict(reference: impl Into<String>) -> SdkError {
    discovery_error(
        "CUSTOMER_PRIVACY_DISCOVERY_PAGE_REPLAY_CONFLICT",
        ErrorCategory::Conflict,
        false,
        reference,
    )
}

fn corrupt_evidence(reference: impl Into<String>) -> SdkError {
    discovery_error(
        "CUSTOMER_PRIVACY_DISCOVERY_EVIDENCE_CORRUPT",
        ErrorCategory::Internal,
        false,
        reference,
    )
}

fn configuration_error(reference: impl Into<String>) -> SdkError {
    discovery_error(
        "CUSTOMER_PRIVACY_DISCOVERY_CONFIGURATION_INVALID",
        ErrorCategory::Internal,
        false,
        reference,
    )
}

fn scope_error(error: impl std::fmt::Display) -> SdkError {
    corrupt_evidence(error.to_string())
}

fn discovery_error(
    code: &'static str,
    category: ErrorCategory,
    retryable: bool,
    reference: impl Into<String>,
) -> SdkError {
    SdkError::new(
        code,
        category,
        retryable,
        "Customer Privacy scope discovery failed closed.",
    )
    .with_internal_reference(reference)
}

fn module_id(value: &str) -> Result<ModuleId, SdkError> {
    ModuleId::try_new(value).map_err(|error| configuration_error(error.to_string()))
}

fn record_id(value: String) -> Result<RecordId, SdkError> {
    RecordId::try_new(value).map_err(|error| incompatible_owner(error.to_string()))
}

fn retention(value: impl Into<String>) -> Result<RetentionPolicyId, SdkError> {
    RetentionPolicyId::try_new(value).map_err(|error| incompatible_owner(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lineage(purpose: &str) -> ScopeDiscoveryLineage {
        let registry = OwnerScopeRegistry::canonical_v1().unwrap();
        ScopeDiscoveryLineage::new(
            RecordId::try_new("privacy-case-1").unwrap(),
            TenantId::try_new("tenant-a").unwrap(),
            RecordId::try_new("party-1").unwrap(),
            7,
            registry.registry_version().clone(),
            *registry.digest(),
            purpose,
            100,
        )
        .unwrap()
    }

    #[test]
    fn canonical_registry_and_frozen_bounds_are_exact() {
        let registry = OwnerScopeRegistry::canonical_v1().unwrap();
        assert_eq!(registry.contracts().len(), EXPECTED_DISCOVERY_OWNER_COUNT);
        assert_eq!(DEFAULT_DISCOVERY_PAGE_SIZE, 64);
        assert_eq!(MAXIMUM_DISCOVERY_PAGE_SIZE, 128);
        assert_eq!(MAXIMUM_DISCOVERY_CURSOR_BYTES, 2_048);
        assert_eq!(DISCOVERY_PHASE, 260);
        assert_eq!(
            SCOPE_DISCOVERY_COORDINATE,
            "customer_privacy.scope.discover@1.0.0"
        );
        assert_eq!(
            SCOPE_SNAPSHOT_RECORD_TYPE,
            "customer-privacy.scope-snapshot"
        );
    }

    #[test]
    fn attempt_identity_is_deterministic_and_purpose_sensitive() {
        assert_eq!(
            discovery_attempt_digest(&lineage("ERASURE")),
            discovery_attempt_digest(&lineage("ERASURE"))
        );
        assert_ne!(
            discovery_attempt_digest(&lineage("ERASURE")),
            discovery_attempt_digest(&lineage("ACCESS"))
        );
    }

    #[test]
    fn page_key_binds_attempt_owner_page_and_request_cursor() {
        let receipt = DiscoveryPageReceipt {
            owner_module_id: ModuleId::try_new("crm.consents").unwrap(),
            capability_id: CapabilityId::try_new("consents.privacy.scope.contribute").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            lineage_digest: [1; 32],
            page_number: 1,
            request_cursor_digest: [2; 32],
            response_cursor_digest: [3; 32],
            owner_cursor_digest: [4; 32],
            page_digest: [5; 32],
            scanned_resource_count: 2,
            emitted_resource_count: 1,
            terminal_complete: true,
        };
        let attempt = [7; 32];
        let key = discovery_page_key_digest(&attempt, &receipt);
        let mut changed = receipt.clone();
        changed.page_number = 2;
        assert_ne!(key, discovery_page_key_digest(&attempt, &changed));
    }

    #[test]
    fn terminal_page_requires_empty_cursor_and_safe_projection_only() {
        let contract = OwnerScopeRegistry::canonical_v1()
            .unwrap()
            .contracts()
            .iter()
            .find(|contract| contract.owner_module_id().as_str() == "crm.consents")
            .unwrap()
            .clone();
        let response = privacy::ConsentsPrivacyScopeContributionResponse {
            contribution: Some(privacy::PrivacyScopeContributionResponseEnvelope {
                owner_module_id: contract.owner_module_id().as_str().to_owned(),
                capability_id: contract.capability_id().as_str().to_owned(),
                capability_version: contract.capability_version().as_str().to_owned(),
                lineage: Some(proto_lineage(&lineage("ERASURE"))),
                resources: vec![privacy::PrivacyScopeResourceReference {
                    resource_type: "consents.authorization".to_owned(),
                    resource_id: "consent-1".to_owned(),
                    resource_version: 1,
                    data_class: 4,
                    evidence_class: 3,
                    retention_policy_id: "crm.consents.authorization".to_owned(),
                }],
                page_evidence: Some(privacy::PrivacyScopeContributionPageEvidence {
                    page_number: 1,
                    scanned_resource_count: 1,
                    emitted_resource_count: 1,
                    next_cursor: String::new(),
                    terminal_complete: true,
                    cursor_digest_sha256: vec![1; 32],
                    page_digest_sha256: vec![2; 32],
                }),
            }),
        }
        .encode_to_vec();
        let page =
            validate_response_page(&lineage("ERASURE"), &contract, 1, "", &response).unwrap();
        assert!(page.receipt.terminal_complete);
        assert_eq!(page.resources.len(), 1);
    }
}
