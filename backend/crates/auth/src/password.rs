//! Password hashing with Argon2.

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use domain::error::ApiError;

/// Hash a password using Argon2id.
pub fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| ApiError::InternalError(format!("Password hash error: {e}")))?
        .to_string();

    Ok(hash)
}

/// Verify a password against an Argon2 hash.
pub fn verify_password(password: &str, hash: &str) -> Result<bool, ApiError> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| ApiError::InternalError(format!("Invalid password hash: {e}")))?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_roundtrip() {
        let password = "correct-horse-battery-staple";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash).unwrap());
    }

    #[test]
    fn verify_wrong_password_fails() {
        let hash = hash_password("correct").unwrap();
        assert!(!verify_password("wrong", &hash).unwrap());
    }

    #[test]
    fn hashes_are_unique() {
        let h1 = hash_password("same-password").unwrap();
        let h2 = hash_password("same-password").unwrap();
        assert_ne!(h1, h2, "different salts should produce different hashes");
    }

    #[test]
    fn empty_password_rejected() {
        let result = hash_password("");
        assert!(result.is_ok(), "hashing empty password should succeed");
        let hash = result.unwrap();
        assert!(
            verify_password("", &hash).unwrap(),
            "empty password should verify against its own hash"
        );
        assert!(
            !verify_password("not-empty", &hash).unwrap(),
            "non-empty password should not verify against empty hash"
        );
    }

    #[test]
    fn verify_invalid_hash_returns_error() {
        let result = verify_password("anything", "not-a-valid-hash");
        assert!(result.is_err());
    }
}
