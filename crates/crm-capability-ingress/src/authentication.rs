use crm_module_sdk::{ActorId, Clock, PortFuture, TenantId};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, RwLock};

const MINIMUM_BEARER_TOKEN_BYTES: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedPrincipal {
    pub actor_id: ActorId,
    pub tenant_ids: BTreeSet<TenantId>,
    pub authentication_id: String,
}

impl AuthenticatedPrincipal {
    pub fn permits_tenant(&self, tenant_id: &TenantId) -> bool {
        self.tenant_ids.contains(tenant_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessTokenGrant {
    pub actor_id: ActorId,
    pub tenant_ids: BTreeSet<TenantId>,
    pub authentication_id: String,
    pub expires_at_unix_nanos: i64,
}

impl AccessTokenGrant {
    fn validate(&self) -> Result<(), AuthenticationStoreError> {
        if self.tenant_ids.is_empty()
            || self.authentication_id.is_empty()
            || self.expires_at_unix_nanos <= 0
        {
            return Err(AuthenticationStoreError::InvalidGrant);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum AuthenticationStoreError {
    TokenTooShort,
    InvalidGrant,
    DuplicateToken,
    Poisoned,
}

impl fmt::Display for AuthenticationStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TokenTooShort => "bearer token does not meet the minimum entropy length",
            Self::InvalidGrant => "access-token grant is invalid",
            Self::DuplicateToken => "access token already exists",
            Self::Poisoned => "access-token store lock is poisoned",
        })
    }
}

impl Error for AuthenticationStoreError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredToken {
    grant: AccessTokenGrant,
    revoked: bool,
}

#[derive(Debug, Default)]
struct AccessTokenState {
    revision: u64,
    tokens: BTreeMap<[u8; 32], StoredToken>,
}

#[derive(Debug, Clone, Default)]
pub struct AccessTokenStore {
    state: Arc<RwLock<AccessTokenState>>,
}

impl AccessTokenStore {
    pub fn issue(
        &self,
        bearer_token: &[u8],
        grant: AccessTokenGrant,
    ) -> Result<u64, AuthenticationStoreError> {
        validate_token(bearer_token)?;
        grant.validate()?;
        let digest = token_digest(bearer_token);
        let mut state = self
            .state
            .write()
            .map_err(|_| AuthenticationStoreError::Poisoned)?;
        if state.tokens.contains_key(&digest) {
            return Err(AuthenticationStoreError::DuplicateToken);
        }
        state.revision = state.revision.saturating_add(1);
        state.tokens.insert(
            digest,
            StoredToken {
                grant,
                revoked: false,
            },
        );
        Ok(state.revision)
    }

    pub fn revoke(&self, bearer_token: &[u8]) -> Result<bool, AuthenticationStoreError> {
        validate_token(bearer_token)?;
        let digest = token_digest(bearer_token);
        let mut state = self
            .state
            .write()
            .map_err(|_| AuthenticationStoreError::Poisoned)?;
        let Some(token) = state.tokens.get_mut(&digest) else {
            return Ok(false);
        };
        if token.revoked {
            return Ok(false);
        }
        token.revoked = true;
        state.revision = state.revision.saturating_add(1);
        Ok(true)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthenticationError {
    MissingCredentials,
    InvalidCredentials,
    ExpiredCredentials,
    RevokedCredentials,
    StoreUnavailable,
}

impl AuthenticationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::MissingCredentials => "AUTHENTICATION_REQUIRED",
            Self::InvalidCredentials => "AUTHENTICATION_INVALID",
            Self::ExpiredCredentials => "AUTHENTICATION_EXPIRED",
            Self::RevokedCredentials => "AUTHENTICATION_REVOKED",
            Self::StoreUnavailable => "AUTHENTICATION_UNAVAILABLE",
        }
    }

    pub fn retryable(&self) -> bool {
        matches!(self, Self::StoreUnavailable)
    }
}

impl fmt::Display for AuthenticationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingCredentials => "authentication credentials are required",
            Self::InvalidCredentials => "authentication credentials are invalid",
            Self::ExpiredCredentials => "authentication credentials have expired",
            Self::RevokedCredentials => "authentication credentials were revoked",
            Self::StoreUnavailable => "authentication is temporarily unavailable",
        })
    }
}

impl Error for AuthenticationError {}

pub trait RequestAuthenticator: Send + Sync {
    fn authenticate<'a>(
        &'a self,
        authorization_value: &'a str,
    ) -> PortFuture<'a, Result<AuthenticatedPrincipal, AuthenticationError>>;
}

