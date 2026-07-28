#![forbid(unsafe_code)]

use crm_customer_privacy::{
    ACTION_PLAN_RECORD_TYPE, ACTION_PLAN_STATE_MAXIMUM_BYTES,
    ACTION_PLAN_STATE_RETENTION_POLICY_ID, ACTION_PLAN_STATE_SCHEMA_ID,
    ACTION_PLAN_STATE_SCHEMA_VERSION, ActionPlanningPolicy, ContributionCompletenessProof,
    DISCOVERY_SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES,
    DISCOVERY_SCOPE_SNAPSHOT_STATE_RETENTION_POLICY_ID, DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_ID,
    DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_VERSION, DiscoveryOwnerScopeContribution,
    DiscoveryScopeSnapshot, MODULE_ID, OwnerScopeContribution, OwnerScopeRegistry,
    PRIVACY_CASE_RECORD_TYPE, PRIVACY_CASE_STATE_MAXIMUM_BYTES,
    PRIVACY_CASE_STATE_RETENTION_POLICY_ID, PRIVACY_CASE_STATE_SCHEMA_ID,
    PRIVACY_CASE_STATE_SCHEMA_VERSION, PrivacyActionPlan, PrivacyCase, PrivacyCaseKind,
    SCOPE_SNAPSHOT_RECORD_TYPE, ScopeDiscoveryLineage, ScopeResource, SubjectVerificationMethod,
    action_plan_state_descriptor_hash, discovery_scope_snapshot_state_descriptor_hash,
    encode_action_plan_state, encode_discovery_scope_snapshot_state, encode_privacy_case_state,
    privacy_case_state_descriptor_hash,
};
use crm_module_sdk::{ActorId, RecordId, SchemaVersion, TenantId};
use std::fmt::Write as _;

const TENANT_ID: &str = "tenant-a";
const CANONICAL_PARTY_ID: &str = "privacy-approval-canonical-party";
const POLICY_VERSION: &str = "privacy-policy/1";
const BOOTSTRAP_BUSINESS_TRANSACTION_ID: &str = "tx-bootstrap-a";

struct Fixture {
    privacy_case: PrivacyCase,
    snapshot: DiscoveryScopeSnapshot,
    plan: PrivacyActionPlan,
    corrupt_planning_link: bool,
}

struct RecordFixture<'a> {
    record_type: &'a str,
    record_id: &'a str,
    version: i64,
    schema_id: &'a str,
    schema_version: &'a str,
    descriptor_hash: &'a [u8; 32],
    maximum_payload_size: u64,
    retention_policy_id: &'a str,
    payload_bytes: &'a [u8],
}

fn main() {
    let fixtures = [
        build_fixture("success", false),
        build_fixture("stale", false),
        build_fixture("corrupt-link", true),
        build_fixture("inactive", false),
    ];

    let mut sql = String::new();
    writeln!(sql, "\\set ON_ERROR_STOP on").unwrap();
    writeln!(sql, "BEGIN;").unwrap();
    writeln!(
        sql,
        "SELECT set_config('app.tenant_id', '{TENANT_ID}', true);"
    )
    .unwrap();
    writeln!(sql, "SELECT set_config('app.actor_id', 'actor-a', true);").unwrap();
    writeln!(
        sql,
        "SELECT set_config('app.request_id', 'request-bootstrap-a', true);"
    )
    .unwrap();
    writeln!(
        sql,
        "SELECT set_config('app.capability_id', 'test.record.mutate', true);"
    )
    .unwrap();
    writeln!(
        sql,
        "SELECT set_config('app.capability_version', '1.0.0', true);"
    )
    .unwrap();
    writeln!(
        sql,
        "SELECT set_config('app.business_transaction_id', '{BOOTSTRAP_BUSINESS_TRANSACTION_ID}', true);"
    )
    .unwrap();

    for fixture in &fixtures {
        append_fixture_sql(&mut sql, fixture);
    }

    writeln!(sql, "COMMIT;").unwrap();
    print!("{sql}");
}

