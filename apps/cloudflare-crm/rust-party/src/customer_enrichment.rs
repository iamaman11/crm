//! Original customer-enrichment immutable provider/mapping definitions.
//! Configuration only: no provider network, credentials, responses or Party updates.
use super::{access, bind};
use crm_customer_enrichment::{
    MappingDraft, MappingNormalization, MappingVersion, ProviderProfileDraft,
    ProviderProfileVersion, RawPayloadPolicy, TargetField, decode_mapping_version_state,
    decode_provider_profile_version_state, encode_mapping_version_state,
    encode_provider_profile_version_state,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use wasm_bindgen::JsValue;
use worker::{D1Database, Result};

const PROFILES: &str = "crm_enrichment_profiles";
const MAPPINGS: &str = "crm_enrichment_mappings";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProfileInput {
    pub(super) provider_key: String,
    pub(super) adapter_kind: String,
    pub(super) adapter_contract_version: String,
    pub(super) purpose_codes: Vec<String>,
    pub(super) license_id: String,
    pub(super) permitted_use_class: String,
    pub(super) residency_region: String,
    pub(super) retention_days: u32,
    pub(super) raw_payload_policy: RawPayloadPolicy,
    pub(super) credential_handle_aliases: Vec<String>,
    pub(super) effective_at_unix_ms: u64,
    pub(super) expires_at_unix_ms: Option<u64>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MappingInput {
    pub(super) mapping_key: String,
    pub(super) provider_profile_version_id: String,
    pub(super) provider_response_field_path: String,
    pub(super) maximum_suggestions_per_response: u32,
    pub(super) confidence_required: bool,
}
#[derive(Deserialize)]
struct StateRow {
    state: String,
}
#[derive(Deserialize)]
struct Receipt {
    original_request_id: Option<String>,
    kind: String,
    digest: String,
    state: String,
}

fn encode_profile(p: &ProviderProfileVersion) -> Result<String> {
    String::from_utf8(
        encode_provider_profile_version_state(p).map_err(|e| worker::Error::RustError(e.code))?,
    )
    .map_err(|e| worker::Error::RustError(e.to_string()))
}
fn decode_profile(s: &str) -> Result<ProviderProfileVersion> {
    decode_provider_profile_version_state(s.as_bytes())
        .map_err(|e| worker::Error::RustError(e.code))
}
fn encode_mapping(m: &MappingVersion) -> Result<String> {
    String::from_utf8(
        encode_mapping_version_state(m).map_err(|e| worker::Error::RustError(e.code))?,
    )
    .map_err(|e| worker::Error::RustError(e.to_string()))
}
fn decode_mapping(s: &str) -> Result<MappingVersion> {
    decode_mapping_version_state(s.as_bytes()).map_err(|e| worker::Error::RustError(e.code))
}
fn digest<T: Serialize>(kind: &str, input: &T) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&json!([kind, input])).expect("typed request serialization")
        )
    )
}
async fn receipt(db: &D1Database, tenant: &str, key: &str) -> Result<Option<Receipt>> {
    bind(
        db,
        "SELECT r.kind,r.digest,r.state,
          CASE r.kind WHEN 'profile' THEN (SELECT request_id FROM crm_enrichment_profiles p
            WHERE p.tenant=r.tenant AND p.idem_key=r.key AND p.idem_digest=r.digest AND p.state=r.state)
          WHEN 'mapping' THEN (SELECT request_id FROM crm_enrichment_mappings m
            WHERE m.tenant=r.tenant AND m.idem_key=r.key AND m.idem_digest=r.digest AND m.state=r.state)
          END AS original_request_id
         FROM crm_enrichment_receipts r WHERE r.tenant=? AND r.key=?",
        &[JsValue::from_str(tenant), JsValue::from_str(key)],
    )?
    .first::<Receipt>(None)
    .await
}
async fn state(db: &D1Database, table: &str, tenant: &str, id: &str) -> Result<Option<String>> {
    // The table comes only from the fixed module constants, never the request.
    let sql = format!("SELECT state FROM {table} WHERE tenant=? AND id=?");
    Ok(bind(
        db,
        &sql,
        &[JsValue::from_str(tenant), JsValue::from_str(id)],
    )?
    .first::<StateRow>(None)
    .await?
    .map(|r| r.state))
}
pub(super) async fn publish_profile(
    input: ProfileInput,
    k: &str,
    db: &D1Database,
    actor: &access::Actor,
    trace: &str,
) -> Result<std::result::Result<Saved<ProviderProfileVersion>, Failure>> {
    let tenant = actor.tenant.as_str();
    if !(8..=120).contains(&k.len()) {
        return Ok(Err(Failure::domain(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCode::InvalidArgument,
        )));
    }
    let hash = digest("profile", &input);
    let replay = |r: Receipt| {
        if r.kind != "profile" || r.digest != hash {
            return Ok(Err(Failure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        let profile = decode_profile(&r.state)?;
        Ok(Ok(Saved {
            value: profile,
            replayed: true,
            original_request_id: r.original_request_id,
        }))
    };
    if let Some(r) = receipt(db, tenant, k).await? {
        return replay(r);
    }
    let draft = ProviderProfileDraft {
        provider_key: input.provider_key,
        adapter_kind: input.adapter_kind,
        adapter_contract_version: input.adapter_contract_version,
        supported_target_fields: vec![TargetField::PartyDisplayName],
        purpose_codes: input.purpose_codes,
        license_id: input.license_id,
        permitted_use_class: input.permitted_use_class,
        residency_region: input.residency_region,
        retention_days: input.retention_days,
        raw_payload_policy: input.raw_payload_policy,
        credential_handle_aliases: input.credential_handle_aliases,
        effective_at_unix_ms: input.effective_at_unix_ms,
        expires_at_unix_ms: input.expires_at_unix_ms,
    };
    let profile = match ProviderProfileVersion::publish(draft) {
        Ok(x) => x,
        Err(e) => return Ok(Err(Failure::domain(e.code, ErrorCode::InvalidArgument))),
    };
    let saved = encode_profile(&profile)?;
    let inserted=bind(db,"INSERT INTO crm_enrichment_profiles (tenant,id,state,idem_key,idem_digest,request_id,created_by) VALUES(?,?,?,?,?,?,?) ON CONFLICT(tenant,id) DO NOTHING",
        &[JsValue::from_str(tenant),
          JsValue::from_str(profile.version_id().as_str()),
          JsValue::from_str(&saved),JsValue::from_str(k),
          JsValue::from_str(&hash),JsValue::from_str(trace),
          JsValue::from_str(&actor.identity)])?.run().await;
    match super::d1_write::confirm(
        inserted,
        receipt(db, tenant, k),
        "ENRICHMENT_PROFILE_CREATE_CONFLICT",
        "ENRICHMENT_WRITE_UNCONFIRMED",
    )
    .await
    {
        Ok(super::d1_write::Confirmation::Applied) => {}
        Ok(super::d1_write::Confirmation::Replayed(saved)) => return replay(saved),
        Err(failure) => return Ok(Err(Failure::Write(failure))),
    }
    Ok(Ok(Saved {
        value: profile,
        replayed: false,
        original_request_id: Some(trace.to_owned()),
    }))
}
pub(super) async fn publish_mapping(
    input: MappingInput,
    k: &str,
    db: &D1Database,
    actor: &access::Actor,
    trace: &str,
) -> Result<std::result::Result<Saved<MappingVersion>, Failure>> {
    let tenant = actor.tenant.as_str();
    if !(8..=120).contains(&k.len()) {
        return Ok(Err(Failure::domain(
            "IDEMPOTENCY_KEY_REQUIRED",
            ErrorCode::InvalidArgument,
        )));
    }
    let hash = digest("mapping", &input);
    let replay = |r: Receipt| {
        if r.kind != "mapping" || r.digest != hash {
            return Ok(Err(Failure::domain(
                "IDEMPOTENCY_CONFLICT",
                ErrorCode::Aborted,
            )));
        }
        Ok(Ok(Saved {
            value: decode_mapping(&r.state)?,
            replayed: true,
            original_request_id: r.original_request_id,
        }))
    };
    if let Some(r) = receipt(db, tenant, k).await? {
        return replay(r);
    }
    let Some(source) = state(db, PROFILES, tenant, &input.provider_profile_version_id).await?
    else {
        return Ok(Err(Failure::domain(
            "ENRICHMENT_PROVIDER_PROFILE_NOT_FOUND",
            ErrorCode::NotFound,
        )));
    };
    let profile = decode_profile(&source)?;
    // The only allowed target for this first usable native slice. The owner
    // domain validates the target-normalization combination and immutable ID.
    if !profile
        .supported_target_fields()
        .contains(&TargetField::PartyDisplayName)
    {
        return Ok(Err(Failure::domain(
            "ENRICHMENT_UNSUPPORTED_TARGET",
            ErrorCode::InvalidArgument,
        )));
    }
    let mapping = match MappingVersion::publish(MappingDraft {
        mapping_key: input.mapping_key,
        provider_profile_version_id: profile.version_id().clone(),
        provider_response_field_path: input.provider_response_field_path,
        target_field: TargetField::PartyDisplayName,
        normalization: MappingNormalization::CanonicalPartyDisplayNameV1,
        maximum_suggestions_per_response: input.maximum_suggestions_per_response,
        confidence_required: input.confidence_required,
    }) {
        Ok(x) => x,
        Err(e) => return Ok(Err(Failure::domain(e.code, ErrorCode::InvalidArgument))),
    };
    let saved = encode_mapping(&mapping)?;
    let inserted=bind(db,"INSERT INTO crm_enrichment_mappings (tenant,id,provider_profile_id,state,idem_key,idem_digest,request_id,created_by) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(tenant,id) DO NOTHING",
        &[JsValue::from_str(tenant),
          JsValue::from_str(mapping.version_id().as_str()),
          JsValue::from_str(&input.provider_profile_version_id),
          JsValue::from_str(&saved),JsValue::from_str(k),
          JsValue::from_str(&hash),JsValue::from_str(trace),
          JsValue::from_str(&actor.identity)])?.run().await;
    match super::d1_write::confirm(
        inserted,
        receipt(db, tenant, k),
        "ENRICHMENT_MAPPING_CREATE_CONFLICT",
        "ENRICHMENT_WRITE_UNCONFIRMED",
    )
    .await
    {
        Ok(super::d1_write::Confirmation::Applied) => {}
        Ok(super::d1_write::Confirmation::Replayed(saved)) => return replay(saved),
        Err(failure) => return Ok(Err(Failure::Write(failure))),
    }
    Ok(Ok(Saved {
        value: mapping,
        replayed: false,
        original_request_id: Some(trace.to_owned()),
    }))
}

pub(super) struct Saved<T> {
    pub(super) value: T,
    pub(super) replayed: bool,
    pub(super) original_request_id: Option<String>,
}
pub(super) use super::d1_write::OwnerFailure as Failure;
use connectrpc::ErrorCode;
pub(super) async fn profile(
    db: &D1Database,
    tenant: &str,
    id: &str,
) -> Result<Option<ProviderProfileVersion>> {
    state(db, PROFILES, tenant, id)
        .await?
        .map(|s| decode_profile(&s))
        .transpose()
}
pub(super) async fn mapping(
    db: &D1Database,
    tenant: &str,
    id: &str,
) -> Result<Option<MappingVersion>> {
    state(db, MAPPINGS, tenant, id)
        .await?
        .map(|s| decode_mapping(&s))
        .transpose()
}

#[derive(Deserialize)]
struct CommitRow {
    state: String,
    request_id: String,
}
async fn committed_row(
    db: &D1Database,
    table: &str,
    kind: &str,
    tenant: &str,
    key: &str,
) -> Result<Option<CommitRow>> {
    // The immutable owner row carries the original request id and digest.
    // Its existing trigger receipt must match tenant, kind, digest and snapshot.
    // No parallel receipt table or migration is required.
    let query = format!(
        "SELECT record.state,record.request_id FROM {table} AS record INNER JOIN crm_enrichment_receipts AS receipt ON receipt.tenant=record.tenant AND receipt.key=record.idem_key AND receipt.kind=? AND receipt.digest=record.idem_digest AND receipt.state=record.state WHERE record.tenant=? AND record.idem_key=? AND length(trim(record.request_id))>0"
    );
    bind(
        db,
        &query,
        &[
            JsValue::from_str(kind),
            JsValue::from_str(tenant),
            JsValue::from_str(key),
        ],
    )?
    .first::<CommitRow>(None)
    .await
}
pub(super) async fn profile_by_receipt(
    db: &D1Database,
    tenant: &str,
    key: &str,
) -> Result<Option<(ProviderProfileVersion, String)>> {
    let Some(row) = committed_row(db, PROFILES, "profile", tenant, key).await? else {
        return Ok(None);
    };
    Ok(Some((decode_profile(&row.state)?, row.request_id)))
}
pub(super) async fn mapping_by_receipt(
    db: &D1Database,
    tenant: &str,
    key: &str,
) -> Result<Option<(MappingVersion, String)>> {
    let Some(row) = committed_row(db, MAPPINGS, "mapping", tenant, key).await? else {
        return Ok(None);
    };
    Ok(Some((decode_mapping(&row.state)?, row.request_id)))
}
#[derive(Deserialize)]
struct PageRow {
    id: String,
    state: String,
}
async fn page(
    db: &D1Database,
    table: &str,
    tenant: &str,
    after: &str,
    size: usize,
) -> Result<(Vec<String>, Option<String>)> {
    // Table is supplied only by the two private constants above.
    let query = format!("SELECT id,state FROM {table} WHERE tenant=? AND id>? ORDER BY id LIMIT ?");
    let mut rows = bind(
        db,
        &query,
        &[
            JsValue::from_str(tenant),
            JsValue::from_str(after),
            JsValue::from_f64((size + 1) as f64),
        ],
    )?
    .all()
    .await?
    .results::<PageRow>()?;
    let next = if rows.len() > size {
        rows.truncate(size);
        rows.last().map(|r| r.id.clone())
    } else {
        None
    };
    Ok((rows.into_iter().map(|r| r.state).collect(), next))
}
pub(super) async fn profiles(
    db: &D1Database,
    tenant: &str,
    after: &str,
    size: usize,
) -> Result<(Vec<ProviderProfileVersion>, Option<String>)> {
    let (states, next) = page(db, PROFILES, tenant, after, size).await?;
    Ok((
        states
            .iter()
            .map(|s| decode_profile(s))
            .collect::<Result<Vec<_>>>()?,
        next,
    ))
}
pub(super) async fn mappings(
    db: &D1Database,
    tenant: &str,
    after: &str,
    size: usize,
) -> Result<(Vec<MappingVersion>, Option<String>)> {
    let (states, next) = page(db, MAPPINGS, tenant, after, size).await?;
    Ok((
        states
            .iter()
            .map(|s| decode_mapping(s))
            .collect::<Result<Vec<_>>>()?,
        next,
    ))
}
