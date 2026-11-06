//! One closed-pilot sign-in boundary. Not a second business authorization engine.
use crate::{ApplicationConfig, build_bootstrap_visibility_registry};
use crm_capability_adapters::{
    AuthorizationGrant, LiveAuthorizationStore, LiveQueryVisibilityStore, QueryVisibilityGrant,
};
use crm_capability_ingress::{AccessTokenGrant, AccessTokenStore};
use crm_capability_runtime::CapabilityDefinition;
use crm_module_sdk::{ActorId, ModuleId, RecordType, TenantId};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const COOKIE: &str = "crm_pilot_session";
const SESSION_SECONDS: i64 = 60 * 60 * 8;
const PASSWORD_MIN: usize = 32;
const POLICY: &str = "pilot-operator/v1";

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperatorInput {
    username: String,
    password: String,
    actor_id: String,
    tenant_id: String,
    capabilities: BTreeSet<String>,
}

#[derive(Clone)]
pub(crate) struct PilotOperator {
    pub(crate) username: String,
    password_digest: [u8; 32],
    pub(crate) actor_id: ActorId,
    pub(crate) tenant_id: TenantId,
    pub(crate) capabilities: BTreeSet<String>,
}

#[derive(Clone)]
struct PilotSession {
    username: String,
    csrf: String,
    expires_at_nanos: i64,
}

#[derive(Clone)]
pub(crate) struct PilotIdentity {
    operators: Arc<Vec<PilotOperator>>,
    sessions: Arc<Mutex<BTreeMap<[u8; 32], PilotSession>>>,
    tokens: AccessTokenStore,
    public_origin: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PilotSessionSnapshot {
    status: &'static str,
    tenant_id: String,
    actor_label: String,
    expires_at_unix_millis: i64,
    csrf_token: String,
    capabilities: Vec<String>,
}

pub(crate) struct PilotAuthenticated {
    pub(crate) authorization: String,
    pub(crate) snapshot: PilotSessionSnapshot,
}

fn now_nanos() -> Result<i64, ()> {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ())?;
    i64::try_from(d.as_nanos()).map_err(|_| ())
}

fn sha(input: &[u8]) -> [u8; 32] {
    let hash = digest::digest(&digest::SHA256, input);
    let mut out = [0_u8; 32];
    out.copy_from_slice(hash.as_ref());
    out
}

fn constant_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut mismatch = 0_u8;
    for (x, y) in a.iter().zip(b) {
        mismatch |= x ^ y;
    }
    mismatch == 0
}

fn secret() -> Result<String, ()> {
    let mut bytes = [0u8; 32];
    SystemRandom::new().fill(&mut bytes).map_err(|_| ())?;
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    Ok(out)
}

fn cookie_token(value: &str) -> Option<&str> {
    let mut result = None;
    for item in value.split(';') {
        let (name, token) = item.trim().split_once('=')?;
        if name == COOKIE {
            if result.is_some()
                || token.len() != 64
                || !token.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return None;
            }
            result = Some(token);
        }
    }
    result
}