fn build_fixture(suffix: &str, corrupt_planning_link: bool) -> Fixture {
    let tenant_id = TenantId::try_new(TENANT_ID).unwrap();
    let case_id = RecordId::try_new(format!("privacy-approval-case-{suffix}")).unwrap();
    let canonical_party_id = RecordId::try_new(CANONICAL_PARTY_ID).unwrap();
    let policy_version = SchemaVersion::try_new(POLICY_VERSION).unwrap();

    let mut privacy_case = PrivacyCase::new(
        case_id.clone(),
        tenant_id.clone(),
        PrivacyCaseKind::Erasure,
        policy_version.clone(),
        1_000_000_000,
        None,
    )
    .unwrap();
    privacy_case.submit(1, 2_000_000_000).unwrap();
    privacy_case
        .verify_subject(
            2,
            canonical_party_id.clone(),
            canonical_party_id.clone(),
            1,
            SubjectVerificationMethod::VerifiedDocument,
            ActorId::try_new("privacy-approval-fixture-verifier").unwrap(),
            3_000_000_000,
        )
        .unwrap();
    privacy_case.begin_scoping(3, 4_000_000_000).unwrap();

    let registry = OwnerScopeRegistry::canonical_v1().unwrap();
    let lineage = ScopeDiscoveryLineage::new(
        case_id,
        tenant_id.clone(),
        canonical_party_id.clone(),
        1,
        registry.registry_version().clone(),
        *registry.digest(),
        "ERASURE_REQUEST",
        5_000,
    )
    .unwrap();
    let contributions = registry
        .contracts()
        .iter()
        .enumerate()
        .map(|(index, contract)| {
            let terminal_cursor_digest = [u8::try_from(index + 1).unwrap(); 32];
            let completeness =
                ContributionCompletenessProof::new(true, 1, 0, 0, terminal_cursor_digest).unwrap();
            let contribution = OwnerScopeContribution::new(
                contract.clone(),
                tenant_id.clone(),
                canonical_party_id.clone(),
                1,
                Vec::<ScopeResource>::new(),
                completeness,
            )
            .unwrap();
            DiscoveryOwnerScopeContribution::new(lineage.clone(), contribution).unwrap()
        })
        .collect::<Vec<_>>();
    let snapshot =
        DiscoveryScopeSnapshot::finalize(lineage, registry, 5_000_000_000, contributions).unwrap();
    privacy_case
        .record_scope(4, snapshot.snapshot_id().clone(), 5_000_000_000)
        .unwrap();

    let plan = PrivacyActionPlan::build(
        &snapshot,
        privacy_case.version(),
        privacy_case.kind(),
        ActionPlanningPolicy::new(policy_version, "EU", true, false).unwrap(),
        6_000_000_000,
    )
    .unwrap();
    privacy_case
        .record_plan(5, plan.plan_id().clone(), true, 6_000_000_000)
        .unwrap();

    Fixture {
        privacy_case,
        snapshot,
        plan,
        corrupt_planning_link,
    }
}

