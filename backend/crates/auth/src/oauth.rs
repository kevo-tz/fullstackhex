//! OAuth2 provider integration (Google, GitHub).

use domain::error::ApiError;
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, ClientId, ClientSecret, CsrfToken, RedirectUrl, TokenUrl,
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
        let google_client = google_client_id.zip(google_client_secret).map(|(id, secret)| {
            BasicClient::new(
                ClientId::new(id),
                Some(ClientSecret::new(secret)),
                AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).unwrap(),
                Some(TokenUrl::new("https://www.googleapis.com/oauth2/v3/token".to_string()).unwrap()),
            )
        });

        let github_client = github_client_id.zip(github_client_secret).map(|(id, secret)| {
            BasicClient::new(
                ClientId::new(id),
                Some(ClientSecret::new(secret)),
                AuthUrl::new("https://github.com/login/oauth/authorize".to_string()).unwrap(),
                Some(TokenUrl::new("https://github.com/login/oauth/access_token".to_string()).unwrap()),
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
                OAuthProvider::Google => {
                    oauth2::Scope::new("email".to_string())
                }
                OAuthProvider::GitHub => {
                    oauth2::Scope::new("user:email".to_string())
                }
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
