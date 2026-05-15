use proptest::prelude::*;

use super::rate_limit;

proptest! {
    /// backoff_params ttl must be non-decreasing as count increases.
    #[test]
    fn backoff_ttl_monotonic(a: u64, b: u64) {
        prop_assume!(a < b);
        let (ttl_a, _) = rate_limit::backoff_params(a);
        let (ttl_b, _) = rate_limit::backoff_params(b);
        assert!(ttl_a <= ttl_b, "ttl should not decrease as count increases");
    }
}
