use crm_core_data::{PostgresDataStore, postgres_sqlx::{self, Postgres, Row, Transaction}};
use crm_customer_privacy::{
    DISCOVERY_SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES,
    DISCOVERY_SCOPE_SNAPSHOT_STATE_RETENTION_POLICY_ID,
    DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_ID,
    DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_VERSION, DiscoveryScopeSnapshot, MODULE_ID,
    SCOPE_SNAPSHOT_RECORD_TYPE, decode_discovery_scope_snapshot_state,
    discovery_attempt_digest as _, discovery_scope_snapshot_state_descriptor_hash,
    discovery_sha256, encode_discovery_scope_snapshot_state,
};
use crm_customer_privacy_application::{
    DiscoveryAttempt, DiscoveryAuditRecord, DiscoveryInvocation, DiscoveryPageReceipt,
    DiscoveryPersistencePort, PersistedDiscoveryPage, discovery_attempt_digest,
};
use crm_module_sdk::{
    CapabilityId, CapabilityVersion, ErrorCategory, ModuleId, PortFuture, RecordId, SdkError,
    TenantId,
};
use std::sync::Arc;

const EXPECTED_OWNER_COUNT: i64 = 9;

#[derive(Debug, Clone)]
pub struct PostgresDiscoveryPersistence {
    store: Arc<PostgresDataStore>,
}

impl PostgresDiscoveryPersistence {
    pub fn new(store: Arc<PostgresDataStore>) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &Arc<PostgresDataStore> {
        &self.store
    }
}

