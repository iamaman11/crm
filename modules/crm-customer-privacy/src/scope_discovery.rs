/// Worker-only exact coordinate for Customer Privacy scope discovery.
pub const SCOPE_DISCOVERY_COORDINATE: &str = "customer_privacy.scope.discover@1.0.0";
/// Canonical persisted-state schema for a fully bound discovery snapshot.
pub const DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_ID: &str =
    "crm.customer-privacy.discovery_scope_snapshot.state";
/// Immutable persisted-state schema version for discovery snapshots.
pub const DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_VERSION: &str = "1.0.0";
/// Governed maximum encoded size for a discovery snapshot.
pub const DISCOVERY_SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES: u64 = 512 * 1024;
/// Retention policy for immutable discovery snapshot evidence.
pub const DISCOVERY_SCOPE_SNAPSHOT_STATE_RETENTION_POLICY_ID: &str =
    "crm.customer_privacy.discovery_scope_snapshot";

const DISCOVERY_SCOPE_SNAPSHOT_STATE_DESCRIPTOR: &[u8] = b"crm.customer-privacy.discovery_scope_snapshot.state/v1:snapshot_id,privacy_case_id,tenant_id,canonical_party_id,identity_resolution_generation_decimal,registry_version,registry_digest,purpose_code,effective_request_at_unix_ms_decimal,captured_at_unix_nanos_decimal,aggregation,binding_digest";
const DISCOVERY_SNAPSHOT_ID_PREFIX: &str = "privacy-discovery-scope-";
const MAXIMUM_DISCOVERY_PURPOSE_CODE_BYTES: usize = 96;

/// Immutable lineage shared by every owner page and the final discovery snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeDiscoveryLineage {
    privacy_case_id: RecordId,
    tenant_id: TenantId,
    canonical_party_id: RecordId,
    identity_resolution_generation: u64,
    registry_version: SchemaVersion,
    registry_digest: [u8; 32],
    purpose_code: String,
    effective_request_at_unix_ms: i64,
}

impl ScopeDiscoveryLineage {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        privacy_case_id: RecordId,
        tenant_id: TenantId,
        canonical_party_id: RecordId,
        identity_resolution_generation: u64,
        registry_version: SchemaVersion,
        registry_digest: [u8; 32],
        purpose_code: impl Into<String>,
        effective_request_at_unix_ms: i64,
    ) -> Result<Self, PrivacyScopeError> {
        if identity_resolution_generation == 0 {
            return Err(PrivacyScopeError::InvalidArgument {
                field: "identity_resolution_generation",
                safe_message: "identity-resolution generation must be positive",
            });
        }
        if registry_digest.iter().all(|byte| *byte == 0) {
            return Err(PrivacyScopeError::InvalidArgument {
                field: "registry_digest",
                safe_message: "registry digest must not be all zeroes",
            });
        }
        let purpose_code = purpose_code.into();
        if !valid_discovery_purpose_code(&purpose_code) {
            return Err(PrivacyScopeError::InvalidArgument {
                field: "purpose_code",
                safe_message: "discovery purpose code is invalid",
            });
        }
        if effective_request_at_unix_ms <= 0 {
            return Err(PrivacyScopeError::InvalidArgument {
                field: "effective_request_at_unix_ms",
                safe_message: "effective request time must be positive",
            });
        }
        Ok(Self {
            privacy_case_id,
            tenant_id,
            canonical_party_id,
            identity_resolution_generation,
            registry_version,
            registry_digest,
            purpose_code,
            effective_request_at_unix_ms,
        })
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

    pub fn registry_version(&self) -> &SchemaVersion {
        &self.registry_version
    }

    pub const fn registry_digest(&self) -> &[u8; 32] {
        &self.registry_digest
    }

    pub fn purpose_code(&self) -> &str {
        &self.purpose_code
    }

    pub const fn effective_request_at_unix_ms(&self) -> i64 {
        self.effective_request_at_unix_ms
    }
}

/// One owner contribution bound to the complete discovery lineage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryOwnerScopeContribution {
    lineage: ScopeDiscoveryLineage,
    contribution: OwnerScopeContribution,
    digest: [u8; 32],
}

impl DiscoveryOwnerScopeContribution {
    pub fn new(
        lineage: ScopeDiscoveryLineage,
        contribution: OwnerScopeContribution,
    ) -> Result<Self, PrivacyScopeError> {
        if contribution.tenant_id() != lineage.tenant_id()
            || contribution.canonical_party_id() != lineage.canonical_party_id()
            || contribution.identity_resolution_generation()
                != lineage.identity_resolution_generation()
        {
            return Err(PrivacyScopeError::LineageMismatch {
                owner_module_id: contribution.contract().owner_module_id().clone(),
            });
        }
        let digest = discovery_owner_contribution_digest(&lineage, &contribution);
        Ok(Self {
            lineage,
            contribution,
            digest,
        })
    }

