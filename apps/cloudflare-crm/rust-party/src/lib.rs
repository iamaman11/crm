//! Sole native workers-rs CRM entrypoint, linked to the original crm-parties owner.
//! ci-local is a strictly isolated workerd acceptance fixture, never a public
//! authentication mechanism. No JS router, duplicate Party rules or second Worker.
use serde::Deserialize;
use serde_json::{Value, json};
use wasm_bindgen::JsValue;
use worker::*;

mod parties;

#[cfg(feature = "native")]
mod access;
#[cfg(feature = "native")]
mod accounts;
#[cfg(feature = "native")]
mod account_overview;
#[cfg(feature = "native")]
mod contact_points;
#[cfg(feature = "native")]
mod party_relationships;
#[cfg(feature = "native")]
mod activity_tasks;
#[cfg(feature = "native")]
mod sales_deals;
#[cfg(feature = "native")]
mod sales_contract;
#[cfg(feature = "native")]
mod connect_sales;
#[cfg(feature = "native")]
mod consents;
#[cfg(feature = "native")]
mod identity_resolution;
#[cfg(feature = "native")]
mod data_quality;
#[cfg(feature = "native")]
mod customer_data_operations;
#[cfg(feature = "native")]
mod customer_privacy;
#[cfg(feature = "native")]
mod customer_enrichment;
#[cfg(feature = "native")]
mod sales_activities_link;
#[cfg(feature = "native-ci")]
mod original_domain_probe;

