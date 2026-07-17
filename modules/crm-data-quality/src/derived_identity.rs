use sha2::{Digest, Sha256};

pub(crate) fn derived_id(prefix: &str, domain: &[u8], parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    format!("{prefix}-{:x}", hasher.finalize())
}
