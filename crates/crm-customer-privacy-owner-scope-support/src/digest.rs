use sha2::{Digest, Sha256};

/// Computes a domain-separated SHA-256 digest over length-framed fields.
///
/// Every value is prefixed with its unsigned 64-bit big-endian byte length, so
/// distinct field boundaries cannot produce the same byte stream by simple
/// concatenation.
pub fn framed_digest(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    append_frame(&mut hasher, domain);
    for field in fields {
        append_frame(&mut hasher, field);
    }
    hasher.finalize().into()
}

/// Appends one length-framed value to an existing SHA-256 digest.
///
/// This supports owner-specific variable-length evidence streams while keeping
/// the framing rule shared and identical.
pub fn append_frame(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value);
}
