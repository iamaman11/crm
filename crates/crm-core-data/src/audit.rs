use crm_module_sdk::ModuleExecutionContext;
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use std::error::Error;
use std::fmt;

const AUDIT_LOCK_NAMESPACE: i64 = 0x4352_4d41_5544_4954;
const AUDIT_HASH_DOMAIN: &[u8] = b"crm.audit.record.sha256/v1";
const MAX_AUDIT_RECORD_ID_BYTES: usize = 180;
const MAX_CANONICALIZATION_PROFILE_BYTES: usize = 80;
const MAX_CANONICAL_ENVELOPE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditIntent {
    pub audit_record_id: String,
    pub canonicalization_profile: String,
    pub canonical_envelope: Vec<u8>,
    pub occurred_at_unix_nanos: i64,
}

impl AuditIntent {
    pub fn validate(&self) -> Result<(), String> {
        if self.audit_record_id.is_empty() || self.audit_record_id.len() > MAX_AUDIT_RECORD_ID_BYTES
        {
            return Err(format!(
                "audit record id must contain between 1 and {MAX_AUDIT_RECORD_ID_BYTES} bytes"
            ));
        }
        if self.canonicalization_profile.is_empty()
            || self.canonicalization_profile.len() > MAX_CANONICALIZATION_PROFILE_BYTES
        {
            return Err(format!(
                "audit canonicalization profile must contain between 1 and {MAX_CANONICALIZATION_PROFILE_BYTES} bytes"
            ));
        }
        if self.canonical_envelope.is_empty()
            || self.canonical_envelope.len() > MAX_CANONICAL_ENVELOPE_BYTES
        {
            return Err(format!(
                "audit canonical envelope must contain between 1 and {MAX_CANONICAL_ENVELOPE_BYTES} bytes"
            ));
        }
        if self.occurred_at_unix_nanos <= 0 {
            return Err("audit occurrence time must be positive".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MaterializedAuditRecord {
    pub audit_sequence: i64,
    pub audit_record_id: String,
    pub canonicalization_profile: String,
    pub previous_hash: [u8; 32],
    pub record_hash: [u8; 32],
    pub canonical_envelope: Vec<u8>,
    pub occurred_at_unix_nanos: i64,
}

#[derive(Debug)]
pub(crate) enum AuditMaterializationError {
    Database(sqlx::Error),
    InvalidIntent(String),
    InvalidStoredValue(String),
}

impl fmt::Display for AuditMaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "audit database operation failed: {error}"),
            Self::InvalidIntent(message) => write!(formatter, "invalid audit intent: {message}"),
            Self::InvalidStoredValue(message) => {
                write!(formatter, "invalid stored audit value: {message}")
            }
        }
    }
}

impl Error for AuditMaterializationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::InvalidIntent(_) | Self::InvalidStoredValue(_) => None,
        }
    }
}

impl From<sqlx::Error> for AuditMaterializationError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

pub(crate) async fn materialize_audit_chain(
    transaction: &mut Transaction<'_, Postgres>,
    context: &ModuleExecutionContext,
    intents: &[AuditIntent],
) -> Result<Vec<MaterializedAuditRecord>, AuditMaterializationError> {
    for intent in intents {
        intent
            .validate()
            .map_err(AuditMaterializationError::InvalidIntent)?;
    }

    let _lock_row = sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, $2))")
        .bind(context.execution.tenant_id.as_str())
        .bind(AUDIT_LOCK_NAMESPACE)
        .fetch_one(&mut **transaction)
        .await?;

    let row =
        sqlx::query("SELECT next_sequence, last_hash FROM crm.audit_heads WHERE tenant_id = $1")
            .bind(context.execution.tenant_id.as_str())
            .fetch_optional(&mut **transaction)
            .await?;

    let (mut sequence, mut previous_hash) = match row {
        Some(row) => {
            let sequence: i64 = row.try_get("next_sequence")?;
            if sequence <= 0 {
                return Err(AuditMaterializationError::InvalidStoredValue(
                    "tenant audit next sequence must be positive".to_owned(),
                ));
            }
            let stored_hash: Vec<u8> = row.try_get("last_hash")?;
            let previous_hash = stored_hash.try_into().map_err(|_| {
                AuditMaterializationError::InvalidStoredValue(
                    "tenant audit head hash must contain exactly 32 bytes".to_owned(),
                )
            })?;
            (sequence, previous_hash)
        }
        None => (1, [0; 32]),
    };

    let mut records = Vec::with_capacity(intents.len());
    for intent in intents {
        let occurred_at_unix_nanos = postgres_timestamp_nanos(intent.occurred_at_unix_nanos);
        let record_hash = audit_record_hash(
            context,
            sequence,
            previous_hash,
            intent,
            occurred_at_unix_nanos,
        )?;
        records.push(MaterializedAuditRecord {
            audit_sequence: sequence,
            audit_record_id: intent.audit_record_id.clone(),
            canonicalization_profile: intent.canonicalization_profile.clone(),
            previous_hash,
            record_hash,
            canonical_envelope: intent.canonical_envelope.clone(),
            occurred_at_unix_nanos,
        });
        previous_hash = record_hash;
        sequence = sequence.checked_add(1).ok_or_else(|| {
            AuditMaterializationError::InvalidStoredValue(
                "tenant audit sequence overflowed i64".to_owned(),
            )
        })?;
    }

    Ok(records)
}