    pub fn lineage(&self) -> &ScopeDiscoveryLineage {
        &self.lineage
    }

    pub fn contribution(&self) -> &OwnerScopeContribution {
        &self.contribution
    }

    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

/// Immutable discovery snapshot binding the existing deterministic aggregation
/// to the privacy case purpose and effective request time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryScopeSnapshot {
    snapshot_id: RecordId,
    lineage: ScopeDiscoveryLineage,
    captured_at_unix_nanos: i64,
    contributions: Vec<DiscoveryOwnerScopeContribution>,
    aggregation: ScopeSnapshot,
    binding_digest: [u8; 32],
}

impl DiscoveryScopeSnapshot {
    pub fn finalize(
        lineage: ScopeDiscoveryLineage,
        registry: OwnerScopeRegistry,
        captured_at_unix_nanos: i64,
        contributions: impl IntoIterator<Item = DiscoveryOwnerScopeContribution>,
    ) -> Result<Self, PrivacyScopeError> {
        let effective_request_at_unix_nanos = lineage
            .effective_request_at_unix_ms
            .checked_mul(1_000_000)
            .ok_or(PrivacyScopeError::InvalidArgument {
                field: "effective_request_at_unix_ms",
                safe_message: "effective request time exceeds the supported range",
            })?;
        if captured_at_unix_nanos < effective_request_at_unix_nanos {
            return Err(PrivacyScopeError::InvalidArgument {
                field: "captured_at_unix_nanos",
                safe_message: "snapshot capture time precedes the effective request time",
            });
        }
        if registry.registry_version() != lineage.registry_version()
            || registry.digest() != lineage.registry_digest()
        {
            return Err(PrivacyScopeError::RegistryConflict {
                safe_message: "discovery lineage does not match the active owner registry",
            });
        }

        let mut by_owner = BTreeMap::new();
        for contribution in contributions {
            let owner = contribution
                .contribution
                .contract()
                .owner_module_id()
                .clone();
            if contribution.lineage != lineage {
                return Err(PrivacyScopeError::LineageMismatch {
                    owner_module_id: owner,
                });
            }
            if by_owner.insert(owner.clone(), contribution).is_some() {
                return Err(PrivacyScopeError::ContributionConflict {
                    owner_module_id: owner,
                    safe_message: "owner contributed more than once to discovery",
                });
            }
        }

        let aggregation = ScopeSnapshot::finalize(
            lineage.privacy_case_id.clone(),
            lineage.tenant_id.clone(),
            lineage.canonical_party_id.clone(),
            lineage.identity_resolution_generation,
            registry,
            captured_at_unix_nanos,
            by_owner
                .values()
                .map(|contribution| contribution.contribution.clone()),
        )?;

        let mut normalized = Vec::with_capacity(aggregation.contributions().len());
        for contribution in aggregation.contributions() {
            let owner = contribution.contract().owner_module_id();
            let wrapped = by_owner.remove(owner).ok_or_else(|| PrivacyScopeError::Incomplete {
                owner_module_id: Some(owner.clone()),
                safe_message: "bound owner contribution is missing",
            })?;
            normalized.push(wrapped);
        }
        if let Some((owner, _)) = by_owner.into_iter().next() {
            return Err(PrivacyScopeError::ContributionConflict {
                owner_module_id: owner,
                safe_message: "unregistered owner contributed to discovery",
            });
        }

        let binding_digest = discovery_snapshot_binding_digest(
            &lineage,
            captured_at_unix_nanos,
            &aggregation,
            &normalized,
        );
        let snapshot_id = RecordId::try_new(format!(
            "{DISCOVERY_SNAPSHOT_ID_PREFIX}{}",
            hex_encode(&binding_digest)
        ))
        .map_err(|_| PrivacyScopeError::InvalidArgument {
            field: "snapshot_id",
            safe_message: "derived discovery snapshot id is invalid",
        })?;

        Ok(Self {
            snapshot_id,
            lineage,
            captured_at_unix_nanos,
            contributions: normalized,
            aggregation,
            binding_digest,
        })
    }

    pub fn snapshot_id(&self) -> &RecordId {
        &self.snapshot_id
    }

    pub fn lineage(&self) -> &ScopeDiscoveryLineage {
        &self.lineage
    }

    pub const fn captured_at_unix_nanos(&self) -> i64 {
        self.captured_at_unix_nanos
    }

    pub fn contributions(&self) -> &[DiscoveryOwnerScopeContribution] {
        &self.contributions
    }

    pub fn aggregation(&self) -> &ScopeSnapshot {
        &self.aggregation
    }

    pub const fn binding_digest(&self) -> &[u8; 32] {
        &self.binding_digest
    }
}


include!("scope_discovery_state.rs");
include!("scope_discovery_tests.rs");
