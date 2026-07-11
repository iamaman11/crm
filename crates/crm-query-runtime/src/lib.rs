#![forbid(unsafe_code)]

//! Governed primitives for permission-bound CRM query execution.
//!
//! Page tokens are opaque transport values. They are integrity-protected and
//! bound to the authenticated/query context that created them so clients cannot
//! move a continuation across tenants, actors, capabilities, filters or sorts.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use crm_module_sdk::{ActorId, CapabilityId, CapabilityVersion, RecordId, RecordType, TenantId};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;

const CURSOR_MAGIC: &[u8; 4] = b"CRMQ";
const CURSOR_VERSION: u8 = 1;
const MAC_BYTES: usize = 32;
const MINIMUM_SIGNING_KEY_BYTES: usize = 32;
pub const MAXIMUM_CURSOR_TOKEN_BYTES: usize = 2_048;
pub const MAXIMUM_SORT_KEY_BYTES: usize = 512;
pub const MAXIMUM_SORT_ID_BYTES: usize = 128;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorBinding {
    pub tenant_id: TenantId,
    pub actor_id: Option<ActorId>,
    pub capability_id: CapabilityId,
    pub capability_version: CapabilityVersion,
    pub resource_type: RecordType,
    pub normalized_filter_hash: [u8; 32],
    pub sort_id: String,
    pub page_size: u32,
}

