from __future__ import annotations

import json
import re
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "crates/crm-customer-accounts-privacy-scope-adapter"
TARGET = ROOT / "crates/crm-contact-points-privacy-scope-adapter"


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected exactly one occurrence, found {count}: {old!r}")
    return text.replace(old, new, 1)


def transform_common(text: str) -> str:
    replacements = [
        ("crm-customer-accounts-privacy-scope-adapter", "crm-contact-points-privacy-scope-adapter"),
        ("crm_customer_accounts_privacy_scope_adapter", "crm_contact_points_privacy_scope_adapter"),
        ("crm-customer-accounts-capability-adapter", "crm-contact-points-capability-adapter"),
        ("crm_customer_accounts_capability_adapter", "crm_contact_points_capability_adapter"),
        ("crm-customer-accounts", "crm-contact-points"),
        ("crm_customer_accounts", "crm_contact_points"),
        ("CustomerAccountsPrivacyScopeQueryAdapter", "ContactPointsPrivacyScopeQueryAdapter"),
        ("CustomerAccountsPrivacyScopeContribution", "ContactPointsPrivacyScopeContribution"),
        ("customer_accounts_privacy_scope_definition", "contact_points_privacy_scope_definition"),
        ("CUSTOMER_ACCOUNTS", "CONTACT_POINTS"),
        ("Customer Accounts", "Contact Points"),
        ("customer_accounts", "contact_points"),
        ("VerifiedAccountResource", "VerifiedContactPointResource"),
        ("AccountPage", "ContactPointPage"),
        ("read_account_page", "read_contact_point_page"),
        ("strict_account_resource", "strict_contact_point_resource"),
        ("account_from_snapshot", "contact_point_from_snapshot"),
        ("account_state_descriptor_hash", "contact_point_state_descriptor_hash"),
        ("ACCOUNT_STATE_", "CONTACT_POINT_STATE_"),
        ("CustomerAccountCapabilityPlanner", "ContactPointCapabilityPlanner"),
        ("CREATE_ACCOUNT_CAPABILITY", "CREATE_CONTACT_POINT_CAPABILITY"),
        ("CREATE_ACCOUNT_SCHEMA", "CREATE_CONTACT_POINT_SCHEMA"),
        ("account_definition", "contact_point_definition"),
        ("account_executor", "contact_point_executor"),
        ("create_account", "create_contact_point"),
        ("corrupt_account_metadata", "corrupt_contact_point_metadata"),
        ("accounts::v1 as accounts", "contact_points::v1 as contact_points"),
        ("accounts::", "contact_points::"),
        ("assert_response_omits_account_state", "assert_response_omits_contact_point_state"),
    ]
    for old, new in replacements:
        text = text.replace(old, new)
    return text


