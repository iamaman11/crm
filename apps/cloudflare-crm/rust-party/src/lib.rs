//! Sole native workers-rs CRM entrypoint, linked to the original crm-parties owner.
//! Signed native-ci keys are isolated to CI; production verifies Cloudflare Access.
use serde::Deserialize;
use serde_json::{Value, json};
use wasm_bindgen::JsValue;
use worker::*;

mod parties;

#[cfg(feature = "native")]
mod access;
#[cfg(feature = "native")]
mod account_overview;
#[cfg(feature = "native")]
mod accounts;
#[cfg(feature = "native")]
mod activity_tasks;
#[cfg(feature = "native")]
mod connect_sales;
#[cfg(feature = "native")]
mod consents;
#[cfg(feature = "native")]
mod contact_points;
#[cfg(feature = "native")]
mod customer_data_operations;
#[cfg(feature = "native")]
mod customer_enrichment;
#[cfg(feature = "native")]
mod customer_privacy;
#[cfg(feature = "native")]
mod data_quality;
#[cfg(feature = "native")]
mod identity_resolution;
#[cfg(feature = "native-ci")]
mod original_domain_probe;
#[cfg(feature = "native")]
mod party_relationships;
#[cfg(feature = "native")]
mod sales_activities_link;
#[cfg(feature = "native")]
mod sales_contract;
#[cfg(feature = "native")]
mod sales_deals;

fn native_request_id() -> Result<String> {
    use js_sys::{Function, Reflect};
    use wasm_bindgen::JsCast;
    let crypto = Reflect::get(&js_sys::global(), &JsValue::from_str("crypto"))
        .map_err(|_| worker::Error::RustError("crypto unavailable".into()))?;
    let func = Reflect::get(&crypto, &JsValue::from_str("randomUUID"))
        .map_err(|_| worker::Error::RustError("randomUUID unavailable".into()))?
        .dyn_into::<Function>()
        .map_err(|_| worker::Error::RustError("crypto.randomUUID unavailable".into()))?;
    func.call0(&crypto)
        .map_err(|_| worker::Error::RustError("request_id generation failed".into()))?
        .as_string()
        .ok_or_else(|| worker::Error::RustError("invalid request_id".into()))
}

fn response(body: Value, status: u16) -> Result<Response> {
    Ok(Response::from_json(&body)?.with_status(status))
}
fn error(code: &str, status: u16) -> Result<Response> {
    let mut reply = response(json!({"code": code, "error": code}), status)?;
    reply.headers_mut().set("x-crm-error-code", code)?;
    Ok(reply)
}
#[cfg(feature = "native")]
fn rpc_error(error: connectrpc::ConnectError) -> Result<Response> {
    // Unary binary Connect errors use the library's standard JSON envelope.
    let mut reply =
        Response::from_bytes(error.to_json().to_vec())?.with_status(error.http_status().as_u16());
    reply
        .headers_mut()
        .set("content-type", "application/json")?;
    reply
        .headers_mut()
        .set("x-crm-error-code", error.code.as_str())?;
    Ok(reply)
}
fn now_nanos() -> i64 {
    (Date::now().as_millis() as i64).saturating_mul(1_000_000)
}
fn bind(db: &D1Database, sql: &str, values: &[JsValue]) -> Result<worker::d1::D1PreparedStatement> {
    db.prepare(sql).bind(values)
}
#[cfg(feature = "native")]
#[derive(Deserialize)]
struct SchemaCount {
    present: i64,
}

