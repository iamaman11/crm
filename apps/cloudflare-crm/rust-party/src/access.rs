//! Cloudflare Access admission for the eventual single native Rust Worker.
//! The Static Assets router does not forward ctx.access, so the signed Access JWT
//! must be independently verified. Never trust caller-provided tenant headers.
use js_sys::{Array, JSON, Object, Reflect};
use serde_json::Value;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Crypto, CryptoKey};
#[cfg(not(feature = "native-ci"))]
use worker::Fetch;
use worker::{D1Database, Date, Env, Request, Result};

const ISSUER: &str = "https://royal-limit-ea43.cloudflareaccess.com";
const AUDIENCE: &str = "e0a664cf4f46f0d13d0ac0d8dd621c718c70842a377bf6738d1a24f0d16cedc7";

fn invalid(message: &str) -> worker::Error {
    worker::Error::RustError(format!("Access verification: {message}"))
}

fn prop(object: &JsValue, name: &str) -> Result<JsValue> {
    Reflect::get(object, &JsValue::from_str(name))
        .map_err(|_| invalid("platform crypto property unavailable"))
}

// Only standard Web Crypto primitives run on the Cloudflare host. No custom
// signing, RSA implementation, JS auth middleware, or second Worker.
async fn verify_signature(jwk: &Value, input: &str, signature: &[u8]) -> Result<bool> {
    let crypto: Crypto = prop(&js_sys::global(), "crypto")?
        .dyn_into()
        .map_err(|_| invalid("crypto unavailable"))?;
    let subtle = crypto.subtle();
    let algorithm = Object::new();
    Reflect::set(&algorithm, &"name".into(), &"RSASSA-PKCS1-v1_5".into())
        .map_err(|_| invalid("crypto algorithm"))?;
    Reflect::set(&algorithm, &"hash".into(), &"SHA-256".into())
        .map_err(|_| invalid("crypto hash"))?;
    let jwk_js = JSON::parse(&jwk.to_string()).map_err(|_| invalid("invalid JSON Web Key"))?;
    let usages = Array::new();
    usages.push(&"verify".into());
    let imported = subtle
        .import_key_with_object(
            "jwk",
            &jwk_js.unchecked_into::<Object>(),
            &algorithm,
            false,
            &usages,
        )
        .map_err(|_| invalid("public key import rejected"))?;
    let key: CryptoKey = JsFuture::from(imported)
        .await
        .map_err(|_| invalid("public key import failed"))?
        .dyn_into()
        .map_err(|_| invalid("invalid imported public key"))?;
    let verified = subtle
        .verify_with_str_and_u8_array_and_u8_array(
            "RSASSA-PKCS1-v1_5",
            &key,
            signature,
            input.as_bytes(),
        )
        .map_err(|_| invalid("signature verification rejected"))?;
    Ok(JsFuture::from(verified)
        .await
        .map_err(|_| invalid("signature verification failed"))?
        .as_bool()
        == Some(true))
}

fn decoded(segment: &str) -> Option<Vec<u8>> {
    if segment.is_empty()
        || segment.len() > 16_384
        || !segment
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return None;
    }
    // atob is a host platform primitive; no hand-written base64 codec.
    let mut padded = segment.replace('-', "+").replace('_', "/");
    if padded.len() % 4 == 1 {
        return None;
    }
    while !padded.len().is_multiple_of(4) {
        padded.push('=');
    }
    let function = prop(&js_sys::global(), "atob")
        .ok()?
        .dyn_into::<js_sys::Function>()
        .ok()?;
    let binary = function
        .call1(&JsValue::NULL, &padded.into())
        .ok()?
        .as_string()?;
    binary
        .chars()
        .map(|c| u8::try_from(c as u32).ok())
        .collect()
}