#[derive(Clone)]
pub struct BearerTokenAuthenticator {
    store: AccessTokenStore,
    clock: Arc<dyn Clock>,
}

impl fmt::Debug for BearerTokenAuthenticator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BearerTokenAuthenticator")
            .field("store", &self.store)
            .field("clock", &"dyn Clock")
            .finish()
    }
}

impl BearerTokenAuthenticator {
    pub fn new(store: AccessTokenStore, clock: Arc<dyn Clock>) -> Self {
        Self { store, clock }
    }
}

impl RequestAuthenticator for BearerTokenAuthenticator {
    fn authenticate<'a>(
        &'a self,
        authorization_value: &'a str,
    ) -> PortFuture<'a, Result<AuthenticatedPrincipal, AuthenticationError>> {
        Box::pin(async move {
            let bearer_token = parse_bearer(authorization_value)?;
            let digest = token_digest(bearer_token);
            let state = self
                .store
                .state
                .read()
                .map_err(|_| AuthenticationError::StoreUnavailable)?;
            let stored = state
                .tokens
                .get(&digest)
                .ok_or(AuthenticationError::InvalidCredentials)?;
            if stored.revoked {
                return Err(AuthenticationError::RevokedCredentials);
            }
            if stored.grant.expires_at_unix_nanos <= self.clock.now_unix_nanos() {
                return Err(AuthenticationError::ExpiredCredentials);
            }
            Ok(AuthenticatedPrincipal {
                actor_id: stored.grant.actor_id.clone(),
                tenant_ids: stored.grant.tenant_ids.clone(),
                authentication_id: stored.grant.authentication_id.clone(),
            })
        })
    }
}

fn parse_bearer(value: &str) -> Result<&[u8], AuthenticationError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AuthenticationError::MissingCredentials);
    }
    let token = value
        .strip_prefix("Bearer ")
        .ok_or(AuthenticationError::InvalidCredentials)?;
    validate_token(token.as_bytes()).map_err(|_| AuthenticationError::InvalidCredentials)?;
    Ok(token.as_bytes())
}

fn validate_token(token: &[u8]) -> Result<(), AuthenticationStoreError> {
    if token.len() < MINIMUM_BEARER_TOKEN_BYTES {
        return Err(AuthenticationStoreError::TokenTooShort);
    }
    Ok(())
}

fn token_digest(token: &[u8]) -> [u8; 32] {
    Sha256::digest(token).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crm_module_sdk::testing::FixedClock;

    const TOKEN: &[u8] = b"0123456789abcdef0123456789abcdef";

    #[tokio::test]
    async fn authenticates_without_storing_plaintext_and_honors_revocation() {
        let clock = Arc::new(FixedClock::new(100));
        let store = AccessTokenStore::default();
        let tenant = TenantId::try_new("tenant-1").unwrap();
        store
            .issue(
                TOKEN,
                AccessTokenGrant {
                    actor_id: ActorId::try_new("actor-1").unwrap(),
                    tenant_ids: BTreeSet::from([tenant.clone()]),
                    authentication_id: "session-1".to_owned(),
                    expires_at_unix_nanos: 1_000,
                },
            )
            .unwrap();
        let authenticator = BearerTokenAuthenticator::new(store.clone(), clock);

        let principal = authenticator
            .authenticate("Bearer 0123456789abcdef0123456789abcdef")
            .await
            .unwrap();
        assert_eq!(principal.actor_id.as_str(), "actor-1");
        assert!(principal.permits_tenant(&tenant));

        store.revoke(TOKEN).unwrap();
        assert_eq!(
            authenticator
                .authenticate("Bearer 0123456789abcdef0123456789abcdef")
                .await
                .unwrap_err(),
            AuthenticationError::RevokedCredentials
        );
    }

    #[tokio::test]
    async fn rejects_expired_token() {
        let clock = Arc::new(FixedClock::new(1_000));
        let store = AccessTokenStore::default();
        store
            .issue(
                TOKEN,
                AccessTokenGrant {
                    actor_id: ActorId::try_new("actor-1").unwrap(),
                    tenant_ids: BTreeSet::from([TenantId::try_new("tenant-1").unwrap()]),
                    authentication_id: "session-1".to_owned(),
                    expires_at_unix_nanos: 1_000,
                },
            )
            .unwrap();
        let authenticator = BearerTokenAuthenticator::new(store, clock);

        assert_eq!(
            authenticator
                .authenticate("Bearer 0123456789abcdef0123456789abcdef")
                .await
                .unwrap_err(),
            AuthenticationError::ExpiredCredentials
        );
    }
}