#[cfg(feature = "native")]
async fn native_doctor(env: &Env, tenant: &str) -> Result<Response> {
    // A truthful live, read-only diagnostic: this probes the active Rust
    // runtime and current D1 tables. CI/browser/Logs proof remains a separate
    // protected release artifact, never an invented live health result.
    const EXPECTED: &[&str] = &[
        "crm_tenants",
        "crm_users",
        "crm_audit",
        "crm_party_records",
        "crm_party_receipts",
        "crm_account_records",
        "crm_account_receipts",
        "crm_contact_point_records",
        "crm_contact_point_receipts",
        "crm_party_relationship_records",
        "crm_party_relationship_receipts",
        "crm_activity_task_records",
        "crm_activity_task_receipts",
        "crm_sales_deal_records",
        "crm_sales_deal_receipts",
        "crm_consent_records",
        "crm_consent_receipts",
        "crm_sales_link_delivery_records",
        "crm_privacy_case_records",
        "crm_privacy_case_receipts",
        "crm_party_import_receipts",
        "crm_enrichment_profiles",
        "crm_enrichment_mappings",
        "crm_enrichment_receipts",
    ];
    let db = env.d1("DB")?;
    let bindings: Vec<JsValue> = EXPECTED
        .iter()
        .map(|name| JsValue::from_str(name))
        .collect();
    let placeholders = EXPECTED.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let schema_query = format!(
        "SELECT COUNT(*) AS present FROM sqlite_master WHERE type='table' AND name IN ({placeholders})"
    );
    let row = bind(&db, &schema_query, &bindings)?
        .first::<SchemaCount>(None)
        .await?;
    let present = row.map_or(0, |value| value.present);
    let schema_ok = present == EXPECTED.len() as i64;
    let mut read_checks = serde_json::Map::new();
    let mut reads_ok = schema_ok;
    // Query each deployed UI owner's tenant scope directly, with real D1 reads.
    // Zero records is a valid empty workspace, not a failed health check.
    if schema_ok {
        for (module, table, column) in [
            ("parties", "crm_party_records", "tenant"),
            ("accounts", "crm_account_records", "tenant"),
            ("contact-points", "crm_contact_point_records", "tenant"),
            (
                "party-relationships",
                "crm_party_relationship_records",
                "tenant",
            ),
            ("activity-tasks", "crm_activity_task_records", "tenant"),
            ("sales-deals", "crm_sales_deal_records", "tenant"),
            ("consents", "crm_consent_records", "tenant"),
            (
                "sales-activities-link",
                "crm_sales_link_delivery_records",
                "tenant",
            ),
            ("customer-privacy", "crm_privacy_case_records", "tenant"),
            (
                "customer-enrichment-profiles",
                "crm_enrichment_profiles",
                "tenant",
            ),
            (
                "customer-enrichment-mappings",
                "crm_enrichment_mappings",
                "tenant",
            ),
        ] {
            // Table and column names are compiled-in constants, not user input.
            let query = format!("SELECT COUNT(*) AS present FROM {table} WHERE {column} = ?");
            let count = match bind(&db, &query, &[JsValue::from_str(tenant)]) {
                Ok(statement) => statement
                    .first::<SchemaCount>(None)
                    .await
                    .ok()
                    .flatten()
                    .map(|x| x.present),
                Err(_) => None,
            };
            if count.is_none() {
                reads_ok = false;
            }
            read_checks.insert(
                module.into(),
                json!({
                    "status": if count.is_some() { "PASS" } else { "FAIL" },
                    "visible_records": count,
                    "scope": "verified_access_tenant"
                }),
            );
        }
    }
    // Real audit-schema probe; if request_id is absent, the doctor must fail,
    // even when all nine table names still exist.
    let audit_trace = if schema_ok {
        match bind(
            &db,
            "SELECT COUNT(request_id) AS present FROM crm_audit WHERE tenant_id = ?",
            &[JsValue::from_str(tenant)],
        ) {
            Ok(statement) => statement
                .first::<SchemaCount>(None)
                .await
                .ok()
                .flatten()
                .map(|x| x.present),
            Err(_) => None,
        }
    } else {
        None
    };
    let trace_ready = audit_trace.is_some();
    let worker_version = env
        .object_var::<Value>("CF_VERSION_METADATA")
        .ok()
        .and_then(|metadata| {
            metadata
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
    let source = env.var("GIT_SHA").ok().map(|value| value.to_string());
    let identity_ok = source.as_deref().is_some_and(|value| {
        value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
    }) && worker_version
        .as_deref()
        .is_some_and(|value| value.len() == 36);
    let healthy = schema_ok && reads_ok && trace_ready && identity_ok;
    response(
        json!({
            "application": "crm-dev",
            "runtime": "workers-rs",
            "entrypoint": "native-workers-rs",
            "source": source.unwrap_or_else(|| "NOT_AVAILABLE".into()),
            "worker_version": worker_version.unwrap_or_else(|| "NOT_AVAILABLE".into()),
            "release": { "source_version": if identity_ok {"PASS"} else {"FAIL"},
                "traffic_percentage": "EXTERNAL_CLOUDFLARE_DEPLOYMENTS_CHECK_REQUIRED",
                "native_only": true },
            "access": {"status": "PASS", "check": "signed JWT and enrolled tenant",
                "principal_scope": "verified_access_tenant"},
            "database": {"status": if schema_ok && reads_ok && trace_ready { "PASS" } else { "FAIL" },
                "tables": {"present": present, "expected": EXPECTED.len()},
                "tenant_reads": read_checks.clone(),
                "audit": {"request_id_column": if trace_ready { "PASS" } else { "FAIL" },
                    "tenant_audit_rows_with_id": audit_trace }},
            "modules": {
                "crm.parties": {"read": read_checks.get("parties"), "business_owner": "original-crm-parties"},
                "crm.customer-enrichment": {
                    "provider_profiles":read_checks.get("customer-enrichment-profiles"),
                    "mapping_versions":read_checks.get("customer-enrichment-mappings"),
                    "business_owner":"original-crm-customer-enrichment",
                    "ui":"REACT_CONFIG_CONNECTED",
                    "profile_mapping_publication":"ORIGINAL_DOMAIN_D1",
                    "provider_dispatch":"NOT_CONNECTED",
                    "suggestion_application":"NOT_CONNECTED",
                    "functional_cycle":"EXTERNAL_PROTECTED_CI_CHECK_REQUIRED"},
                "crm.customer-privacy": {"read":read_checks.get("customer-privacy"),
                    "business_owner":"original-crm-customer-privacy",
                    "ui":"REACT_INTAKE_CONNECTED",
                    "draft_submit_cancel":"ORIGINAL_DOMAIN_D1",
                    "subject_verification":"NOT_CONNECTED",
                    "restriction_enforcement":"NOT_CONNECTED",
                    "erasure_execution":"NOT_CONNECTED",
                    "functional_cycle":"EXTERNAL_PROTECTED_CI_CHECK_REQUIRED"},
                "crm.customer-data-operations": {"read":read_checks.get("parties"),
                    "business_owner":"original-crm-customer-data-operations",
                    "api":"ADMIN_BOUNDED_PARTY_CSV", "ui":"REACT_EXPORT",
                    "scope":"MAX_100_PARTIES_FAIL_CLOSED",
                    "export_job":"NOT_CONNECTED", "import":"BOUNDED_ATOMIC_CREATE_ONLY_CSV"},
                "crm.data-quality": {"read":read_checks.get("parties"),
                    "business_owner":"original-crm-data-quality",
                    "api":"READ_ONLY_PARTY_NAME_RULES", "ui":"READ_ONLY_NAME_REVIEW",
                    "rule_set":"BUILTIN_V1_NOT_TENANT_PERSISTED",
                    "findings":"NOT_CONNECTED", "completeness_score":"NOT_CONNECTED"},
                "crm.identity-resolution": {"read":read_checks.get("parties"),
                    "business_owner":"original-crm-identity-resolution",
                    "api":"READ_ONLY_PARTY_COMPARISON", "ui":"READ_ONLY_COMPARISON",
                    "candidate_search":"READ_ONLY_EXACT_RECORDED_EMAIL", "match_evidence":"NOT_CONNECTED",
                    "merge":"NOT_CONNECTED"},
                "crm.sales-activities-link": {"read":read_checks.get("sales-activities-link"),
                    "business_owner":"original-crm-sales-activities-link",
                    "api":"native-workers-rs","delivery":"SYNCHRONOUS_ATOMIC_D1",
                    "ui":"INTERNAL_LINK_NO_SCREEN",
                    "functional_cycle":"EXTERNAL_PROTECTED_CI_CHECK_REQUIRED"},
                "crm.consents": {"read":read_checks.get("consents"),
                    "business_owner":"original-crm-consents","api":"native-workers-rs",
                    "ui":"REACT_CONNECTED","grant_evidence":"NOT_CONNECTED",
                    "operator_suppressions":"ENABLED",
                    "functional_cycle":"EXTERNAL_PROTECTED_CI_CHECK_REQUIRED"},
                "crm.sales": {"read":read_checks.get("sales-deals"),
                    "business_owner":"original-crm-sales", "api":"native-workers-rs",
                    "ui":"REACT_CONNECTED", "account":"ORIGINAL_DOMAIN_D1",
                    "primary_contact":"NOT_CONNECTED",
                    "functional_cycle":"EXTERNAL_PROTECTED_CI_CHECK_REQUIRED"},
                "crm.activities": {"read":read_checks.get("activity-tasks"),
                    "business_owner":"original-crm-activities", "api":"native-workers-rs",
                    "ui":"REACT_CONNECTED","completion":"ORIGINAL_DOMAIN",
                    "functional_cycle":"EXTERNAL_PROTECTED_CI_CHECK_REQUIRED"},
                "crm.party-relationships": {"read":read_checks.get("party-relationships"),
                    "business_owner":"original-crm-party-relationships",
                    "api":"native-workers-rs", "ui":"REACT_CONNECTED",
                    "functional_cycle":"EXTERNAL_PROTECTED_CI_CHECK_REQUIRED"},
                "crm.contact-points": {"read":read_checks.get("contact-points"),
                    "business_owner":"original-crm-contact-points",
                    "api":"native-workers-rs", "ui":"REACT_CONNECTED",
                    "verification":"TRUSTED_EVIDENCE_NOT_CONNECTED",
                    "consent":"SEPARATE_DOMAIN_NOT_CONNECTED",
                    "functional_cycle":"EXTERNAL_PROTECTED_CI_CHECK_REQUIRED"},
                "crm.customer-accounts": {"read":read_checks.get("accounts"),
                    "business_owner":"original-crm-customer-accounts",
                    "api":"native-workers-rs", "ui":"REACT_CONNECTED",
                    "functional_cycle":"EXTERNAL_PROTECTED_CI_CHECK_REQUIRED"},
                "crm.account-overview": {"read": "owner_composition_on_one_d1",
                    "business_owner":"native-worker-read-projection",
                    "ui":"REACT_CONNECTED",
                    "functional_cycle":"EXTERNAL_PROTECTED_CI_CHECK_REQUIRED"}
            },
            "functional_cycle": {
                "browser_buttons": "EXTERNAL_PROTECTED_CI_CHECK_REQUIRED",
                "crud_writes": "EXTERNAL_PROTECTED_CI_CHECK_REQUIRED",
                "d1_request_id": "EXTERNAL_PROTECTED_CI_CHECK_REQUIRED",
                "cloudflare_logs": "EXTERNAL_PROTECTED_CI_CHECK_REQUIRED"
            }
        }),
        if healthy { 200 } else { 503 },
    )
}

// The only handwritten RPC activation and authorization table.
// Paths are exact: a newly generated method is denied until explicitly classified.
// CI compares this table against compiled Buf descriptors, registered Rust traits
// and signed workerd evidence. HTTP POST does not imply write permission.
#[cfg(feature = "native")]
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize)]
enum RpcPermission {
    Read,
    AdminRead,
    Write,
}

