pub fn discovery_scope_snapshot_state_descriptor_hash() -> [u8; 32] {
    Sha256::digest(DISCOVERY_SCOPE_SNAPSHOT_STATE_DESCRIPTOR).into()
}

pub fn encode_discovery_scope_snapshot_state(
    snapshot: &DiscoveryScopeSnapshot,
) -> Result<Vec<u8>, SdkError> {
    let bytes = persisted_state_json::to_vec(&DiscoveryScopeSnapshotStateV1::from(snapshot))
        .map_err(|error| {
            persisted_error(format!(
                "discovery scope snapshot serialization failed: {error}"
            ))
        })?;
    validate_discovery_state_size(&bytes)?;
    Ok(bytes)
}

pub fn decode_discovery_scope_snapshot_state(
    bytes: &[u8],
) -> Result<DiscoveryScopeSnapshot, SdkError> {
    validate_discovery_state_size(bytes)?;
    let state: DiscoveryScopeSnapshotStateV1 = persisted_state_json::from_slice(bytes).map_err(
        |error| persisted_error(format!("discovery scope snapshot JSON is invalid: {error}")),
    )?;
    let snapshot = state.into_domain()?;
    if encode_discovery_scope_snapshot_state(&snapshot)? != bytes {
        return Err(persisted_error(
            "persisted discovery scope snapshot is not the strict canonical v1 encoding",
        ));
    }
    Ok(snapshot)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DiscoveryScopeSnapshotStateV1 {
    snapshot_id: String,
    privacy_case_id: String,
    tenant_id: String,
    canonical_party_id: String,
    identity_resolution_generation: String,
    registry_version: String,
    registry_digest: String,
    purpose_code: String,
    effective_request_at_unix_ms: String,
    captured_at_unix_nanos: String,
    aggregation: ScopeSnapshotStateV1,
    binding_digest: String,
}

impl From<&DiscoveryScopeSnapshot> for DiscoveryScopeSnapshotStateV1 {
    fn from(snapshot: &DiscoveryScopeSnapshot) -> Self {
        Self {
            snapshot_id: snapshot.snapshot_id.as_str().to_owned(),
            privacy_case_id: snapshot.lineage.privacy_case_id.as_str().to_owned(),
            tenant_id: snapshot.lineage.tenant_id.as_str().to_owned(),
            canonical_party_id: snapshot.lineage.canonical_party_id.as_str().to_owned(),
            identity_resolution_generation: snapshot
                .lineage
                .identity_resolution_generation
                .to_string(),
            registry_version: snapshot.lineage.registry_version.as_str().to_owned(),
            registry_digest: hex_encode(&snapshot.lineage.registry_digest),
            purpose_code: snapshot.lineage.purpose_code.clone(),
            effective_request_at_unix_ms: snapshot
                .lineage
                .effective_request_at_unix_ms
                .to_string(),
            captured_at_unix_nanos: snapshot.captured_at_unix_nanos.to_string(),
            aggregation: ScopeSnapshotStateV1::from(&snapshot.aggregation),
            binding_digest: hex_encode(&snapshot.binding_digest),
        }
    }
}

impl DiscoveryScopeSnapshotStateV1 {
    fn into_domain(self) -> Result<DiscoveryScopeSnapshot, SdkError> {
        let expected_snapshot_id = self.snapshot_id;
        let expected_binding_digest = hex_decode(&self.binding_digest, "binding_digest")?;
        let lineage = ScopeDiscoveryLineage::new(
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
            SchemaVersion::try_new(self.registry_version)
                .map_err(|error| persisted_error(format!("registry version is invalid: {error}")))?,
            hex_decode(&self.registry_digest, "registry_digest")?,
            self.purpose_code,
            decimal_i64(
                self.effective_request_at_unix_ms,
                "effective_request_at_unix_ms",
            )?,
        )
        .map_err(scope_error)?;
        let captured_at_unix_nanos =
            decimal_i64(self.captured_at_unix_nanos, "captured_at_unix_nanos")?;
        let decoded_aggregation = self.aggregation.into_domain()?;
        let contributions = decoded_aggregation
            .contributions()
            .iter()
            .cloned()
            .map(|contribution| {
                DiscoveryOwnerScopeContribution::new(lineage.clone(), contribution)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(scope_error)?;
        let snapshot = DiscoveryScopeSnapshot::finalize(
            lineage,
            decoded_aggregation.registry().clone(),
            captured_at_unix_nanos,
            contributions,
        )
        .map_err(scope_error)?;
        if snapshot.aggregation != decoded_aggregation {
            return Err(persisted_error(
                "persisted discovery lineage does not match its aggregation snapshot",
            ));
        }
        if snapshot.snapshot_id.as_str() != expected_snapshot_id {
            return Err(persisted_error(
                "persisted discovery snapshot id does not match deterministic content",
            ));
        }
        if snapshot.binding_digest != expected_binding_digest {
            return Err(persisted_error(
                "persisted discovery binding digest does not match deterministic content",
            ));
        }
        Ok(snapshot)
    }
}

fn valid_discovery_purpose_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAXIMUM_DISCOVERY_PURPOSE_CODE_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

fn discovery_lineage_digest(lineage: &ScopeDiscoveryLineage) -> [u8; 32] {
    let mut hasher = framed_hasher(b"crm.customer-privacy.discovery-lineage/v1");
    hash_field(&mut hasher, lineage.privacy_case_id.as_str().as_bytes());
    hash_field(&mut hasher, lineage.tenant_id.as_str().as_bytes());
    hash_field(&mut hasher, lineage.canonical_party_id.as_str().as_bytes());
    hash_field(
        &mut hasher,
        lineage
            .identity_resolution_generation
            .to_string()
            .as_bytes(),
    );
    hash_field(&mut hasher, lineage.registry_version.as_str().as_bytes());
    hash_field(&mut hasher, &lineage.registry_digest);
    hash_field(&mut hasher, lineage.purpose_code.as_bytes());
    hash_field(
        &mut hasher,
        &lineage.effective_request_at_unix_ms.to_be_bytes(),
    );
    hasher.finalize().into()
}

fn discovery_owner_contribution_digest(
    lineage: &ScopeDiscoveryLineage,
    contribution: &OwnerScopeContribution,
) -> [u8; 32] {
    let mut hasher = framed_hasher(b"crm.customer-privacy.discovery-owner-contribution/v1");
    hash_field(&mut hasher, &discovery_lineage_digest(lineage));
    hash_field(&mut hasher, contribution.digest());
    hasher.finalize().into()
}

fn discovery_snapshot_binding_digest(
    lineage: &ScopeDiscoveryLineage,
    captured_at_unix_nanos: i64,
    aggregation: &ScopeSnapshot,
    contributions: &[DiscoveryOwnerScopeContribution],
) -> [u8; 32] {
    let mut hasher = framed_hasher(b"crm.customer-privacy.discovery-snapshot/v1");
    hash_field(&mut hasher, &discovery_lineage_digest(lineage));
    hash_field(&mut hasher, &captured_at_unix_nanos.to_be_bytes());
    hash_field(&mut hasher, aggregation.snapshot_id().as_str().as_bytes());
    hash_field(&mut hasher, aggregation.completeness_digest());
    for contribution in contributions {
        hash_field(&mut hasher, contribution.digest());
    }
    hasher.finalize().into()
}

fn validate_discovery_state_size(bytes: &[u8]) -> Result<(), SdkError> {
    if bytes.len() as u64 > DISCOVERY_SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES {
        return Err(persisted_error(
            "discovery scope snapshot exceeds its governed maximum size",
        ));
    }
    Ok(())
}

