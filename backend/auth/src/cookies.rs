use axum::http::{HeaderMap, HeaderValue, header};
use domain::error::ApiError;

/// Parse a named cookie value from a `Cookie` header string.
///
/// Returns `None` if the cookie is not found or the value is empty.
pub fn parse_cookie_value<'a>(cookie_header: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}=");
    let val = cookie_header.split(';').find_map(|c| {
        let c = c.trim();
        c.strip_prefix(&prefix)
    })?;
    if val.is_empty() { None } else { Some(val) }
}

pub fn set_cookie(
    headers: &mut HeaderMap,
    name: &str,
    value: &str,
    max_age: u64,
    http_only: bool,
    secure: bool,
) -> Result<(), ApiError> {
    let cookie = {
        let mut parts = vec![format!(
            "{name}={value}; Path=/; Max-Age={max_age}; SameSite=Lax"
        )];
        if http_only {
            parts.push("HttpOnly".into());
        }
        if secure {
            parts.push("Secure".into());
        }
        parts.join("; ")
    };
    let header_value: HeaderValue = cookie.parse().map_err(|e| {
        ApiError::InternalError(format!("failed to construct Set-Cookie header: {e}"))
    })?;
    headers.append(header::SET_COOKIE, header_value);
    Ok(())
}

/// Set cookies for access_token, refresh_token, and csrf_token.
/// Returns the generated CSRF token for inclusion in the response body.
/// Optionally sets a session cookie if `session_id` is provided.
pub fn set_auth_cookies(
    headers: &mut HeaderMap,
    access_token: &str,
    refresh_token: &str,
    session_id: Option<&str>,
    jwt_expiry: u64,
    refresh_expiry: u64,
    cookie_secure: bool,
) -> Result<String, ApiError> {
    let csrf_token = super::csrf::generate_csrf_token()?;
    set_cookie(
        headers,
        "access_token",
        access_token,
        jwt_expiry,
        true,
        true,
    )?;
    set_cookie(
        headers,
        "refresh_token",
        refresh_token,
        refresh_expiry,
        true,
        true,
    )?;
    set_cookie(
        headers,
        "csrf_token",
        &csrf_token,
        jwt_expiry,
        false,
        cookie_secure,
    )?;
    if let Some(sid) = session_id {
        set_cookie(headers, "session", sid, jwt_expiry, true, true)?;
    }
    Ok(csrf_token)
}
