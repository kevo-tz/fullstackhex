//! OAuth2 provider integration (Google, GitHub).

use domain::error::ApiError;
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, TokenResponse,
    TokenUrl,
};
use serde::{Deserialize, Serialize};

/// Supported OAuth providers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OAuthProvider {
    Google,
    GitHub,
}

impl std::fmt::Display for OAuthProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OAuthProvider::Google => write!(f, "google"),
            OAuthProvider::GitHub => write!(f, "github"),
        }
    }
}

/// User info returned from an OAuth provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthUserInfo {
    pub provider: OAuthProvider,
    pub provider_id: String,
    pub email: String,
    pub name: Option<String>,
}

/// OAuth service for handling provider flows.
pub struct OAuthService {
    google_client: Option<BasicClient>,
    github_client: Option<BasicClient>,
}

impl OAuthService {
    /// Create a new OAuth service from auth config fields.
    pub fn new(
        google_client_id: Option<String>,
        google_client_secret: Option<String>,
        github_client_id: Option<String>,
        github_client_secret: Option<String>,
    ) -> Self {
        let google_client = google_client_id
            .zip(google_client_secret)
            .map(|(id, secret)| {
                BasicClient::new(
                    ClientId::new(id),
                    Some(ClientSecret::new(secret)),
                    AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
                        .unwrap(),
                    Some(
                        TokenUrl::new("https://www.googleapis.com/oauth2/v3/token".to_string())
                            .unwrap(),
                    ),
                )
            });

        let github_client = github_client_id
            .zip(github_client_secret)
            .map(|(id, secret)| {
                BasicClient::new(
                    ClientId::new(id),
                    Some(ClientSecret::new(secret)),
                    AuthUrl::new("https://github.com/login/oauth/authorize".to_string()).unwrap(),
                    Some(
                        TokenUrl::new("https://github.com/login/oauth/access_token".to_string())
                            .unwrap(),
                    ),
                )
            });

        Self {
            google_client,
            github_client,
        }
    }

    /// Get the OAuth redirect URL for a provider.
    pub fn get_redirect_url(
        &self,
        provider: &OAuthProvider,
        redirect_url: &str,
    ) -> Result<(String, CsrfToken), ApiError> {
        let client = self.get_client(provider)?;

        let redirect_url = RedirectUrl::new(redirect_url.to_string())
            .map_err(|e| ApiError::InternalError(format!("Invalid redirect URL: {e}")))?;

        let (url, csrf_token) = client
            .authorize_url(CsrfToken::new_random)
            .set_redirect_uri(std::borrow::Cow::Owned(redirect_url))
            .add_scope(match provider {
                OAuthProvider::Google => oauth2::Scope::new("email".to_string()),
                OAuthProvider::GitHub => oauth2::Scope::new("user:email".to_string()),
            })
            .url();

        Ok((url.to_string(), csrf_token))
    }

    /// Check if a provider is configured.
    pub fn is_configured(&self, provider: &OAuthProvider) -> bool {
        match provider {
            OAuthProvider::Google => self.google_client.is_some(),
            OAuthProvider::GitHub => self.github_client.is_some(),
        }
    }

    /// Exchange an authorization code for an access token and fetch user info.
    pub async fn exchange_code(
        &self,
        provider: &OAuthProvider,
        code: &str,
    ) -> Result<OAuthUserInfo, ApiError> {
        let client = self.get_client(provider)?;

        let token = client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| ApiError::InternalError(format!("Token exchange failed: {e}")))?;

        let access_token = token.access_token().secret();

        match provider {
            OAuthProvider::Google => fetch_google_user_info(access_token).await,
            OAuthProvider::GitHub => fetch_github_user_info(access_token).await,
        }
    }

    fn get_client(&self, provider: &OAuthProvider) -> Result<&BasicClient, ApiError> {
        match provider {
            OAuthProvider::Google => self.google_client.as_ref().ok_or_else(|| {
                ApiError::ServiceUnavailable("Google OAuth not configured".to_string())
            }),
            OAuthProvider::GitHub => self.github_client.as_ref().ok_or_else(|| {
                ApiError::ServiceUnavailable("GitHub OAuth not configured".to_string())
            }),
        }
    }
}

async fn fetch_google_user_info(access_token: &str) -> Result<OAuthUserInfo, ApiError> {
    let resp = reqwest::Client::new()
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .map_err(|e| ApiError::InternalError(format!("Google userinfo request failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(ApiError::InternalError(format!(
            "Google userinfo failed: HTTP {}",
            resp.status()
        )));
    }

    let data: GoogleUserInfo = resp
        .json()
        .await
        .map_err(|e| ApiError::InternalError(format!("Google userinfo parse failed: {e}")))?;

    Ok(OAuthUserInfo {
        provider: OAuthProvider::Google,
        provider_id: data.id,
        email: data.email,
        name: data.name,
    })
}