impl PilotIdentity {
    pub(crate) fn from_environment(
        config: &ApplicationConfig,
        tokens: AccessTokenStore,
    ) -> Result<Option<Self>, &'static str> {
        let value = match std::env::var("CRM_PILOT_OPERATORS") {
            Ok(s) if !s.trim().is_empty() => s,
            Ok(_) | Err(std::env::VarError::NotPresent) => return Ok(None),
            Err(_) => return Err("CRM_PILOT_OPERATORS is invalid"),
        };
        let public_origin = std::env::var("CRM_PUBLIC_ORIGIN")
            .map_err(|_| "CRM_PUBLIC_ORIGIN is required for pilot login")?;
        if !public_origin.starts_with("https://")
            || public_origin.contains('/') && public_origin["https://".len()..].contains('/')
            || public_origin.contains('?')
            || public_origin.contains('#')
            || public_origin.len() < "https://a.b".len()
        {
            return Err("CRM_PUBLIC_ORIGIN must be an HTTPS origin without a path");
        }
        let input: Vec<OperatorInput> =
            serde_json::from_str(&value).map_err(|_| "CRM_PILOT_OPERATORS is not valid JSON")?;
        if input.is_empty() || input.len() > 16 {
            return Err("CRM_PILOT_OPERATORS requires 1 to 16 operators");
        }
        let mut names = BTreeSet::new();
        let mut actors = BTreeSet::new();
        let mut operators = Vec::with_capacity(input.len());
        for op in input {
            if op.username.is_empty()
                || op.username.len() > 80
                || !op
                    .username
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                || op.password.len() < PASSWORD_MIN
                || op.capabilities.is_empty()
                || !names.insert(op.username.clone())
            {
                return Err("CRM_PILOT_OPERATORS has invalid or duplicate operator");
            }
            let actor_id = ActorId::try_new(op.actor_id).map_err(|_| "invalid pilot actor")?;
            let tenant_id = TenantId::try_new(op.tenant_id).map_err(|_| "invalid pilot tenant")?;
            if !config.tenant_ids.contains(&tenant_id) || !actors.insert(actor_id.clone()) {
                return Err(
                    "pilot operator must have a unique actor and an active configured tenant",
                );
            }
            operators.push(PilotOperator {
                username: op.username,
                password_digest: sha(op.password.as_bytes()),
                actor_id,
                tenant_id,
                capabilities: op.capabilities,
            });
        }
        Ok(Some(Self {
            operators: Arc::new(operators),
            sessions: Arc::new(Mutex::new(BTreeMap::new())),
            tokens,
            public_origin,
        }))
    }

    pub(crate) fn provision(
        &self,
        grants: &LiveAuthorizationStore,
        visibility: &LiveQueryVisibilityStore,
        mutation: &[CapabilityDefinition],
        query: &[CapabilityDefinition],
    ) -> Result<(), &'static str> {
        let registry = build_bootstrap_visibility_registry()
            .map_err(|_| "pilot visibility registry unavailable")?;
        for operator in self.operators.iter() {
            let mut found = BTreeSet::new();
            for definition in mutation.iter().chain(query) {
                if !operator
                    .capabilities
                    .contains(definition.capability_id.as_str())
                {
                    continue;
                }
                found.insert(definition.capability_id.as_str().to_owned());
                grants
                    .upsert(AuthorizationGrant {
                        tenant_id: operator.tenant_id.clone(),
                        actor_id: operator.actor_id.clone(),
                        policy_id: definition.authorization_policy_id.clone(),
                        capability_id: definition.capability_id.clone(),
                        capability_version: definition.capability_version.clone(),
                        owner_module_id: definition.owner_module_id.clone(),
                        policy_version: POLICY.to_owned(),
                        expires_at_unix_nanos: None,
                    })
                    .map_err(|_| "pilot grant failed")?;
                if query
                    .iter()
                    .any(|candidate| candidate.capability_id == definition.capability_id)
                {
                    for resource in registry
                        .resources_for(definition)
                        .map_err(|_| "pilot query visibility unavailable")?
                    {
                        visibility
                            .upsert(QueryVisibilityGrant {
                                tenant_id: operator.tenant_id.clone(),
                                actor_id: operator.actor_id.clone(),
                                capability_id: definition.capability_id.clone(),
                                capability_version: definition.capability_version.clone(),
                                owner_module_id: ModuleId::try_new(resource.owner_module_id)
                                    .map_err(|_| "pilot resource owner invalid")?,
                                record_type: RecordType::try_new(resource.resource_type)
                                    .map_err(|_| "pilot resource type invalid")?,
                                record_id: None,
                                allowed_fields: resource.allowed_fields,
                                policy_version: POLICY.to_owned(),
                                expires_at_unix_nanos: None,
                            })
                            .map_err(|_| "pilot visibility grant failed")?;
                    }
                }
            }
            if found != operator.capabilities {
                return Err("pilot operator names a capability not in production composition");
            }
        }
        Ok(())
    }

    pub(crate) fn public_origin(&self) -> &str {
        &self.public_origin
    }

    pub(crate) fn sign_in(&self, name: &str, password: &str) -> Result<String, ()> {
        let digest = sha(password.as_bytes());
        // Compare every configured operator; do not reveal account existence.
        let operator = self
            .operators
            .iter()
            .find(|o| o.username == name && constant_equal(&o.password_digest, &digest))
            .ok_or(())?;
        let now = now_nanos()?;
        let until = now.checked_add(SESSION_SECONDS * 1_000_000_000).ok_or(())?;
        let token = secret()?;
        let csrf = secret()?;
        self.tokens
            .issue(
                token.as_bytes(),
                AccessTokenGrant {
                    actor_id: operator.actor_id.clone(),
                    tenant_ids: BTreeSet::from([operator.tenant_id.clone()]),
                    authentication_id: format!("pilot:{}", operator.username),
                    expires_at_unix_nanos: until,
                },
            )
            .map_err(|_| ())?;
        let mut sessions = self.sessions.lock().map_err(|_| ())?;
        sessions.retain(|_, v| v.expires_at_nanos > now);
        if sessions.len() >= 128 {
            self.tokens.revoke(token.as_bytes()).ok();
            return Err(());
        }
        sessions.insert(
            sha(token.as_bytes()),
            PilotSession {
                username: operator.username.clone(),
                csrf,
                expires_at_nanos: until,
            },
        );
        Ok(token)
    }

    pub(crate) fn authorize(
        &self,
        cookies: &str,
        csrf: Option<&str>,
    ) -> Option<PilotAuthenticated> {
        let token = cookie_token(cookies)?;
        let now = now_nanos().ok()?;
        let session = self
            .sessions
            .lock()
            .ok()?
            .get(&sha(token.as_bytes()))?
            .clone();
        if session.expires_at_nanos <= now {
            return None;
        }
        if let Some(provided) = csrf
            && !constant_equal(provided.as_bytes(), session.csrf.as_bytes())
        {
            return None;
        }
        let operator = self
            .operators
            .iter()
            .find(|o| o.username == session.username)?;
        Some(PilotAuthenticated {
            authorization: format!("Bearer {token}"),
            snapshot: PilotSessionSnapshot {
                status: "authenticated",
                tenant_id: operator.tenant_id.as_str().to_owned(),
                actor_label: operator.username.clone(),
                expires_at_unix_millis: session.expires_at_nanos / 1_000_000,
                csrf_token: session.csrf,
                capabilities: operator.capabilities.iter().cloned().collect(),
            },
        })
    }

    pub(crate) fn sign_out(&self, cookies: &str, csrf: &str) -> bool {
        let Some(token) = cookie_token(cookies) else {
            return false;
        };
        if self.authorize(cookies, Some(csrf)).is_none() {
            return false;
        }
        let removed = self
            .sessions
            .lock()
            .ok()
            .and_then(|mut s| s.remove(&sha(token.as_bytes())));
        if removed.is_none() {
            return false;
        }
        self.tokens.revoke(token.as_bytes()).is_ok()
    }
}

