//! CSRF token generation and validation for cookie auth mode.
//!
//! Uses the double-submit pattern: token stored in cookie,
//! validated against X-CSRF-Token header.

use rand::Rng;

/// Generate a random CSRF token (32 bytes hex-encoded).
pub fn generate_csrf_token() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    hex::encode(bytes)
}

/// Validate a CSRF token by comparing the cookie value with the header value.
///
/// Uses constant-time comparison to prevent timing attacks.
/// Rejects empty tokens — both must be non-empty and match.
pub fn validate_csrf_token(cookie_token: &str, header_token: &str) -> bool {
    if cookie_token.is_empty() || header_token.is_empty() {
        return false;
    }
    if cookie_token.len() != header_token.len() {
        return false;
    }
    // Constant-time comparison
    cookie_token
        .bytes()
        .zip(header_token.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_creates_64_char_hex() {
        let token = generate_csrf_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn matching_tokens_validate() {
        let token = generate_csrf_token();
        assert!(validate_csrf_token(&token, &token));
    }

    #[test]
    fn mismatched_tokens_fail() {
        let t1 = generate_csrf_token();
        let t2 = generate_csrf_token();
        assert!(!validate_csrf_token(&t1, &t2));
    }

    #[test]
    fn different_lengths_fail() {
        assert!(!validate_csrf_token("short", "longer-token"));
    }

    #[test]
    fn empty_tokens_are_rejected() {
        assert!(!validate_csrf_token("", ""));
    }
}