fn audit_record_hash(
    context: &ModuleExecutionContext,
    sequence: i64,
    previous_hash: [u8; 32],
    intent: &AuditIntent,
    occurred_at_unix_nanos: i64,
) -> Result<[u8; 32], AuditMaterializationError> {
    let mut hasher = Sha256::new();
    hasher.update(AUDIT_HASH_DOMAIN);
    append_field(&mut hasher, context.execution.tenant_id.as_str().as_bytes())?;
    hasher.update(sequence.to_be_bytes());
    append_field(&mut hasher, intent.audit_record_id.as_bytes())?;
    append_field(
        &mut hasher,
        context
            .execution
            .business_transaction_id
            .as_str()
            .as_bytes(),
    )?;
    append_field(&mut hasher, context.execution.actor_id.as_str().as_bytes())?;
    append_field(
        &mut hasher,
        context.execution.capability_id.as_str().as_bytes(),
    )?;
    append_field(
        &mut hasher,
        context.execution.capability_version.as_str().as_bytes(),
    )?;
    append_field(&mut hasher, intent.canonicalization_profile.as_bytes())?;
    hasher.update(previous_hash);
    append_field(&mut hasher, &intent.canonical_envelope)?;
    hasher.update(occurred_at_unix_nanos.to_be_bytes());
    Ok(hasher.finalize().into())
}

const fn postgres_timestamp_nanos(value: i64) -> i64 {
    (value / 1_000) * 1_000
}

fn append_field(hasher: &mut Sha256, value: &[u8]) -> Result<(), AuditMaterializationError> {
    let length = u64::try_from(value.len()).map_err(|_| {
        AuditMaterializationError::InvalidIntent("audit hash field length exceeds u64".to_owned())
    })?;
    hasher.update(length.to_be_bytes());
    hasher.update(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::{
        ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId,
        CorrelationId, ExecutionContext, IdempotencyKey, ModuleId, RequestId, SchemaVersion,
        TenantId, TraceId,
    };

    fn context() -> ModuleExecutionContext {
        ModuleExecutionContext {
            module_id: ModuleId::try_new("crm.test").unwrap(),
            execution: ExecutionContext {
                tenant_id: TenantId::try_new("tenant-a").unwrap(),
                actor_id: ActorId::try_new("actor-a").unwrap(),
                request_id: RequestId::try_new("request-a").unwrap(),
                correlation_id: CorrelationId::try_new("correlation-a").unwrap(),
                causation_id: CausationId::try_new("causation-a").unwrap(),
                trace_id: TraceId::try_new("trace-a").unwrap(),
                capability_id: CapabilityId::try_new("test.audit.write").unwrap(),
                capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
                business_transaction_id: BusinessTransactionId::try_new("transaction-a").unwrap(),
                idempotency_key: IdempotencyKey::try_new("idempotency-a").unwrap(),
                schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
                request_started_at_unix_nanos: 1,
            },
        }
    }

    fn intent() -> AuditIntent {
        AuditIntent {
            audit_record_id: "audit-a".to_owned(),
            canonicalization_profile: "crm.cjson/v1".to_owned(),
            canonical_envelope: br#"{"action":"create"}"#.to_vec(),
            occurred_at_unix_nanos: 2,
        }
    }

    #[test]
    fn hash_is_deterministic_and_chain_sensitive() {
        let context = context();
        let intent = intent();
        let persisted_time = postgres_timestamp_nanos(intent.occurred_at_unix_nanos);
        let first = audit_record_hash(&context, 1, [0; 32], &intent, persisted_time).unwrap();
        let same = audit_record_hash(&context, 1, [0; 32], &intent, persisted_time).unwrap();
        let chained = audit_record_hash(&context, 2, first, &intent, persisted_time).unwrap();

        assert_eq!(first, same);
        assert_ne!(first, chained);
        assert_ne!(first, [0; 32]);
    }

    #[test]
    fn hash_uses_postgresql_microsecond_timestamp_precision() {
        let context = context();
        let mut first_intent = intent();
        first_intent.occurred_at_unix_nanos = 1_234_567;
        let mut same_persisted_time = first_intent.clone();
        same_persisted_time.occurred_at_unix_nanos = 1_234_999;

        let first_time = postgres_timestamp_nanos(first_intent.occurred_at_unix_nanos);
        let second_time = postgres_timestamp_nanos(same_persisted_time.occurred_at_unix_nanos);
        assert_eq!(first_time, second_time);
        assert_eq!(
            audit_record_hash(&context, 1, [0; 32], &first_intent, first_time).unwrap(),
            audit_record_hash(&context, 1, [0; 32], &same_persisted_time, second_time,).unwrap(),
        );
    }

    #[test]
    fn intent_is_bounded_and_requires_canonical_evidence() {
        assert!(intent().validate().is_ok());
        let mut invalid = intent();
        invalid.canonical_envelope.clear();
        assert!(invalid.validate().is_err());
    }
}
