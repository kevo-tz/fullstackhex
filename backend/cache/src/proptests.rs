use proptest::prelude::*;

use super::rate_limit;
use super::session::Session;

proptest! {
    /// backoff_params ttl must be non-decreasing as count increases.
    #[test]
    fn backoff_ttl_monotonic(a: u64, b: u64) {
        prop_assume!(a < b);
        let (ttl_a, _) = rate_limit::backoff_params(a);
        let (ttl_b, _) = rate_limit::backoff_params(b);
        assert!(ttl_a <= ttl_b, "ttl should not decrease as count increases");
    }

    /// backoff_params must return exact TTL and label for each bucket.
    #[test]
    fn backoff_params_exact_buckets(count: u64) {
        let (ttl, label) = rate_limit::backoff_params(count);
        match count {
            0..=4 => {
                assert_eq!(ttl, 60);
                assert_eq!(label, "tracking");
            }
            5..=9 => {
                assert_eq!(ttl, 60);
                assert_eq!(label, "60s");
            }
            10..=19 => {
                assert_eq!(ttl, 300);
                assert_eq!(label, "5min");
            }
            _ => {
                // 20+ including u64::MAX
                assert_eq!(ttl, 1800);
                assert_eq!(label, "30min");
            }
        }
    }

    /// backoff_params must not panic for any u64 input.
    #[test]
    fn backoff_params_never_panics(count: u64) {
        let (_ttl, _label) = rate_limit::backoff_params(count);
        // Just verifying no panic — any return is valid
    }

    /// Session serde round-trip: serializing and deserializing arbitrary
    /// session data must preserve all fields.
    #[test]
    fn session_serde_roundtrip(
        user_id: String,
        email: String,
        name: Option<String>,
        provider: String,
        created_at: u64,
    ) {
        let session = Session {
            user_id: user_id.clone(),
            email: email.clone(),
            name: name.clone(),
            provider: provider.clone(),
            created_at,
        };
        let json = serde_json::to_string(&session).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.user_id, user_id);
        assert_eq!(deserialized.email, email);
        assert_eq!(deserialized.name, name);
        assert_eq!(deserialized.provider, provider);
        assert_eq!(deserialized.created_at, created_at);
    }
}