async fn fetch_github_user_info(access_token: &str) -> Result<OAuthUserInfo, ApiError> {
    let client = reqwest::Client::new();

    let resp = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {access_token}"))
        .header("User-Agent", "fullstackhex")
        .send()
        .await
        .map_err(|e| ApiError::InternalError(format!("GitHub user request failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(ApiError::InternalError(format!(
            "GitHub user failed: HTTP {}",
            resp.status()
        )));
    }

    let user: GitHubUser = resp
        .json()
        .await
        .map_err(|e| ApiError::InternalError(format!("GitHub user parse failed: {e}")))?;

    // Fetch primary email if not public
    let email = match user.email {
        Some(e) => e,
        None => fetch_github_primary_email(access_token)
            .await
            .unwrap_or_default(),
    };

    Ok(OAuthUserInfo {
        provider: OAuthProvider::GitHub,
        provider_id: user.id.to_string(),
        email,
        name: user.name.or(Some(user.login)),
    })
}

async fn fetch_github_primary_email(access_token: &str) -> Option<String> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.github.com/user/emails")
        .header("Authorization", format!("Bearer {access_token}"))
        .header("User-Agent", "fullstackhex")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let emails: Vec<GitHubEmail> = resp.json().await.ok()?;
    emails.into_iter().find(|e| e.primary).map(|e| e.email)
}

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    id: String,
    email: String,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubUser {
    id: i64,
    login: String,
    email: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubEmail {
    email: String,
    primary: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_display_google() {
        assert_eq!(OAuthProvider::Google.to_string(), "google");
    }

    #[test]
    fn provider_display_github() {
        assert_eq!(OAuthProvider::GitHub.to_string(), "github");
    }

    #[test]
    fn oauth_service_unconfigured() {
        let svc = OAuthService::new(None, None, None, None);
        assert!(!svc.is_configured(&OAuthProvider::Google));
        assert!(!svc.is_configured(&OAuthProvider::GitHub));
    }

    #[test]
    fn oauth_service_google_configured() {
        let svc = OAuthService::new(
            Some("id".to_string()),
            Some("secret".to_string()),
            None,
            None,
        );
        assert!(svc.is_configured(&OAuthProvider::Google));
        assert!(!svc.is_configured(&OAuthProvider::GitHub));
    }

    #[test]
    fn oauth_service_github_configured() {
        let svc = OAuthService::new(
            None,
            None,
            Some("gh-id".to_string()),
            Some("gh-secret".to_string()),
        );
        assert!(svc.is_configured(&OAuthProvider::GitHub));
        assert!(!svc.is_configured(&OAuthProvider::Google));
    }

    #[test]
    fn get_redirect_url_google_builds_correct_url() {
        let svc = OAuthService::new(
            Some("google-id".to_string()),
            Some("google-secret".to_string()),
            None,
            None,
        );
        let (url, _csrf) = svc
            .get_redirect_url(
                &OAuthProvider::Google,
                "http://localhost:8001/auth/oauth/google/callback",
            )
            .unwrap();
        assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth"));
        assert!(url.contains("client_id=google-id"));
        assert!(url.contains("scope=email"));
        assert!(url.contains("redirect_uri"));
    }

    #[test]
    fn get_redirect_url_github_builds_correct_url() {
        let svc = OAuthService::new(
            None,
            None,
            Some("gh-id".to_string()),
            Some("gh-secret".to_string()),
        );
        let (url, _csrf) = svc
            .get_redirect_url(
                &OAuthProvider::GitHub,
                "http://localhost:8001/auth/oauth/github/callback",
            )
            .unwrap();
        assert!(url.starts_with("https://github.com/login/oauth/authorize"));
        assert!(url.contains("client_id=gh-id"));
        assert!(url.contains("scope=user%3Aemail"));
        assert!(url.contains("redirect_uri"));
    }

    #[test]
    fn get_redirect_url_unconfigured_provider() {
        let svc = OAuthService::new(None, None, None, None);
        let err = svc
            .get_redirect_url(&OAuthProvider::Google, "http://localhost/cb")
            .unwrap_err();
        assert!(matches!(err, ApiError::ServiceUnavailable(_)));
    }

    #[test]
    fn oauth_user_info_serialization() {
        let info = OAuthUserInfo {
            provider: OAuthProvider::Google,
            provider_id: "12345".to_string(),
            email: "test@example.com".to_string(),
            name: Some("Test User".to_string()),
        };
        let json = serde_json::to_string(&info).unwrap();
        let decoded: OAuthUserInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.provider, OAuthProvider::Google);
        assert_eq!(decoded.email, "test@example.com");
        assert_eq!(decoded.name.unwrap(), "Test User");
    }
}
