#![forbid(unsafe_code)]

pub mod domain;
pub mod persistence;

pub use domain::*;
pub use persistence::*;

/// Stable crate identity for repository tooling.
pub const CRATE_NAME: &str = "crm-contact-points";
/// Immutable governed module identity.
pub const MODULE_ID: &str = "crm.contact-points";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_identity_is_explicit() {
        assert_eq!(CRATE_NAME, "crm-contact-points");
        assert_eq!(MODULE_ID, "crm.contact-points");
    }
}
