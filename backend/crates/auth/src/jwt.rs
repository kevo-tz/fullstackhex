//! JWT token creation and validation.

use domain::error::ApiError;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

/// JWT claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID).
    pub sub: String,
    /// Email.
    pub email: String,
    /// Display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Auth provider (local, google, github).
    pub provider: String,
    /// Expiration time (Unix timestamp).
    pub exp: u64,
    /// Issued at (Unix timestamp).
    pub iat: u64,
    /// Issuer.
    pub iss: String,
    /// JWT ID (unique token ID for blacklist).
    pub jti: String,
}

/// JWT service for creating and validating tokens.
#[derive(Clone)]
pub struct JwtService {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    issuer: String,
    expiry: u64,
}

impl JwtService {
    /// Create a new JWT service.
    pub fn new(secret: String, issuer: String, expiry: u64) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            issuer,
            expiry,
        }
    }

    /// Create a new access token for a user.
    pub fn create_token(
        &self,
        user_id: &str,
        email: &str,
        name: Option<&str>,
        provider: &str,
    ) -> Result<String, ApiError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = Claims {
            sub: user_id.to_string(),
            email: email.to_string(),
            name: name.map(|s| s.to_string()),
            provider: provider.to_string(),
            exp: now + self.expiry,
            iat: now,
            iss: self.issuer.clone(),
            jti: uuid::Uuid::new_v4().to_string(),
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| ApiError::InternalError(format!("JWT encode error: {e}")))
    }

    /// Validate a token and return the claims.
    pub fn validate_token(&self, token: &str) -> Result<Claims, ApiError> {
        let mut validation = Validation::default();
        validation.set_issuer(&[&self.issuer]);

        decode::<Claims>(token, &self.decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                    ApiError::Unauthorized("Token expired".to_string())
                }
                _ => ApiError::Unauthorized(format!("Invalid token: {e}")),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_service() -> JwtService {
        JwtService::new(
            "test-secret-key-for-testing".to_string(),
            "test-issuer".to_string(),
            900,
        )
    }

    #[test]
    fn create_and_validate_token() {
        let svc = test_service();
        let token = svc
            .create_token("user-123", "test@example.com", Some("Test"), "local")
            .unwrap();
        let claims = svc.validate_token(&token).unwrap();
        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.email, "test@example.com");
        assert_eq!(claims.name, Some("Test".to_string()));
        assert_eq!(claims.provider, "local");
        assert_eq!(claims.iss, "test-issuer");
    }

    #[test]
    fn validate_invalid_token_fails() {
        let svc = test_service();
        let result = svc.validate_token("invalid-token");
        assert!(result.is_err());
    }

    #[test]
    fn validate_wrong_secret_fails() {
        let svc1 = test_service();
        let svc2 = JwtService::new("wrong-secret".to_string(), "test-issuer".to_string(), 900);
        let token = svc1
            .create_token("user-123", "test@example.com", None, "local")
            .unwrap();
        assert!(svc2.validate_token(&token).is_err());
    }

    #[test]
    fn claims_have_jti() {
        let svc = test_service();
        let token1 = svc.create_token("u1", "a@b.com", None, "local").unwrap();
        let token2 = svc.create_token("u1", "a@b.com", None, "local").unwrap();
        let c1 = svc.validate_token(&token1).unwrap();
        let c2 = svc.validate_token(&token2).unwrap();
        assert_ne!(c1.jti, c2.jti, "each token should have a unique JTI");
    }
}
