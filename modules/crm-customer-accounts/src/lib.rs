#![forbid(unsafe_code)]

/// Stable crate identity for repository tooling.
pub const CRATE_NAME: &str = "crm-customer-accounts";
/// Immutable governed module identity.
pub const MODULE_ID: &str = "crm.customer-accounts";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_identity_is_explicit() {
        assert_eq!(CRATE_NAME, "crm-customer-accounts");
        assert_eq!(MODULE_ID, "crm.customer-accounts");
    }
}
