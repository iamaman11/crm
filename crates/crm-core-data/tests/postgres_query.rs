#![cfg(feature = "postgres-integration")]

use crm_core_data::{
    AuditIntent, BatchMutationPlan, EventEvidence, IdempotencyEvidence, PostgresDataStore,
    RecordGetQuery, RecordListQuery, RecordMutation, RecordQuerySort,
};
use crm_module_sdk::{
    ActorId, BusinessTransactionId, CapabilityId, CapabilityVersion, CausationId, CorrelationId,
    DataClass, DomainEvent, EventType, ExecutionContext, IdempotencyKey, ModuleExecutionContext,
    ModuleId, PayloadEncoding, RecordId, RecordRef, RecordType, RequestId, RetentionPolicyId,
    SchemaId, SchemaVersion, TenantId, TraceId, TypedPayload,
};
use sqlx::{PgPool, Row};
use std::collections::BTreeSet;

const TENANT: &str = "tenant-a";
const OTHER_TENANT: &str = "tenant-b";
const ACTOR: &str = "actor-a";
const OWNER: &str = "crm.test";
const RECORD_TYPE: &str = "test.query_record";
const TRANSACTION_ID: &str = "phase6h-query-seed-tx";
const REQUEST_STARTED_AT: i64 = 1_700_000_400_000_000_000;
const RECORD_IDS: [&str; 5] = [
    "phase6h-query-001",
    "phase6h-query-002",
    "phase6h-query-003",
    "phase6h-query-004",
    "phase6h-query-005",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvidenceCounts {
    records: i64,
    outbox: i64,
    audits: i64,
    idempotency: i64,
    transactions: i64,
}

#[tokio::test(flavor = "current_thread")]
async fn tenant_scoped_get_and_keyset_list_are_stable_and_read_only() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping PostgreSQL query acceptance because DATABASE_URL is absent");
        return;
    };
    let admin_database_url = std::env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must accompany DATABASE_URL");
    let store = PostgresDataStore::connect(&database_url, 4)
        .await
        .expect("connect runtime query store");
    let admin = PgPool::connect(&admin_database_url)
        .await
        .expect("connect query evidence reader");

    let seed = seed_plan();
    let seeded = store.execute_batch(&seed).await.expect("seed query records");
    assert!(!seeded.replayed);
    assert_eq!(seeded.records.len(), RECORD_IDS.len());

    let before_reads = evidence_counts(&admin).await;

    let existing = store
        .get_record_for_query(&RecordGetQuery {
            tenant_id: tenant(TENANT),
            owner_module_id: module(OWNER),
            record_type: record_type(RECORD_TYPE),
            record_id: record_id(RECORD_IDS[2]),
        })
        .await
        .expect("get existing tenant record")
        .expect("existing query record");
    assert_eq!(existing.reference.record_id.as_str(), RECORD_IDS[2]);
    assert_eq!(existing.version, 1);
    assert_eq!(existing.payload.bytes, RECORD_IDS[2].as_bytes());

    let missing = store
        .get_record_for_query(&RecordGetQuery {
            tenant_id: tenant(TENANT),
            owner_module_id: module(OWNER),
            record_type: record_type(RECORD_TYPE),
            record_id: record_id("phase6h-query-missing"),
        })
        .await
        .expect("get missing tenant record");
    assert!(missing.is_none());

    let foreign_tenant = store
        .get_record_for_query(&RecordGetQuery {
            tenant_id: tenant(OTHER_TENANT),
            owner_module_id: module(OWNER),
            record_type: record_type(RECORD_TYPE),
            record_id: record_id(RECORD_IDS[2]),
        })
        .await
        .expect("cross-tenant query remains non-disclosing");
    assert!(foreign_tenant.is_none());

    let wrong_owner = store
        .get_record_for_query(&RecordGetQuery {
            tenant_id: tenant(TENANT),
            owner_module_id: module("crm.sales"),
            record_type: record_type(RECORD_TYPE),
            record_id: record_id(RECORD_IDS[2]),
        })
        .await
        .expect("owner-isolated query");
    assert!(wrong_owner.is_none());

    let wrong_type = store
        .get_record_for_query(&RecordGetQuery {
            tenant_id: tenant(TENANT),
            owner_module_id: module(OWNER),
            record_type: record_type("test.other_record"),
            record_id: record_id(RECORD_IDS[2]),
        })
        .await
        .expect("record-type-isolated query");
    assert!(wrong_type.is_none());

    for sort in [
        RecordQuerySort::CreatedAtAscending,
        RecordQuerySort::UpdatedAtDescending,
    ] {
        let baseline = store
            .list_records_for_query(&RecordListQuery {
                tenant_id: tenant(TENANT),
                owner_module_id: module(OWNER),
                record_type: record_type(RECORD_TYPE),
                page_size: 100,
                sort,
                after: None,
            })
            .await
            .expect("read complete stable baseline");
        assert_eq!(baseline.records.len(), RECORD_IDS.len());
        assert!(baseline.next.is_none());
        let expected = record_ids(&baseline.records);

        let mut after = None;
        let mut paged = Vec::new();
        loop {
            let page = store
                .list_records_for_query(&RecordListQuery {
                    tenant_id: tenant(TENANT),
                    owner_module_id: module(OWNER),
                    record_type: record_type(RECORD_TYPE),
                    page_size: 2,
                    sort,
                    after,
                })
                .await
                .expect("read stable keyset page");
            paged.extend(record_ids(&page.records));
            match page.next {
                Some(next) => after = Some(next),
                None => break,
            }
        }

        assert_eq!(paged, expected);
        assert_eq!(paged.len(), RECORD_IDS.len());
        assert_eq!(paged.iter().collect::<BTreeSet<_>>().len(), RECORD_IDS.len());
    }

    let other_tenant_page = store
        .list_records_for_query(&RecordListQuery {
            tenant_id: tenant(OTHER_TENANT),
            owner_module_id: module(OWNER),
            record_type: record_type(RECORD_TYPE),
            page_size: 10,
            sort: RecordQuerySort::CreatedAtAscending,
            after: None,
        })
        .await
        .expect("cross-tenant list remains non-disclosing");
    assert!(other_tenant_page.records.is_empty());
    assert!(other_tenant_page.next.is_none());

    assert_eq!(evidence_counts(&admin).await, before_reads);
}