impl DiscoveryPersistencePort for PostgresDiscoveryPersistence {
    fn begin_attempt<'a>(
        &'a self,
        invocation: &'a DiscoveryInvocation,
        expected_attempt_digest: [u8; 32],
    ) -> PortFuture<'a, Result<DiscoveryAttempt, SdkError>> {
        Box::pin(async move {
            if discovery_attempt_digest(&invocation.lineage) != expected_attempt_digest {
                return Err(evidence_conflict("application attempt digest is inconsistent"));
            }
            let generation = i64::try_from(invocation.lineage.identity_resolution_generation())
                .map_err(|_| evidence_conflict("identity generation exceeds PostgreSQL range"))?;
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_invocation(&mut transaction, invocation).await?;
            postgres_sqlx::query(
                r#"
                INSERT INTO crm.customer_privacy_discovery_attempts (
                  tenant_id, attempt_digest, privacy_case_id, canonical_party_id,
                  identity_resolution_generation, registry_version, registry_digest,
                  purpose_code, effective_request_at_unix_ms, captured_at_unix_nanos
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(invocation.lineage.tenant_id().as_str())
            .bind(expected_attempt_digest.as_slice())
            .bind(invocation.lineage.privacy_case_id().as_str())
            .bind(invocation.lineage.canonical_party_id().as_str())
            .bind(generation)
            .bind(invocation.lineage.registry_version().as_str())
            .bind(invocation.lineage.registry_digest().as_slice())
            .bind(invocation.lineage.purpose_code())
            .bind(invocation.lineage.effective_request_at_unix_ms())
            .bind(invocation.proposed_captured_at_unix_nanos)
            .execute(&mut *transaction)
            .await
            .map_err(database_error)?;

            let row = postgres_sqlx::query(
                r#"
                SELECT privacy_case_id, canonical_party_id, identity_resolution_generation,
                       registry_version, registry_digest, purpose_code,
                       effective_request_at_unix_ms, captured_at_unix_nanos
                FROM crm.customer_privacy_discovery_attempts
                WHERE tenant_id = $1 AND attempt_digest = $2
                "#,
            )
            .bind(invocation.lineage.tenant_id().as_str())
            .bind(expected_attempt_digest.as_slice())
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error)?;
            let registry_digest = digest_column(&row, "registry_digest")?;
            let persisted_generation: i64 = row.try_get("identity_resolution_generation").map_err(database_error)?;
            let persisted_capture: i64 = row.try_get("captured_at_unix_nanos").map_err(database_error)?;
            if row.try_get::<String, _>("privacy_case_id").map_err(database_error)?
                != invocation.lineage.privacy_case_id().as_str()
                || row.try_get::<String, _>("canonical_party_id").map_err(database_error)?
                    != invocation.lineage.canonical_party_id().as_str()
                || persisted_generation != generation
                || row.try_get::<String, _>("registry_version").map_err(database_error)?
                    != invocation.lineage.registry_version().as_str()
                || registry_digest != *invocation.lineage.registry_digest()
                || row.try_get::<String, _>("purpose_code").map_err(database_error)?
                    != invocation.lineage.purpose_code()
                || row.try_get::<i64, _>("effective_request_at_unix_ms").map_err(database_error)?
                    != invocation.lineage.effective_request_at_unix_ms()
                || persisted_capture <= 0
            {
                return Err(evidence_conflict("persisted discovery attempt lineage conflicts"));
            }
            transaction.commit().await.map_err(database_error)?;
            Ok(DiscoveryAttempt {
                attempt_digest: expected_attempt_digest,
                captured_at_unix_nanos: persisted_capture,
            })
        })
    }

    fn load_owner_pages<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        attempt_digest: [u8; 32],
        owner_module_id: &'a ModuleId,
    ) -> PortFuture<'a, Result<Vec<PersistedDiscoveryPage>, SdkError>> {
        Box::pin(async move {
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_tenant(&mut transaction, tenant_id).await?;
            let rows = postgres_sqlx::query(
                r#"
                SELECT owner_module_id, capability_id, capability_version, lineage_digest,
                       page_number, request_cursor_digest, response_cursor_digest,
                       owner_cursor_digest, page_digest, scanned_resource_count,
                       emitted_resource_count, terminal_complete, response_bytes, response_digest
                FROM crm.customer_privacy_discovery_owner_pages
                WHERE tenant_id = $1 AND attempt_digest = $2 AND owner_module_id = $3
                ORDER BY page_number ASC
                "#,
            )
            .bind(tenant_id.as_str())
            .bind(attempt_digest.as_slice())
            .bind(owner_module_id.as_str())
            .fetch_all(&mut *transaction)
            .await
            .map_err(database_error)?;
            transaction.commit().await.map_err(database_error)?;
            rows.into_iter().map(decode_page).collect()
        })
    }

    fn accept_owner_page<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        attempt_digest: [u8; 32],
        page: &'a PersistedDiscoveryPage,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            if page.response_bytes.len() as u64 > DISCOVERY_SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES {
                return Err(evidence_conflict("owner response exceeds governed payload maximum"));
            }
            let response_digest = discovery_sha256(&page.response_bytes);
            let scanned = i64::try_from(page.receipt.scanned_resource_count)
                .map_err(|_| evidence_conflict("scanned count exceeds PostgreSQL range"))?;
            let emitted = i64::try_from(page.receipt.emitted_resource_count)
                .map_err(|_| evidence_conflict("emitted count exceeds PostgreSQL range"))?;
            let page_number = i32::try_from(page.receipt.page_number)
                .map_err(|_| evidence_conflict("page number exceeds PostgreSQL range"))?;
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_tenant(&mut transaction, tenant_id).await?;
            postgres_sqlx::query(
                r#"
                INSERT INTO crm.customer_privacy_discovery_owner_pages (
                  tenant_id, attempt_digest, owner_module_id, capability_id,
                  capability_version, lineage_digest, page_number,
                  request_cursor_digest, response_cursor_digest, owner_cursor_digest,
                  page_digest, scanned_resource_count, emitted_resource_count,
                  terminal_complete, response_bytes, response_digest
                )
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(tenant_id.as_str())
            .bind(attempt_digest.as_slice())
            .bind(page.receipt.owner_module_id.as_str())
            .bind(page.receipt.capability_id.as_str())
            .bind(page.receipt.capability_version.as_str())
            .bind(page.receipt.lineage_digest.as_slice())
            .bind(page_number)
            .bind(page.receipt.request_cursor_digest.as_slice())
            .bind(page.receipt.response_cursor_digest.as_slice())
            .bind(page.receipt.owner_cursor_digest.as_slice())
            .bind(page.receipt.page_digest.as_slice())
            .bind(scanned)
            .bind(emitted)
            .bind(page.receipt.terminal_complete)
            .bind(page.response_bytes.as_slice())
            .bind(response_digest.as_slice())
            .execute(&mut *transaction)
            .await
            .map_err(database_error)?;
            let row = postgres_sqlx::query(
                r#"
                SELECT owner_module_id, capability_id, capability_version, lineage_digest,
                       page_number, request_cursor_digest, response_cursor_digest,
                       owner_cursor_digest, page_digest, scanned_resource_count,
                       emitted_resource_count, terminal_complete, response_bytes, response_digest
                FROM crm.customer_privacy_discovery_owner_pages
                WHERE tenant_id = $1 AND attempt_digest = $2
                  AND owner_module_id = $3 AND page_number = $4
                "#,
            )
            .bind(tenant_id.as_str())
            .bind(attempt_digest.as_slice())
            .bind(page.receipt.owner_module_id.as_str())
            .bind(page_number)
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error)?;
            let persisted = decode_page(row)?;
            if persisted != *page {
                return Err(evidence_conflict("owner page replay conflicts with durable evidence"));
            }
            transaction.commit().await.map_err(database_error)?;
            Ok(())
        })
    }

    fn advance_checkpoint<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        attempt_digest: [u8; 32],
        owner_module_id: &'a ModuleId,
        contiguous_page_number: u32,
        terminal_complete: bool,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let page_number = i32::try_from(contiguous_page_number)
                .map_err(|_| evidence_conflict("checkpoint page exceeds PostgreSQL range"))?;
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_tenant(&mut transaction, tenant_id).await?;
            let row = postgres_sqlx::query(
                r#"
                SELECT count(*) AS page_count,
                       bool_or(page_number = $4 AND terminal_complete) AS target_terminal
                FROM crm.customer_privacy_discovery_owner_pages
                WHERE tenant_id = $1 AND attempt_digest = $2
                  AND owner_module_id = $3 AND page_number BETWEEN 1 AND $4
                "#,
            )
            .bind(tenant_id.as_str())
            .bind(attempt_digest.as_slice())
            .bind(owner_module_id.as_str())
            .bind(page_number)
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error)?;
            let count: i64 = row.try_get("page_count").map_err(database_error)?;
            let target_terminal: Option<bool> = row.try_get("target_terminal").map_err(database_error)?;
            if count != i64::from(page_number)
                || (terminal_complete && target_terminal != Some(true))
                || (!terminal_complete && target_terminal == Some(true))
            {
                return Err(evidence_conflict(
                    "checkpoint can advance only across a contiguous durable page prefix",
                ));
            }
            postgres_sqlx::query(
                r#"
                INSERT INTO crm.customer_privacy_discovery_checkpoints (
                  tenant_id, attempt_digest, owner_module_id,
                  contiguous_page_number, terminal_complete
                ) VALUES ($1,$2,$3,$4,$5)
                ON CONFLICT (tenant_id, attempt_digest, owner_module_id) DO UPDATE
                SET contiguous_page_number = EXCLUDED.contiguous_page_number,
                    terminal_complete = EXCLUDED.terminal_complete,
                    updated_at = clock_timestamp()
                WHERE crm.customer_privacy_discovery_checkpoints.contiguous_page_number <= EXCLUDED.contiguous_page_number
                  AND NOT crm.customer_privacy_discovery_checkpoints.terminal_complete
                "#,
            )
            .bind(tenant_id.as_str())
            .bind(attempt_digest.as_slice())
            .bind(owner_module_id.as_str())
            .bind(page_number)
            .bind(terminal_complete)
            .execute(&mut *transaction)
            .await
            .map_err(database_error)?;
            let row = postgres_sqlx::query(
                r#"
                SELECT contiguous_page_number, terminal_complete
                FROM crm.customer_privacy_discovery_checkpoints
                WHERE tenant_id = $1 AND attempt_digest = $2 AND owner_module_id = $3
                "#,
            )
            .bind(tenant_id.as_str())
            .bind(attempt_digest.as_slice())
            .bind(owner_module_id.as_str())
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error)?;
            let stored_page: i32 = row.try_get("contiguous_page_number").map_err(database_error)?;
            let stored_terminal: bool = row.try_get("terminal_complete").map_err(database_error)?;
            if stored_page < page_number || (terminal_complete && !stored_terminal) {
                return Err(evidence_conflict("checkpoint replay conflicts with durable state"));
            }
            transaction.commit().await.map_err(database_error)?;
            Ok(())
        })
    }

    fn finalize_snapshot<'a>(
        &'a self,
        attempt: &'a DiscoveryAttempt,
        snapshot: &'a DiscoveryScopeSnapshot,
    ) -> PortFuture<'a, Result<DiscoveryScopeSnapshot, SdkError>> {
        Box::pin(async move {
            if discovery_attempt_digest(snapshot.lineage()) != attempt.attempt_digest
                || snapshot.captured_at_unix_nanos() != attempt.captured_at_unix_nanos
            {
                return Err(evidence_conflict("snapshot does not belong to discovery attempt"));
            }
            let payload = encode_discovery_scope_snapshot_state(snapshot)?;
            let tenant_id = snapshot.lineage().tenant_id();
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_tenant(&mut transaction, tenant_id).await?;
            let row = postgres_sqlx::query(
                r#"
                SELECT count(*) AS owner_count,
                       count(*) FILTER (WHERE terminal_complete) AS terminal_count
                FROM crm.customer_privacy_discovery_checkpoints
                WHERE tenant_id = $1 AND attempt_digest = $2
                "#,
            )
            .bind(tenant_id.as_str())
            .bind(attempt.attempt_digest.as_slice())
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error)?;
            let owner_count: i64 = row.try_get("owner_count").map_err(database_error)?;
            let terminal_count: i64 = row.try_get("terminal_count").map_err(database_error)?;
            if owner_count != EXPECTED_OWNER_COUNT || terminal_count != EXPECTED_OWNER_COUNT {
                return Err(evidence_conflict(
                    "snapshot finalization requires nine terminal owner checkpoints",
                ));
            }
            let transaction_id = format!("privacy-discovery-{}", hex(&attempt.attempt_digest[..12]));
            postgres_sqlx::query(
                r#"
                INSERT INTO crm.records (
                  tenant_id, record_type, record_id, version, owner_module_id,
                  schema_id, schema_version, descriptor_hash, data_class,
                  payload_encoding, maximum_payload_size, retention_policy_id,
                  payload_bytes, last_business_transaction_id
                )
                VALUES ($1,$2,$3,1,$4,$5,$6,$7,'confidential','json',$8,$9,$10,$11)
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(tenant_id.as_str())
            .bind(SCOPE_SNAPSHOT_RECORD_TYPE)
            .bind(snapshot.snapshot_id().as_str())
            .bind(MODULE_ID)
            .bind(DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_ID)
            .bind(DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_VERSION)
            .bind(discovery_scope_snapshot_state_descriptor_hash().as_slice())
            .bind(i64::try_from(DISCOVERY_SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES).unwrap())
            .bind(DISCOVERY_SCOPE_SNAPSHOT_STATE_RETENTION_POLICY_ID)
            .bind(payload.as_slice())
            .bind(transaction_id)
            .execute(&mut *transaction)
            .await
            .map_err(database_error)?;
            let rehydrated = load_snapshot_in_transaction(
                &mut transaction,
                tenant_id,
                snapshot.snapshot_id(),
            )
            .await?
            .ok_or_else(|| evidence_conflict("finalized snapshot record is missing"))?;
            if &rehydrated != snapshot {
                return Err(evidence_conflict(
                    "existing finalized snapshot conflicts with deterministic content",
                ));
            }
            postgres_sqlx::query(
                r#"
                INSERT INTO crm.customer_privacy_discovery_snapshots (
                  tenant_id, attempt_digest, snapshot_id, snapshot_binding_digest
                ) VALUES ($1,$2,$3,$4)
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(tenant_id.as_str())
            .bind(attempt.attempt_digest.as_slice())
            .bind(snapshot.snapshot_id().as_str())
            .bind(snapshot.binding_digest().as_slice())
            .execute(&mut *transaction)
            .await
            .map_err(database_error)?;
            let row = postgres_sqlx::query(
                r#"
                SELECT snapshot_id, snapshot_binding_digest
                FROM crm.customer_privacy_discovery_snapshots
                WHERE tenant_id = $1 AND attempt_digest = $2
                "#,
            )
            .bind(tenant_id.as_str())
            .bind(attempt.attempt_digest.as_slice())
            .fetch_one(&mut *transaction)
            .await
            .map_err(database_error)?;
            if row.try_get::<String, _>("snapshot_id").map_err(database_error)?
                != snapshot.snapshot_id().as_str()
                || digest_column(&row, "snapshot_binding_digest")? != *snapshot.binding_digest()
            {
                return Err(evidence_conflict("snapshot finalization replay conflicts"));
            }
            transaction.commit().await.map_err(database_error)?;
            Ok(rehydrated)
        })
    }

    fn load_snapshot<'a>(
        &'a self,
        tenant_id: &'a TenantId,
        snapshot_id: &'a RecordId,
    ) -> PortFuture<'a, Result<Option<DiscoveryScopeSnapshot>, SdkError>> {
        Box::pin(async move {
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_tenant(&mut transaction, tenant_id).await?;
            let snapshot = load_snapshot_in_transaction(&mut transaction, tenant_id, snapshot_id).await?;
            transaction.commit().await.map_err(database_error)?;
            Ok(snapshot)
        })
    }

    fn record_audit<'a>(
        &'a self,
        record: &'a DiscoveryAuditRecord,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        Box::pin(async move {
            let audit_digest = audit_digest(record);
            let page_number = record
                .page_number
                .map(i32::try_from)
                .transpose()
                .map_err(|_| evidence_conflict("audit page number exceeds PostgreSQL range"))?;
            let safe_count = record
                .count
                .map(i64::try_from)
                .transpose()
                .map_err(|_| evidence_conflict("audit count exceeds PostgreSQL range"))?;
            let mut transaction = self.store.pool().begin().await.map_err(database_error)?;
            bind_tenant(&mut transaction, &record.tenant_id).await?;
            postgres_sqlx::query(
                r#"
                INSERT INTO crm.customer_privacy_discovery_audit (
                  tenant_id, audit_digest, event_type, privacy_case_id, attempt_digest,
                  owner_module_id, page_number, snapshot_id, safe_count,
                  policy_reference, occurred_at
                ) VALUES (
                  $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,
                  TIMESTAMPTZ 'epoch' + ($11::bigint / 1000) * INTERVAL '1 microsecond'
                )
                ON CONFLICT DO NOTHING
                "#,
            )
            .bind(record.tenant_id.as_str())
            .bind(audit_digest.as_slice())
            .bind(record.event.label())
            .bind(record.privacy_case_id.as_str())
            .bind(record.attempt_digest.as_slice())
            .bind(record.owner_module_id.as_ref().map(ModuleId::as_str))
            .bind(page_number)
            .bind(record.snapshot_id.as_ref().map(RecordId::as_str))
            .bind(safe_count)
            .bind(record.policy_reference.as_deref())
            .bind(record.occurred_at_unix_nanos)
            .execute(&mut *transaction)
            .await
            .map_err(database_error)?;
            transaction.commit().await.map_err(database_error)?;
            Ok(())
        })
    }
}

