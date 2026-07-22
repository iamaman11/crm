impl PrivacyCase {
    /// Timestamp of the latest accepted aggregate transition.
    pub const fn last_transition_at_unix_nanos(&self) -> i64 {
        self.last_transition_at_unix_nanos
    }
}
