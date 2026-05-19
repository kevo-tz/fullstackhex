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