def generate_crate() -> None:
    if TARGET.exists():
        print("Contact Points privacy crate already exists; preserving current branch state")
        return

    shutil.copytree(SOURCE, TARGET)
    for path in TARGET.rglob("*"):
        if path.is_file():
            path.write_text(transform_common(path.read_text()))

    postgres = TARGET / "src/postgres.rs"
    text = postgres.read_text()
    start = text.index("    let account = contact_point_from_snapshot")
    end = text.index("\n}\n\nfn configured", start)
    replacement = '''    let contact_point = contact_point_from_snapshot(&snapshot)
        .map_err(|error| stored_state_invalid(format!("{}: {}", error.code, error.safe_message)))?;
    if contact_point.party_ref().as_str() != canonical_party_id.as_str() {
        return Ok(None);
    }
    let resource_version = u64::try_from(contact_point.version())
        .map_err(|_| stored_state_invalid("persisted Contact Point version must be positive"))?;
    Ok(Some(VerifiedContactPointResource {
        record_id: record_id.clone(),
        resource_version,
    }))'''
    text = text[:start] + replacement + text[end:]
    text = text.replace("persisted Account metadata", "persisted Contact Point metadata")
    text = text.replace("Account privacy scope", "Contact Point privacy scope")
    text = text.replace("Account version", "Contact Point version")
    postgres.write_text(text)

    tests = TARGET / "src/tests.rs"
    text = tests.read_text()
    text = text.replace('"account-001"', '"contact-point-001"')
    text = text.replace('"account-127"', '"contact-point-127"')
    text = text.replace("Account", "Contact Point")
    tests.write_text(text)

    support = TARGET / "tests/postgres_scope/support.rs"
    text = support.read_text()
    text = re.sub(
        r"use crm_contact_points_capability_adapter::\{.*?\};",
        '''use crm_contact_points_capability_adapter::{
    CREATE_CAPABILITY as CREATE_CONTACT_POINT_CAPABILITY,
    CREATE_REQUEST_SCHEMA as CREATE_CONTACT_POINT_SCHEMA,
    UPDATE_CAPABILITY as UPDATE_CONTACT_POINT_CAPABILITY,
    UPDATE_REQUEST_SCHEMA as UPDATE_CONTACT_POINT_SCHEMA,
    VERIFY_CAPABILITY as VERIFY_CONTACT_POINT_CAPABILITY,
    VERIFY_REQUEST_SCHEMA as VERIFY_CONTACT_POINT_SCHEMA,
};''',
        text,
        count=1,
        flags=re.S,
    )
    text = text.replace(
        "contact_points::v1 as contact_points, customer::v1 as customer, customer_privacy::v1 as privacy,",
        "contact_points::v1 as contact_points, core::v1 as core, customer::v1 as customer, customer_privacy::v1 as privacy,",
    )
    text = text.replace('format!("Account Scope Subject {party_id}")', 'format!("Contact Point Scope Subject {party_id}")')

    create_start = text.index("pub(crate) async fn create_contact_point(")
    create_end = text.index("#[allow(clippy::too_many_arguments)]", create_start)
    create_helpers = '''pub(crate) async fn create_contact_point(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    contact_point_id: &str,
    party_id: &str,
    kind: contact_points::ContactPointKind,
    value: &str,
    preferred: bool,
    seed: u8,
) {
    executor
        .execute(
            definition,
            capability_request(
                crm_contact_points::MODULE_ID,
                CREATE_CONTACT_POINT_CAPABILITY,
                CREATE_CONTACT_POINT_SCHEMA,
                tenant,
                &format!("contact-point-{contact_point_id}"),
                600_000_000 + i64::from(seed),
                seed,
                &contact_points::CreateContactPointRequest {
                    contact_point_ref: Some(customer::ContactPointRef {
                        contact_point_id: contact_point_id.to_owned(),
                    }),
                    party_ref: Some(customer::PartyRef {
                        party_id: party_id.to_owned(),
                    }),
                    kind: kind as i32,
                    value: value.to_owned(),
                    preferred,
                    valid_from: None,
                    valid_until: None,
                },
            ),
        )
        .await
        .expect("create authoritative Contact Point fixture");
}

pub(crate) async fn verify_contact_point(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    contact_point_id: &str,
    expected_version: i64,
    evidence_ref: &str,
    seed: u8,
) {
    executor
        .execute(
            definition,
            capability_request(
                crm_contact_points::MODULE_ID,
                VERIFY_CONTACT_POINT_CAPABILITY,
                VERIFY_CONTACT_POINT_SCHEMA,
                tenant,
                &format!("verify-contact-point-{contact_point_id}"),
                700_000_000 + i64::from(seed),
                seed,
                &contact_points::VerifyContactPointRequest {
                    contact_point_ref: Some(customer::ContactPointRef {
                        contact_point_id: contact_point_id.to_owned(),
                    }),
                    expected_version,
                    evidence_ref: evidence_ref.to_owned(),
                },
            ),
        )
        .await
        .expect("verify authoritative Contact Point fixture");
}

pub(crate) async fn update_contact_point_status(
    executor: &Arc<dyn TransactionalCapabilityExecutor>,
    definition: &CapabilityDefinition,
    tenant: &str,
    contact_point_id: &str,
    expected_version: i64,
    value: &str,
    status: contact_points::ContactPointStatus,
    preferred: bool,
    seed: u8,
) {
    executor
        .execute(
            definition,
            capability_request(
                crm_contact_points::MODULE_ID,
                UPDATE_CONTACT_POINT_CAPABILITY,
                UPDATE_CONTACT_POINT_SCHEMA,
                tenant,
                &format!("update-contact-point-{contact_point_id}"),
                800_000_000 + i64::from(seed),
                seed,
                &contact_points::UpdateContactPointRequest {
                    contact_point_ref: Some(customer::ContactPointRef {
                        contact_point_id: contact_point_id.to_owned(),
                    }),
                    expected_version,
                    value: value.to_owned(),
                    status: status as i32,
                    preferred,
                    valid_from: None,
                    valid_until: None,
                },
            ),
        )
        .await
        .expect("update authoritative Contact Point fixture");
}

'''
    text = text[:create_start] + create_helpers + text[create_end:]
    text = text.replace("corrupt isolated Account", "corrupt isolated Contact Point")
    text = text.replace("Account metadata", "Contact Point metadata")
    text = text.replace("crm.contact-points.account.state.invalid", "crm.contact-points.contact-point.state.invalid")
    support.write_text(text)

    postgres_test = TARGET / "tests/postgres_scope.rs"
    text = postgres_test.read_text()
    text = re.sub(
        r"use crm_contact_points_capability_adapter::\{.*?\};",
        '''use crm_contact_points_capability_adapter::{
    CREATE_CAPABILITY as CREATE_CONTACT_POINT_CAPABILITY,
    UPDATE_CAPABILITY as UPDATE_CONTACT_POINT_CAPABILITY,
    VERIFY_CAPABILITY as VERIFY_CONTACT_POINT_CAPABILITY,
    ContactPointCapabilityPlanner,
    capability_definition as contact_point_definition,
};''',
        text,
        count=1,
        flags=re.S,
    )
    text = text.replace(
        "use crm_proto_contracts::crm::{contact_points::v1 as contact_points, customer_privacy::v1 as privacy};",
        "use crm_proto_contracts::crm::{contact_points::v1 as contact_points, customer_privacy::v1 as privacy};",
    )

    init_start = text.index("    let contact_point_executor:")
    init_end_marker = "    let contact_point_definition = contact_point_definition(CREATE_CONTACT_POINT_CAPABILITY).unwrap();"
    init_end = text.index(init_end_marker, init_start) + len(init_end_marker)
    init = '''    let contact_point_executor: Arc<dyn TransactionalCapabilityExecutor> =
        Arc::new(PostgresTransactionalAggregateExecutor::new(
            store.clone(),
            Arc::new(ContactPointCapabilityPlanner),
        ));
    let party_definition = party_definition(CREATE_PARTY_CAPABILITY).unwrap();
    let create_contact_point_definition =
        contact_point_definition(CREATE_CONTACT_POINT_CAPABILITY).unwrap();
    let update_contact_point_definition =
        contact_point_definition(UPDATE_CONTACT_POINT_CAPABILITY).unwrap();
    let verify_contact_point_definition =
        contact_point_definition(VERIFY_CONTACT_POINT_CAPABILITY).unwrap();'''
    text = text[:init_start] + init + text[init_end:]

    create_start = text.index("    create_contact_point(", text.index("for (party_id"))
    create_end = text.index("    let adapter =", create_start)
    create_calls = '''    create_contact_point(
        &contact_point_executor,
        &create_contact_point_definition,
        TENANT_A,
        "contact-point-001",
        "party-scope",
        contact_points::ContactPointKind::Email,
        "Scope.Primary@EXAMPLE.COM",
        true,
        21,
    )
    .await;
    create_contact_point(
        &contact_point_executor,
        &create_contact_point_definition,
        TENANT_A,
        "contact-point-002",
        "party-other",
        contact_points::ContactPointKind::Phone,
        "+12025550102",
        false,
        22,
    )
    .await;
    create_contact_point(
        &contact_point_executor,
        &create_contact_point_definition,
        TENANT_A,
        "contact-point-003",
        "party-scope",
        contact_points::ContactPointKind::Messaging,
        "scope-member@example.net",
        false,
        23,
    )
    .await;
    verify_contact_point(
        &contact_point_executor,
        &verify_contact_point_definition,
        TENANT_A,
        "contact-point-003",
        1,
        "private-verification-evidence-003",
        24,
    )
    .await;
    update_contact_point_status(
        &contact_point_executor,
        &update_contact_point_definition,
        TENANT_A,
        "contact-point-003",
        2,
        "scope-member@example.net",
        contact_points::ContactPointStatus::Inactive,
        false,
        25,
    )
    .await;
    create_contact_point(
        &contact_point_executor,
        &create_contact_point_definition,
        TENANT_A,
        "contact-point-malformed",
        "party-scope",
        contact_points::ContactPointKind::Web,
        "https://private.example.test/profile",
        false,
        26,
    )
    .await;
    create_contact_point(
        &contact_point_executor,
        &create_contact_point_definition,
        TENANT_A,
        "contact-point-redirected",
        "party-redirected",
        contact_points::ContactPointKind::Postal,
        "Private Postal Address",
        false,
        27,
    )
    .await;

'''
    text = text[:create_start] + create_calls + text[create_end:]
    text = text.replace('"account-001"', '"contact-point-001"')
    text = text.replace('"account-003"', '"contact-point-003"')
    text = text.replace('"account-malformed"', '"contact-point-malformed"')
    text = text.replace('"account-redirected"', '"contact-point-redirected"')
    text = text.replace("Private Account account-001", "Scope.Primary@EXAMPLE.COM")
    text = text.replace("Private Account account-003", "scope-member@example.net")
    text = text.replace("Private Account account-redirected", "Private Postal Address")
    text = text.replace('"party_associations"', '"verification"')
    text = text.replace("Account scope", "Contact Point scope")
    text = text.replace("Account state", "Contact Point state")
    text = text.replace("Account metadata", "Contact Point metadata")
    text = text.replace("Account", "Contact Point")
    postgres_test.write_text(text)