// Cloudflare Access service-token JWTs carry an EMPTY sub but a signed
// common_name (their Client ID). Human JWTs carry a nonempty sub and email.
// Keep these separate so a service JWT cannot impersonate an email principal.
fn identity_key(claims: &Value) -> Option<String> {
    // Access sometimes omits sub on signed non-identity service assertions.
    // Absent and empty sub both require a valid signed common_name; a malformed
    // non-string sub still fails closed. Human identities always need sub+email.
    let sub = match claims.get("sub") {
        Some(Value::String(sub)) => sub.as_str(),
        None => "",
        _ => return None,
    };
    if sub.is_empty() {
        let name = claims.get("common_name").and_then(Value::as_str)?;
        if (8..=120).contains(&name.len())
            && name.ends_with(".access")
            && name
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-')
        {
            return Some(format!("service:{}", name.to_ascii_lowercase()));
        }
        return None;
    }
    let email = claims.get("email").and_then(Value::as_str)?;
    let email = email.trim().to_lowercase();
    if !email.is_empty()
        && email.len() <= 254
        && email.contains('@')
        && !email.chars().any(char::is_whitespace)
    {
        Some(email)
    } else {
        None
    }
}

fn valid_claims(claims: &Value) -> bool {
    let now = (Date::now().as_millis() / 1000) as i64;
    let aud_ok = match claims.get("aud") {
        Some(Value::String(s)) => s == AUDIENCE,
        Some(Value::Array(items)) => items.iter().any(|x| x.as_str() == Some(AUDIENCE)),
        _ => false,
    };
    claims.get("iss").and_then(Value::as_str) == Some(ISSUER)
        && aud_ok
        && claims.get("exp").and_then(Value::as_i64).is_some_and(|exp| exp > now)
        && claims.get("nbf").and_then(Value::as_i64).is_none_or(|nbf| nbf <= now)
        && claims.get("iat").and_then(Value::as_i64).is_none_or(|iat| iat <= now + 60)
        // identity_key checks presence of sub and requires either a signed
        // human email with nonempty sub, or a service common_name with empty sub.
        && identity_key(claims).is_some()
}

// Report only a fixed rejection category; never emit JWT claims or credentials.
fn invalid_claim_reason(claims: &Value) -> &'static str {
    let now = (Date::now().as_millis() / 1000) as i64;
    if claims.get("iss").and_then(Value::as_str) != Some(ISSUER) {
        return "jwt_issuer_invalid";
    }
    let aud_ok = match claims.get("aud") {
        Some(Value::String(s)) => s == AUDIENCE,
        Some(Value::Array(items)) => items.iter().any(|x| x.as_str() == Some(AUDIENCE)),
        _ => false,
    };
    if !aud_ok {
        return "jwt_audience_invalid";
    }
    if !claims
        .get("exp")
        .and_then(Value::as_i64)
        .is_some_and(|exp| exp > now)
    {
        return "jwt_exp_invalid";
    }
    if !claims
        .get("nbf")
        .and_then(Value::as_i64)
        .is_none_or(|nbf| nbf <= now)
    {
        return "jwt_nbf_future";
    }
    if !claims
        .get("iat")
        .and_then(Value::as_i64)
        .is_none_or(|iat| iat <= now + 60)
    {
        return "jwt_iat_future";
    }
    if identity_key(claims).is_none() {
        // These are static categories only: no token/claim values are logged.
        return match claims.get("sub") {
            None => "jwt_identity_sub_absent",
            Some(Value::String(sub)) if sub.is_empty() => match claims.get("common_name") {
                None => "jwt_service_name_absent",
                Some(Value::String(name)) if name.is_empty() => "jwt_service_name_empty",
                Some(Value::String(name)) if !(8..=120).contains(&name.len()) => {
                    "jwt_service_name_length"
                }
                Some(Value::String(name)) if !name.ends_with(".access") => {
                    "jwt_service_name_suffix"
                }
                Some(Value::String(_)) => "jwt_service_name_charset",
                _ => "jwt_service_name_type",
            },
            Some(Value::String(_)) => match claims.get("email") {
                Some(Value::String(_)) => "jwt_human_email_invalid",
                _ => "jwt_human_email_absent",
            },
            _ => "jwt_identity_sub_type",
        };
    }
    "jwt_claims_invalid_other"
}

