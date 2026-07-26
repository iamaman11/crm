use crate::errors::invalid_contract;
use sha2::{Digest, Sha256};

pub(crate) fn framed_digest(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    append_frame(&mut hasher, domain);
    for field in fields {
        append_frame(&mut hasher, field);
    }
    hasher.finalize().into()
}

pub(crate) fn append_frame(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}

pub(crate) fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

pub(crate) fn decode_hex(value: &str) -> Result<Vec<u8>, crm_module_sdk::SdkError> {
    if value.is_empty() || value.len() % 2 != 0 {
        return Err(cursor_invalid());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = nibble(pair[0]).ok_or_else(cursor_invalid)?;
            let low = nibble(pair[1]).ok_or_else(cursor_invalid)?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn cursor_invalid() -> crm_module_sdk::SdkError {
    invalid_contract(
        "CONSENTS_PRIVACY_SCOPE_CURSOR_INVALID",
        "The Consents privacy scope cursor is invalid.",
    )
}
