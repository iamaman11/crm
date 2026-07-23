use crate::canonicalization::persisted_state_json;
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, DataClass, ErrorCategory, ModuleId, RecordId,
    RetentionPolicyId, SchemaVersion, SdkError, TenantId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

pub const CANONICAL_SCOPE_REGISTRY_VERSION: &str =
    "crm.customer-privacy.scope-registry/1.0.0";
pub const SCOPE_SNAPSHOT_STATE_SCHEMA_ID: &str =
    "crm.customer-privacy.scope_snapshot.state";
pub const SCOPE_SNAPSHOT_STATE_SCHEMA_VERSION: &str = "1.0.0";
pub const SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES: u64 = 256 * 1024;
pub const SCOPE_SNAPSHOT_STATE_RETENTION_POLICY_ID: &str =
    "crm.customer_privacy.scope_snapshot";

const SCOPE_SNAPSHOT_STATE_DESCRIPTOR: &[u8] = b"crm.customer-privacy.scope_snapshot.state/v1:snapshot_id,privacy_case_id,tenant_id,canonical_party_id,identity_resolution_generation_decimal,registry_version,registry_digest,captured_at_decimal,contracts,contributions,completeness_digest";
const MAX_RESOURCE_TYPE_BYTES: usize = 180;
const SNAPSHOT_ID_PREFIX: &str = "privacy-scope-";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivacyScopeError {
    InvalidArgument {
        field: &'static str,
        safe_message: &'static str,
    },
    RegistryConflict {
        safe_message: &'static str,
    },
    Incomplete {
        owner_module_id: Option<ModuleId>,
        safe_message: &'static str,
    },
    LineageMismatch {
        owner_module_id: ModuleId,
    },
    ContributionConflict {
        owner_module_id: ModuleId,
        safe_message: &'static str,
    },
}

impl PrivacyScopeError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidArgument { .. } => "CUSTOMER_PRIVACY_SCOPE_INVALID_ARGUMENT",
            Self::RegistryConflict { .. } => "CUSTOMER_PRIVACY_SCOPE_REGISTRY_CONFLICT",
            Self::Incomplete { .. } => "CUSTOMER_PRIVACY_SCOPE_INCOMPLETE",
            Self::LineageMismatch { .. } => "CUSTOMER_PRIVACY_SCOPE_LINEAGE_MISMATCH",
            Self::ContributionConflict { .. } => {
                "CUSTOMER_PRIVACY_SCOPE_CONTRIBUTION_CONFLICT"
            }
        }
    }

    pub const fn retryable(&self) -> bool {
        matches!(self, Self::Incomplete { .. })
    }
}

impl fmt::Display for PrivacyScopeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument {
                field,
                safe_message,
            } => write!(formatter, "{field}: {safe_message}"),
            Self::RegistryConflict { safe_message } => formatter.write_str(safe_message),
            Self::Incomplete {
                owner_module_id,
                safe_message,
            } => {
                if let Some(owner_module_id) = owner_module_id {
                    write!(formatter, "{safe_message}: {owner_module_id}")
                } else {
                    formatter.write_str(safe_message)
                }
            }
            Self::LineageMismatch { owner_module_id } => write!(
                formatter,
                "owner contribution does not match the requested subject lineage: {owner_module_id}"
            ),
            Self::ContributionConflict {
                owner_module_id,
                safe_message,
            } => write!(formatter, "{safe_message}: {owner_module_id}"),
        }
    }
}

impl Error for PrivacyScopeError {}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceClass {
    DestroyableSubjectData,
    RetainMinimizedEvidence,
    ImmutableRequiredEvidence,
    DerivedRebuildableState,
    CryptoShreddableData,
}