fn seed_plan() -> BatchMutationPlan {
    let context = ModuleExecutionContext {
        module_id: module(OWNER),
        execution: ExecutionContext {
            tenant_id: tenant(TENANT),
            actor_id: ActorId::try_new(ACTOR).unwrap(),
            request_id: RequestId::try_new("phase6h-query-seed-request").unwrap(),
            correlation_id: CorrelationId::try_new("phase6h-query-seed-correlation").unwrap(),
            causation_id: CausationId::try_new("phase6h-query-seed-causation").unwrap(),
            trace_id: TraceId::try_new("phase6h-query-seed-trace").unwrap(),
            capability_id: CapabilityId::try_new("test.record.mutate").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            idempotency_key: IdempotencyKey::try_new("phase6h-query-seed-idempotency").unwrap(),
            business_transaction_id: BusinessTransactionId::try_new(TRANSACTION_ID).unwrap(),
            schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
            request_started_at_unix_nanos: REQUEST_STARTED_AT,
        },
    };

    let records = RECORD_IDS
        .iter()
        .map(|value| RecordMutation::Create {
            reference: reference(value),
            payload: payload(value),
        })
        .collect::<Vec<_>>();
    let events = RECORD_IDS
        .iter()
        .enumerate()
        .map(|(index, value)| EventEvidence {
            event_id: format!("phase6h-query-event-{index}"),
            event: DomainEvent {
                event_type: EventType::try_new("test.query_record.created").unwrap(),
                aggregate: reference(value),
                expected_aggregate_version: None,
                payload: payload(value),
                deduplication_key: format!("phase6h-query-created-{index}"),
            },
            aggregate_version: 1,
            event_sequence: 1,
            occurred_at_unix_nanos: REQUEST_STARTED_AT + index as i64 + 1,
        })
        .collect();

    BatchMutationPlan {
        context,
        records,
        relationships: Vec::new(),
        events,
        idempotency: IdempotencyEvidence {
            scope: "test.record.mutate@1.0.0".to_owned(),
            key: "phase6h-query-seed-idempotency".to_owned(),
            request_hash: [0x61; 32],
            expires_at_unix_nanos: REQUEST_STARTED_AT + 86_400_000_000_000,
        },
        audits: vec![AuditIntent {
            audit_record_id: "phase6h-query-seed-audit".to_owned(),
            canonicalization_profile: "crm.cjson/v1".to_owned(),
            canonical_envelope: br#"{"phase":"6h","operation":"query-seed"}"#.to_vec(),
            occurred_at_unix_nanos: REQUEST_STARTED_AT + 10,
        }],
    }
}

