use proptest::prelude::*;

proptest! {
    /// backoff_for_attempt must produce durations that are monotonically
    /// non-decreasing with respect to the attempt number.
    #[test]
    fn backoff_is_monotonic(a: u32, b: u32) {
        let t_a = super::backoff_for_attempt(a);
        let t_b = super::backoff_for_attempt(b);
        if a < b {
            assert!(t_a <= t_b, "backoff regressed: attempt {a} > attempt {b}");
        }
    }

    /// backoff_for_attempt must never exceed the maximum backoff.
    #[test]
    fn backoff_saturates(attempt: u32) {
        let dur = super::backoff_for_attempt(attempt);
        assert!(
            dur.as_millis() <= 30_000,
            "backoff {attempt}: {dur:?} exceeds 30s max"
        );
    }
}