fn native_request_id() -> Result<String> {
    use js_sys::{Function, Reflect};
    use wasm_bindgen::JsCast;
    let crypto = Reflect::get(&js_sys::global(), &JsValue::from_str("crypto"))
        .map_err(|_| worker::Error::RustError("crypto unavailable".into()))?;
    let func = Reflect::get(&crypto, &JsValue::from_str("randomUUID"))
        .map_err(|_| worker::Error::RustError("randomUUID unavailable".into()))?
        .dyn_into::<Function>()
        .map_err(|_| worker::Error::RustError("crypto.randomUUID unavailable".into()))?;
    func.call0(&crypto).map_err(|_| worker::Error::RustError("request_id generation failed".into()))?.as_string()
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
fn now_nanos() -> i64 {
    (Date::now().as_millis() as i64).saturating_mul(1_000_000)
}
fn bind(db: &D1Database, sql: &str, values: &[JsValue]) -> Result<worker::d1::D1PreparedStatement> {
    db.prepare(sql).bind(values)
}
fn correlation_id(req: &Request, trusted: Option<&str>) -> Result<String> {
    // Native's independently generated id takes precedence over any client header.
    if let Some(id) = trusted { return Ok(id.to_owned()); }
    // The public native and integrated entries overwrite this internal header.
    // The isolated CI build generates its own identifier if the header is absent.
    let id = req.headers().get("x-crm-internal-request-id")?
        .filter(|value| value.len() == 36 &&
            value.bytes().all(|byte| byte.is_ascii_hexdigit() || byte == b'-'));
    match id { Some(id) => Ok(id), None => native_request_id() }
}
#[cfg(feature = "native")]
#[derive(Deserialize)]
struct SchemaCount { present: i64 }

#[cfg(feature = "native")]
async fn native_doctor(env: &Env, tenant: &str) -> Result<Response> {
    // A truthful live, read-only diagnostic: this probes the active Rust
    // runtime and current D1 tables. CI/browser/Logs proof remains a separate
    // protected release artifact, never an invented live health result.
    const EXPECTED: [&str; 23] = [
        "crm_tenants", "crm_users", "crm_audit", "crm_party_records", "crm_party_receipts",
        "crm_account_records", "crm_account_receipts",
        "crm_contact_point_records", "crm_contact_point_receipts",
        "crm_party_relationship_records", "crm_party_relationship_receipts",
        "crm_activity_task_records", "crm_activity_task_receipts",
        "crm_sales_deal_records", "crm_sales_deal_receipts",
        "crm_consent_records", "crm_consent_receipts",
        "crm_sales_link_delivery_records",
        "crm_privacy_case_records", "crm_privacy_case_receipts",
        "crm_enrichment_profiles", "crm_enrichment_mappings", "crm_enrichment_receipts",
    ];
    let db = env.d1("DB")?;
    let bindings: Vec<JsValue> = EXPECTED.iter().map(|name| JsValue::from_str(name)).collect();
    let row = bind(&db,
        "SELECT COUNT(*) AS present FROM sqlite_master WHERE type='table' AND name IN (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
        &bindings)?.first::<SchemaCount>(None).await?;
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
            ("party-relationships", "crm_party_relationship_records", "tenant"),
            ("activity-tasks", "crm_activity_task_records", "tenant"),
            ("sales-deals", "crm_sales_deal_records", "tenant"),
            ("consents", "crm_consent_records", "tenant"),
            ("sales-activities-link", "crm_sales_link_delivery_records", "tenant"),
            ("customer-privacy", "crm_privacy_case_records", "tenant"),
            ("customer-enrichment-profiles", "crm_enrichment_profiles", "tenant"),
            ("customer-enrichment-mappings", "crm_enrichment_mappings", "tenant"),
        ] {
            // Table and column names are compiled-in constants, not user input.
            let query = format!("SELECT COUNT(*) AS present FROM {table} WHERE {column} = ?");
            let count = match bind(&db, &query, &[JsValue::from_str(tenant)]) {
                Ok(statement) => statement.first::<SchemaCount>(None).await.ok().flatten().map(|x|x.present),
                Err(_) => None,
            };
            if count.is_none() { reads_ok = false; }
            read_checks.insert(module.into(), json!({
                "status": if count.is_some() { "PASS" } else { "FAIL" },
                "visible_records": count,
                "scope": "verified_access_tenant"
            }));
        }
    }
    // Real audit-schema probe; if request_id is absent, the doctor must fail,
    // even when all nine table names still exist.
    let audit_trace = if schema_ok {
        match bind(&db,
            "SELECT COUNT(request_id) AS present FROM crm_audit WHERE tenant_id = ?",
            &[JsValue::from_str(tenant)]) {
            Ok(statement) => statement.first::<SchemaCount>(None).await.ok().flatten().map(|x|x.present),
            Err(_) => None,
        }
    } else { None };
    let trace_ready = audit_trace.is_some();
    let worker_version = env.object_var::<Value>("CF_VERSION_METADATA").ok()
        .and_then(|metadata| metadata.get("id").and_then(Value::as_str).map(str::to_owned));
    let source = env.var("GIT_SHA").ok().map(|value| value.to_string());
    let identity_ok = source.as_deref().is_some_and(|value| value.len()==40
        && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        && worker_version.as_deref().is_some_and(|value| value.len()==36);
    let healthy = schema_ok && reads_ok && trace_ready && identity_ok;
    response(json!({
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
    }), if healthy {200} else {503})
}

// The only handwritten RPC activation and authorization table.
// Paths are exact: a newly generated method is denied until explicitly classified.
// CI compares this table against compiled Buf descriptors, registered Rust traits
// and signed workerd evidence. HTTP POST does not imply write permission.
#[cfg(feature = "native")]
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize)]
enum RpcPermission { Read, Write }

#[cfg(feature = "native")]
#[derive(Clone, Copy, PartialEq, Eq, serde::Serialize)]
enum RpcActivation { Published, Candidate }

#[cfg(feature = "native")]
#[derive(Clone, Copy, serde::Serialize)]
struct RpcPolicy {
    permission: RpcPermission,
    activation: RpcActivation,
}