#[cfg(feature = "native")]
#[derive(Clone, Copy, serde::Serialize)]
struct RpcPolicy {
    permission: RpcPermission,
}

#[cfg(feature = "native")]
const RPC_POLICIES: &[(&str, RpcPolicy)] = &[
    (
        "/crm.customer_360.v2.Customer360Service/GetAccountOverview",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.customer_data_operations.v2.PartyCsvService/PreviewPartyExport",
        RpcPolicy {
            permission: RpcPermission::AdminRead,
        },
    ),
    (
        "/crm.customer_data_operations.v2.PartyCsvService/PreviewPartyImport",
        RpcPolicy {
            permission: RpcPermission::AdminRead,
        },
    ),
    (
        "/crm.customer_data_operations.v2.PartyCsvService/ApplyPartyImport",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.customer_enrichment.v2.EnrichmentConfigurationService/PublishProviderProfile",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.customer_enrichment.v2.EnrichmentConfigurationService/PublishMapping",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.customer_enrichment.v2.EnrichmentConfigurationService/GetProviderProfile",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.customer_enrichment.v2.EnrichmentConfigurationService/GetMapping",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.customer_enrichment.v2.EnrichmentConfigurationService/ListProviderProfiles",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.customer_enrichment.v2.EnrichmentConfigurationService/ListMappings",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.customer_privacy.v2.PrivacyCaseService/CreatePrivacyCase",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.customer_privacy.v2.PrivacyCaseService/SubmitPrivacyCase",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.customer_privacy.v2.PrivacyCaseService/CancelPrivacyCase",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.customer_privacy.v2.PrivacyCaseService/GetPrivacyCase",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.customer_privacy.v2.PrivacyCaseService/ListPrivacyCases",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.parties.v3.PartyService/CreateParty",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.parties.v3.PartyService/UpdateParty",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.parties.v3.PartyService/GetParty",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.parties.v3.PartyService/ListParties",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.accounts.v2.AccountService/CreateAccount",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.accounts.v2.AccountService/UpdateAccount",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.accounts.v2.AccountService/GetAccount",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.accounts.v2.AccountService/ListAccounts",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.contact_points.v2.ContactPointService/CreateContactPoint",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.contact_points.v2.ContactPointService/UpdateContactPoint",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.contact_points.v2.ContactPointService/GetContactPoint",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.contact_points.v2.ContactPointService/ListContactPoints",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.party_relationships.v2.PartyRelationshipService/CreatePartyRelationship",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.party_relationships.v2.PartyRelationshipService/UpdatePartyRelationship",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.party_relationships.v2.PartyRelationshipService/GetPartyRelationship",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.party_relationships.v2.PartyRelationshipService/ListPartyRelationships",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.activities.v2.TaskService/CreateTask",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.activities.v2.TaskService/CompleteTask",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.activities.v2.TaskService/GetTask",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.activities.v2.TaskService/ListTasks",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.data_quality.v2.DataQualityService/EvaluatePartyName",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.identity_resolution.v2.IdentityResolutionService/FindSharedEmailCandidates",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.identity_resolution.v2.IdentityResolutionService/CompareParties",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.consents.v2.ConsentAuthorizationService/EvaluateConsent",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.consents.v2.ConsentAuthorizationService/SuppressCommunication",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.consents.v2.ConsentAuthorizationService/ListConsentAuthorizations",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.sales.v2.DealService/CreateDeal",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.sales.v2.DealService/UpdateDeal",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.sales.v2.DealService/AdvanceStage",
        RpcPolicy {
            permission: RpcPermission::Write,
        },
    ),
    (
        "/crm.sales.v2.DealService/GetDeal",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
    (
        "/crm.sales.v2.DealService/ListDeals",
        RpcPolicy {
            permission: RpcPermission::Read,
        },
    ),
];

#[cfg(feature = "native")]
fn rpc_policy(path: &str) -> Option<RpcPolicy> {
    RPC_POLICIES
        .iter()
        .find(|(method, _)| *method == path)
        .map(|(_, rule)| *rule)
}

#[cfg(feature = "native")]
async fn dispatch(
    req: Request,
    env: Env,
    tenant: &str,
    request_id: &str,
    actor: &access::Actor,
) -> Result<Response> {
    let path = req.path();
    let method = req.method();
    if path.starts_with("/crm.") {
        if method != Method::Post {
            return rpc_error(connectrpc::ConnectError::unimplemented("POST required"));
        }
        // One explicit policy authorizes every registered RPC.
        let Some(rule) = rpc_policy(&path) else {
            return rpc_error(connectrpc::ConnectError::unimplemented("Unknown RPC"));
        };
        if matches!(
            rule.permission,
            RpcPermission::Write | RpcPermission::AdminRead
        ) && actor.role != "admin"
        {
            return rpc_error(connectrpc::ConnectError::permission_denied("FORBIDDEN"));
        }
        return connect_sales::route(req, env, tenant, &actor.identity, request_id).await;
    }
    let db = env.d1("DB")?;
    // Explicitly CI-only, signed-Access-gated native workerd proof. This gate
    // performs no D1 writes and makes no claims about module persistence.
    #[cfg(feature = "native-ci")]
    if path == "/api/_ci/original-domain-check" && method == Method::Get {
        let code = original_domain_probe::check();
        let router = connect_sales::router(env, tenant, &actor.identity, request_id);
        let mut methods: Vec<String> = router
            .methods()
            .map(|path| format!("/{}", path.trim_start_matches('/')))
            .collect();
        methods.sort();
        let mut policy_paths = std::collections::BTreeSet::new();
        let policy_ok = RPC_POLICIES.iter().all(|(path, rule)| {
            policy_paths.insert(*path)
                && router.has_method(path)
                && rpc_policy(path).is_some_and(|found| found.permission == rule.permission)
        }) && methods.iter().all(|path| rpc_policy(path).is_some())
            && rpc_policy("/crm.unregistered.v1.Service/Unknown").is_none();
        let checked = code == 0 && policy_ok;
        return response(
            json!({
                "result": if checked { "PASS" } else { "FAIL" },
                "code": code,
                "rpc": {"registered_methods": methods, "policies": RPC_POLICIES,
                    "compiled_policy_check": policy_ok},
                "scope": "in-memory original owner domain contracts",
                "owners_executed": ["crm.customer-accounts", "crm.contact-points",
                    "crm.sales", "crm.activities"],
                "original_modules_compiled": 13,
                "d1_owner_persistence": "NOT_TESTED",
                "remaining_owners": "NOT_TESTED"
            }),
            if checked { 200 } else { 503 },
        );
    }
    if path == "/api/customer-data-operations/parties.csv" {
        return customer_data_operations::route(req, db, tenant, Some(&actor.role)).await;
    }
    #[cfg(feature = "native")]
    {
        if path == "/api/health" && method == Method::Get {
            return response(json!({"service":"crm-dev","status":"ok"}), 200);
        }
        if path == "/api/session" && method == Method::Get {
            return response(json!({"authenticated":true,"role":actor.role}), 200);
        }
        if path == "/api/version" && method == Method::Get {
            return response(
                json!({"source":env.var("GIT_SHA")
                .map(|x|x.to_string()).unwrap_or_default(),
                "database":"D1","api":"crm.dev/v1"}),
                200,
            );
        }
    }
    let _ = env;
    error("NOT_FOUND", 404)
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    #[cfg(feature = "native")]
    {
        // One correlation boundary, shared by Access, native domain and D1.
        // Never trust a caller-supplied request identifier or tenant header.
        let request_id = native_request_id()?;
        let started = Date::now().as_millis();
        let path = req.path();
        let method = req.method();
        // Read version binding before dispatch takes ownership of Env.
        let source_sha = env.var("GIT_SHA").ok().map(|value| value.to_string());
        let operation = path.as_str();
        let (result, phase) = match access::actor(&req, &env).await {
            Ok(Some(actor)) if path == "/api/doctor" && method == Method::Get => {
                (native_doctor(&env, &actor.tenant).await, "diagnostics")
            }
            Ok(Some(actor)) => (
                dispatch(req, env, &actor.tenant, &request_id, &actor).await,
                "domain_d1",
            ),
            Ok(None) if path.starts_with("/crm.") => (
                rpc_error(connectrpc::ConnectError::unauthenticated("AUTH_REQUIRED")),
                "access",
            ),
            Ok(None) => (error("AUTH_REQUIRED", 401), "access"),
            Err(_) if path.starts_with("/crm.") => (
                rpc_error(connectrpc::ConnectError::unavailable(
                    "ACCESS_INTERNAL_ERROR",
                )),
                "access",
            ),
            Err(_) => (error("ACCESS_INTERNAL_ERROR", 503), "access"),
        };
        let mut reply = match result {
            Ok(reply) => reply,
            Err(_) if path.starts_with("/crm.") => {
                rpc_error(connectrpc::ConnectError::internal("INTERNAL_ERROR"))?
            }
            Err(_) => error("INTERNAL_ERROR", 500)?,
        };
        reply.headers_mut().set("x-request-id", &request_id)?;
        reply
            .headers_mut()
            .set("x-crm-runtime", "native-workers-rs")?;
        if let Some(value) = source_sha
            && value.len() == 40
            && value.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            reply.headers_mut().set("x-crm-source-sha", &value)?;
        }
        let elapsed = Date::now().as_millis().saturating_sub(started);
        let error_code = reply.headers().get("x-crm-error-code").ok().flatten();
        console_log!(
            "{}",
            json!({
                "event":"crm.native.request", "request_id":request_id,
                "operation":operation, "phase":phase,
                "status":reply.status_code(), "error_code":error_code, "duration_ms":elapsed
            })
        );
        Ok(reply)
    }

    #[cfg(not(feature = "native"))]
    {
        let _ = (req, env);
        error("AUTH_REQUIRED", 403)
    }
}
