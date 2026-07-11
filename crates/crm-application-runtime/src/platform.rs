use crm_capability_runtime::{
    CapabilityDefinition, CapabilityRequest, CapabilitySemanticValidator,
};
use crm_module_sdk::{Clock, ErrorCategory, PortFuture, RandomSource, SdkError};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_unix_nanos(&self) -> i64 {
        let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
            return -1;
        };
        i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX)
    }
}

/// Process-local entropy source for correlation/request identifiers.
///
/// These bytes are used only for non-secret execution identities. They combine
/// process/time/counter state through SHA-256 and are not exposed as
/// authentication credentials or signing keys.
#[derive(Debug)]
pub struct ProcessIdentitySource {
    counter: AtomicU64,
}

impl Default for ProcessIdentitySource {
    fn default() -> Self {
        Self {
            counter: AtomicU64::new(1),
        }
    }
}

impl RandomSource for ProcessIdentitySource {
    fn fill_bytes(&self, buffer: &mut [u8]) -> Result<(), SdkError> {
        let mut offset = 0_usize;
        while offset < buffer.len() {
            let counter = self.counter.fetch_add(1, Ordering::Relaxed);
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| identity_unavailable())?
                .as_nanos();
            let mut hasher = Sha256::new();
            hasher.update(b"crm.process-identity/v1");
            hasher.update(std::process::id().to_be_bytes());
            hasher.update(counter.to_be_bytes());
            hasher.update(now.to_be_bytes());
            let digest = hasher.finalize();
            let remaining = buffer.len() - offset;
            let take = remaining.min(digest.len());
            buffer[offset..offset + take].copy_from_slice(&digest[..take]);
            offset += take;
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ContractBoundMutationSemanticValidator;

impl CapabilitySemanticValidator for ContractBoundMutationSemanticValidator {
    fn validate<'a>(
        &'a self,
        _definition: &'a CapabilityDefinition,
        _request: &'a CapabilityRequest,
    ) -> PortFuture<'a, Result<(), SdkError>> {
        // Gateway contract matching has already validated exact payload identity.
        // Domain semantic invariants remain synchronous inside the owner planner
        // after live authorization and before the first PostgreSQL side effect.
        Box::pin(async { Ok(()) })
    }
}

fn identity_unavailable() -> SdkError {
    SdkError::new(
        "PROCESS_IDENTITY_UNAVAILABLE",
        ErrorCategory::Unavailable,
        true,
        "Execution identity generation is temporarily unavailable.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_identity_source_fills_distinct_non_secret_id_material() {
        let source = ProcessIdentitySource::default();
        let mut first = [0_u8; 32];
        let mut second = [0_u8; 32];
        source.fill_bytes(&mut first).unwrap();
        source.fill_bytes(&mut second).unwrap();
        assert_ne!(first, [0; 32]);
        assert_ne!(first, second);
    }
}