impl CursorBinding {
    pub fn validate(&self) -> Result<(), CursorError> {
        if self.sort_id.is_empty() || self.sort_id.len() > MAXIMUM_SORT_ID_BYTES {
            return Err(CursorError::InvalidBinding);
        }
        if self.page_size == 0 {
            return Err(CursorError::InvalidBinding);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorContinuation {
    pub sort_key: Vec<u8>,
    pub record_id: RecordId,
}

impl CursorContinuation {
    pub fn validate(&self) -> Result<(), CursorError> {
        if self.sort_key.len() > MAXIMUM_SORT_KEY_BYTES {
            return Err(CursorError::ContinuationTooLarge);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct CursorCodec {
    signing_key: Vec<u8>,
}

impl fmt::Debug for CursorCodec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CursorCodec")
            .field("signing_key", &"<redacted>")
            .finish()
    }
}

impl CursorCodec {
    pub fn new(signing_key: impl AsRef<[u8]>) -> Result<Self, CursorError> {
        let signing_key = signing_key.as_ref();
        if signing_key.len() < MINIMUM_SIGNING_KEY_BYTES {
            return Err(CursorError::SigningKeyTooShort);
        }
        Ok(Self {
            signing_key: signing_key.to_vec(),
        })
    }

    pub fn encode(
        &self,
        binding: &CursorBinding,
        continuation: &CursorContinuation,
    ) -> Result<String, CursorError> {
        binding.validate()?;
        continuation.validate()?;

        let payload = encode_payload(binding, continuation)?;
        let tag = self.sign(&payload)?;
        let mut token_bytes = Vec::with_capacity(payload.len() + tag.len());
        token_bytes.extend_from_slice(&payload);
        token_bytes.extend_from_slice(&tag);
        let token = URL_SAFE_NO_PAD.encode(token_bytes);
        if token.len() > MAXIMUM_CURSOR_TOKEN_BYTES {
            return Err(CursorError::TokenTooLarge);
        }
        Ok(token)
    }

    pub fn decode(
        &self,
        token: &str,
        expected_binding: &CursorBinding,
    ) -> Result<CursorContinuation, CursorError> {
        expected_binding.validate()?;
        if token.is_empty() || token.len() > MAXIMUM_CURSOR_TOKEN_BYTES {
            return Err(CursorError::MalformedToken);
        }
        let token_bytes = URL_SAFE_NO_PAD
            .decode(token)
            .map_err(|_| CursorError::MalformedToken)?;
        if token_bytes.len() <= MAC_BYTES {
            return Err(CursorError::MalformedToken);
        }
        let split = token_bytes.len() - MAC_BYTES;
        let (payload, tag) = token_bytes.split_at(split);
        self.verify(payload, tag)?;
        let (actual_binding, continuation) = decode_payload(payload)?;
        if &actual_binding != expected_binding {
            return Err(CursorError::BindingMismatch);
        }
        Ok(continuation)
    }

    fn sign(&self, payload: &[u8]) -> Result<[u8; MAC_BYTES], CursorError> {
        let mut mac = HmacSha256::new_from_slice(&self.signing_key)
            .map_err(|_| CursorError::SigningUnavailable)?;
        mac.update(payload);
        Ok(mac.finalize().into_bytes().into())
    }

    fn verify(&self, payload: &[u8], tag: &[u8]) -> Result<(), CursorError> {
        let mut mac = HmacSha256::new_from_slice(&self.signing_key)
            .map_err(|_| CursorError::SigningUnavailable)?;
        mac.update(payload);
        mac.verify_slice(tag)
            .map_err(|_| CursorError::IntegrityFailed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageSizePolicy {
    pub default_size: u32,
    pub maximum_size: u32,
}

impl PageSizePolicy {
    pub fn validate(self) -> Result<Self, CursorError> {
        if self.default_size == 0 || self.maximum_size == 0 || self.default_size > self.maximum_size
        {
            return Err(CursorError::InvalidPagePolicy);
        }
        Ok(self)
    }

    pub fn resolve(self, requested: i32) -> Result<u32, CursorError> {
        self.validate()?;
        if requested < 0 {
            return Err(CursorError::InvalidPageSize);
        }
        if requested == 0 {
            return Ok(self.default_size);
        }
        let requested = u32::try_from(requested).map_err(|_| CursorError::InvalidPageSize)?;
        if requested > self.maximum_size {
            return Err(CursorError::PageSizeTooLarge);
        }
        Ok(requested)
    }
}

pub fn normalized_filter_hash<'a>(
    fields: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> [u8; 32] {
    let mut fields = fields.into_iter().collect::<Vec<_>>();
    fields.sort_by(|left, right| left.0.cmp(right.0).then(left.1.cmp(right.1)));
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"crm.query-filter/v1");
    for (name, value) in fields {
        hash_field(&mut hasher, name.as_bytes());
        hash_field(&mut hasher, value);
    }
    hasher.finalize().into()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorError {
    SigningKeyTooShort,
    InvalidBinding,
    ContinuationTooLarge,
    TokenTooLarge,
    MalformedToken,
    IntegrityFailed,
    BindingMismatch,
    UnsupportedVersion,
    InvalidStoredValue,
    SigningUnavailable,
    InvalidPagePolicy,
    InvalidPageSize,
    PageSizeTooLarge,
}

impl CursorError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::SigningKeyTooShort | Self::SigningUnavailable => {
                "QUERY_CURSOR_CONFIGURATION_INVALID"
            }
            Self::InvalidBinding => "QUERY_CURSOR_BINDING_INVALID",
            Self::ContinuationTooLarge | Self::TokenTooLarge => "QUERY_CURSOR_TOO_LARGE",
            Self::MalformedToken | Self::InvalidStoredValue => "QUERY_CURSOR_INVALID",
            Self::IntegrityFailed => "QUERY_CURSOR_TAMPERED",
            Self::BindingMismatch => "QUERY_CURSOR_BINDING_MISMATCH",
            Self::UnsupportedVersion => "QUERY_CURSOR_VERSION_UNSUPPORTED",
            Self::InvalidPagePolicy => "QUERY_PAGE_POLICY_INVALID",
            Self::InvalidPageSize => "QUERY_PAGE_SIZE_INVALID",
            Self::PageSizeTooLarge => "QUERY_PAGE_SIZE_EXCEEDS_LIMIT",
        }
    }

    pub const fn safe_message(&self) -> &'static str {
        match self {
            Self::SigningKeyTooShort | Self::SigningUnavailable | Self::InvalidPagePolicy => {
                "The query service is temporarily unavailable."
            }
            Self::PageSizeTooLarge => "The requested page size exceeds the allowed limit.",
            Self::InvalidPageSize => "The requested page size is invalid.",
            Self::ContinuationTooLarge | Self::TokenTooLarge => "The page cursor is too large.",
            Self::UnsupportedVersion => "The page cursor version is not supported.",
            Self::BindingMismatch => "The page cursor does not belong to this query.",
            Self::InvalidBinding
            | Self::MalformedToken
            | Self::IntegrityFailed
            | Self::InvalidStoredValue => "The page cursor is invalid.",
        }
    }
}

impl fmt::Display for CursorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.safe_message())
    }
}

impl Error for CursorError {}