def update_workspace() -> None:
    cargo = ROOT / "Cargo.toml"
    text = cargo.read_text()
    member = '  "crates/crm-contact-points-privacy-scope-adapter",\n'
    if member not in text:
        anchor = '  "crates/crm-contact-points-query-adapter",\n'
        text = replace_once(text, anchor, anchor + member, "workspace member")
        cargo.write_text(text)

    policy_path = ROOT / "architecture-policy.json"
    policy = json.loads(policy_path.read_text())
    consumers = policy["restricted_dependency_consumers"][
        "crm-customer-privacy-owner-scope-support"
    ]
    manifest = "crates/crm-contact-points-privacy-scope-adapter/Cargo.toml"
    if manifest not in consumers:
        consumers.append(manifest)
        consumers.sort()
    markers = policy["restricted_source_markers"][".begin_bound_read_transaction("]
    source = "crates/crm-contact-points-privacy-scope-adapter/src/postgres.rs"
    if source not in markers:
        markers.append(source)
        markers.sort()
    policy_path.write_text(json.dumps(policy, indent=2) + "\n")


def write_fixture() -> None:
    path = ROOT / "database/tests/0027_contact_points_privacy_scope_fixture.sql"
    if path.exists():
        return
    path.write_text(r'''\set ON_ERROR_STOP on

-- Minimal durable registry fixture required only to create authoritative
-- Contact Point records for the non-runtime owner privacy-scope PostgreSQL
-- acceptance suite. Party creation remains supplied by fixture 0024.
INSERT INTO crm.module_versions (
  module_id,
  version,
  canonicalization_profile,
  manifest_sha256,
  normalized_manifest_json,
  published_at,
  publisher_id
)
VALUES (
  'crm.contact-points',
  '0.3.0',
  'crm.cjson/v1',
  decode(repeat('94', 32), 'hex'),
  '{"module_id":"crm.contact-points","version":"0.3.0"}'::jsonb,
  clock_timestamp(),
  'customer-platform'
)
ON CONFLICT (module_id, version) DO NOTHING;

INSERT INTO crm.capability_registry (
  capability_id,
  capability_version,
  owner_module_id,
  owner_module_version,
  service_name,
  method_name,
  input_descriptor_hash,
  output_descriptor_hash,
  risk_level,
  idempotency_required,
  audit_required,
  approval_required,
  ai_callable,
  marketplace_callable,
  bulk_allowed,
  export_allowed,
  data_classes_touched
)
VALUES (
  'contact-points.contact-point.create',
  '1.0.0',
  'crm.contact-points',
  '0.3.0',
  'crm.contact_points.v1.ContactPointService',
  'CreateContactPoint',
  decode(repeat('95', 32), 'hex'),
  decode(repeat('96', 32), 'hex'),
  'medium',
  true,
  true,
  false,
  false,
  false,
  false,
  false,
  ARRAY['personal']::text[]
)
ON CONFLICT (capability_id, capability_version) DO UPDATE
SET owner_module_id = EXCLUDED.owner_module_id,
    owner_module_version = EXCLUDED.owner_module_version,
    service_name = EXCLUDED.service_name,
    method_name = EXCLUDED.method_name,
    input_descriptor_hash = EXCLUDED.input_descriptor_hash,
    output_descriptor_hash = EXCLUDED.output_descriptor_hash,
    risk_level = EXCLUDED.risk_level,
    idempotency_required = EXCLUDED.idempotency_required,
    audit_required = EXCLUDED.audit_required,
    approval_required = EXCLUDED.approval_required,
    ai_callable = EXCLUDED.ai_callable,
    marketplace_callable = EXCLUDED.marketplace_callable,
    bulk_allowed = EXCLUDED.bulk_allowed,
    export_allowed = EXCLUDED.export_allowed,
    data_classes_touched = EXCLUDED.data_classes_touched;
''')


def write_permanent_workflow() -> None:
    target = ROOT / ".github/workflows/contact-points-privacy-scope.yml"
    if target.exists():
        return
    source = (ROOT / ".github/workflows/customer-accounts-privacy-scope.yml").read_text()
    replacements = [
        ("Customer Accounts", "Contact Points"),
        ("customer-accounts", "contact-points"),
        ("customer_accounts", "contact_points"),
        ("crm_customer_accounts", "crm_contact_points"),
        ('"proto/crm/accounts/**"', '"proto/crm/contact_points/**"'),
        ("0026_customer_accounts", "0027_contact_points"),
        ("5439", "5440"),
    ]
    for old, new in replacements:
        source = source.replace(old, new)
    target.write_text(source)


def main() -> None:
    generate_crate()
    update_workspace()
    write_fixture()
    write_permanent_workflow()


if __name__ == "__main__":
    main()
