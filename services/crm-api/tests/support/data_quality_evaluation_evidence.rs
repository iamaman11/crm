use super::data_quality_evaluation_fixture::TENANT;
use crm_core_data::{PostgresDataStore, RecordGetQuery};
use crm_module_sdk::{ModuleId, RecordId, RecordType, TenantId};
use serde_json::Value;
use sqlx::PgPool;

pub struct PersistedEvidence {
    pub version: i64,
    pub json: Value,
    pub bytes: Vec<u8>,
}

pub async fn load_record(
    store: &PostgresDataStore,
    record_type: &str,
    record_id: &str,
) -> PersistedEvidence {
    let snapshot = store
        .get_record_for_query(&RecordGetQuery {
            tenant_id: TenantId::try_new(TENANT).unwrap(),
            owner_module_id: ModuleId::try_new("crm.data-quality").unwrap(),
            record_type: RecordType::try_new(record_type).unwrap(),
            record_id: RecordId::try_new(record_id).unwrap(),
        })
        .await
        .expect("load evaluation evidence record")
        .unwrap_or_else(|| panic!("missing evaluation evidence {record_type}/{record_id}"));
    let bytes = snapshot.payload.bytes;
    let json = serde_json::from_slice(&bytes).expect("decode profiled evaluation JSON");
    PersistedEvidence {
        version: snapshot.version,
        json,
        bytes,
    }
}

pub async fn record_count(admin: &PgPool, record_type: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.records
          WHERE tenant_id = $1 AND owner_module_id = 'crm.data-quality'
            AND record_type = $2",
    )
    .bind(TENANT)
    .bind(record_type)
    .fetch_one(admin)
    .await
    .expect("count evaluation records")
}

pub async fn event_count(admin: &PgPool, event_type: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.outbox_events
          WHERE tenant_id = $1 AND event_type = $2",
    )
    .bind(TENANT)
    .bind(event_type)
    .fetch_one(admin)
    .await
    .expect("count evaluation events")
}

pub async fn audit_count(admin: &PgPool, capability_id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM crm.audit_records
          WHERE tenant_id = $1 AND capability_id = $2",
    )
    .bind(TENANT)
    .bind(capability_id)
    .fetch_one(admin)
    .await
    .expect("count evaluation audits")
}