async fn verify(req: &Request, env: &Env) -> Result<Option<String>> {
    // Only the release version probe emits a safe, static rejection category.
    // Never log or expose Access assertions, service credentials, or claims.
    let reject = |reason: &'static str| -> Result<Option<String>> {
        if req.path().starts_with("/crm.")
            || matches!(req.path().as_str(), "/api/version" | "/api/session")
        {
            worker::console_log!(
                "{}",
                serde_json::json!({"event": "crm.auth.reject", "reason": reason})
            );
        }
        Ok(None)
    };
    let token = match req.headers().get("cf-access-jwt-assertion")? {
        Some(t) if t.len() <= 16_384 => t,
        _ => return reject("jwt_header_absent_or_oversize"),
    };
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 3 {
        return reject("jwt_segments_invalid");
    }
    let (Some(header), Some(payload), Some(signature)) =
        (decoded(parts[0]), decoded(parts[1]), decoded(parts[2]))
    else {
        return reject("jwt_base64_invalid");
    };
    let Ok(header) = serde_json::from_slice::<Value>(&header) else {
        return reject("jwt_header_json_invalid");
    };
    let Ok(claims) = serde_json::from_slice::<Value>(&payload) else {
        return reject("jwt_payload_json_invalid");
    };
    if header.get("alg").and_then(Value::as_str) != Some("RS256") {
        return reject("jwt_alg_invalid");
    }
    if !valid_claims(&claims) {
        return reject(invalid_claim_reason(&claims));
    }
    let Some(kid) = header
        .get("kid")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 256)
    else {
        return reject("jwt_key_id_invalid");
    };
    #[cfg(feature = "native-ci")]
    let jwks: Value = {
        // Fail-closed: test public keys are accepted by a CI-only binary on loopback.
        let host = req.url()?.host_str().unwrap_or_default().to_owned();
        if host != "localhost" && host != "127.0.0.1" {
            return reject("ci_host_invalid");
        }
        if env.var("CRM_CI_LOCAL_ONLY")?.to_string() != "yes" {
            return reject("ci_fixture_missing");
        }
        serde_json::from_str(&env.var("CRM_NATIVE_CI_JWKS")?.to_string())
            .map_err(|_| invalid("invalid local JWKS fixture"))?
    };
    #[cfg(not(feature = "native-ci"))]
    let jwks: Value = {
        let url = format!("{ISSUER}/cdn-cgi/access/certs");
        let mut response = Fetch::Url(url.parse()?).send().await?;
        if response.status_code() != 200 {
            return reject("jwks_http_not_200");
        }
        response.json().await?
    };
    let key = jwks.get("keys").and_then(Value::as_array).and_then(|keys| {
        keys.iter().find(|k| {
            k.get("kid").and_then(Value::as_str) == Some(kid)
                && k.get("kty").and_then(Value::as_str) == Some("RSA")
                && k.get("n").and_then(Value::as_str).is_some()
                && k.get("e").and_then(Value::as_str).is_some()
                && k.get("use")
                    .and_then(Value::as_str)
                    .is_none_or(|u| u == "sig")
                && k.get("alg")
                    .and_then(Value::as_str)
                    .is_none_or(|a| a == "RS256")
        })
    });
    let Some(key) = key else {
        return reject("jwks_key_not_found");
    };
    if !verify_signature(key, &format!("{}.{}", parts[0], parts[1]), &signature).await? {
        return reject("jwt_signature_invalid");
    }
    Ok(identity_key(&claims))
}

