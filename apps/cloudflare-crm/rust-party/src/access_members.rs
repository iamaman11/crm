//! SK-07.B: original Access authority for human membership in crm_users.
//! The signer, tenant and role are ALWAYS the cryptographically verified Actor;
//! the UI may propose an email, but never provide an identity or tenant claim.
use super::{bind, d1_write};
use connectrpc::ErrorCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Member {
    pub email: String,
    pub role: String,
    pub active: i64,
    pub version: i64,
}
#[derive(Deserialize)]
struct Receipt {
    digest: String,
    email: String,
    state: String,
    original_request_id: String,
}
pub(super) struct Saved {
    pub member: Member,
    pub replayed: bool,
    pub original_request_id: String,
}
pub(super) type Failure = d1_write::OwnerFailure;
pub(super) struct MemberContext<'a> {
    pub(super) db: &'a D1Database,
    pub(super) tenant: &'a str,
    pub(super) actor: &'a str,
}

fn reject(code: &'static str, kind: ErrorCode) -> Failure {
    Failure::domain(code, kind)
}
pub(super) fn email(raw: &str) -> std::result::Result<String, Failure> {
    // Human Access identity only; a signed service name is never a member.
    // The normalized email is the exact admission key in crm_users.
    let normal = raw.trim().to_lowercase();
    if raw.trim() != raw
        || normal.is_empty()
        || normal.len() > 254
        || !normal.contains('@')
        || normal.starts_with('@')
        || normal.ends_with('@')
        || normal.chars().any(char::is_whitespace)
        || normal.starts_with("service:")
        || normal.contains('/')
        || normal.contains('\\')
    {
        return Err(reject(
            "ACCESS_MEMBER_EMAIL_INVALID",
            ErrorCode::InvalidArgument,
        ));
    }
    Ok(normal)
}
pub(super) fn role(raw: &str) -> std::result::Result<&'static str, Failure> {
    match raw {
        "admin" => Ok("admin"),
        "viewer" => Ok("viewer"),
        _ => Err(reject(
            "ACCESS_MEMBER_ROLE_INVALID",
            ErrorCode::InvalidArgument,
        )),
    }
}
pub(super) async fn get(db: &D1Database, tenant: &str, member: &str) -> Result<Option<Member>> {
    bind(
        db,
        "SELECT email,role,active,version FROM crm_users WHERE tenant_id=? AND email=?",
        &[JsValue::from_str(tenant), JsValue::from_str(member)],
    )?
    .first::<Member>(None)
    .await
}
pub(super) async fn list(
    db: &D1Database,
    tenant: &str,
    after: &str,
    size: usize,
    include_inactive: bool,
) -> Result<(Vec<Member>, Option<String>)> {
    let sql = if include_inactive {
        "SELECT email,role,active,version FROM crm_users WHERE tenant_id=? AND email>? ORDER BY email LIMIT ?"
    } else {
        "SELECT email,role,active,version FROM crm_users WHERE tenant_id=? AND active=1 AND email>? ORDER BY email LIMIT ?"
    };
    let rows = bind(
        db,
        sql,
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(after),
            JsValue::from_f64((size + 1) as f64),
        ],
    )?
    .all()
    .await?
    .results::<Member>()?;
    let next = if rows.len() > size {
        rows.get(size - 1).map(|m| m.email.clone())
    } else {
        None
    };
    Ok((rows.into_iter().take(size).collect(), next))
}
async fn prior(db: &D1Database, tenant: &str, key: &str) -> Result<Option<Receipt>> {
    bind(db,
      "SELECT digest,email,state,original_request_id FROM crm_member_receipts WHERE tenant=? AND key=?",
      &[JsValue::from_str(tenant),JsValue::from_str(key)])?
      .first::<Receipt>(None).await
}
pub(super) async fn proof(
    db: &D1Database,
    tenant: &str,
    email: &str,
    key: &str,
) -> Result<Option<(String, i64)>> {
    let Some(p) = prior(db, tenant, key).await? else {
        return Ok(None);
    };
    let member: Member = serde_json::from_str(&p.state)
        .map_err(|e| worker::Error::RustError(format!("member receipt decode: {e}")))?;
    if p.email != email
        || member.email != email
        || member.version <= 0
        || p.original_request_id.is_empty()
    {
        return Ok(None);
    }
    Ok(Some((p.original_request_id, member.version)))
}
fn digest(action: &str, email: &str, role: &str, active: &str, version: i64) -> Result<String> {
    let bytes = serde_json::to_vec(&json!([action, email, role, active, version]))
        .map_err(|e| worker::Error::RustError(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
async fn replay(
    db: &D1Database,
    tenant: &str,
    key: &str,
    email: &str,
    digest: &str,
) -> Result<Option<std::result::Result<Saved, Failure>>> {
    let Some(p) = prior(db, tenant, key).await? else {
        return Ok(None);
    };
    if p.digest != digest || p.email != email {
        return Ok(Some(Err(reject(
            "IDEMPOTENCY_CONFLICT",
            ErrorCode::Aborted,
        ))));
    }
    let member: Member = serde_json::from_str(&p.state)
        .map_err(|e| worker::Error::RustError(format!("member receipt decode: {e}")))?;
    Ok(Some(Ok(Saved {
        member,
        replayed: true,
        original_request_id: p.original_request_id,
    })))
}
async fn confirm(
    ctx: &MemberContext<'_>,
    key: &str,
    email: &str,
    digest: &str,
    trace: &str,
    write: Result<worker::D1Result>,
    zero: &'static str,
) -> Result<std::result::Result<Saved, Failure>> {
    match d1_write::confirm(
        write,
        prior(ctx.db, ctx.tenant, key),
        zero,
        "ACCESS_MEMBER_WRITE_UNCONFIRMED",
    )
    .await
    {
        Ok(d1_write::Confirmation::Applied) => {
            let member = get(ctx.db, ctx.tenant, email)
                .await?
                .ok_or_else(|| worker::Error::RustError("member write missing".into()))?;
            Ok(Ok(Saved {
                member,
                replayed: false,
                original_request_id: trace.into(),
            }))
        }
        Ok(d1_write::Confirmation::Replayed(p)) => {
            if p.digest != digest || p.email != email {
                return Ok(Err(reject("IDEMPOTENCY_CONFLICT", ErrorCode::Aborted)));
            }
            let member: Member = serde_json::from_str(&p.state)
                .map_err(|e| worker::Error::RustError(format!("member receipt decode: {e}")))?;
            Ok(Ok(Saved {
                member,
                replayed: true,
                original_request_id: p.original_request_id,
            }))
        }
        Err(failure) => Ok(Err(Failure::Write(failure))),
    }
}
pub(super) async fn provision(
    ctx: &MemberContext<'_>,
    member: &str,
    new_role: &str,
    key: &str,
    trace: &str,
) -> Result<std::result::Result<Saved, Failure>> {
    let member = match email(member) {
        Ok(v) => v,
        Err(e) => return Ok(Err(e)),
    };
    let new_role = match role(new_role) {
        Ok(v) => v,
        Err(e) => return Ok(Err(e)),
    };
    let hash = digest("provision", &member, new_role, "true", 0)?;
    if let Some(existing) = replay(ctx.db, ctx.tenant, key, &member, &hash).await? {
        return Ok(existing);
    }
    let write=bind(ctx.db,
      "INSERT INTO crm_users(email,tenant_id,role,active,version,idem_key,idem_digest,request_id,actor) VALUES(?,?,?,1,1,?,?,?,?) ON CONFLICT(email) DO NOTHING",
      &[JsValue::from_str(&member),JsValue::from_str(ctx.tenant),
        JsValue::from_str(new_role),JsValue::from_str(key),
        JsValue::from_str(&hash),JsValue::from_str(trace),
        JsValue::from_str(ctx.actor)])?.run().await;
    confirm(
        ctx,
        key,
        &member,
        &hash,
        trace,
        write,
        "ACCESS_MEMBER_ALREADY_EXISTS",
    )
    .await
}
pub(super) async fn update(
    ctx: &MemberContext<'_>,
    member: &str,
    new_role: Option<&str>,
    active: Option<bool>,
    expected: i64,
    key: &str,
    trace: &str,
) -> Result<std::result::Result<Saved, Failure>> {
    let member = match email(member) {
        Ok(v) => v,
        Err(e) => return Ok(Err(e)),
    };
    if ctx.actor == member {
        return Ok(Err(reject(
            "ACCESS_MEMBER_SELF_LOCKOUT",
            ErrorCode::PermissionDenied,
        )));
    }
    if new_role.is_none() && active.is_none() {
        return Ok(Err(reject(
            "ACCESS_MEMBER_NO_CHANGES",
            ErrorCode::InvalidArgument,
        )));
    }
    let selected = match new_role {
        Some(value) => match role(value) {
            Ok(v) => Some(v),
            Err(e) => return Ok(Err(e)),
        },
        None => None,
    };
    // Presence matters: Keep and explicit Set(false) are different intents.
    let hash = digest(
        "update",
        &member,
        selected.unwrap_or("keep"),
        active
            .map(|v| if v { "true" } else { "false" })
            .unwrap_or("keep"),
        expected,
    )?;
    if let Some(existing) = replay(ctx.db, ctx.tenant, key, &member, &hash).await? {
        return Ok(existing);
    }
    // One atomic CAS, prevents disabling the last active administrator even
    // if two administrators race. No preflight read authorizes the update.
    // Deactivating a human cannot strand open shared work in any of their
    // teams. The roster and other eligible members are checked in this CAS.
    let work = super::access_teams::open_work("tm.tenant", "tm.id");
    let coverage = super::sales_territories::owner_in_use("?8", "?7", false);
    let sql = format!(
        "UPDATE crm_users SET role=COALESCE(?,role),active=COALESCE(?,active),version=version+1,idem_key=?,idem_digest=?,request_id=?,actor=? WHERE email=? AND tenant_id=? AND version=? AND email<>? AND (role<>'admin' OR (COALESCE(?,role)='admin' AND COALESCE(?,active)=1) OR (SELECT COUNT(*) FROM crm_users WHERE tenant_id=? AND active=1 AND role='admin')>1) AND (active=0 OR COALESCE(?14,active)=1 OR NOT EXISTS (SELECT 1 FROM crm_access_teams AS tm WHERE tm.tenant=?8 AND json_extract(tm.state,'$.active')=1 AND EXISTS (SELECT 1 FROM json_each(tm.state,'$.member_emails') AS target WHERE target.value=?7) AND NOT EXISTS (SELECT 1 FROM json_each(tm.state,'$.member_emails') AS roster JOIN crm_users AS u ON u.tenant_id=tm.tenant AND u.email=roster.value AND u.active=1 WHERE u.email<>?7 AND u.email NOT LIKE 'service:%') AND {work})) AND (active=0 OR COALESCE(?14,active)=1 OR NOT {coverage})"
    );
    let write = bind(
        ctx.db,
        &sql,
        &[
            selected.map(JsValue::from_str).unwrap_or(JsValue::NULL),
            active
                .map(|v| JsValue::from_f64(if v { 1.0 } else { 0.0 }))
                .unwrap_or(JsValue::NULL),
            JsValue::from_str(key),
            JsValue::from_str(&hash),
            JsValue::from_str(trace),
            JsValue::from_str(ctx.actor),
            JsValue::from_str(&member),
            JsValue::from_str(ctx.tenant),
            JsValue::from_f64(expected as f64),
            JsValue::from_str(ctx.actor),
            selected.map(JsValue::from_str).unwrap_or(JsValue::NULL),
            active
                .map(|v| JsValue::from_f64(if v { 1.0 } else { 0.0 }))
                .unwrap_or(JsValue::NULL),
            JsValue::from_str(ctx.tenant),
            active
                .map(|v| JsValue::from_f64(if v { 1.0 } else { 0.0 }))
                .unwrap_or(JsValue::NULL),
        ],
    )?
    .run()
    .await;
    confirm(
        ctx,
        key,
        &member,
        &hash,
        trace,
        write,
        "ACCESS_MEMBER_CONFLICT",
    )
    .await
}
pub(super) async fn valid_assignee(db: &D1Database, tenant: &str, target: &str) -> Result<bool> {
    Ok(bind(
        db,
        "SELECT 1 AS eligible FROM crm_users WHERE tenant_id=? AND email=? AND active=1",
        &[JsValue::from_str(tenant), JsValue::from_str(target)],
    )?
    .first::<serde_json::Value>(None)
    .await?
    .is_some())
}
