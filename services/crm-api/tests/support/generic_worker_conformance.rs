use std::fmt::Debug;

#[derive(Debug, Clone, Copy)]
pub struct WorkerConformanceSuite {
    representative: &'static str,
}

impl WorkerConformanceSuite {
    pub const fn new(representative: &'static str) -> Self {
        Self { representative }
    }

    pub fn assert_no_side_effects<T>(&self, guarantee: &str, before: &T, after: &T)
    where
        T: PartialEq + Debug + ?Sized,
    {
        assert_eq!(
            after, before,
            "{} violated {guarantee}: worker state changed",
            self.representative
        );
    }

    pub fn assert_retryable_failure_preserves_progress<P, E>(
        &self,
        progress_before: &P,
        progress_after: &P,
        effects_before: &E,
        effects_after: &E,
    ) where
        P: PartialEq + Debug + ?Sized,
        E: PartialEq + Debug + ?Sized,
    {
        self.assert_no_side_effects(
            "retryable failure checkpoint preservation",
            progress_before,
            progress_after,
        );
        self.assert_no_side_effects(
            "retryable failure target-effect isolation",
            effects_before,
            effects_after,
        );
    }

    pub fn assert_exact_recovery<T>(&self, expected: &T, actual: &T)
    where
        T: PartialEq + Debug + ?Sized,
    {
        assert_eq!(
            actual, expected,
            "{} did not converge to the exact expected recovery state",
            self.representative
        );
    }
}

// This source file is included by several independent integration-test crates.
// Each crate uses a different subset, so anchor every generic entrypoint to keep
// the shared API type-checked without suppressing legitimate dead-code lints.
const _: fn(&WorkerConformanceSuite, &str, &(), &()) =
    WorkerConformanceSuite::assert_no_side_effects::<()>;
const _: fn(&WorkerConformanceSuite, &(), &(), &(), &()) =
    WorkerConformanceSuite::assert_retryable_failure_preserves_progress::<(), ()>;
const _: fn(&WorkerConformanceSuite, &(), &()) =
    WorkerConformanceSuite::assert_exact_recovery::<()>;