fn encode_payload(
    binding: &CursorBinding,
    continuation: &CursorContinuation,
) -> Result<Vec<u8>, CursorError> {
    let mut output = Vec::new();
    output.extend_from_slice(CURSOR_MAGIC);
    output.push(CURSOR_VERSION);
    push_string(&mut output, binding.tenant_id.as_str())?;
    match &binding.actor_id {
        Some(actor_id) => {
            output.push(1);
            push_string(&mut output, actor_id.as_str())?;
        }
        None => output.push(0),
    }
    push_string(&mut output, binding.capability_id.as_str())?;
    push_string(&mut output, binding.capability_version.as_str())?;
    push_string(&mut output, binding.resource_type.as_str())?;
    output.extend_from_slice(&binding.normalized_filter_hash);
    push_string(&mut output, &binding.sort_id)?;
    output.extend_from_slice(&binding.page_size.to_be_bytes());
    push_bytes(&mut output, &continuation.sort_key)?;
    push_string(&mut output, continuation.record_id.as_str())?;
    Ok(output)
}

fn decode_payload(payload: &[u8]) -> Result<(CursorBinding, CursorContinuation), CursorError> {
    let mut reader = Reader::new(payload);
    if reader.take(CURSOR_MAGIC.len())? != CURSOR_MAGIC {
        return Err(CursorError::MalformedToken);
    }
    let version = reader.byte()?;
    if version != CURSOR_VERSION {
        return Err(CursorError::UnsupportedVersion);
    }
    let tenant_id =
        TenantId::try_new(reader.string()?).map_err(|_| CursorError::InvalidStoredValue)?;
    let actor_id = match reader.byte()? {
        0 => None,
        1 => Some(ActorId::try_new(reader.string()?).map_err(|_| CursorError::InvalidStoredValue)?),
        _ => return Err(CursorError::InvalidStoredValue),
    };
    let capability_id =
        CapabilityId::try_new(reader.string()?).map_err(|_| CursorError::InvalidStoredValue)?;
    let capability_version = CapabilityVersion::try_new(reader.string()?)
        .map_err(|_| CursorError::InvalidStoredValue)?;
    let resource_type =
        RecordType::try_new(reader.string()?).map_err(|_| CursorError::InvalidStoredValue)?;
    let mut normalized_filter_hash = [0_u8; 32];
    normalized_filter_hash.copy_from_slice(reader.take(32)?);
    let sort_id = reader.string()?;
    let page_size = u32::from_be_bytes(
        reader
            .take(4)?
            .try_into()
            .map_err(|_| CursorError::InvalidStoredValue)?,
    );
    let sort_key = reader.bytes()?;
    let record_id =
        RecordId::try_new(reader.string()?).map_err(|_| CursorError::InvalidStoredValue)?;
    if !reader.is_finished() {
        return Err(CursorError::InvalidStoredValue);
    }
    let binding = CursorBinding {
        tenant_id,
        actor_id,
        capability_id,
        capability_version,
        resource_type,
        normalized_filter_hash,
        sort_id,
        page_size,
    };
    binding.validate()?;
    let continuation = CursorContinuation {
        sort_key,
        record_id,
    };
    continuation.validate()?;
    Ok((binding, continuation))
}

fn push_string(output: &mut Vec<u8>, value: &str) -> Result<(), CursorError> {
    push_bytes(output, value.as_bytes())
}