pub(crate) fn set_cookie(token: &str) -> String {
    format!(
        "{COOKIE}={token}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age={SESSION_SECONDS}"
    )
}
pub(crate) fn clear_cookie() -> String {
    format!("{COOKIE}=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_cookie_is_rejected() {
        let token = "a".repeat(64);
        assert_eq!(
            cookie_token(&format!("{COOKIE}={token}")),
            Some(token.as_str())
        );
        assert_eq!(
            cookie_token(&format!("{COOKIE}={token}; {COOKIE}={token}")),
            None
        );
        assert_eq!(cookie_token("x=1"), None);
        assert_eq!(cookie_token("x"), None);
    }

    #[test]
    fn secret_is_random_and_cookie_flags_are_strict() {
        let a = secret().unwrap();
        let b = secret().unwrap();
        assert_ne!(a, b);
        assert_eq!(a.len(), 64);
        assert!(set_cookie(&a).contains("HttpOnly; Secure; SameSite=Strict"));
        assert!(clear_cookie().contains("Max-Age=0"));
    }

    fn fixture() -> PilotIdentity {
        let configured = [
            (
                "alice",
                "01234567890123456789012345678901",
                "actor-a",
                "tenant-a",
            ),
            (
                "bob",
                "abcdefghijklmnopqrstuvwxyzABCDEF",
                "actor-b",
                "tenant-b",
            ),
        ];
        let operators = configured
            .into_iter()
            .map(|(username, password, actor, tenant)| PilotOperator {
                username: username.to_owned(),
                password_digest: sha(password.as_bytes()),
                actor_id: ActorId::try_new(actor).unwrap(),
                tenant_id: TenantId::try_new(tenant).unwrap(),
                capabilities: BTreeSet::from(["search.global.query".to_owned()]),
            })
            .collect();
        PilotIdentity {
            operators: Arc::new(operators),
            sessions: Arc::new(Mutex::new(BTreeMap::new())),
            tokens: AccessTokenStore::default(),
            public_origin: "https://crm.example.test".to_owned(),
        }
    }

    #[test]
    fn two_operators_get_distinct_tenant_bound_sessions_and_logout_revokes() {
        let pilot = fixture();
        assert!(pilot.sign_in("alice", "bad-password").is_err());
        let alice = pilot
            .sign_in("alice", "01234567890123456789012345678901")
            .unwrap();
        let bob = pilot
            .sign_in("bob", "abcdefghijklmnopqrstuvwxyzABCDEF")
            .unwrap();
        let alice_cookie = format!("{COOKIE}={alice}");
        let bob_cookie = format!("{COOKIE}={bob}");
        let a = pilot.authorize(&alice_cookie, None).unwrap();
        let b = pilot.authorize(&bob_cookie, None).unwrap();
        assert_eq!(a.snapshot.tenant_id, "tenant-a");
        assert_eq!(b.snapshot.tenant_id, "tenant-b");
        assert_ne!(a.authorization, b.authorization);
        assert!(pilot.authorize(&alice_cookie, Some("bad-csrf")).is_none());
        assert!(!pilot.sign_out(&alice_cookie, "bad-csrf"));
        assert!(pilot.authorize(&alice_cookie, None).is_some());
        assert!(pilot.sign_out(&alice_cookie, &a.snapshot.csrf_token));
        assert!(pilot.authorize(&alice_cookie, None).is_none());
        assert!(pilot.authorize(&bob_cookie, None).is_some());
    }

    #[test]
    fn expired_sessions_fail_closed() {
        let pilot = fixture();
        let token = pilot
            .sign_in("alice", "01234567890123456789012345678901")
            .unwrap();
        let cookie = format!("{COOKIE}={token}");
        pilot
            .sessions
            .lock()
            .unwrap()
            .get_mut(&sha(token.as_bytes()))
            .unwrap()
            .expires_at_nanos = 0;
        assert!(pilot.authorize(&cookie, None).is_none());
        assert!(!pilot.sign_out(&cookie, "invalid"));
    }

    #[test]
    fn compares_digest_in_constant_time_shape() {
        assert!(constant_equal(&sha(b"a"), &sha(b"a")));
        assert!(!constant_equal(&sha(b"a"), &sha(b"b")));
    }
}
