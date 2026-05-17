use axum::http::{HeaderMap, HeaderValue, header};
use domain::error::ApiError;

pub fn set_cookie(
    headers: &mut HeaderMap,
    name: &str,
    value: &str,
    max_age: u64,
    http_only: bool,
    secure: bool,
) -> Result<(), ApiError> {
    let cookie = {
        let mut parts = vec![format!("{name}={value}; Path=/; Max-Age={max_age}; SameSite=Lax")];
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