async fn bind_invocation(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &DiscoveryInvocation,
) -> Result<(), SdkError> {
    postgres_sqlx::query(
        r#"
        SELECT set_config('app.tenant_id', $1, true),
               set_config('app.actor_id', $2, true),
               set_config('app.request_id', $3, true),
               set_config('app.capability_id', 'customer_privacy.scope.discover', true),
               set_config('app.capability_version', '1.0.0', true),
               set_config('app.business_transaction_id', $4, true)
        "#,
    )
    .bind(invocation.lineage.tenant_id().as_str())
    .bind(invocation.actor_id.as_str())
    .bind(invocation.request_id.as_str())
    .bind(format!(
        "privacy-discovery-{}",
        hex(&discovery_attempt_digest(&invocation.lineage)[..12])
    ))
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(())
}

async fn bind_tenant(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
) -> Result<(), SdkError> {
    postgres_sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
        .bind(tenant_id.as_str())
        .execute(&mut **transaction)
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn load_snapshot_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    tenant_id: &TenantId,
    snapshot_id: &RecordId,
) -> Result<Option<DiscoveryScopeSnapshot>, SdkError> {
    let row = postgres_sqlx::query(
        r#"
        SELECT version, owner_module_id, schema_id, schema_version, descriptor_hash,
               data_class, payload_encoding, maximum_payload_size,
               retention_policy_id, payload_bytes
        FROM crm.records
        WHERE tenant_id = $1 AND record_type = $2 AND record_id = $3
          AND deleted_at IS NULL
        "#,
    )
    .bind(tenant_id.as_str())
    .bind(SCOPE_SNAPSHOT_RECORD_TYPE)
    .bind(snapshot_id.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?;
    row.map(|row| {
        let descriptor = digest_column(&row, "descriptor_hash")?;
        let maximum: i64 = row.try_get("maximum_payload_size").map_err(database_error)?;
        if row.try_get::<i64, _>("version").map_err(database_error)? != 1
            || row.try_get::<String, _>("owner_module_id").map_err(database_error)? != MODULE_ID
            || row.try_get::<String, _>("schema_id").map_err(database_error)?
                != DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_ID
            || row.try_get::<String, _>("schema_version").map_err(database_error)?
                != DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_VERSION
            || descriptor != discovery_scope_snapshot_state_descriptor_hash()
            || row.try_get::<String, _>("data_class").map_err(database_error)? != "confidential"
            || row.try_get::<String, _>("payload_encoding").map_err(database_error)? != "json"
            || maximum != i64::try_from(DISCOVERY_SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES).unwrap()
            || row.try_get::<String, _>("retention_policy_id").map_err(database_error)?
                != DISCOVERY_SCOPE_SNAPSHOT_STATE_RETENTION_POLICY_ID
        {
            return Err(evidence_conflict("snapshot record envelope drifted"));
        }
        let bytes: Vec<u8> = row.try_get("payload_bytes").map_err(database_error)?;
        let snapshot = decode_discovery_scope_snapshot_state(&bytes)?;
        if snapshot.snapshot_id() != snapshot_id || snapshot.lineage().tenant_id() != tenant_id {
            return Err(evidence_conflict("snapshot payload identity differs from envelope"));
        }
        Ok(snapshot)
    })
    .transpose()
}

fn decode_page(row: postgres_sqlx::postgres::PgRow) -> Result<PersistedDiscoveryPage, SdkError> {
    let response_bytes: Vec<u8> = row.try_get("response_bytes").map_err(database_error)?;
    if digest_column(&row, "response_digest")? != discovery_sha256(&response_bytes) {
        return Err(evidence_conflict("owner response digest mismatch"));
    }
    let page_number: i32 = row.try_get("page_number").map_err(database_error)?;
    let scanned: i64 = row.try_get("scanned_resource_count").map_err(database_error)?;
    let emitted: i64 = row.try_get("emitted_resource_count").map_err(database_error)?;
    Ok(PersistedDiscoveryPage {
        receipt: DiscoveryPageReceipt {
            owner_module_id: ModuleId::try_new(
                row.try_get::<String, _>("owner_module_id").map_err(database_error)?,
            )
            .map_err(identifier_error)?,
            capability_id: CapabilityId::try_new(
                row.try_get::<String, _>("capability_id").map_err(database_error)?,
            )
            .map_err(identifier_error)?,
            capability_version: CapabilityVersion::try_new(
                row.try_get::<String, _>("capability_version").map_err(database_error)?,
            )
            .map_err(identifier_error)?,
            lineage_digest: digest_column(&row, "lineage_digest")?,
            page_number: u32::try_from(page_number)
                .map_err(|_| evidence_conflict("stored page number is invalid"))?,
            request_cursor_digest: digest_column(&row, "request_cursor_digest")?,
            response_cursor_digest: digest_column(&row, "response_cursor_digest")?,
            owner_cursor_digest: digest_column(&row, "owner_cursor_digest")?,
            page_digest: digest_column(&row, "page_digest")?,
            scanned_resource_count: u64::try_from(scanned)
                .map_err(|_| evidence_conflict("stored scanned count is invalid"))?,
            emitted_resource_count: u64::try_from(emitted)
                .map_err(|_| evidence_conflict("stored emitted count is invalid"))?,
            terminal_complete: row.try_get("terminal_complete").map_err(database_error)?,
        },
        response_bytes,
    })
}

fn digest_column(row: &postgres_sqlx::postgres::PgRow, column: &str) -> Result<[u8; 32], SdkError> {
    let bytes: Vec<u8> = row.try_get(column).map_err(database_error)?;
    bytes
        .try_into()
        .map_err(|_| evidence_conflict(format!("stored {column} is not SHA-256")))
}

fn audit_digest(record: &DiscoveryAuditRecord) -> [u8; 32] {
    let mut bytes = Vec::new();
    frame(&mut bytes, b"crm.customer-privacy.discovery-audit/v1");
    frame(&mut bytes, record.tenant_id.as_str().as_bytes());
    frame(&mut bytes, record.event.label().as_bytes());
    frame(&mut bytes, record.privacy_case_id.as_str().as_bytes());
    frame(&mut bytes, &record.attempt_digest);
    frame(
        &mut bytes,
        record
            .owner_module_id
            .as_ref()
            .map(ModuleId::as_str)
            .unwrap_or("")
            .as_bytes(),
    );
    frame(
        &mut bytes,
        &record.page_number.unwrap_or(0).to_be_bytes(),
    );
    frame(
        &mut bytes,
        record
            .snapshot_id
            .as_ref()
            .map(RecordId::as_str)
            .unwrap_or("")
            .as_bytes(),
    );
    frame(&mut bytes, &record.count.unwrap_or(0).to_be_bytes());
    frame(
        &mut bytes,
        record.policy_reference.as_deref().unwrap_or("").as_bytes(),
    );
    frame(&mut bytes, &record.occurred_at_unix_nanos.to_be_bytes());
    discovery_sha256(&bytes)
}

fn frame(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u64).to_be_bytes());
    output.extend_from_slice(value);
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn database_error(error: postgres_sqlx::Error) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_DISCOVERY_STORAGE_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Customer Privacy discovery storage is temporarily unavailable.",
    )
    .with_internal_reference(error.to_string())
}

