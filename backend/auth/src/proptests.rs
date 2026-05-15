use proptest::prelude::*;

use crate::jwt::{Claims, JwtService};

/// Helper: create a test JWT service with fixed secret.
fn test_service() -> JwtService {
    JwtService::new(
        "test-secret-key-for-testing".to_string(),
        "test-issuer".to_string(),
        900,
    )
}

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

    /// JWT token creation and validation round-trip: creating a token with
    /// arbitrary user data and validating it must return matching claims.
    #[test]
    fn jwt_roundtrip(user_id: String, email: String, name: Option<String>, provider: String) {
        let svc = test_service();
        let name_str = name.as_deref();
        if let Ok(token) = svc.create_token(&user_id, &email, name_str, &provider) {
            let claims = svc.validate_token(&token).unwrap();
            assert_eq!(claims.sub, user_id);
            assert_eq!(claims.email, email);
            assert_eq!(claims.name, name);
            assert_eq!(claims.provider, provider);
            assert_eq!(claims.iss, "test-issuer");
            assert!(
                claims.exp > claims.iat,
                "JWT must expire after issuance: exp={} iat={}",
                claims.exp,
                claims.iat
            );
            assert!(!claims.jti.is_empty(), "JWT must have a non-empty JTI");
        }
    }

    /// Claims serde round-trip: arbitrary claims must serialize and
    /// deserialize without data loss.
    #[test]
    fn claims_serde_roundtrip(
        sub: String,
        email: String,
        name: Option<String>,
        provider: String,
        exp: u64,
        iat: u64,
    ) {
        prop_assume!(exp > iat, "claims must have exp > iat");
        let claims = Claims {
            sub,
            email,
            name,
            provider,
            exp,
            iat,
            iss: "test-issuer".to_string(),
            jti: uuid::Uuid::new_v4().to_string(),
        };
        let json = serde_json::to_string(&claims).unwrap();
        let deserialized: Claims = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.sub, claims.sub);
        assert_eq!(deserialized.email, claims.email);
        assert_eq!(deserialized.name, claims.name);
        assert_eq!(deserialized.provider, claims.provider);
        assert_eq!(deserialized.exp, claims.exp);
        assert_eq!(deserialized.iat, claims.iat);
        assert_eq!(deserialized.iss, claims.iss);
        assert_eq!(deserialized.jti, claims.jti);
    }

    /// validate_registration must reject emails without '@' and short passwords.
    #[test]
    fn registration_validation(email: String, password: String, name: Option<String>) {
        prop_assume!(password.len() <= 1024, "password too long for test");
        let body = crate::routes::RegisterRequest { email, password, name };
        let result = crate::routes::validate_registration(&body);
        let email_ok = !body.email.is_empty() && body.email.contains('@');
        let password_ok = body.password.len() >= 8;
        if email_ok && password_ok {
            assert!(result.is_ok(), "valid input should be accepted");
        } else {
            assert!(result.is_err(), "invalid input should be rejected");
        }
    }
}