fn reference(value: &str) -> RecordRef {
    RecordRef {
        record_type: record_type(RECORD_TYPE),
        record_id: record_id(value),
    }
}

fn payload(value: &str) -> TypedPayload {
    TypedPayload {
        owner: module(OWNER),
        schema_id: SchemaId::try_new("crm.test.query_record.v1").unwrap(),
        schema_version: SchemaVersion::try_new("1.0.0").unwrap(),
        descriptor_hash: [0x51; 32],
        data_class: DataClass::Internal,
        encoding: PayloadEncoding::Protobuf,
        maximum_size_bytes: 128,
        retention_policy_id: RetentionPolicyId::try_new("standard").unwrap(),
        bytes: value.as_bytes().to_vec(),
    }
}

fn record_ids(records: &[crm_module_sdk::RecordSnapshot]) -> Vec<String> {
    records
        .iter()
        .map(|record| record.reference.record_id.as_str().to_owned())
        .collect()
}

fn tenant(value: &str) -> TenantId {
    TenantId::try_new(value).unwrap()
}

fn module(value: &str) -> ModuleId {
    ModuleId::try_new(value).unwrap()
}

fn record_type(value: &str) -> RecordType {
    RecordType::try_new(value).unwrap()
}

fn record_id(value: &str) -> RecordId {
    RecordId::try_new(value).unwrap()
}

async fn evidence_counts(pool: &PgPool) -> EvidenceCounts {
    EvidenceCounts {
        records: scalar_count(
            pool,
            "SELECT count(*) FROM crm.records WHERE tenant_id = $1 AND record_type = $2",
        )
        .await,
        outbox: scalar_count(
            pool,
            "SELECT count(*) FROM crm.outbox_events WHERE tenant_id = $1 AND aggregate_type = $2",
        )
        .await,
        audits: scalar_count(
            pool,
            "SELECT count(*) FROM crm.audit_records WHERE tenant_id = $1 AND business_transaction_id = $2",
        )
        .await,
        idempotency: scalar_count(
            pool,
            "SELECT count(*) FROM crm.idempotency_records WHERE tenant_id = $1 AND business_transaction_id = $2",
        )
        .await,
        transactions: scalar_count(
            pool,
            "SELECT count(*) FROM crm.business_transactions WHERE tenant_id = $1 AND business_transaction_id = $2",
        )
        .await,
    }
}

async fn scalar_count(pool: &PgPool, query: &'static str) -> i64 {
    let second = if query.contains("record_type") || query.contains("aggregate_type") {
        RECORD_TYPE
    } else {
        TRANSACTION_ID
    };
    sqlx::query(query)
        .bind(TENANT)
        .bind(second)
        .fetch_one(pool)
        .await
        .expect("count query acceptance evidence")
        .try_get(0)
        .expect("valid query acceptance count")
}