fn evidence_conflict(reference: impl Into<String>) -> SdkError {
    SdkError::new(
        "CUSTOMER_PRIVACY_DISCOVERY_PERSISTED_EVIDENCE_INVALID",
        ErrorCategory::Conflict,
        false,
        "Customer Privacy discovery evidence failed strict validation.",
    )
    .with_internal_reference(reference)
}

fn identifier_error(error: impl std::fmt::Display) -> SdkError {
    evidence_conflict(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_customer_privacy::{OwnerScopeRegistry, ScopeDiscoveryLineage};
    use crm_module_sdk::{RecordId, SchemaVersion};

    #[test]
    fn audit_digest_excludes_resource_payload_and_is_deterministic() {
        let record = DiscoveryAuditRecord {
            event: crm_customer_privacy_application::DiscoveryAuditEvent::OwnerPageAccepted,
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            privacy_case_id: RecordId::try_new("case-a").unwrap(),
            attempt_digest: [1; 32],
            owner_module_id: Some(ModuleId::try_new("crm.parties").unwrap()),
            page_number: Some(1),
            snapshot_id: None,
            count: Some(2),
            policy_reference: None,
            occurred_at_unix_nanos: 10,
        };
        assert_eq!(audit_digest(&record), audit_digest(&record));
    }

    #[test]
    fn attempt_digest_matches_application_identity() {
        let registry = OwnerScopeRegistry::canonical_v1().unwrap();
        let lineage = ScopeDiscoveryLineage::new(
            RecordId::try_new("case-a").unwrap(),
            TenantId::try_new("tenant-a").unwrap(),
            RecordId::try_new("party-a").unwrap(),
            1,
            SchemaVersion::try_new(registry.registry_version().as_str()).unwrap(),
            *registry.digest(),
            "ERASURE",
            1,
        )
        .unwrap();
        assert_eq!(discovery_attempt_digest(&lineage), discovery_attempt_digest(&lineage));
    }
}
