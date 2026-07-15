use crm_module_sdk::{ActorId, TenantId};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fmt;
use std::net::SocketAddr;

const MINIMUM_SECRET_BYTES: usize = 32;
const NANOS_PER_SECOND: u64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationConfig {
    pub database_url: String,
    pub http_bind: SocketAddr,
    pub grpc_bind: SocketAddr,
    pub bearer_token: String,
    pub actor_id: ActorId,
    pub tenant_ids: BTreeSet<TenantId>,
    pub cursor_signing_key: Vec<u8>,
    pub approval_signing_key: Vec<u8>,
    pub default_timeout_millis: u64,
    pub maximum_timeout_millis: u64,
    pub query_default_page_size: u32,
    pub query_maximum_page_size: u32,
    pub query_scan_multiplier: u32,
    pub maximum_connections: u32,
    pub bootstrap_allow_phase6: bool,
    pub export_retention_policies: BTreeMap<String, u64>,
}

impl ApplicationConfig {
    pub fn from_env() -> Result<Self, ApplicationConfigError> {
        let config = Self {
            database_url: required("CRM_DATABASE_URL")?,
            http_bind: parse_or_default("CRM_HTTP_BIND", "127.0.0.1:8080")?,
            grpc_bind: parse_or_default("CRM_GRPC_BIND", "127.0.0.1:9090")?,
            bearer_token: required("CRM_API_BEARER_TOKEN")?,
            actor_id: ActorId::try_new(required("CRM_API_ACTOR_ID")?)
                .map_err(|_| ApplicationConfigError::Invalid("CRM_API_ACTOR_ID"))?,
            tenant_ids: parse_tenants(&required("CRM_API_TENANTS")?)?,
            cursor_signing_key: secret("CRM_CURSOR_SIGNING_KEY")?,
            approval_signing_key: secret("CRM_APPROVAL_SIGNING_KEY")?,
            default_timeout_millis: parse_or_default("CRM_DEFAULT_TIMEOUT_MILLIS", "5000")?,
            maximum_timeout_millis: parse_or_default("CRM_MAXIMUM_TIMEOUT_MILLIS", "30000")?,
            query_default_page_size: parse_or_default("CRM_QUERY_DEFAULT_PAGE_SIZE", "50")?,
            query_maximum_page_size: parse_or_default("CRM_QUERY_MAXIMUM_PAGE_SIZE", "200")?,
            query_scan_multiplier: parse_or_default("CRM_QUERY_SCAN_MULTIPLIER", "4")?,
            maximum_connections: parse_or_default("CRM_DATABASE_MAX_CONNECTIONS", "16")?,
            bootstrap_allow_phase6: parse_bool("CRM_BOOTSTRAP_ALLOW_PHASE6", false)?,
            export_retention_policies: parse_retention_policies(
                env::var("CRM_EXPORT_RETENTION_POLICIES").ok().as_deref(),
            )?,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ApplicationConfigError> {
        if self.database_url.is_empty()
            || self.bearer_token.is_empty()
            || self.tenant_ids.is_empty()
            || self.cursor_signing_key.len() < MINIMUM_SECRET_BYTES
            || self.approval_signing_key.len() < MINIMUM_SECRET_BYTES
            || self.default_timeout_millis == 0
            || self.maximum_timeout_millis == 0
            || self.default_timeout_millis > self.maximum_timeout_millis
            || self.query_default_page_size == 0
            || self.query_maximum_page_size == 0
            || self.query_default_page_size > self.query_maximum_page_size
            || self.query_scan_multiplier == 0
            || self.maximum_connections == 0
            || self
                .export_retention_policies
                .iter()
                .any(|(policy_id, seconds)| {
                    policy_id.is_empty()
                        || policy_id.chars().any(char::is_control)
                        || *seconds == 0
                        || seconds.checked_mul(NANOS_PER_SECOND).is_none()
                        || seconds
                            .checked_mul(NANOS_PER_SECOND)
                            .and_then(|value| i64::try_from(value).ok())
                            .is_none()
                })
        {
            return Err(ApplicationConfigError::Invalid("application configuration"));
        }
        if self.http_bind == self.grpc_bind {
            return Err(ApplicationConfigError::Invalid("listener addresses"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationConfigError {
    Missing(&'static str),
    Invalid(&'static str),
}

impl fmt::Display for ApplicationConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(name) => write!(formatter, "required configuration is missing: {name}"),
            Self::Invalid(name) => write!(formatter, "configuration is invalid: {name}"),
        }
    }
}

impl Error for ApplicationConfigError {}

fn required(name: &'static str) -> Result<String, ApplicationConfigError> {
    env::var(name)
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or(ApplicationConfigError::Missing(name))
}

fn secret(name: &'static str) -> Result<Vec<u8>, ApplicationConfigError> {
    let value = required(name)?.into_bytes();
    if value.len() < MINIMUM_SECRET_BYTES {
        return Err(ApplicationConfigError::Invalid(name));
    }
    Ok(value)
}

fn parse_or_default<T>(
    name: &'static str,
    default: &'static str,
) -> Result<T, ApplicationConfigError>
where
    T: std::str::FromStr,
{
    env::var(name)
        .unwrap_or_else(|_| default.to_owned())
        .parse::<T>()
        .map_err(|_| ApplicationConfigError::Invalid(name))
}

fn parse_bool(name: &'static str, default: bool) -> Result<bool, ApplicationConfigError> {
    match env::var(name) {
        Ok(value) => match value.to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Ok(true),
            "false" | "0" | "no" => Ok(false),
            _ => Err(ApplicationConfigError::Invalid(name)),
        },
        Err(_) => Ok(default),
    }
}

fn parse_tenants(value: &str) -> Result<BTreeSet<TenantId>, ApplicationConfigError> {
    let tenants = value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            TenantId::try_new(value.to_owned())
                .map_err(|_| ApplicationConfigError::Invalid("CRM_API_TENANTS"))
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if tenants.is_empty() {
        return Err(ApplicationConfigError::Invalid("CRM_API_TENANTS"));
    }
    Ok(tenants)
}

fn parse_retention_policies(
    value: Option<&str>,
) -> Result<BTreeMap<String, u64>, ApplicationConfigError> {
    let Some(value) = value else {
        return Ok(BTreeMap::new());
    };
    let mut policies = BTreeMap::new();
    for entry in value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let (policy_id, seconds) = entry
            .split_once('=')
            .ok_or(ApplicationConfigError::Invalid(
                "CRM_EXPORT_RETENTION_POLICIES",
            ))?;
        let policy_id = policy_id.trim();
        let seconds = seconds
            .trim()
            .parse::<u64>()
            .map_err(|_| ApplicationConfigError::Invalid("CRM_EXPORT_RETENTION_POLICIES"))?;
        if policy_id.is_empty()
            || policy_id.chars().any(char::is_control)
            || seconds == 0
            || seconds
                .checked_mul(NANOS_PER_SECOND)
                .and_then(|value| i64::try_from(value).ok())
                .is_none()
            || policies.insert(policy_id.to_owned(), seconds).is_some()
        {
            return Err(ApplicationConfigError::Invalid(
                "CRM_EXPORT_RETENTION_POLICIES",
            ));
        }
    }
    Ok(policies)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_listener_collision_and_short_secrets() {
        let mut config = valid_config();
        config.grpc_bind = config.http_bind;
        assert!(config.validate().is_err());

        let mut config = valid_config();
        config.cursor_signing_key = vec![1; 31];
        assert!(config.validate().is_err());
    }

    #[test]
    fn retention_policy_configuration_is_explicit_and_bounded() {
        assert_eq!(
            parse_retention_policies(Some("standard=3600,short=60")).unwrap(),
            BTreeMap::from([("short".to_owned(), 60), ("standard".to_owned(), 3600)])
        );
        assert!(parse_retention_policies(Some("standard=0")).is_err());
        assert!(parse_retention_policies(Some("standard=60,standard=120")).is_err());
        assert!(parse_retention_policies(Some("missing-separator")).is_err());
    }

    fn valid_config() -> ApplicationConfig {
        ApplicationConfig {
            database_url: "postgres://example".to_owned(),
            http_bind: "127.0.0.1:18080".parse().unwrap(),
            grpc_bind: "127.0.0.1:19090".parse().unwrap(),
            bearer_token: "token".to_owned(),
            actor_id: ActorId::try_new("actor-1").unwrap(),
            tenant_ids: BTreeSet::from([TenantId::try_new("tenant-1").unwrap()]),
            cursor_signing_key: vec![1; 32],
            approval_signing_key: vec![2; 32],
            default_timeout_millis: 5_000,
            maximum_timeout_millis: 30_000,
            query_default_page_size: 50,
            query_maximum_page_size: 200,
            query_scan_multiplier: 4,
            maximum_connections: 16,
            bootstrap_allow_phase6: false,
            export_retention_policies: BTreeMap::from([("standard".to_owned(), 3_600)]),
        }
    }
}