fn push_bytes(output: &mut Vec<u8>, value: &[u8]) -> Result<(), CursorError> {
    let length = u16::try_from(value.len()).map_err(|_| CursorError::TokenTooLarge)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    Ok(())
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn byte(&mut self) -> Result<u8, CursorError> {
        Ok(self.take(1)?[0])
    }

    fn string(&mut self) -> Result<String, CursorError> {
        String::from_utf8(self.bytes()?).map_err(|_| CursorError::InvalidStoredValue)
    }

    fn bytes(&mut self) -> Result<Vec<u8>, CursorError> {
        let length = u16::from_be_bytes(
            self.take(2)?
                .try_into()
                .map_err(|_| CursorError::InvalidStoredValue)?,
        ) as usize;
        Ok(self.take(length)?.to_vec())
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], CursorError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(CursorError::InvalidStoredValue)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(CursorError::InvalidStoredValue)?;
        self.offset = end;
        Ok(value)
    }

    const fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codec() -> CursorCodec {
        CursorCodec::new([0x42; 32]).unwrap()
    }

    fn binding() -> CursorBinding {
        CursorBinding {
            tenant_id: TenantId::try_new("tenant-a").unwrap(),
            actor_id: Some(ActorId::try_new("actor-a").unwrap()),
            capability_id: CapabilityId::try_new("sales.deal.list").unwrap(),
            capability_version: CapabilityVersion::try_new("1.0.0").unwrap(),
            resource_type: RecordType::try_new("sales.deal").unwrap(),
            normalized_filter_hash: normalized_filter_hash([
                ("owner", b"actor-a".as_slice()),
                ("status", b"open".as_slice()),
            ]),
            sort_id: "updated_at_desc_record_id_asc".to_owned(),
            page_size: 50,
        }
    }

    fn continuation() -> CursorContinuation {
        CursorContinuation {
            sort_key: 1_700_000_000_000_000_i64.to_be_bytes().to_vec(),
            record_id: RecordId::try_new("deal-00042").unwrap(),
        }
    }

    #[test]
    fn cursor_round_trip_preserves_keyset_continuation() {
        let token = codec().encode(&binding(), &continuation()).unwrap();
        assert!(!token.contains("tenant-a"));
        assert!(!token.contains("deal-00042"));
        assert_eq!(codec().decode(&token, &binding()).unwrap(), continuation());
    }

    #[test]
    fn one_byte_tampering_is_rejected_before_payload_use() {
        let token = codec().encode(&binding(), &continuation()).unwrap();
        let mut decoded = URL_SAFE_NO_PAD.decode(token).unwrap();
        decoded[8] ^= 0x01;
        let tampered = URL_SAFE_NO_PAD.encode(decoded);
        assert_eq!(
            codec().decode(&tampered, &binding()).unwrap_err(),
            CursorError::IntegrityFailed
        );
    }

    #[test]
    fn cursor_cannot_move_between_query_bindings() {
        let token = codec().encode(&binding(), &continuation()).unwrap();
        let mut wrong = binding();
        wrong.tenant_id = TenantId::try_new("tenant-b").unwrap();
        assert_eq!(
            codec().decode(&token, &wrong).unwrap_err(),
            CursorError::BindingMismatch
        );
        let mut wrong = binding();
        wrong.actor_id = Some(ActorId::try_new("actor-b").unwrap());
        assert_eq!(
            codec().decode(&token, &wrong).unwrap_err(),
            CursorError::BindingMismatch
        );
        let mut wrong = binding();
        wrong.capability_id = CapabilityId::try_new("activities.task.list").unwrap();
        assert_eq!(
            codec().decode(&token, &wrong).unwrap_err(),
            CursorError::BindingMismatch
        );
        let mut wrong = binding();
        wrong.page_size = 25;
        assert_eq!(
            codec().decode(&token, &wrong).unwrap_err(),
            CursorError::BindingMismatch
        );
    }

    #[test]
    fn normalized_filter_hash_is_order_independent_but_value_sensitive() {
        let left = normalized_filter_hash([
            ("status", b"open".as_slice()),
            ("owner", b"actor-a".as_slice()),
        ]);
        let right = normalized_filter_hash([
            ("owner", b"actor-a".as_slice()),
            ("status", b"open".as_slice()),
        ]);
        let changed = normalized_filter_hash([
            ("owner", b"actor-b".as_slice()),
            ("status", b"open".as_slice()),
        ]);
        assert_eq!(left, right);
        assert_ne!(left, changed);
    }

    #[test]
    fn page_size_policy_has_deterministic_default_and_hard_maximum() {
        let policy = PageSizePolicy {
            default_size: 50,
            maximum_size: 200,
        };
        assert_eq!(policy.resolve(0).unwrap(), 50);
        assert_eq!(policy.resolve(1).unwrap(), 1);
        assert_eq!(policy.resolve(200).unwrap(), 200);
        assert_eq!(
            policy.resolve(-1).unwrap_err(),
            CursorError::InvalidPageSize
        );
        assert_eq!(
            policy.resolve(201).unwrap_err(),
            CursorError::PageSizeTooLarge
        );
    }

    #[test]
    fn cursor_secret_and_continuation_have_governed_bounds() {
        assert_eq!(
            CursorCodec::new([0_u8; 31]).unwrap_err(),
            CursorError::SigningKeyTooShort
        );
        let oversized = CursorContinuation {
            sort_key: vec![0_u8; MAXIMUM_SORT_KEY_BYTES + 1],
            record_id: RecordId::try_new("deal-1").unwrap(),
        };
        assert_eq!(
            codec().encode(&binding(), &oversized).unwrap_err(),
            CursorError::ContinuationTooLarge
        );
    }
}
