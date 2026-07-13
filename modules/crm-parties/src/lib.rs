#![forbid(unsafe_code)]

/// Stable crate identity for repository tooling.
pub const CRATE_NAME: &str = "crm-parties";
/// Immutable governed module identity.
pub const MODULE_ID: &str = "crm.parties";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_identity_is_explicit() {
        assert_eq!(CRATE_NAME, "crm-parties");
        assert_eq!(MODULE_ID, "crm.parties");
    }
}
