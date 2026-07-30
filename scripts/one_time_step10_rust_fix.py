from __future__ import annotations

from pathlib import Path


def replace_exact(path: str, old: str, new: str, label: str, expected: int = 1) -> None:
    target = Path(path)
    text = target.read_text(encoding="utf-8")
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{label}: found {count}, expected {expected}")
    target.write_text(text.replace(old, new), encoding="utf-8")


source = "crates/crm-customer-privacy-postgres/src/access_export.rs"
replace_exact(
    source,
    'const ACCESS_EXPORT_AUDIT_DOMAIN: &[u8] = b"crm.customer-privacy.access-export-audit/v1";\n',
    '''const ACCESS_EXPORT_AUDIT_DOMAIN: &[u8] = b"crm.customer-privacy.access-export-audit/v1";
const ACCESS_EXPORT_TRANSACTION_DOMAIN: &[u8] =
    b"crm.customer-privacy.access-export-transaction/v1";
const ACCESS_EXPORT_REQUEST_HASH_DOMAIN: &[u8] =
    b"crm.customer-privacy.access-export-request/v1";
const ACCESS_EXPORT_EVENT_SCHEMA_ID: &str =
    "crm.customer-privacy.access_export_reference.event";
const ACCESS_EXPORT_EVENT_SCHEMA_VERSION: &str = "1.0.0";
const ACCESS_EXPORT_EVENT_DESCRIPTOR: &[u8] =
    b"crm.customer-privacy.access_export_reference.event/v1:reference_state";
const ACCESS_EXPORT_PREPARED_EVENT_TYPE: &str =
    "customer_privacy.access_export.internal.reference_prepared";
const ACCESS_EXPORT_COMPLETED_EVENT_TYPE: &str =
    "customer_privacy.access_export.internal.reference_completed";
const ACCESS_EXPORT_IDEMPOTENCY_SCOPE_PREFIX: &str =
    "customer_privacy.access_export.reference";
const AUDIT_CANONICALIZATION_PROFILE: &str = "crm.cjson/v1";
const AUDIT_LOCK_NAMESPACE: i64 = 0x4352_4d41_5544_4954;
''',
    "transaction evidence constants",
)
replace_exact(
    source,
    '''            insert_reference(&mut transaction, &prepared, invocation).await?;
            append_access_export_audit(
                &mut transaction,
                invocation,
                "access_export_prepared",
                &prepared,
                invocation.request_started_at_unix_nanos,
            )
            .await?;
''',
    '''            let business_transaction_id =
                access_export_transaction_id(invocation, "prepared", &prepared);
            bind_business_transaction(&mut transaction, &business_transaction_id).await?;
            insert_reference(
                &mut transaction,
                &prepared,
                invocation,
                &business_transaction_id,
            )
            .await?;
            append_access_export_audit(
                &mut transaction,
                invocation,
                "access_export_prepared",
                &prepared,
                invocation.request_started_at_unix_nanos,
            )
            .await?;
            insert_access_export_transaction_evidence(
                &mut transaction,
                invocation,
                "prepared",
                ACCESS_EXPORT_PREPARED_EVENT_TYPE,
                &prepared,
                1,
                invocation.request_started_at_unix_nanos,
                &business_transaction_id,
            )
            .await?;
''',
    "prepare transactional evidence",
)
replace_exact(
    source,
    '''            update_reference(&mut transaction, prepared, &completed, invocation).await?;
            append_access_export_audit(
                &mut transaction,
                invocation,
                "access_export_completed",
                &completed,
                result.completed_at_unix_nanos,
            )
            .await?;
''',
    '''            let business_transaction_id =
                access_export_transaction_id(invocation, "completed", &completed);
            bind_business_transaction(&mut transaction, &business_transaction_id).await?;
            update_reference(
                &mut transaction,
                prepared,
                &completed,
                invocation,
                &business_transaction_id,
            )
            .await?;
            append_access_export_audit(
                &mut transaction,
                invocation,
                "access_export_completed",
                &completed,
                result.completed_at_unix_nanos,
            )
            .await?;
            insert_access_export_transaction_evidence(
                &mut transaction,
                invocation,
                "completed",
                ACCESS_EXPORT_COMPLETED_EVENT_TYPE,
                &completed,
                2,
                result.completed_at_unix_nanos,
                &business_transaction_id,
            )
            .await?;
''',
    "completion transactional evidence",
)
replace_exact(
    source,
    '''async fn insert_reference(
    transaction: &mut Transaction<'_, Postgres>,
    reference: &PrivacyAccessExportReference,
    invocation: &AccessExportInvocation,
) -> Result<(), SdkError> {
''',
    '''async fn insert_reference(
    transaction: &mut Transaction<'_, Postgres>,
    reference: &PrivacyAccessExportReference,
    invocation: &AccessExportInvocation,
    business_transaction_id: &str,
) -> Result<(), SdkError> {
''',
    "insert reference transaction id",
)
replace_exact(
    source,
    '    .bind(transaction_id(invocation))\n    .execute(&mut **transaction)\n',
    '    .bind(business_transaction_id)\n    .execute(&mut **transaction)\n',
    "insert reference business transaction binding",
)
replace_exact(
    source,
    '''async fn update_reference(
    transaction: &mut Transaction<'_, Postgres>,
    prepared: &PrivacyAccessExportReference,
    completed: &PrivacyAccessExportReference,
    invocation: &AccessExportInvocation,
) -> Result<(), SdkError> {
''',
    '''async fn update_reference(
    transaction: &mut Transaction<'_, Postgres>,
    prepared: &PrivacyAccessExportReference,
    completed: &PrivacyAccessExportReference,
    invocation: &AccessExportInvocation,
    business_transaction_id: &str,
) -> Result<(), SdkError> {
''',
    "update reference transaction id",
)
replace_exact(
    source,
    '    .bind(transaction_id(invocation))\n    .execute(&mut **transaction)\n',
    '    .bind(business_transaction_id)\n    .execute(&mut **transaction)\n',
    "update reference business transaction binding",
)
replace_exact(
    source,
    '''async fn append_access_export_audit(
''',
    '''#[allow(clippy::too_many_arguments)]
async fn insert_access_export_transaction_evidence(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &AccessExportInvocation,
    phase: &str,
    event_type: &str,
    reference: &PrivacyAccessExportReference,
    aggregate_version: i64,
    occurred_at_unix_nanos: i64,
    business_transaction_id: &str,
) -> Result<(), SdkError> {
    if aggregate_version <= 0 {
        return Err(access_export_evidence_invalid(
            "access export aggregate version must be positive",
        ));
    }
    let payload = encode_access_export_reference(reference)?;
    let request_hash = access_export_request_hash(
        invocation,
        phase,
        reference,
        business_transaction_id,
        &payload,
    );
    let suffix = &hex(&discovery_sha256(business_transaction_id.as_bytes()))[..24];
    let event_id = format!("privacy-access-export-event-{suffix}");
    let audit_id = format!("privacy-access-export-audit-{suffix}");
    let idempotency_scope = format!("{ACCESS_EXPORT_IDEMPOTENCY_SCOPE_PREFIX}.{phase}");

    postgres_sqlx::query(
        r#"
        INSERT INTO crm.idempotency_records (
          tenant_id, idempotency_scope, idempotency_key, request_hash,
          status, business_transaction_id, expires_at
        ) VALUES ($1,$2,$3,$4,'completed',$5,clock_timestamp() + INTERVAL '24 hours')
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(&idempotency_scope)
    .bind(business_transaction_id)
    .bind(request_hash.as_slice())
    .bind(business_transaction_id)
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;

    postgres_sqlx::query(
        r#"
        INSERT INTO crm.outbox_events (
          tenant_id, event_id, business_transaction_id,
          aggregate_type, aggregate_id, aggregate_version, event_sequence,
          event_type, deduplication_key, schema_id, schema_version, descriptor_hash,
          data_class, payload_encoding, maximum_payload_size, retention_policy_id,
          payload_bytes, occurred_at
        ) VALUES (
          $1,$2,$3,$4,$5,$6,$6,$7,$8,$9,$10,$11,
          'confidential','json',$12,$13,$14,
          TIMESTAMPTZ 'epoch' + ($15::bigint / 1000) * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(&event_id)
    .bind(business_transaction_id)
    .bind(ACCESS_EXPORT_REFERENCE_RECORD_TYPE)
    .bind(reference.reference_id().as_str())
    .bind(aggregate_version)
    .bind(event_type)
    .bind(&event_id)
    .bind(ACCESS_EXPORT_EVENT_SCHEMA_ID)
    .bind(ACCESS_EXPORT_EVENT_SCHEMA_VERSION)
    .bind(discovery_sha256(ACCESS_EXPORT_EVENT_DESCRIPTOR).as_slice())
    .bind(checked_i64(
        ACCESS_EXPORT_STATE_MAXIMUM_BYTES,
        "access export event maximum payload size",
    )?)
    .bind(ACCESS_EXPORT_STATE_RETENTION_POLICY_ID)
    .bind(payload.as_slice())
    .bind(occurred_at_unix_nanos)
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;

    let _audit_lock =
        postgres_sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, $2))")
            .bind(invocation.tenant_id.as_str())
            .bind(AUDIT_LOCK_NAMESPACE)
            .fetch_one(&mut **transaction)
            .await
            .map_err(database_error)?;
    let head = postgres_sqlx::query(
        "SELECT next_sequence, last_hash FROM crm.audit_heads WHERE tenant_id = $1",
    )
    .bind(invocation.tenant_id.as_str())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(database_error)?;
    let (sequence, previous_hash) = match head {
        Some(row) => {
            let sequence: i64 = row.try_get("next_sequence").map_err(database_error)?;
            if sequence <= 0 {
                return Err(access_export_evidence_invalid(
                    "tenant audit next sequence must be positive",
                ));
            }
            let previous_hash = row
                .try_get::<Vec<u8>, _>("last_hash")
                .map_err(database_error)?
                .try_into()
                .map_err(|_| {
                    access_export_evidence_invalid("tenant audit hash must contain 32 bytes")
                })?;
            (sequence, previous_hash)
        }
        None => (1, [0; 32]),
    };
    let occurred_at = (occurred_at_unix_nanos / 1_000) * 1_000;
    let audit_hash = access_export_transaction_audit_hash(
        invocation,
        sequence,
        previous_hash,
        &audit_id,
        business_transaction_id,
        &payload,
        occurred_at,
    );
    postgres_sqlx::query(
        r#"
        INSERT INTO crm.audit_records (
          tenant_id, audit_sequence, audit_record_id, business_transaction_id,
          actor_id, capability_id, capability_version, canonicalization_profile,
          previous_hash, record_hash, canonical_envelope, occurred_at
        ) VALUES (
          $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,
          TIMESTAMPTZ 'epoch' + ($12::bigint / 1000) * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(sequence)
    .bind(&audit_id)
    .bind(business_transaction_id)
    .bind(invocation.actor_id.as_str())
    .bind(invocation.initiating_capability_id.as_str())
    .bind(invocation.initiating_capability_version.as_str())
    .bind(AUDIT_CANONICALIZATION_PROFILE)
    .bind(previous_hash.as_slice())
    .bind(audit_hash.as_slice())
    .bind(payload.as_slice())
    .bind(occurred_at)
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;

    postgres_sqlx::query(
        r#"
        INSERT INTO crm.business_transactions (
          tenant_id, business_transaction_id, actor_id, request_id,
          correlation_id, trace_id, capability_id, capability_version,
          expected_outbox_events, expected_audit_records, expected_idempotency_records
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,1,1,1)
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(business_transaction_id)
    .bind(invocation.actor_id.as_str())
    .bind(invocation.request_id.as_str())
    .bind(invocation.correlation_id.as_str())
    .bind(invocation.trace_id.as_str())
    .bind(invocation.initiating_capability_id.as_str())
    .bind(invocation.initiating_capability_version.as_str())
    .execute(&mut **transaction)
    .await
    .map_err(database_error)?;
    Ok(())
}

async fn append_access_export_audit(
''',
    "transaction evidence helper",
)
replace_exact(
    source,
    '''               set_config('app.capability_id', $4, true),
               set_config('app.capability_version', $5, true),
               set_config('app.business_transaction_id', $6, true)
''',
    '''               set_config('app.capability_id', $4, true),
               set_config('app.capability_version', $5, true)
''',
    "defer business transaction binding",
)
replace_exact(
    source,
    '''    .bind(invocation.initiating_capability_version.as_str())
    .bind(transaction_id(invocation))
    .execute(&mut **transaction)
''',
    '''    .bind(invocation.initiating_capability_version.as_str())
    .execute(&mut **transaction)
''',
    "remove provisional transaction id",
)
replace_exact(
    source,
    '''async fn lock_customer_subject(
''',
    '''async fn bind_business_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    business_transaction_id: &str,
) -> Result<(), SdkError> {
    postgres_sqlx::query("SELECT set_config('app.business_transaction_id', $1, true)")
        .bind(business_transaction_id)
        .execute(&mut **transaction)
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn lock_customer_subject(
''',
    "business transaction binder",
)
replace_exact(
    source,
    '''fn transaction_id(invocation: &AccessExportInvocation) -> String {
    let digest = discovery_sha256(invocation.request_id.as_str().as_bytes());
    format!("privacy-access-export-{}", hex(&digest[..12]))
}
''',
    '''fn access_export_transaction_id(
    invocation: &AccessExportInvocation,
    phase: &str,
    reference: &PrivacyAccessExportReference,
) -> String {
    let mut bytes = Vec::new();
    for field in [
        ACCESS_EXPORT_TRANSACTION_DOMAIN,
        invocation.tenant_id.as_str().as_bytes(),
        invocation.privacy_case_id.as_str().as_bytes(),
        invocation.action_plan_id.as_str().as_bytes(),
        phase.as_bytes(),
        reference.reference_id().as_str().as_bytes(),
        reference.digest().as_slice(),
    ] {
        append_digest_field(&mut bytes, field);
    }
    let digest = discovery_sha256(&bytes);
    format!("privacy-access-export-{phase}-{}", hex(&digest[..12]))
}

fn access_export_request_hash(
    invocation: &AccessExportInvocation,
    phase: &str,
    reference: &PrivacyAccessExportReference,
    business_transaction_id: &str,
    payload: &[u8],
) -> [u8; 32] {
    let mut bytes = Vec::new();
    for field in [
        ACCESS_EXPORT_REQUEST_HASH_DOMAIN,
        invocation.tenant_id.as_str().as_bytes(),
        invocation.privacy_case_id.as_str().as_bytes(),
        invocation.action_plan_id.as_str().as_bytes(),
        phase.as_bytes(),
        reference.reference_id().as_str().as_bytes(),
        reference.digest().as_slice(),
        business_transaction_id.as_bytes(),
        payload,
    ] {
        append_digest_field(&mut bytes, field);
    }
    discovery_sha256(&bytes)
}

fn access_export_transaction_audit_hash(
    invocation: &AccessExportInvocation,
    sequence: i64,
    previous_hash: [u8; 32],
    audit_id: &str,
    business_transaction_id: &str,
    canonical_envelope: &[u8],
    occurred_at_unix_nanos: i64,
) -> [u8; 32] {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"crm.audit.record.sha256/v1");
    append_digest_field(&mut bytes, invocation.tenant_id.as_str().as_bytes());
    bytes.extend_from_slice(&sequence.to_be_bytes());
    for field in [
        audit_id.as_bytes(),
        business_transaction_id.as_bytes(),
        invocation.actor_id.as_str().as_bytes(),
        invocation.initiating_capability_id.as_str().as_bytes(),
        invocation.initiating_capability_version.as_str().as_bytes(),
        AUDIT_CANONICALIZATION_PROFILE.as_bytes(),
    ] {
        append_digest_field(&mut bytes, field);
    }
    bytes.extend_from_slice(&previous_hash);
    append_digest_field(&mut bytes, canonical_envelope);
    bytes.extend_from_slice(&occurred_at_unix_nanos.to_be_bytes());
    discovery_sha256(&bytes)
}
''',
    "phase-specific transaction identity",
)