#[cfg(feature = "native")]
const RPC_POLICIES: &[(&str, RpcPolicy)] = &[
    ("/crm.parties.v2.PartyService/GetParty", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Published }),
    ("/crm.parties.v3.PartyService/CreateParty", RpcPolicy { permission: RpcPermission::Write, activation: RpcActivation::Candidate }),
    ("/crm.parties.v3.PartyService/UpdateParty", RpcPolicy { permission: RpcPermission::Write, activation: RpcActivation::Candidate }),
    ("/crm.parties.v3.PartyService/GetParty", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Candidate }),
    ("/crm.parties.v3.PartyService/ListParties", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Candidate }),
    ("/crm.accounts.v2.AccountService/CreateAccount", RpcPolicy { permission: RpcPermission::Write, activation: RpcActivation::Candidate }),
    ("/crm.accounts.v2.AccountService/UpdateAccount", RpcPolicy { permission: RpcPermission::Write, activation: RpcActivation::Candidate }),
    ("/crm.accounts.v2.AccountService/GetAccount", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Candidate }),
    ("/crm.accounts.v2.AccountService/ListAccounts", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Candidate }),
    ("/crm.contact_points.v2.ContactPointService/CreateContactPoint", RpcPolicy { permission: RpcPermission::Write, activation: RpcActivation::Candidate }),
    ("/crm.contact_points.v2.ContactPointService/UpdateContactPoint", RpcPolicy { permission: RpcPermission::Write, activation: RpcActivation::Candidate }),
    ("/crm.contact_points.v2.ContactPointService/GetContactPoint", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Candidate }),
    ("/crm.contact_points.v2.ContactPointService/ListContactPoints", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Candidate }),
    ("/crm.party_relationships.v2.PartyRelationshipService/CreatePartyRelationship", RpcPolicy { permission: RpcPermission::Write, activation: RpcActivation::Candidate }),
    ("/crm.party_relationships.v2.PartyRelationshipService/UpdatePartyRelationship", RpcPolicy { permission: RpcPermission::Write, activation: RpcActivation::Candidate }),
    ("/crm.party_relationships.v2.PartyRelationshipService/GetPartyRelationship", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Candidate }),
    ("/crm.party_relationships.v2.PartyRelationshipService/ListPartyRelationships", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Candidate }),
    ("/crm.activities.v2.TaskService/CreateTask", RpcPolicy { permission: RpcPermission::Write, activation: RpcActivation::Candidate }),
    ("/crm.activities.v2.TaskService/CompleteTask", RpcPolicy { permission: RpcPermission::Write, activation: RpcActivation::Candidate }),
    ("/crm.activities.v2.TaskService/GetTask", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Candidate }),
    ("/crm.activities.v2.TaskService/ListTasks", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Candidate }),
    ("/crm.data_quality.v2.DataQualityService/EvaluatePartyName", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Candidate }),
    ("/crm.identity_resolution.v2.IdentityResolutionService/FindSharedEmailCandidates", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Candidate }),
    ("/crm.identity_resolution.v2.IdentityResolutionService/CompareParties", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Candidate }),
    ("/crm.consents.v2.ConsentAuthorizationService/EvaluateConsent", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Candidate }),
    ("/crm.consents.v2.ConsentAuthorizationService/SuppressCommunication", RpcPolicy { permission: RpcPermission::Write, activation: RpcActivation::Candidate }),
    ("/crm.consents.v2.ConsentAuthorizationService/ListConsentAuthorizations", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Candidate }),
    ("/crm.sales.v2.DealService/CreateDeal", RpcPolicy { permission: RpcPermission::Write, activation: RpcActivation::Candidate }),
    ("/crm.sales.v2.DealService/UpdateDeal", RpcPolicy { permission: RpcPermission::Write, activation: RpcActivation::Candidate }),
    ("/crm.sales.v2.DealService/AdvanceStage", RpcPolicy { permission: RpcPermission::Write, activation: RpcActivation::Candidate }),
    ("/crm.sales.v2.DealService/GetDeal", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Candidate }),
    ("/crm.sales.v2.DealService/ListDeals", RpcPolicy { permission: RpcPermission::Read, activation: RpcActivation::Candidate }),
];

