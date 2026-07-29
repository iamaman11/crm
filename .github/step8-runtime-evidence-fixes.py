from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


def replace_last(text: str, old: str, new: str, label: str) -> str:
    position = text.rfind(old)
    if position < 0:
        raise SystemExit(f"{label}: match not found")
    return text[:position] + new + text[position + len(old):]


path = Path("crates/crm-customer-privacy-postgres/src/execution.rs")
text = path.read_text()
text = replace_once(
    text,
    "const MAXIMUM_ITEMS: usize = 16_384;\n",
    """const MAXIMUM_ITEMS: usize = 16_384;
const EXECUTION_CASE_EVENT_TYPE: &str =
    "customer_privacy.owner_execution.case_transitioned";
const EXECUTION_CASE_IDEMPOTENCY_SCOPE: &str =
    "customer_privacy.owner_execution.case-transition";
const AUDIT_CANONICALIZATION_PROFILE: &str = "crm.cjson/v1";
const AUDIT_LOCK_NAMESPACE: i64 = 0x4352_4d41_5544_4954;
""",
    "governed case-transition evidence constants",
)
text = replace_once(
    text,
    """    let payload = privacy_case_persisted_payload(privacy_case)?;""",
    """    let business_transaction_id =
        case_transition_transaction_id(invocation, privacy_case.version());
    bind_business_transaction(transaction, &business_transaction_id).await?;
    let payload = privacy_case_persisted_payload(privacy_case)?;""",
    "case transition transaction binding",
)
text = replace_last(
    text,
    """    .bind(payload.bytes)
    .bind(transaction_id(invocation))
    .bind(checked_i64(expected_version, "expected case version")?)""",
    """    .bind(payload.bytes.as_slice())
    .bind(&business_transaction_id)
    .bind(checked_i64(expected_version, "expected case version")?)""",
    "case transition business transaction value",
)
text = replace_once(
    text,
    """    if result.rows_affected() != 1 {
        return Err(execution_conflict(
            "privacy case changed before owner execution could commit",
        ));
    }
    Ok(())
}""",
    """    if result.rows_affected() != 1 {
        return Err(execution_conflict(
            "privacy case changed before owner execution could commit",
        ));
    }
    insert_case_transition_evidence(
        transaction,
        invocation,
        privacy_case,
        &payload,
        &business_transaction_id,
    )
    .await?;
    Ok(())
}""",
    "governed case transition evidence",
)
text = replace_once(
    text,
    """fn transaction_id(invocation: &OwnerExecutionInvocation) -> String {""",
    """async fn bind_business_transaction(
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

async fn insert_case_transition_evidence(
    transaction: &mut Transaction<'_, Postgres>,
    invocation: &OwnerExecutionInvocation,
    privacy_case: &PrivacyCase,
    payload: &TypedPayload,
    business_transaction_id: &str,
) -> Result<(), SdkError> {
    let resulting_version =
        checked_i64(privacy_case.version(), "case transition version")?;
    let maximum =
        checked_i64(payload.maximum_size_bytes, "case transition maximum payload size")?;
    let suffix = &hex(&discovery_sha256(business_transaction_id.as_bytes()))[..24];
    let event_id = format!("privacy-owner-execution-event-{suffix}");
    let audit_id = format!("privacy-owner-execution-audit-{suffix}");
    let request_hash =
        case_transition_request_hash(invocation, privacy_case, payload, business_transaction_id);

    postgres_sqlx::query(
        r#"
        INSERT INTO crm.idempotency_records (
          tenant_id, idempotency_scope, idempotency_key, request_hash,
          status, business_transaction_id, expires_at
        ) VALUES ($1,$2,$3,$4,'completed',$5,clock_timestamp() + INTERVAL '24 hours')
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(EXECUTION_CASE_IDEMPOTENCY_SCOPE)
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
          $12,'json',$13,$14,$15,
          TIMESTAMPTZ 'epoch' + ($16::bigint / 1000) * INTERVAL '1 microsecond'
        )
        "#,
    )
    .bind(invocation.tenant_id.as_str())
    .bind(&event_id)
    .bind(business_transaction_id)
    .bind(PRIVACY_CASE_RECORD_TYPE)
    .bind(privacy_case.case_id().as_str())
    .bind(resulting_version)
    .bind(EXECUTION_CASE_EVENT_TYPE)
    .bind(&event_id)
    .bind(payload.schema_id.as_str())
    .bind(payload.schema_version.as_str())
    .bind(payload.descriptor_hash.as_slice())
    .bind(data_class_label(payload.data_class))
    .bind(maximum)
    .bind(payload.retention_policy_id.as_str())
    .bind(payload.bytes.as_slice())
    .bind(invocation.planned_at_unix_nanos)
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
            let sequence = row
                .try_get::<i64, _>("next_sequence")
                .map_err(database_error)?;
            if sequence <= 0 {
                return Err(evidence_invalid(
                    "tenant audit next sequence must be positive",
                ));
            }
            let previous_hash = row
                .try_get::<Vec<u8>, _>("last_hash")
                .map_err(database_error)?
                .try_into()
                .map_err(|_| evidence_invalid("tenant audit hash must contain 32 bytes"))?;
            (sequence, previous_hash)
        }
        None => (1, [0; 32]),
    };
    let occurred_at = (invocation.planned_at_unix_nanos / 1_000) * 1_000;
    let audit_hash = case_transition_audit_hash(
        invocation,
        sequence,
        previous_hash,
        &audit_id,
        business_transaction_id,
        payload.bytes.as_slice(),
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
    .bind(payload.bytes.as_slice())
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

fn case_transition_request_hash(
    invocation: &OwnerExecutionInvocation,
    privacy_case: &PrivacyCase,
    payload: &TypedPayload,
    business_transaction_id: &str,
) -> [u8; 32] {
    let resulting_version = privacy_case.version().to_be_bytes();
    let mut bytes = Vec::new();
    for value in [
        b"crm.customer-privacy.owner-execution.case-transition-request/v1".as_slice(),
        invocation.tenant_id.as_str().as_bytes(),
        invocation.privacy_case_id.as_str().as_bytes(),
        invocation.action_plan_id.as_str().as_bytes(),
        invocation.retention_decision_id.as_str().as_bytes(),
        invocation.actor_id.as_str().as_bytes(),
        invocation.request_id.as_str().as_bytes(),
        invocation.correlation_id.as_str().as_bytes(),
        invocation.trace_id.as_str().as_bytes(),
        invocation.initiating_capability_id.as_str().as_bytes(),
        invocation.initiating_capability_version.as_str().as_bytes(),
        business_transaction_id.as_bytes(),
        resulting_version.as_slice(),
        payload.bytes.as_slice(),
    ] {
        append_digest_field(&mut bytes, value);
    }
    discovery_sha256(&bytes)
}

fn case_transition_audit_hash(
    invocation: &OwnerExecutionInvocation,
    sequence: i64,
    previous_hash: [u8; 32],
    audit_id: &str,
    business_transaction_id: &str,
    canonical_envelope: &[u8],
    occurred_at: i64,
) -> [u8; 32] {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"crm.audit.record.sha256/v1");
    append_digest_field(&mut bytes, invocation.tenant_id.as_str().as_bytes());
    bytes.extend_from_slice(&sequence.to_be_bytes());
    for value in [
        audit_id.as_bytes(),
        business_transaction_id.as_bytes(),
        invocation.actor_id.as_str().as_bytes(),
        invocation.initiating_capability_id.as_str().as_bytes(),
        invocation.initiating_capability_version.as_str().as_bytes(),
        AUDIT_CANONICALIZATION_PROFILE.as_bytes(),
    ] {
        append_digest_field(&mut bytes, value);
    }
    bytes.extend_from_slice(&previous_hash);
    append_digest_field(&mut bytes, canonical_envelope);
    bytes.extend_from_slice(&occurred_at.to_be_bytes());
    discovery_sha256(&bytes)
}

fn case_transition_transaction_id(
    invocation: &OwnerExecutionInvocation,
    resulting_case_version: u64,
) -> String {
    let resulting_case_version = resulting_case_version.to_be_bytes();
    let mut bytes = Vec::new();
    for value in [
        b"crm.customer-privacy.owner-execution.case-transition/v1".as_slice(),
        invocation.tenant_id.as_str().as_bytes(),
        invocation.privacy_case_id.as_str().as_bytes(),
        invocation.initiating_capability_id.as_str().as_bytes(),
        invocation.initiating_capability_version.as_str().as_bytes(),
        invocation.request_id.as_str().as_bytes(),
        resulting_case_version.as_slice(),
    ] {
        append_digest_field(&mut bytes, value);
    }
    let digest = discovery_sha256(&bytes);
    format!("privacy-owner-transition-{}", hex(&digest[..12]))
}

fn transaction_id(invocation: &OwnerExecutionInvocation) -> String {""",
    "governed case-transition evidence helpers",
)
path.write_text(text)