// Signed Cloudflare Access identity, one tenant, no JS actor delegation.
#[derive(Clone)]
pub struct Actor {
    pub tenant: String,
    pub role: String,
    // Verified Access identity, never supplied by the HTTP request body.
    pub identity: String,
}
impl Actor {
    // The CRM deployment is a dev/test application. Reuse the already enrolled
    // signed service administrator for unattended UI acceptance. It remains a
    // SERVICE actor in the audit; this never turns it into a human identity.
    pub fn is_dev_test_agent(&self) -> bool {
        self.role == "admin" && self.identity.starts_with("service:")
    }
}
async fn enrolled(db: &D1Database, identity: &str) -> Result<Option<Actor>> {
    #[derive(serde::Deserialize)]
    struct Row {
        tenant_id: String,
        role: String,
    }
    let user = db
        .prepare("SELECT tenant_id, role FROM crm_users WHERE email = ? AND active = 1")
        .bind(&[JsValue::from_str(identity)])?
        .first::<Row>(None)
        .await?;
    Ok(user.map(|row| Actor {
        tenant: row.tenant_id,
        role: row.role,
        identity: identity.to_owned(),
    }))
}

pub async fn actor(req: &Request, env: &Env) -> Result<Option<Actor>> {
    let Some(identity) = verify(req, env).await? else {
        return Ok(None);
    };
    let db = env.d1("DB")?;
    if let Some(actor) = enrolled(&db, &identity).await? {
        return Ok(Some(actor));
    }
    // Signed service principals are never self-enrolled. Granting a new
    // service its own admin tenant would silently split it from the owner's
    // existing CRM and bypass the explicit provisioning decision. Only an
    // already active crm_users row may authorize a service identity.
    if identity.starts_with("service:") {
        if req.path().starts_with("/crm.")
            || matches!(req.path().as_str(), "/api/version" | "/api/session")
        {
            worker::console_log!(
                "{}",
                serde_json::json!({"event": "crm.auth.reject", "reason": "service_not_enrolled"})
            );
        }
        return Ok(None);
    }
    // Preserve the existing CRM onboarding contract for human identities:
    // a verified previously unknown person receives one isolated test workspace.
    // This is the only enrollment path; user-supplied identity/tenant headers
    // never become trusted. INSERT OR IGNORE handles simultaneous first visits.
    let tenant = super::native_request_id()?;
    db.batch(vec![
        db.prepare("INSERT OR IGNORE INTO crm_tenants (id, name) VALUES (?, ?)")
            .bind(&[JsValue::from_str(&tenant), JsValue::from_str("Test CRM")])?,
        db.prepare("INSERT OR IGNORE INTO crm_users (email, tenant_id, role) VALUES (?, ?, ?)")
            .bind(&[
                JsValue::from_str(&identity),
                JsValue::from_str(&tenant),
                JsValue::from_str("admin"),
            ])?,
    ])
    .await?;
    enrolled(&db, &identity).await
}

#[cfg(test)]
mod tests {
    use super::identity_key;
    use serde_json::json;

    #[test]
    fn service_assertion_without_sub_requires_signed_client_identity() {
        let client = "e367826f93b8d71185e03fe518aff3b4.access";
        let expected = Some(format!("service:{client}"));
        assert_eq!(identity_key(&json!({"sub": "", "common_name": client})), expected);
        assert_eq!(
            identity_key(&json!({"common_name": client})),
            Some(format!("service:{client}"))
        );
        assert_eq!(identity_key(&json!({"sub": 9, "common_name": client})), None);
        assert_eq!(identity_key(&json!({"common_name": "invalid"})), None);
        assert_eq!(identity_key(&json!({"email": "owner@example.test"})), None);
        assert_eq!(
            identity_key(&json!({"sub": "human", "common_name": client})),
            None
        );
        assert_eq!(
            identity_key(&json!({"sub": "human", "email": "Owner@Example.test"})),
            Some("owner@example.test".to_owned())
        );
    }
}