#[cfg(feature = "native")]
fn rpc_policy(path: &str) -> Option<RpcPolicy> {
    RPC_POLICIES.iter().find(|(method, _)| *method == path).map(|(_, rule)| *rule)
}

fn legacy_transport_disabled(path: &str) -> bool {
    let diagnostic = matches!(path, "/api/health" | "/api/doctor" |
        "/api/session" | "/api/version");
    let ci_only = cfg!(feature = "native-ci") && path == "/api/_ci/original-domain-check";
    if path.starts_with("/api/") {
        return !diagnostic && !ci_only;
    }
    if !path.starts_with("/crm.") { return false; }
    #[cfg(feature = "native")]
    {
        return match rpc_policy(path) {
            Some(rule) if rule.activation == RpcActivation::Published => false,
            Some(rule) if rule.activation == RpcActivation::Candidate &&
                cfg!(feature = "native-ci") => false,
            _ => true,
        };
    }
    #[cfg(not(feature = "native"))]
    {
        path != "/crm.parties.v2.PartyService/GetParty"
    }
}

async fn dispatch(req: Request, env: Env, tenant: &str, trusted_id: Option<&str>,
    role: Option<&str>, verified_actor: Option<&str>) -> Result<Response> {
    let request_id = correlation_id(&req, trusted_id)?;
    let path = req.path();
    let method = req.method();
    // Contract-only transition: legacy CRUD and partial Sales RPC are not
    // callable even by authenticated admins. No secondary transport or fallback.
    // This guard is compiled into the Worker; it is not a mutable browser flag.
    if legacy_transport_disabled(&path) {
        return error("LEGACY_TRANSPORT_DISABLED", 410);
    }
    // The fail-closed gate above owns activation; this is the ONLY native
    // Connect dispatch path, not a second manually synchronized service list.
    #[cfg(feature = "native")]
    if path.starts_with("/crm.") {
        if method != Method::Post { return error("NOT_FOUND",404); }
        // The same exact-path policy owns both activation and authorization.
        let Some(rule) = rpc_policy(&path) else {
            return error("LEGACY_TRANSPORT_DISABLED",410);
        };
        if rule.permission == RpcPermission::Write && role != Some("admin") {
            return error("FORBIDDEN",403);
        }
        let Some(actor)=verified_actor else{return error("AUTH_REQUIRED",401)};
        return connect_sales::route(req,env,tenant,actor,&request_id).await;
    }
    let db = env.d1("DB")?;
    // Explicitly CI-only, signed-Access-gated native workerd proof. This gate
    // performs no D1 writes and makes no claims about module persistence.
    #[cfg(feature = "native-ci")]
    if path == "/api/_ci/original-domain-check" && method == Method::Get {
        let code = original_domain_probe::check();
        let router = connect_sales::router(env, tenant,
            verified_actor.ok_or_else(|| worker::Error::RustError("verified actor missing".into()))?,
            &request_id);
        let mut methods: Vec<String> = router.methods()
            .map(|path| format!("/{}", path.trim_start_matches('/'))).collect();
        methods.sort();
        let mut policy_paths = std::collections::BTreeSet::new();
        let policy_ok = RPC_POLICIES.iter().all(|(path, rule)|
            policy_paths.insert(*path) && router.has_method(path) &&
            rpc_policy(path).is_some_and(|found|
                found.permission == rule.permission && found.activation == rule.activation))
            && methods.iter().all(|path| rpc_policy(path).is_some())
            && rpc_policy("/crm.unregistered.v1.Service/Unknown").is_none();
        let checked = code == 0 && policy_ok;
        return response(json!({
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
        }), if checked { 200 } else { 503 });
    }
    #[cfg(feature = "native")]
    if path == "/api/consents" || path.starts_with("/api/consents/") {
        let Some(actor)=verified_actor else{return error("AUTH_REQUIRED",401)};
        let Some(role)=role else{return error("AUTH_REQUIRED",401)};
        return consents::route(req,db,tenant,actor,role,&request_id).await;
    }
    #[cfg(feature = "native")]
    if path.starts_with("/api/customer-enrichment/") {
        let Some(actor)=verified_actor else{return error("AUTH_REQUIRED",401)};
        let Some(role)=role else{return error("AUTH_REQUIRED",401)};
        return customer_enrichment::route(req,db,tenant,actor,role,&request_id).await;
    }
    #[cfg(feature = "native")]
    if path == "/api/customer-privacy/cases" ||
       path.starts_with("/api/customer-privacy/cases/") {
        let Some(actor)=verified_actor else{return error("AUTH_REQUIRED",401)};
        let Some(role)=role else{return error("AUTH_REQUIRED",401)};
        return customer_privacy::route(req,db,tenant,actor,role,&request_id).await;
    }
    #[cfg(feature = "native")]
    if path.starts_with("/api/customer-data-operations/") {
        return customer_data_operations::route(req, db, tenant, role, &request_id).await;
    }
    #[cfg(feature = "native")]
    if path.starts_with("/api/data-quality/") {
        return data_quality::route(req, db, tenant).await;
    }
    #[cfg(feature = "native")]
    if path == "/api/identity-resolution" || path.starts_with("/api/identity-resolution/") {
        return identity_resolution::route(req, db, tenant).await;
    }
    #[cfg(feature = "native")]
    if path == "/api/sales-deals" || path.starts_with("/api/sales-deals/") {
        let Some(actor)=verified_actor else{return error("AUTH_REQUIRED",401)};
        return sales_deals::route(req,db,tenant,actor,&request_id).await;
    }
    #[cfg(feature = "native")]
    if path == "/api/activity-tasks" || path.starts_with("/api/activity-tasks/") {
        let Some(actor)=verified_actor else{return error("AUTH_REQUIRED",401)};
        return activity_tasks::route(req,db,tenant,actor,&request_id).await;
    }
    #[cfg(feature = "native")]
    if path == "/api/party-relationships" || path.starts_with("/api/party-relationships/") {
        return party_relationships::route(req, db, tenant, &request_id).await;
    }
    #[cfg(feature = "native")]
    if path == "/api/contact-points" || path.starts_with("/api/contact-points/") {
        return contact_points::route(req, db, tenant, &request_id).await;
    }
    #[cfg(feature = "native")]
    if let Some(id) = path.strip_prefix("/api/accounts/")
        .and_then(|segment| segment.strip_suffix("/overview"))
    {
        if id.is_empty() || id.contains('/') || method != Method::Get {
            return error("NOT_FOUND", 404);
        }
        return account_overview::overview(&db, tenant, id).await;
    }
    #[cfg(feature = "native")]
    if path == "/api/accounts" || path.starts_with("/api/accounts/") {
        return accounts::route(req, db, tenant, &request_id).await;
    }
    if path == "/api/parties" || path.starts_with("/api/parties/") {
        return parties::route(req, db, tenant, &request_id).await;
    }
    #[cfg(feature = "native")]
    {
        if path == "/api/health" && method == Method::Get {
            return response(json!({"service":"crm-dev","status":"ok"}),200);
        }
        if path == "/api/session" && method == Method::Get {
            return response(json!({"authenticated":true,"role":role}),200);
        }
        if path == "/api/version" && method == Method::Get {
            return response(json!({"source":env.var("GIT_SHA")
                .map(|x|x.to_string()).unwrap_or_default(),
                "database":"D1","api":"crm.dev/v1"}),200);
        }
    }
    let _ = (env, role);
    error("NOT_FOUND",404)
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    #[cfg(all(feature = "ci-local", feature = "native"))]
    compile_error!("ci-local and native are mutually exclusive");

    // Only the compiled-in test mode trusts a loopback CI-only header.
    #[cfg(feature = "ci-local")]
    {
        if env.var("CRM_CI_LOCAL_ONLY").ok().map(|v| v.to_string()) != Some("yes".into()) {
            return error("AUTH_REQUIRED", 403);
        }
        let Some(tenant) = req.headers().get("x-crm-ci-tenant")?
            .filter(|s| s == "tenant-a" || s == "tenant-b") else {
            return error("AUTH_REQUIRED", 403);
        };
        return dispatch(req, env, &tenant, None, None, None).await;
    }

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
        let operation = match (path.as_str(), method.clone()) {
            ("/api/customer-enrichment/profiles", Method::Get) => "customer-enrichment.profiles_list",
            ("/api/customer-enrichment/profiles", Method::Post) => "customer-enrichment.profile_publish",
            ("/api/customer-enrichment/mappings", Method::Get) => "customer-enrichment.mappings_list",
            ("/api/customer-enrichment/mappings", Method::Post) => "customer-enrichment.mapping_publish",
            (p, Method::Get) if p.starts_with("/api/customer-enrichment/profiles/") => "customer-enrichment.profile_read",
            (p, Method::Get) if p.starts_with("/api/customer-enrichment/mappings/") => "customer-enrichment.mapping_read",
            ("/api/customer-privacy/cases", Method::Get) => "customer-privacy.list",
            ("/api/customer-privacy/cases", Method::Post) => "customer-privacy.create",
            (p, Method::Get) if p.starts_with("/api/customer-privacy/cases/") => "customer-privacy.read",
            (p, Method::Post) if p.starts_with("/api/customer-privacy/cases/") && p.ends_with("/submit") => "customer-privacy.submit",
            (p, Method::Post) if p.starts_with("/api/customer-privacy/cases/") && p.ends_with("/cancel") => "customer-privacy.cancel",
            ("/api/customer-data-operations/import-preview", Method::Post) => "customer-data-operations.import-preview",
            ("/api/customer-data-operations/import-apply", Method::Post) => "customer-data-operations.import-apply",
            ("/api/customer-data-operations/preview", Method::Get) => "customer-data-operations.preview",
            ("/api/customer-data-operations/parties.csv", Method::Get) => "customer-data-operations.export",
            (p, Method::Get) if p.starts_with("/api/data-quality/parties/") && p.ends_with("/evaluate") => "data-quality.evaluate",
            ("/api/identity-resolution/candidates", Method::Get) => "identity-resolution.candidates",
            (p, Method::Get) if p.starts_with("/api/identity-resolution/compare/") => "identity-resolution.compare",
            ("/api/consents", Method::Get) => "consents.list",
            ("/api/consents/evaluate", Method::Post) => "consents.evaluate",
            ("/api/consents/suppress", Method::Post) => "consents.suppress",
            (p, Method::Post) if p.starts_with("/crm.") => p,
            ("/api/sales-deals", Method::Get) => "sales-deals.list",
            ("/api/sales-deals", Method::Post) => "sales-deals.create",
            (p, Method::Get) if p.starts_with("/api/sales-deals/") => "sales-deals.read",
            (p, Method::Patch) if p.starts_with("/api/sales-deals/") => "sales-deals.update",
            (p, Method::Post) if p.starts_with("/api/sales-deals/") && p.ends_with("/advance") => "sales-deals.advance",
            ("/api/activity-tasks", Method::Get) => "activity-tasks.list",
            ("/api/activity-tasks", Method::Post) => "activity-tasks.create",
            (p, Method::Get) if p.starts_with("/api/activity-tasks/") => "activity-tasks.read",
            (p, Method::Post) if p.starts_with("/api/activity-tasks/") && p.ends_with("/complete") => "activity-tasks.complete",
            ("/api/party-relationships", Method::Get) => "party-relationships.list",
            ("/api/party-relationships", Method::Post) => "party-relationships.create",
            (p, Method::Get) if p.starts_with("/api/party-relationships/") => "party-relationships.read",
            (p, Method::Patch) if p.starts_with("/api/party-relationships/") => "party-relationships.update",
            ("/api/contact-points", Method::Get) => "contact-points.list",
            ("/api/contact-points", Method::Post) => "contact-points.create",
            (p, Method::Get) if p.starts_with("/api/contact-points/") => "contact-points.read",
            (p, Method::Patch) if p.starts_with("/api/contact-points/") => "contact-points.update",
            ("/api/accounts", Method::Get) => "accounts.list",
            ("/api/accounts", Method::Post) => "accounts.create",
            (p, Method::Get) if p.starts_with("/api/accounts/") && p.ends_with("/overview") => "accounts.overview",
            (p, Method::Get) if p.starts_with("/api/accounts/") => "accounts.read",
            (p, Method::Patch) if p.starts_with("/api/accounts/") => "accounts.update",
            ("/api/parties", Method::Get) => "parties.list",
            ("/api/parties", Method::Post) => "parties.create",
            (p, Method::Get) if p.starts_with("/api/parties/") => "parties.read",
            (p, Method::Patch) if p.starts_with("/api/parties/") => "parties.update",
            ("/api/doctor", Method::Get) => "doctor.read",
            ("/api/health", Method::Get) => "health.read",
            ("/api/session", Method::Get) => "session.read",
            ("/api/version", Method::Get) => "version.read",
            _ => "unmatched",
        };
        let (result, phase) = match access::actor(&req, &env).await {
            Ok(Some(actor)) if path == "/api/doctor" && method == Method::Get =>
                (native_doctor(&env, &actor.tenant).await, "diagnostics"),
            Ok(Some(actor)) =>
                (dispatch(req, env, &actor.tenant, Some(&request_id), Some(&actor.role), Some(&actor.identity)).await, "domain_d1"),
            Ok(None) => (error("AUTH_REQUIRED", 401), "access"),
            Err(_) => (error("ACCESS_INTERNAL_ERROR", 503), "access"),
        };
        let mut reply = match result {
            Ok(reply) => reply,
            Err(_) => error("INTERNAL_ERROR", 500)?,
        };
        reply.headers_mut().set("x-request-id", &request_id)?;
        let module = if path == "/api/doctor" { "crm.diagnostics" }
            else if path.starts_with("/api/parties") ||
                (path.starts_with("/crm.parties.v2.PartyService/") ||
                 path.starts_with("/crm.parties.v3.PartyService/")) { "crm.parties" }
            else if path.starts_with("/api/accounts") { "crm.customer-accounts" }
            else if path.starts_with("/api/contact-points") { "crm.contact-points" }
            else if path.starts_with("/api/party-relationships") { "crm.party-relationships" }
            else if path.starts_with("/api/activity-tasks") { "crm.activities" }
            else if path.starts_with("/api/sales-deals") || path.starts_with("/crm.sales.v2.DealService/") { "crm.sales" }
            else if path.starts_with("/api/consents") { "crm.consents" }
            else if path.starts_with("/api/identity-resolution") { "crm.identity-resolution" }
            else if path.starts_with("/api/data-quality") { "crm.data-quality" }
            else if path.starts_with("/api/customer-data-operations") { "crm.customer-data-operations" }
            else if path.starts_with("/api/customer-privacy") { "crm.customer-privacy" }
            else if path.starts_with("/api/customer-enrichment") { "crm.customer-enrichment" }
            else { "crm.platform" };
        reply.headers_mut().set("x-crm-module", module)?;
        reply.headers_mut().set("x-crm-runtime", "native-workers-rs")?;
        if let Some(value) = source_sha {
            if value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                reply.headers_mut().set("x-crm-source-sha", &value)?;
            }
        }
        let elapsed = Date::now().as_millis().saturating_sub(started);
        let error_code = reply.headers().get("x-crm-error-code").ok().flatten();
        console_log!("{}", json!({
            "event":"crm.native.request", "request_id":request_id,
            "module":module,
            "operation":operation, "phase":phase,
            "status":reply.status_code(), "error_code":error_code, "duration_ms":elapsed
        }));
        return Ok(reply);
    }

    #[cfg(not(any(feature = "ci-local", feature = "native")))]
    {
        let _ = (req, env);
        error("AUTH_REQUIRED", 403)
    }
}
