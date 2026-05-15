use proptest::prelude::*;

proptest! {
    /// validate_csrf_token must return false when given an empty token.
    #[test]
    fn rejects_empty(cookie_token: String, header_token: String) {
        let result = crate::csrf::validate_csrf_token(&cookie_token, &header_token);
        if cookie_token.is_empty() || header_token.is_empty() {
            assert!(!result, "validate_csrf_token should reject empty tokens");
        }
    }

    /// validate_csrf_token must be symmetric: swapping cookie and header
    /// should produce the same result.
    #[test]
    fn is_symmetric(a: String, b: String) {
        let forward = crate::csrf::validate_csrf_token(&a, &b);
        let reverse = crate::csrf::validate_csrf_token(&b, &a);
        assert_eq!(forward, reverse, "validate_csrf_token must be symmetric");
    }
}
