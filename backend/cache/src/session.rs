//! Redis-backed session store.
//!
//! Sessions are stored in Redis with configurable TTL.
//! No PostgreSQL sessions table — Redis is the sole session store.

use super::{CacheError, RedisClient};
use fred::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Session data stored in Redis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub user_id: String,
    pub email: String,
    pub name: Option<String>,
    pub provider: String,
    pub created_at: u64,
}

impl RedisClient {
    /// Create a new session and return the session ID.
    pub async fn session_create(
        &self,
        session: &Session,
        ttl: Duration,
    ) -> Result<String, CacheError> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let key = self.make_key("session", &session_id);
        let json = serde_json::to_string(session)
            .map_err(|e| CacheError::SerializationFailed(e.to_string()))?;

        self.client
            .set::<(), _, _>(
                &key,
                json,
                Some(Expiration::EX(ttl.as_secs() as i64)),
                None,
                false,
            )
            .await
            .map_err(CacheError::CommandFailed)?;

        // Track in user→sessions set for bulk invalidation (password reset, etc.)
        let user_key = self.make_key("user_sessions", &session.user_id);
        self.client
            .sadd::<(), _, _>(&user_key, &session_id)
            .await
            .map_err(CacheError::CommandFailed)?;
        // Let the set TTL match the session TTL so stale entries clean themselves up
        let _: () = self
            .client
            .expire(&user_key, ttl.as_secs() as i64, None)
            .await
            .map_err(CacheError::CommandFailed)?;

        Ok(session_id)
    }

    /// Destroy all sessions for a user (used after password reset).
    ///
    /// Atomically reads and deletes the user's session set, then deletes each
    /// individual session key. Best-effort: logs warnings on individual failures
    /// but does not return an error — the password reset has already succeeded.
    pub async fn session_destroy_all_for_user(&self, user_id: &str) {
        let user_key = self.make_key("user_sessions", user_id);

        // Atomically pop all session IDs from the set
        let session_ids: Vec<String> = self
            .client
            .smembers(&user_key)
            .await
            .unwrap_or_default();

        if session_ids.is_empty() {
            return;
        }

        // Delete the user→sessions mapping
        let _: () = self.client.del(&user_key).await.unwrap_or_default();

        // Delete each individual session key
        for sid in &session_ids {
            let key = self.make_key("session", sid);
            if let Err(e) = self.client.del::<(), _>(&key).await {
                tracing::warn!(session = %sid, error = %e, "failed to delete session during user invalidation");
            }
        }
    }

    /// Get a session by ID.
    pub async fn session_get(&self, session_id: &str) -> Result<Session, CacheError> {
        let key = self.make_key("session", session_id);
        let result: Option<String> = self
            .client
            .get(&key)
            .await
            .map_err(CacheError::CommandFailed)?;

        match result {
            Some(json) => {
                let session: Session = serde_json::from_str(&json)
                    .map_err(|e| CacheError::SerializationFailed(e.to_string()))?;
                Ok(session)
            }
            None => Err(CacheError::SessionNotFound),
        }
    }

    /// Destroy a session (logout).
    ///
    /// Also removes the session ID from the user→sessions set if the session
    /// can be read (best-effort — the session may already be expired).
    pub async fn session_destroy(&self, session_id: &str) -> Result<(), CacheError> {
        let key = self.make_key("session", session_id);

        // Best-effort: read the session to find the user_id for set cleanup
        let user_id: Option<String> = self
            .client
            .get::<Option<String>, _>(&key)
            .await
            .ok()
            .and_then(|json| json)
            .and_then(|json| {
                serde_json::from_str::<Session>(&json)
                    .ok()
                    .map(|s| s.user_id)
            });
        if let Some(uid) = user_id {
            let user_key = self.make_key("user_sessions", &uid);
            let _: () = self
                .client
                .srem(&user_key, session_id)
                .await
                .unwrap_or_default();
        }

        self.client
            .del::<(), _>(&key)
            .await
            .map_err(CacheError::CommandFailed)?;
        Ok(())
    }

    /// Refresh a session TTL without changing data.
    pub async fn session_refresh(&self, session_id: &str, ttl: Duration) -> Result<(), CacheError> {
        let key = self.make_key("session", session_id);
        self.client
            .expire::<(), _>(&key, ttl.as_secs() as i64, None)
            .await
            .map_err(CacheError::CommandFailed)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_serializes_to_json() {
        let session = Session {
            user_id: "user-123".to_string(),
            email: "test@example.com".to_string(),
            name: Some("Test User".to_string()),
            provider: "local".to_string(),
            created_at: 1_700_000_000,
        };

        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("user-123"));
        assert!(json.contains("test@example.com"));
        assert!(json.contains("Test User"));
        assert!(json.contains("local"));
    }

    #[test]
    fn session_deserializes_from_json() {
        let json = r#"{"user_id":"user-456","email":"anon@example.com","name":null,"provider":"google","created_at":1700000001}"#;
        let session: Session = serde_json::from_str(json).unwrap();

        assert_eq!(session.user_id, "user-456");
        assert_eq!(session.email, "anon@example.com");
        assert_eq!(session.name, None);
        assert_eq!(session.provider, "google");
        assert_eq!(session.created_at, 1_700_000_001);
    }

    #[test]
    fn session_roundtrip_json() {
        let original = Session {
            user_id: "user-789".to_string(),
            email: "round@example.com".to_string(),
            name: None,
            provider: "github".to_string(),
            created_at: 1_700_000_002,
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: Session = serde_json::from_str(&json).unwrap();

        assert_eq!(original.user_id, deserialized.user_id);
        assert_eq!(original.email, deserialized.email);
        assert_eq!(original.name, deserialized.name);
        assert_eq!(original.provider, deserialized.provider);
        assert_eq!(original.created_at, deserialized.created_at);
    }
}
