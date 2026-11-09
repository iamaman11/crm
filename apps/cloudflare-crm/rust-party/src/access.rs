//! Cloudflare Access admission for the eventual single native Rust Worker.
//! The Static Assets router does not forward ctx.access, so the signed Access JWT
//! must be independently verified. Never trust caller-provided tenant headers.
use js_sys::{Array, JSON, Object, Reflect};
use serde_json::Value;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use worker::{Date, D1Database, Env, Request, Result};
#[cfg(not(feature = "native-ci"))]
use worker::Fetch;
use web_sys::{Crypto, CryptoKey};

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
        .dyn_into().map_err(|_| invalid("crypto unavailable"))?;
    let subtle = crypto.subtle();
    let algorithm = Object::new();
    Reflect::set(&algorithm, &"name".into(), &"RSASSA-PKCS1-v1_5".into())
        .map_err(|_| invalid("crypto algorithm"))?;
    Reflect::set(&algorithm, &"hash".into(), &"SHA-256".into())
        .map_err(|_| invalid("crypto hash"))?;
    let jwk_js = JSON::parse(&jwk.to_string())
        .map_err(|_| invalid("invalid JSON Web Key"))?;
    let usages = Array::new();
    usages.push(&"verify".into());
    let imported = subtle.import_key_with_object(
        "jwk", &jwk_js.unchecked_into::<Object>(), &algorithm, false, &usages,
    ).map_err(|_| invalid("public key import rejected"))?;
    let key: CryptoKey = JsFuture::from(imported).await
        .map_err(|_| invalid("public key import failed"))?
        .dyn_into().map_err(|_| invalid("invalid imported public key"))?;
    let verified = subtle.verify_with_str_and_u8_array_and_u8_array(
        "RSASSA-PKCS1-v1_5", &key, signature, input.as_bytes(),
    ).map_err(|_| invalid("signature verification rejected"))?;
    Ok(JsFuture::from(verified).await
        .map_err(|_| invalid("signature verification failed"))?
        .as_bool() == Some(true))
}

fn decoded(segment: &str) -> Option<Vec<u8>> {
    if segment.is_empty() || segment.len() > 16_384
        || !segment.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return None;
    }
    // atob is a host platform primitive; no hand-written base64 codec.
    let mut padded = segment.replace('-', "+").replace('_', "/");
    if padded.len() % 4 == 1 { return None; }
    while padded.len() % 4 != 0 { padded.push('='); }
    let function = prop(&js_sys::global(), "atob").ok()?
        .dyn_into::<js_sys::Function>().ok()?;
    let binary = function.call1(&JsValue::NULL, &padded.into()).ok()?.as_string()?;
    binary.chars().map(|c| u8::try_from(c as u32).ok()).collect()
}

// Cloudflare Access service-token JWTs carry an EMPTY sub but a signed
// common_name (their Client ID). Human JWTs carry a nonempty sub and email.
// Keep these separate so a service JWT cannot impersonate an email principal.
fn identity_key(claims: &Value) -> Option<String> {
    let sub = claims.get("sub").and_then(Value::as_str)?;
    if sub.is_empty() {
        let name = claims.get("common_name").and_then(Value::as_str)?;
        if (8..=120).contains(&name.len()) && name.ends_with(".access")
            && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-')
        {
            return Some(format!("service:{}", name.to_ascii_lowercase()));
        }
        return None;
    }
    let email = claims.get("email").and_then(Value::as_str)?;
    let email = email.trim().to_lowercase();
    if !email.is_empty() && email.len() <= 254 && email.contains('@')
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

async fn verify(req: &Request, env: &Env) -> Result<Option<String>> {
    let token = match req.headers().get("cf-access-jwt-assertion")? {
        Some(t) if t.len() <= 16_384 => t,
        _ => return Ok(None),
    };
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 3 { return Ok(None); }
    let (Some(header), Some(payload), Some(signature)) =
        (decoded(parts[0]), decoded(parts[1]), decoded(parts[2]))
    else { return Ok(None); };
    let Ok(header) = serde_json::from_slice::<Value>(&header) else { return Ok(None); };
    let Ok(claims) = serde_json::from_slice::<Value>(&payload) else { return Ok(None); };
    if header.get("alg").and_then(Value::as_str) != Some("RS256")
        || !valid_claims(&claims)
    {
        return Ok(None);
    }
    let Some(kid) = header.get("kid").and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 256)
    else { return Ok(None); };
    #[cfg(feature = "native-ci")]
    let jwks: Value = {
        // Fail-closed: test public keys are accepted by a CI-only binary on loopback.
        let host = req.url()?.host_str().unwrap_or_default().to_owned();
        if host != "localhost" && host != "127.0.0.1" {
            return Ok(None);
        }
        if env.var("CRM_CI_LOCAL_ONLY")?.to_string() != "yes" {
            return Ok(None);
        }
        serde_json::from_str(&env.var("CRM_NATIVE_CI_JWKS")?.to_string())
            .map_err(|_| invalid("invalid local JWKS fixture"))?
    };
    #[cfg(not(feature = "native-ci"))]
    let jwks: Value = {
        let url = format!("{ISSUER}/cdn-cgi/access/certs");
        let mut response = Fetch::Url(url.parse()?).send().await?;
        if response.status_code() != 200 { return Ok(None); }
        response.json().await?
    };
    let key = jwks.get("keys").and_then(Value::as_array)
        .and_then(|keys| keys.iter().find(|k| {
            k.get("kid").and_then(Value::as_str) == Some(kid)
                && k.get("kty").and_then(Value::as_str) == Some("RSA")
                && k.get("n").and_then(Value::as_str).is_some()
                && k.get("e").and_then(Value::as_str).is_some()
                && k.get("use").and_then(Value::as_str).is_none_or(|u| u == "sig")
                && k.get("alg").and_then(Value::as_str).is_none_or(|a| a == "RS256")
        }));
    let Some(key) = key else { return Ok(None); };
    if !verify_signature(key, &format!("{}.{}", parts[0], parts[1]), &signature).await? {
        return Ok(None);
    }
    Ok(identity_key(&claims))
}

async fn enrolled(db: &D1Database, identity: &str) -> Result<Option<String>> {
    #[derive(serde::Deserialize)]
    struct Row { tenant_id: String }
    let tenant = db.prepare("SELECT tenant_id FROM crm_users WHERE email = ? AND active = 1")
        .bind(&[JsValue::from_str(identity)])?
        .first::<Row>(None).await?;
    Ok(tenant.map(|row| row.tenant_id))
}

// This candidate is deliberately read-only for identity enrollment:
// it cannot create a tenant based on a forged or incomplete claim. The native
// release requires a validated enrollment implementation before cutover.
pub async fn tenant(req: &Request, env: &Env) -> Result<Option<String>> {
    let Some(identity) = verify(req, env).await? else { return Ok(None); };
    enrolled(&env.d1("DB")?, &identity).await
}