fn append_fixture_sql(sql: &mut String, fixture: &Fixture) {
    let case_bytes = encode_privacy_case_state(&fixture.privacy_case).unwrap();
    let snapshot_bytes = encode_discovery_scope_snapshot_state(&fixture.snapshot).unwrap();
    let plan_bytes = encode_action_plan_state(&fixture.plan).unwrap();
    let case_descriptor_hash = privacy_case_state_descriptor_hash();
    let snapshot_descriptor_hash = discovery_scope_snapshot_state_descriptor_hash();
    let plan_descriptor_hash = action_plan_state_descriptor_hash();

    append_record_sql(
        sql,
        RecordFixture {
            record_type: PRIVACY_CASE_RECORD_TYPE,
            record_id: fixture.privacy_case.case_id().as_str(),
            version: i64::try_from(fixture.privacy_case.version()).unwrap(),
            schema_id: PRIVACY_CASE_STATE_SCHEMA_ID,
            schema_version: PRIVACY_CASE_STATE_SCHEMA_VERSION,
            descriptor_hash: &case_descriptor_hash,
            maximum_payload_size: PRIVACY_CASE_STATE_MAXIMUM_BYTES,
            retention_policy_id: PRIVACY_CASE_STATE_RETENTION_POLICY_ID,
            payload_bytes: &case_bytes,
        },
    );
    append_record_sql(
        sql,
        RecordFixture {
            record_type: SCOPE_SNAPSHOT_RECORD_TYPE,
            record_id: fixture.snapshot.snapshot_id().as_str(),
            version: 1,
            schema_id: DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_ID,
            schema_version: DISCOVERY_SCOPE_SNAPSHOT_STATE_SCHEMA_VERSION,
            descriptor_hash: &snapshot_descriptor_hash,
            maximum_payload_size: DISCOVERY_SCOPE_SNAPSHOT_STATE_MAXIMUM_BYTES,
            retention_policy_id: DISCOVERY_SCOPE_SNAPSHOT_STATE_RETENTION_POLICY_ID,
            payload_bytes: &snapshot_bytes,
        },
    );
    append_record_sql(
        sql,
        RecordFixture {
            record_type: ACTION_PLAN_RECORD_TYPE,
            record_id: fixture.plan.plan_id().as_str(),
            version: 1,
            schema_id: ACTION_PLAN_STATE_SCHEMA_ID,
            schema_version: ACTION_PLAN_STATE_SCHEMA_VERSION,
            descriptor_hash: &plan_descriptor_hash,
            maximum_payload_size: ACTION_PLAN_STATE_MAXIMUM_BYTES,
            retention_policy_id: ACTION_PLAN_STATE_RETENTION_POLICY_ID,
            payload_bytes: &plan_bytes,
        },
    );

    let plan_digest = if fixture.corrupt_planning_link {
        [0xee; 32]
    } else {
        *fixture.plan.digest()
    };
    writeln!(
        sql,
        "INSERT INTO crm.customer_privacy_action_plans (tenant_id, privacy_case_id, source_case_version, resulting_case_version, scope_snapshot_id, plan_id, plan_digest, approval_required, planned_at) VALUES ('{TENANT_ID}', '{}', 5, 6, '{}', '{}', decode('{}', 'hex'), true, TIMESTAMPTZ 'epoch' + 6000000 * INTERVAL '1 microsecond');",
        fixture.privacy_case.case_id().as_str(),
        fixture.snapshot.snapshot_id().as_str(),
        fixture.plan.plan_id().as_str(),
        hex(&plan_digest),
    )
    .unwrap();
}

fn append_record_sql(sql: &mut String, record: RecordFixture<'_>) {
    writeln!(
        sql,
        "INSERT INTO crm.records (tenant_id, record_type, record_id, version, owner_module_id, schema_id, schema_version, descriptor_hash, data_class, payload_encoding, maximum_payload_size, retention_policy_id, payload_bytes, last_business_transaction_id) VALUES ('{TENANT_ID}', '{}', '{}', {}, '{MODULE_ID}', '{}', '{}', decode('{}', 'hex'), 'confidential', 'json', {}, '{}', decode('{}', 'hex'), '{BOOTSTRAP_BUSINESS_TRANSACTION_ID}');",
        sql_literal(record.record_type),
        sql_literal(record.record_id),
        record.version,
        sql_literal(record.schema_id),
        sql_literal(record.schema_version),
        hex(record.descriptor_hash),
        record.maximum_payload_size,
        sql_literal(record.retention_policy_id),
        hex(record.payload_bytes),
    )
    .unwrap();
}

fn sql_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