test = "crates/crm-application-runtime/tests/customer_privacy_access_export_postgres.rs"
replace_exact(
    test,
    '''    assert_eq!(
        reference_version(&admin, TENANT_A, prepared.reference_id().as_str()).await,
        1
    );

    let preparation_replay = persistence
''',
    '''    assert_eq!(
        reference_version(&admin, TENANT_A, prepared.reference_id().as_str()).await,
        1
    );
    assert_eq!(
        customer_privacy_access_export_evidence_counts(&admin, TENANT_A).await,
        (1, 1, 1, 1)
    );

    let preparation_replay = persistence
''',
    "prepared evidence assertion",
)
replace_exact(
    test,
    '''        AccessExportPreparation::Complete { .. } => {
            panic!("prepared reference must not become complete without target evidence")
        }
    }

    let file_store = Arc::new(MemoryFileStore::default());
''',
    '''        AccessExportPreparation::Complete { .. } => {
            panic!("prepared reference must not become complete without target evidence")
        }
    }
    assert_eq!(
        customer_privacy_access_export_evidence_counts(&admin, TENANT_A).await,
        (1, 1, 1, 1)
    );

    let file_store = Arc::new(MemoryFileStore::default());
''',
    "prepare replay evidence stability",
)
replace_exact(
    test,
    '''    assert_eq!(
        reference_version(&admin, TENANT_A, completed.reference_id().as_str()).await,
        2
    );

    let (completion_replay, completed_now) = persistence
''',
    '''    assert_eq!(
        reference_version(&admin, TENANT_A, completed.reference_id().as_str()).await,
        2
    );
    assert_eq!(
        customer_privacy_access_export_evidence_counts(&admin, TENANT_A).await,
        (2, 2, 2, 2)
    );

    let (completion_replay, completed_now) = persistence
''',
    "completed evidence assertion",
)
replace_exact(
    test,
    '''    assert!(!completed_now);
    assert_eq!(completion_replay, completed);

    let final_replay = persistence
''',
    '''    assert!(!completed_now);
    assert_eq!(completion_replay, completed);
    assert_eq!(
        customer_privacy_access_export_evidence_counts(&admin, TENANT_A).await,
        (2, 2, 2, 2)
    );

    let final_replay = persistence
''',
    "completion replay evidence stability",
)
replace_exact(
    test,
    '''    assert_eq!(reference_count(&admin, TENANT_B).await, 0);

    let mut conflicting = application_target;
''',
    '''    assert_eq!(reference_count(&admin, TENANT_B).await, 0);
    assert_eq!(
        customer_privacy_access_export_evidence_counts(&admin, TENANT_B).await,
        (0, 0, 0, 0)
    );

    let mut conflicting = application_target;
''',
    "cross-tenant evidence concealment",
)
replace_exact(
    test,
    '''async fn cleanup(admin: &PgPool) {
''',
    '''async fn customer_privacy_access_export_evidence_counts(
    admin: &PgPool,
    tenant: &str,
) -> (i64, i64, i64, i64) {
    sqlx::query_as(
        r#"
        SELECT
          (SELECT count(*) FROM crm.business_transactions
             WHERE tenant_id = $1
               AND capability_id = 'customer_privacy.access_export.request'),
          (SELECT count(*) FROM crm.outbox_events
             WHERE tenant_id = $1
               AND event_type LIKE 'customer_privacy.access_export.internal.%'),
          (SELECT count(*) FROM crm.audit_records
             WHERE tenant_id = $1
               AND capability_id = 'customer_privacy.access_export.request'),
          (SELECT count(*) FROM crm.idempotency_records
             WHERE tenant_id = $1
               AND idempotency_scope LIKE 'customer_privacy.access_export.reference.%')
        "#,
    )
    .bind(tenant)
    .fetch_one(admin)
    .await
    .expect("count Customer Privacy access-export transaction evidence")
}

async fn cleanup(admin: &PgPool) {
''',
    "transaction evidence count helper",
)