impl EvidenceClass {
    pub const fn label(self) -> &'static str {
        match self {
            Self::DestroyableSubjectData => "destroyable_subject_data",
            Self::RetainMinimizedEvidence => "retain_minimized_evidence",
            Self::ImmutableRequiredEvidence => "immutable_required_evidence",
            Self::DerivedRebuildableState => "derived_rebuildable_state",
            Self::CryptoShreddableData => "crypto_shreddable_data",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerScopeContract {
    owner_module_id: ModuleId,
    capability_id: CapabilityId,
    capability_version: CapabilityVersion,
}

impl OwnerScopeContract {
    pub fn new(
        owner_module_id: ModuleId,
        capability_id: CapabilityId,
        capability_version: CapabilityVersion,
    ) -> Self {
        Self {
            owner_module_id,
            capability_id,
            capability_version,
        }
    }

    pub fn owner_module_id(&self) -> &ModuleId {
        &self.owner_module_id
    }

    pub fn capability_id(&self) -> &CapabilityId {
        &self.capability_id
    }

    pub fn capability_version(&self) -> &CapabilityVersion {
        &self.capability_version
    }

    fn coordinate_key(&self) -> (&str, &str) {
        (
            self.capability_id.as_str(),
            self.capability_version.as_str(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerScopeRegistry {
    registry_version: SchemaVersion,
    contracts: Vec<OwnerScopeContract>,
    digest: [u8; 32],
}

impl OwnerScopeRegistry {
    pub fn new(
        registry_version: SchemaVersion,
        contracts: impl IntoIterator<Item = OwnerScopeContract>,
    ) -> Result<Self, PrivacyScopeError> {
        let mut contracts: Vec<_> = contracts.into_iter().collect();
        if contracts.is_empty() {
            return Err(PrivacyScopeError::InvalidArgument {
                field: "contracts",
                safe_message: "at least one owner scope contract is required",
            });
        }
        contracts.sort_by(|left, right| {
            left.owner_module_id
                .cmp(&right.owner_module_id)
                .then_with(|| left.coordinate_key().cmp(&right.coordinate_key()))
        });

        for pair in contracts.windows(2) {
            let left = &pair[0];
            let right = &pair[1];
            if left.owner_module_id == right.owner_module_id {
                return Err(PrivacyScopeError::RegistryConflict {
                    safe_message: "an owner module has more than one active scope contract",
                });
            }
        }

        let mut coordinates = contracts
            .iter()
            .map(OwnerScopeContract::coordinate_key)
            .collect::<Vec<_>>();
        coordinates.sort_unstable();
        if coordinates.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(PrivacyScopeError::RegistryConflict {
                safe_message: "a scope capability coordinate is assigned to multiple owners",
            });
        }

        let digest = registry_digest(&registry_version, &contracts);
        Ok(Self {
            registry_version,
            contracts,
            digest,
        })
    }

    pub fn canonical_v1() -> Result<Self, PrivacyScopeError> {
        let version = SchemaVersion::try_new(CANONICAL_SCOPE_REGISTRY_VERSION).map_err(|_| {
            PrivacyScopeError::InvalidArgument {
                field: "registry_version",
                safe_message: "canonical registry version is invalid",
            }
        })?;
        let version_id = CapabilityVersion::try_new("1.0.0").map_err(|_| {
            PrivacyScopeError::InvalidArgument {
                field: "capability_version",
                safe_message: "canonical capability version is invalid",
            }
        })?;

        let entries = [
            ("crm.parties", "parties.privacy.scope.contribute"),
            (
                "crm.customer-accounts",
                "customer_accounts.privacy.scope.contribute",
            ),
            (
                "crm.contact-points",
                "contact_points.privacy.scope.contribute",
            ),
            (
                "crm.party-relationships",
                "party_relationships.privacy.scope.contribute",
            ),
            ("crm.consents", "consents.privacy.scope.contribute"),
            (
                "crm.identity-resolution",
                "identity_resolution.privacy.scope.contribute",
            ),
            (
                "crm.customer-data-operations",
                "customer_data.privacy.scope.contribute",
            ),
            ("crm.data-quality", "data_quality.privacy.scope.contribute"),
            (
                "crm.customer-enrichment",
                "customer_enrichment.privacy.scope.contribute",
            ),
        ];

        let mut contracts = Vec::with_capacity(entries.len());
        for (owner, capability) in entries {
            let owner_module_id =
                ModuleId::try_new(owner).map_err(|_| PrivacyScopeError::InvalidArgument {
                    field: "owner_module_id",
                    safe_message: "canonical owner module id is invalid",
                })?;
            let capability_id = CapabilityId::try_new(capability).map_err(|_| {
                PrivacyScopeError::InvalidArgument {
                    field: "capability_id",
                    safe_message: "canonical scope capability id is invalid",
                }
            })?;
            contracts.push(OwnerScopeContract::new(
                owner_module_id,
                capability_id,
                version_id.clone(),
            ));
        }
        Self::new(version, contracts)
    }

    pub fn registry_version(&self) -> &SchemaVersion {
        &self.registry_version
    }

    pub fn contracts(&self) -> &[OwnerScopeContract] {
        &self.contracts
    }

    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeResource {
    resource_type: String,
    resource_id: RecordId,
    resource_version: u64,
    data_class: DataClass,
    evidence_class: EvidenceClass,
    retention_policy_id: RetentionPolicyId,
}

impl ScopeResource {
    pub fn new(
        resource_type: impl Into<String>,
        resource_id: RecordId,
        resource_version: u64,
        data_class: DataClass,
        evidence_class: EvidenceClass,
        retention_policy_id: RetentionPolicyId,
    ) -> Result<Self, PrivacyScopeError> {
        let resource_type = resource_type.into();
        validate_resource_type(&resource_type)?;
        if resource_version == 0 {
            return Err(PrivacyScopeError::InvalidArgument {
                field: "resource_version",
                safe_message: "resource version must be positive",
            });
        }
        Ok(Self {
            resource_type,
            resource_id,
            resource_version,
            data_class,
            evidence_class,
            retention_policy_id,
        })
    }

    pub fn resource_type(&self) -> &str {
        &self.resource_type
    }

    pub fn resource_id(&self) -> &RecordId {
        &self.resource_id
    }

    pub const fn resource_version(&self) -> u64 {
        self.resource_version
    }

    pub const fn data_class(&self) -> DataClass {
        self.data_class
    }

    pub const fn evidence_class(&self) -> EvidenceClass {
        self.evidence_class
    }

    pub fn retention_policy_id(&self) -> &RetentionPolicyId {
        &self.retention_policy_id
    }

    fn identity_key(&self) -> (&str, &str) {
        (self.resource_type.as_str(), self.resource_id.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContributionCompletenessProof {
    complete: bool,
    page_count: u32,
    scanned_resource_count: u64,
    emitted_resource_count: u64,
    terminal_cursor_digest: [u8; 32],
}

impl ContributionCompletenessProof {
    pub fn new(
        complete: bool,
        page_count: u32,
        scanned_resource_count: u64,
        emitted_resource_count: u64,
        terminal_cursor_digest: [u8; 32],
    ) -> Result<Self, PrivacyScopeError> {
        if page_count == 0 {
            return Err(PrivacyScopeError::InvalidArgument {
                field: "page_count",
                safe_message: "completeness proof must include at least one bounded page",
            });
        }
        if scanned_resource_count < emitted_resource_count {
            return Err(PrivacyScopeError::InvalidArgument {
                field: "scanned_resource_count",
                safe_message: "scanned resource count cannot be lower than emitted count",
            });
        }
        if terminal_cursor_digest.iter().all(|byte| *byte == 0) {
            return Err(PrivacyScopeError::InvalidArgument {
                field: "terminal_cursor_digest",
                safe_message: "terminal cursor digest must not be all zeroes",
            });
        }
        Ok(Self {
            complete,
            page_count,
            scanned_resource_count,
            emitted_resource_count,
            terminal_cursor_digest,
        })
    }

    pub const fn complete(&self) -> bool {
        self.complete
    }

    pub const fn page_count(&self) -> u32 {
        self.page_count
    }

    pub const fn scanned_resource_count(&self) -> u64 {
        self.scanned_resource_count
    }

    pub const fn emitted_resource_count(&self) -> u64 {
        self.emitted_resource_count
    }

    pub const fn terminal_cursor_digest(&self) -> &[u8; 32] {
        &self.terminal_cursor_digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerScopeContribution {
    contract: OwnerScopeContract,
    tenant_id: TenantId,
    canonical_party_id: RecordId,
    identity_resolution_generation: u64,
    resources: Vec<ScopeResource>,
    completeness: ContributionCompletenessProof,
    digest: [u8; 32],
}

impl OwnerScopeContribution {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        contract: OwnerScopeContract,
        tenant_id: TenantId,
        canonical_party_id: RecordId,
        identity_resolution_generation: u64,
        resources: impl IntoIterator<Item = ScopeResource>,
        completeness: ContributionCompletenessProof,
    ) -> Result<Self, PrivacyScopeError> {
        let resources = normalize_resources(contract.owner_module_id(), resources)?;
        if completeness.emitted_resource_count != resources.len() as u64 {
            return Err(PrivacyScopeError::ContributionConflict {
                owner_module_id: contract.owner_module_id.clone(),
                safe_message: "completeness emitted count does not match normalized resources",
            });
        }
        let digest = contribution_digest(
            &contract,
            &tenant_id,
            &canonical_party_id,
            identity_resolution_generation,
            &resources,
            &completeness,
        );
        Ok(Self {
            contract,
            tenant_id,
            canonical_party_id,
            identity_resolution_generation,
            resources,
            completeness,
            digest,
        })
    }

    pub fn contract(&self) -> &OwnerScopeContract {
        &self.contract
    }

    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    pub fn canonical_party_id(&self) -> &RecordId {
        &self.canonical_party_id
    }

    pub const fn identity_resolution_generation(&self) -> u64 {
        self.identity_resolution_generation
    }

    pub fn resources(&self) -> &[ScopeResource] {
        &self.resources
    }

    pub fn completeness(&self) -> &ContributionCompletenessProof {
        &self.completeness
    }

    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopedResource {
    owner_module_id: ModuleId,
    resource: ScopeResource,
}

impl ScopedResource {
    pub fn owner_module_id(&self) -> &ModuleId {
        &self.owner_module_id
    }

    pub fn resource(&self) -> &ScopeResource {
        &self.resource
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeSnapshot {
    snapshot_id: RecordId,
    privacy_case_id: RecordId,
    tenant_id: TenantId,
    canonical_party_id: RecordId,
    identity_resolution_generation: u64,
    registry: OwnerScopeRegistry,
    captured_at_unix_nanos: i64,
    contributions: Vec<OwnerScopeContribution>,
    resources: Vec<ScopedResource>,
    completeness_digest: [u8; 32],
}

impl ScopeSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn finalize(
        privacy_case_id: RecordId,
        tenant_id: TenantId,
        canonical_party_id: RecordId,
        identity_resolution_generation: u64,
        registry: OwnerScopeRegistry,
        captured_at_unix_nanos: i64,
        contributions: impl IntoIterator<Item = OwnerScopeContribution>,
    ) -> Result<Self, PrivacyScopeError> {
        if captured_at_unix_nanos < 0 {
            return Err(PrivacyScopeError::InvalidArgument {
                field: "captured_at_unix_nanos",
                safe_message: "snapshot timestamp must not be negative",
            });
        }

        let mut by_owner = BTreeMap::new();
        for contribution in contributions {
            let owner = contribution.contract.owner_module_id.clone();
            if by_owner.insert(owner.clone(), contribution).is_some() {
                return Err(PrivacyScopeError::ContributionConflict {
                    owner_module_id: owner,
                    safe_message: "owner contributed more than once",
                });
            }
        }

        if by_owner.len() != registry.contracts.len() {
            let missing_owner = registry
                .contracts
                .iter()
                .find(|contract| !by_owner.contains_key(&contract.owner_module_id))
                .map(|contract| contract.owner_module_id.clone());
            return Err(PrivacyScopeError::Incomplete {
                owner_module_id: missing_owner,
                safe_message: "scope snapshot cannot finalize without every registered owner",
            });
        }

        let mut normalized = Vec::with_capacity(registry.contracts.len());
        let mut flattened = Vec::new();
        for contract in &registry.contracts {
            let contribution = by_owner.remove(&contract.owner_module_id).ok_or_else(|| {
                PrivacyScopeError::Incomplete {
                    owner_module_id: Some(contract.owner_module_id.clone()),
                    safe_message: "registered owner contribution is missing",
                }
            })?;
            if contribution.contract != *contract {
                return Err(PrivacyScopeError::ContributionConflict {
                    owner_module_id: contract.owner_module_id.clone(),
                    safe_message: "owner contribution contract does not match the registry",
                });
            }
            if contribution.tenant_id != tenant_id
                || contribution.canonical_party_id != canonical_party_id
                || contribution.identity_resolution_generation
                    != identity_resolution_generation
            {
                return Err(PrivacyScopeError::LineageMismatch {
                    owner_module_id: contract.owner_module_id.clone(),
                });
            }
            if !contribution.completeness.complete {
                return Err(PrivacyScopeError::Incomplete {
                    owner_module_id: Some(contract.owner_module_id.clone()),
                    safe_message: "owner contribution did not prove terminal completeness",
                });
            }
            for resource in &contribution.resources {
                flattened.push(ScopedResource {
                    owner_module_id: contract.owner_module_id.clone(),
                    resource: resource.clone(),
                });
            }
            normalized.push(contribution);
        }

        flattened.sort_by(scoped_resource_cmp);
        let completeness_digest = snapshot_completeness_digest(&registry, &normalized, &flattened);
        let snapshot_digest = snapshot_identity_digest(
            &privacy_case_id,
            &tenant_id,
            &canonical_party_id,
            identity_resolution_generation,
            registry.digest(),
            &completeness_digest,
            captured_at_unix_nanos,
        );
        let snapshot_id = RecordId::try_new(format!(
            "{SNAPSHOT_ID_PREFIX}{}",
            hex_encode(&snapshot_digest)
        ))
        .map_err(|_| PrivacyScopeError::InvalidArgument {
            field: "snapshot_id",
            safe_message: "derived scope snapshot id is invalid",
        })?;

        Ok(Self {
            snapshot_id,
            privacy_case_id,
            tenant_id,
            canonical_party_id,
            identity_resolution_generation,
            registry,
            captured_at_unix_nanos,
            contributions: normalized,
            resources: flattened,
            completeness_digest,
        })
    }

    pub fn snapshot_id(&self) -> &RecordId {
        &self.snapshot_id
    }

    pub fn privacy_case_id(&self) -> &RecordId {
        &self.privacy_case_id
    }

    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    pub fn canonical_party_id(&self) -> &RecordId {
        &self.canonical_party_id
    }

    pub const fn identity_resolution_generation(&self) -> u64 {
        self.identity_resolution_generation
    }

    pub fn registry(&self) -> &OwnerScopeRegistry {
        &self.registry
    }

    pub const fn captured_at_unix_nanos(&self) -> i64 {
        self.captured_at_unix_nanos
    }

    pub fn contributions(&self) -> &[OwnerScopeContribution] {
        &self.contributions
    }

    pub fn resources(&self) -> &[ScopedResource] {
        &self.resources
    }

    pub const fn completeness_digest(&self) -> &[u8; 32] {
        &self.completeness_digest
    }
}

pub fn scope_snapshot_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(SCOPE_SNAPSHOT_STATE_DESCRIPTOR).into()
}

pub fn encode_scope_snapshot_state(snapshot: &ScopeSnapshot) -> Result<Vec<u8>, SdkError> {
    let bytes = persisted_state_json::to_vec(&ScopeSnapshotStateV1::from(snapshot))
        .map_err(|error| persisted_error(format!("scope snapshot serialization failed: {error}")))?;
    validate_state_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_scope_snapshot_state(bytes: &[u8]) -> Result<ScopeSnapshot, SdkError> {
    validate_state_size(bytes)?;
    let state: ScopeSnapshotStateV1 = persisted_state_json::from_slice(bytes)
        .map_err(|error| persisted_error(format!("scope snapshot JSON is invalid: {error}")))?;
    let snapshot = state.into_domain()?;
    if encode_scope_snapshot_state(&snapshot)? != bytes {
        return Err(persisted_error(
            "persisted scope snapshot is not the strict canonical v1 encoding",
        ));
    }
    Ok(snapshot)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeSnapshotStateV1 {
    snapshot_id: String,
    privacy_case_id: String,
    tenant_id: String,
    canonical_party_id: String,
    identity_resolution_generation: String,
    registry_version: String,
    registry_digest: String,
    captured_at_unix_nanos: String,
    contracts: Vec<OwnerScopeContractStateV1>,
    contributions: Vec<OwnerScopeContributionStateV1>,
    completeness_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerScopeContractStateV1 {
    owner_module_id: String,
    capability_id: String,
    capability_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerScopeContributionStateV1 {
    owner_module_id: String,
    capability_id: String,
    capability_version: String,
    tenant_id: String,
    canonical_party_id: String,
    identity_resolution_generation: String,
    resources: Vec<ScopeResourceStateV1>,
    completeness: ContributionCompletenessProofStateV1,
    digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeResourceStateV1 {
    resource_type: String,
    resource_id: String,
    resource_version: String,
    data_class: DataClass,
    evidence_class: EvidenceClass,
    retention_policy_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContributionCompletenessProofStateV1 {
    complete: bool,
    page_count: u32,
    scanned_resource_count: String,
    emitted_resource_count: String,
    terminal_cursor_digest: String,
}

impl From<&ScopeSnapshot> for ScopeSnapshotStateV1 {
    fn from(snapshot: &ScopeSnapshot) -> Self {
        Self {
            snapshot_id: snapshot.snapshot_id.as_str().to_owned(),
            privacy_case_id: snapshot.privacy_case_id.as_str().to_owned(),
            tenant_id: snapshot.tenant_id.as_str().to_owned(),
            canonical_party_id: snapshot.canonical_party_id.as_str().to_owned(),
            identity_resolution_generation: snapshot
                .identity_resolution_generation
                .to_string(),
            registry_version: snapshot.registry.registry_version.as_str().to_owned(),
            registry_digest: hex_encode(snapshot.registry.digest()),
            captured_at_unix_nanos: snapshot.captured_at_unix_nanos.to_string(),
            contracts: snapshot
                .registry
                .contracts
                .iter()
                .map(OwnerScopeContractStateV1::from)
                .collect(),
            contributions: snapshot
                .contributions
                .iter()
                .map(OwnerScopeContributionStateV1::from)
                .collect(),
            completeness_digest: hex_encode(&snapshot.completeness_digest),
        }
    }
}

impl ScopeSnapshotStateV1 {
    fn into_domain(self) -> Result<ScopeSnapshot, SdkError> {
        let expected_snapshot_id = self.snapshot_id;
        let expected_registry_digest = hex_decode(&self.registry_digest, "registry_digest")?;
        let expected_completeness_digest =
            hex_decode(&self.completeness_digest, "completeness_digest")?;
        let registry = OwnerScopeRegistry::new(
            SchemaVersion::try_new(self.registry_version)
                .map_err(|error| persisted_error(format!("registry version is invalid: {error}")))?,
            self.contracts
                .into_iter()
                .map(OwnerScopeContractStateV1::into_domain)
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(scope_error)?;
        if registry.digest != expected_registry_digest {
            return Err(persisted_error(
                "persisted scope registry digest does not match its contracts",
            ));
        }
        let snapshot = ScopeSnapshot::finalize(
            RecordId::try_new(self.privacy_case_id)
                .map_err(|error| persisted_error(format!("privacy case id is invalid: {error}")))?,
            TenantId::try_new(self.tenant_id)
                .map_err(|error| persisted_error(format!("tenant id is invalid: {error}")))?,
            RecordId::try_new(self.canonical_party_id).map_err(|error| {
                persisted_error(format!("canonical Party id is invalid: {error}"))
            })?,
            decimal_u64(
                self.identity_resolution_generation,
                "identity_resolution_generation",
            )?,
            registry,
            decimal_i64(self.captured_at_unix_nanos, "captured_at_unix_nanos")?,
            self.contributions
                .into_iter()
                .map(OwnerScopeContributionStateV1::into_domain)
                .collect::<Result<Vec<_>, _>>()?,
        )
        .map_err(scope_error)?;
        if snapshot.snapshot_id.as_str() != expected_snapshot_id {
            return Err(persisted_error(
                "persisted scope snapshot id does not match deterministic content",
            ));
        }
        if snapshot.completeness_digest != expected_completeness_digest {
            return Err(persisted_error(
                "persisted completeness digest does not match owner contributions",
            ));
        }
        Ok(snapshot)
    }
}

impl From<&OwnerScopeContract> for OwnerScopeContractStateV1 {
    fn from(contract: &OwnerScopeContract) -> Self {
        Self {
            owner_module_id: contract.owner_module_id.as_str().to_owned(),
            capability_id: contract.capability_id.as_str().to_owned(),
            capability_version: contract.capability_version.as_str().to_owned(),
        }
    }
}

impl OwnerScopeContractStateV1 {
    fn into_domain(self) -> Result<OwnerScopeContract, SdkError> {
        Ok(OwnerScopeContract::new(
            ModuleId::try_new(self.owner_module_id)
                .map_err(|error| persisted_error(format!("owner module id is invalid: {error}")))?,
            CapabilityId::try_new(self.capability_id)
                .map_err(|error| persisted_error(format!("capability id is invalid: {error}")))?,
            CapabilityVersion::try_new(self.capability_version).map_err(|error| {
                persisted_error(format!("capability version is invalid: {error}"))
            })?,
        ))
    }
}

impl From<&OwnerScopeContribution> for OwnerScopeContributionStateV1 {
    fn from(contribution: &OwnerScopeContribution) -> Self {
        Self {
            owner_module_id: contribution.contract.owner_module_id.as_str().to_owned(),
            capability_id: contribution.contract.capability_id.as_str().to_owned(),
            capability_version: contribution
                .contract
                .capability_version
                .as_str()
                .to_owned(),
            tenant_id: contribution.tenant_id.as_str().to_owned(),
            canonical_party_id: contribution.canonical_party_id.as_str().to_owned(),
            identity_resolution_generation: contribution
                .identity_resolution_generation
                .to_string(),
            resources: contribution
                .resources
                .iter()
                .map(ScopeResourceStateV1::from)
                .collect(),
            completeness: ContributionCompletenessProofStateV1::from(
                &contribution.completeness,
            ),
            digest: hex_encode(&contribution.digest),
        }
    }
}

impl OwnerScopeContributionStateV1 {
    fn into_domain(self) -> Result<OwnerScopeContribution, SdkError> {
        let owner_module_id = ModuleId::try_new(self.owner_module_id)
            .map_err(|error| persisted_error(format!("owner module id is invalid: {error}")))?;
        let expected_digest = hex_decode(&self.digest, "contribution.digest")?;
        let contribution = OwnerScopeContribution::new(
            OwnerScopeContract::new(
                owner_module_id,
                CapabilityId::try_new(self.capability_id).map_err(|error| {
                    persisted_error(format!("capability id is invalid: {error}"))
                })?,
                CapabilityVersion::try_new(self.capability_version).map_err(|error| {
                    persisted_error(format!("capability version is invalid: {error}"))
                })?,
            ),
            TenantId::try_new(self.tenant_id)
                .map_err(|error| persisted_error(format!("tenant id is invalid: {error}")))?,
            RecordId::try_new(self.canonical_party_id).map_err(|error| {
                persisted_error(format!("canonical Party id is invalid: {error}"))
            })?,
            decimal_u64(
                self.identity_resolution_generation,
                "identity_resolution_generation",
            )?,
            self.resources
                .into_iter()
                .map(ScopeResourceStateV1::into_domain)
                .collect::<Result<Vec<_>, _>>()?,
            self.completeness.into_domain()?,
        )
        .map_err(scope_error)?;
        if contribution.digest != expected_digest {
            return Err(persisted_error(
                "persisted owner contribution digest does not match its content",
            ));
        }
        Ok(contribution)
    }
}

impl From<&ScopeResource> for ScopeResourceStateV1 {
    fn from(resource: &ScopeResource) -> Self {
        Self {
            resource_type: resource.resource_type.clone(),
            resource_id: resource.resource_id.as_str().to_owned(),
            resource_version: resource.resource_version.to_string(),
            data_class: resource.data_class,
            evidence_class: resource.evidence_class,
            retention_policy_id: resource.retention_policy_id.as_str().to_owned(),
        }
    }
}

impl ScopeResourceStateV1 {
    fn into_domain(self) -> Result<ScopeResource, SdkError> {
        ScopeResource::new(
            self.resource_type,
            RecordId::try_new(self.resource_id)
                .map_err(|error| persisted_error(format!("resource id is invalid: {error}")))?,
            decimal_u64(self.resource_version, "resource_version")?,
            self.data_class,
            self.evidence_class,
            RetentionPolicyId::try_new(self.retention_policy_id).map_err(|error| {
                persisted_error(format!("retention policy id is invalid: {error}"))
            })?,
        )
        .map_err(scope_error)
    }
}

impl From<&ContributionCompletenessProof>
    for ContributionCompletenessProofStateV1
{
    fn from(proof: &ContributionCompletenessProof) -> Self {
        Self {
            complete: proof.complete,
            page_count: proof.page_count,
            scanned_resource_count: proof.scanned_resource_count.to_string(),
            emitted_resource_count: proof.emitted_resource_count.to_string(),
            terminal_cursor_digest: hex_encode(&proof.terminal_cursor_digest),
        }
    }
}

impl ContributionCompletenessProofStateV1 {
    fn into_domain(self) -> Result<ContributionCompletenessProof, SdkError> {
        ContributionCompletenessProof::new(
            self.complete,
            self.page_count,
            decimal_u64(self.scanned_resource_count, "scanned_resource_count")?,
            decimal_u64(self.emitted_resource_count, "emitted_resource_count")?,
            hex_decode(&self.terminal_cursor_digest, "terminal_cursor_digest")?,
        )
        .map_err(scope_error)
    }
}

fn validate_resource_type(value: &str) -> Result<(), PrivacyScopeError> {
    if value.is_empty() {
        return Err(PrivacyScopeError::InvalidArgument {
            field: "resource_type",
            safe_message: "resource type must not be empty",
        });
    }
    if value.len() > MAX_RESOURCE_TYPE_BYTES {
        return Err(PrivacyScopeError::InvalidArgument {
            field: "resource_type",
            safe_message: "resource type exceeds the bounded maximum",
        });
    }
    if value.chars().any(char::is_control) {
        return Err(PrivacyScopeError::InvalidArgument {
            field: "resource_type",
            safe_message: "resource type must not contain control characters",
        });
    }
    Ok(())
}

fn normalize_resources(
    owner_module_id: &ModuleId,
    resources: impl IntoIterator<Item = ScopeResource>,
) -> Result<Vec<ScopeResource>, PrivacyScopeError> {
    let mut resources: Vec<_> = resources.into_iter().collect();
    resources.sort_by(scope_resource_cmp);
    let mut normalized: Vec<ScopeResource> = Vec::with_capacity(resources.len());
    for resource in resources {
        if let Some(previous) = normalized.last()
            && previous.identity_key() == resource.identity_key() {
                if previous == &resource {
                    continue;
                }
                return Err(PrivacyScopeError::ContributionConflict {
                    owner_module_id: owner_module_id.clone(),
                    safe_message:
                        "one owner resource has conflicting version or classification",
                });
            }
        normalized.push(resource);
    }
    Ok(normalized)
}

fn scope_resource_cmp(left: &ScopeResource, right: &ScopeResource) -> Ordering {
    left.resource_type
        .cmp(&right.resource_type)
        .then_with(|| left.resource_id.cmp(&right.resource_id))
        .then_with(|| left.resource_version.cmp(&right.resource_version))
        .then_with(|| data_class_label(left.data_class).cmp(data_class_label(right.data_class)))
        .then_with(|| left.evidence_class.cmp(&right.evidence_class))
        .then_with(|| left.retention_policy_id.cmp(&right.retention_policy_id))
}

fn scoped_resource_cmp(left: &ScopedResource, right: &ScopedResource) -> Ordering {
    left.owner_module_id
        .cmp(&right.owner_module_id)
        .then_with(|| scope_resource_cmp(&left.resource, &right.resource))
}

fn data_class_label(value: DataClass) -> &'static str {
    match value {
        DataClass::Public => "public",
        DataClass::Internal => "internal",
        DataClass::Confidential => "confidential",
        DataClass::Restricted => "restricted",
        DataClass::Personal => "personal",
        DataClass::SensitivePersonal => "sensitive_personal",
        DataClass::Biometric => "biometric",
        DataClass::Financial => "financial",
        DataClass::Credential => "credential",
    }
}

fn registry_digest(
    version: &SchemaVersion,
    contracts: &[OwnerScopeContract],
) -> [u8; 32] {
    let mut hasher = framed_hasher(b"crm.customer-privacy.scope-registry/v1");
    hash_field(&mut hasher, version.as_str().as_bytes());
    for contract in contracts {
        hash_field(&mut hasher, contract.owner_module_id.as_str().as_bytes());
        hash_field(&mut hasher, contract.capability_id.as_str().as_bytes());
        hash_field(
            &mut hasher,
            contract.capability_version.as_str().as_bytes(),
        );
    }
    hasher.finalize().into()
}

fn contribution_digest(
    contract: &OwnerScopeContract,
    tenant_id: &TenantId,
    canonical_party_id: &RecordId,
    identity_resolution_generation: u64,
    resources: &[ScopeResource],
    completeness: &ContributionCompletenessProof,
) -> [u8; 32] {
    let mut hasher = framed_hasher(b"crm.customer-privacy.scope-contribution/v1");
    hash_field(
        &mut hasher,
        contract.owner_module_id.as_str().as_bytes(),
    );
    hash_field(&mut hasher, contract.capability_id.as_str().as_bytes());
    hash_field(
        &mut hasher,
        contract.capability_version.as_str().as_bytes(),
    );
    hash_field(&mut hasher, tenant_id.as_str().as_bytes());
    hash_field(&mut hasher, canonical_party_id.as_str().as_bytes());
    hash_field(
        &mut hasher,
        identity_resolution_generation.to_string().as_bytes(),
    );
    hash_field(&mut hasher, &[u8::from(completeness.complete)]);
    hash_field(&mut hasher, &completeness.page_count.to_be_bytes());
    hash_field(
        &mut hasher,
        &completeness.scanned_resource_count.to_be_bytes(),
    );
    hash_field(
        &mut hasher,
        &completeness.emitted_resource_count.to_be_bytes(),
    );
    hash_field(&mut hasher, &completeness.terminal_cursor_digest);
    for resource in resources {
        hash_field(&mut hasher, resource.resource_type.as_bytes());
        hash_field(&mut hasher, resource.resource_id.as_str().as_bytes());
        hash_field(&mut hasher, &resource.resource_version.to_be_bytes());
        hash_field(&mut hasher, data_class_label(resource.data_class).as_bytes());
        hash_field(&mut hasher, resource.evidence_class.label().as_bytes());
        hash_field(
            &mut hasher,
            resource.retention_policy_id.as_str().as_bytes(),
        );
    }
    hasher.finalize().into()
}

fn snapshot_completeness_digest(
    registry: &OwnerScopeRegistry,
    contributions: &[OwnerScopeContribution],
    resources: &[ScopedResource],
) -> [u8; 32] {
    let mut hasher = framed_hasher(b"crm.customer-privacy.scope-completeness/v1");
    hash_field(&mut hasher, registry.digest());
    hash_field(
        &mut hasher,
        &(registry.contracts.len() as u64).to_be_bytes(),
    );
    hash_field(
        &mut hasher,
        &(contributions.len() as u64).to_be_bytes(),
    );
    hash_field(&mut hasher, &(resources.len() as u64).to_be_bytes());
    for contribution in contributions {
        hash_field(&mut hasher, contribution.digest());
    }
    hasher.finalize().into()
}

#[allow(clippy::too_many_arguments)]
fn snapshot_identity_digest(
    privacy_case_id: &RecordId,
    tenant_id: &TenantId,
    canonical_party_id: &RecordId,
    identity_resolution_generation: u64,
    registry_digest: &[u8; 32],
    completeness_digest: &[u8; 32],
    captured_at_unix_nanos: i64,
) -> [u8; 32] {
    let mut hasher = framed_hasher(b"crm.customer-privacy.scope-snapshot/v1");
    hash_field(&mut hasher, privacy_case_id.as_str().as_bytes());
    hash_field(&mut hasher, tenant_id.as_str().as_bytes());
    hash_field(&mut hasher, canonical_party_id.as_str().as_bytes());
    hash_field(
        &mut hasher,
        identity_resolution_generation.to_string().as_bytes(),
    );
    hash_field(&mut hasher, registry_digest);
    hash_field(&mut hasher, completeness_digest);
    hash_field(&mut hasher, &captured_at_unix_nanos.to_be_bytes());
    hasher.finalize().into()
}

fn framed_hasher(domain: &[u8]) -> Sha256 {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, domain);
    hasher
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn hex_encode(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn hex_decode(value: &str, field: &str) -> Result<[u8; 32], SdkError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(persisted_error(format!(
            "{field} must be a 64-character hexadecimal digest"
        )));
    }
    let mut output = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(chunk[0]).ok_or_else(|| {
            persisted_error(format!("{field} contains invalid hexadecimal data"))
        })?;
        let low = hex_nibble(chunk[1]).ok_or_else(|| {
            persisted_error(format!("{field} contains invalid hexadecimal data"))
        })?;
        output[index] = (high << 4) | low;
    }
    Ok(output)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn decimal_u64(value: String, field: &str) -> Result<u64, SdkError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(persisted_error(format!(
            "{field} must use canonical unsigned decimal encoding"
        )));
    }
    value
        .parse::<u64>()
        .map_err(|error| persisted_error(format!("{field} is invalid: {error}")))
}

fn decimal_i64(value: String, field: &str) -> Result<i64, SdkError> {
    if value.is_empty()
        || value.starts_with('+')
        || value == "-0"
        || (value.len() > 1 && value.starts_with('0'))
        || (value.starts_with('-')
            && (value.len() == 1
                || value.as_bytes().get(1) == Some(&b'0')
                || !value[1..].bytes().all(|byte| byte.is_ascii_digit())))
        || (!value.starts_with('-') && !value.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(persisted_error(format!(
            "{field} must use canonical signed decimal encoding"
        )));
    }
    value
        .parse::<i64>()
        .map_err(|error| persisted_error(format!("{field} is invalid: {error}")))
}

fn validate_state_size(bytes: &[u8]) -> Result<(), SdkError> {
    if bytes.len() as u64 > SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(
            "scope snapshot exceeds its governed maximum size",
        ));
    }
    Ok(())
}

fn scope_error(error: PrivacyScopeError) -> SdkError {
    persisted_error(format!("{}: {error}", error.code()))
}

fn persisted_error(internal_reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_SCOPE_SNAPSHOT_INVALID",
        ErrorCategory::Internal,
        false,
        "Persisted customer privacy scope evidence is invalid.",
    )
    .with_internal_reference(internal_reference)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tenant_id() -> TenantId {
        TenantId::try_new("tenant-a").unwrap()
    }

    fn record_id(value: &str) -> RecordId {
        RecordId::try_new(value).unwrap()
    }

    fn retention_policy() -> RetentionPolicyId {
        RetentionPolicyId::try_new("privacy-owner/default").unwrap()
    }

    fn terminal_digest(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn resource(owner_index: usize) -> ScopeResource {
        ScopeResource::new(
            format!("owner-{owner_index}.resource"),
            record_id(&format!("resource-{owner_index}")),
            7,
            DataClass::Personal,
            EvidenceClass::DestroyableSubjectData,
            retention_policy(),
        )
        .unwrap()
    }

    fn contribution(
        contract: &OwnerScopeContract,
        owner_index: usize,
        complete: bool,
    ) -> OwnerScopeContribution {
        let resources = vec![resource(owner_index)];
        OwnerScopeContribution::new(
            contract.clone(),
            tenant_id(),
            record_id("party-canonical"),
            9,
            resources,
            ContributionCompletenessProof::new(
                complete,
                1,
                1,
                1,
                terminal_digest((owner_index + 1) as u8),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn complete_contributions(
        registry: &OwnerScopeRegistry,
    ) -> Vec<OwnerScopeContribution> {
        registry
            .contracts()
            .iter()
            .enumerate()
            .map(|(index, contract)| contribution(contract, index, true))
            .collect()
    }

    #[test]
    fn canonical_registry_is_exact_versioned_and_deterministic() {
        let first = OwnerScopeRegistry::canonical_v1().unwrap();
        let second = OwnerScopeRegistry::canonical_v1().unwrap();

        assert_eq!(first, second);
        assert_eq!(first.contracts().len(), 9);
        assert_eq!(
            first.registry_version().as_str(),
            CANONICAL_SCOPE_REGISTRY_VERSION
        );
        assert!(first.digest().iter().any(|byte| *byte != 0));
        assert_eq!(
            first.contracts()[0].owner_module_id().as_str(),
            "crm.consents"
        );
        assert_eq!(
            first.contracts()[8].owner_module_id().as_str(),
            "crm.party-relationships"
        );
    }

    #[test]
    fn snapshot_rejects_missing_or_nonterminal_owner_contribution() {
        let registry = OwnerScopeRegistry::canonical_v1().unwrap();
        let mut contributions = complete_contributions(&registry);
        contributions.pop();
        let error = ScopeSnapshot::finalize(
            record_id("case-1"),
            tenant_id(),
            record_id("party-canonical"),
            9,
            registry.clone(),
            100,
            contributions,
        )
        .unwrap_err();
        assert_eq!(error.code(), "CUSTOMER_PRIVACY_SCOPE_INCOMPLETE");
        assert!(error.retryable());

        let mut contributions = complete_contributions(&registry);
        contributions[0] = contribution(&registry.contracts()[0], 0, false);
        let error = ScopeSnapshot::finalize(
            record_id("case-1"),
            tenant_id(),
            record_id("party-canonical"),
            9,
            registry,
            100,
            contributions,
        )
        .unwrap_err();
        assert_eq!(error.code(), "CUSTOMER_PRIVACY_SCOPE_INCOMPLETE");
    }

    #[test]
    fn snapshot_sorts_owners_and_resources_and_deduplicates_exact_items() {
        let registry = OwnerScopeRegistry::canonical_v1().unwrap();
        let mut contributions = complete_contributions(&registry);
        let duplicate = resource(0);
        contributions[0] = OwnerScopeContribution::new(
            registry.contracts()[0].clone(),
            tenant_id(),
            record_id("party-canonical"),
            9,
            vec![duplicate.clone(), duplicate],
            ContributionCompletenessProof::new(true, 1, 2, 1, terminal_digest(1)).unwrap(),
        )
        .unwrap();
        contributions.reverse();

        let first = ScopeSnapshot::finalize(
            record_id("case-1"),
            tenant_id(),
            record_id("party-canonical"),
            9,
            registry.clone(),
            100,
            contributions.clone(),
        )
        .unwrap();
        contributions.reverse();
        let second = ScopeSnapshot::finalize(
            record_id("case-1"),
            tenant_id(),
            record_id("party-canonical"),
            9,
            registry,
            100,
            contributions,
        )
        .unwrap();

        assert_eq!(first, second);
        assert_eq!(first.contributions().len(), 9);
        assert_eq!(first.resources().len(), 9);
        assert_eq!(
            first.contributions()[0].contract().owner_module_id().as_str(),
            "crm.consents"
        );
    }

    #[test]
    fn snapshot_rejects_lineage_or_resource_classification_conflict() {
        let registry = OwnerScopeRegistry::canonical_v1().unwrap();
        let contract = registry.contracts()[0].clone();
        let original_resource = resource(0);
        let conflicting = ScopeResource::new(
            original_resource.resource_type().to_owned(),
            original_resource.resource_id().clone(),
            original_resource.resource_version(),
            DataClass::SensitivePersonal,
            EvidenceClass::RetainMinimizedEvidence,
            retention_policy(),
        )
        .unwrap();
        let error = OwnerScopeContribution::new(
            contract.clone(),
            tenant_id(),
            record_id("party-canonical"),
            9,
            vec![original_resource, conflicting],
            ContributionCompletenessProof::new(true, 1, 2, 2, terminal_digest(1)).unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            error.code(),
            "CUSTOMER_PRIVACY_SCOPE_CONTRIBUTION_CONFLICT"
        );

        let mut contributions = complete_contributions(&registry);
        contributions[0] = OwnerScopeContribution::new(
            contract,
            tenant_id(),
            record_id("party-other"),
            9,
            vec![resource(0)],
            ContributionCompletenessProof::new(true, 1, 1, 1, terminal_digest(1)).unwrap(),
        )
        .unwrap();
        let error = ScopeSnapshot::finalize(
            record_id("case-1"),
            tenant_id(),
            record_id("party-canonical"),
            9,
            registry,
            100,
            contributions,
        )
        .unwrap_err();
        assert_eq!(
            error.code(),
            "CUSTOMER_PRIVACY_SCOPE_LINEAGE_MISMATCH"
        );
    }

    #[test]
    fn canonical_scope_snapshot_round_trip_rejects_noncanonical_bytes() {
        let registry = OwnerScopeRegistry::canonical_v1().unwrap();
        let snapshot = ScopeSnapshot::finalize(
            record_id("case-1"),
            tenant_id(),
            record_id("party-canonical"),
            9,
            registry,
            100,
            complete_contributions(&OwnerScopeRegistry::canonical_v1().unwrap()),
        )
        .unwrap();

        let bytes = encode_scope_snapshot_state(&snapshot).unwrap();
        assert_eq!(decode_scope_snapshot_state(&bytes).unwrap(), snapshot);
        assert!(scope_snapshot_state_descriptor_hash()
            .iter()
            .any(|byte| *byte != 0));

        let mut spaced = bytes.clone();
        spaced.insert(1, b' ');
        assert!(decode_scope_snapshot_state(&spaced).is_err());

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_owned(), serde_json::Value::Bool(true));
        let unknown = serde_json::to_vec(&value).unwrap();
        assert!(decode_scope_snapshot_state(&unknown).is_err());
    }
}
