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

        Ok(session_id)
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
    pub async fn session_destroy(&self, session_id: &str) -> Result<(), CacheError> {
        let key = self.make_key("session", session_id);
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
