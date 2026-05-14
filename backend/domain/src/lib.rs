//! Core business logic for FullStackHex.
//!
//! This crate is the domain layer — models, services, and use cases
//! that are independent of the web framework, database, or transport.

pub mod error;
pub mod time;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppConfig {
    pub max_page_size: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self { max_page_size: 100 }
    }
}

/// Feature flags controlled via environment variables.
///
/// All flags default to `false` when the env var is not set.
/// Flags are loaded once at startup and are NOT hot-reloadable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FeatureFlags {
    pub chat_enabled: bool,
    pub storage_readonly: bool,
    pub maintenance_mode: bool,
}

impl FeatureFlags {
    /// Load feature flags from environment variables.
    pub fn from_env() -> Self {
        Self {
            chat_enabled: Self::env_bool("FEATURE_CHAT"),
            storage_readonly: Self::env_bool("FEATURE_STORAGE_READONLY"),
            maintenance_mode: Self::env_bool("FEATURE_MAINTENANCE"),
        }
    }

    fn env_bool(key: &str) -> bool {
        std::env::var(key).ok().map_or(false, |v| {
            v.eq_ignore_ascii_case("true") || v == "1"
        })
    }
}

/// A user's note in the notes CRUD demo.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Note {
    pub id: String,
    pub user_id: String,
    pub title: String,
    pub body: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Input for creating a new note.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateNoteInput {
    pub title: String,
    pub body: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_reasonable() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.max_page_size, 100);
    }

    #[test]
    fn note_serde_roundtrip() {
        let note = Note {
            id: "uuid-1".into(),
            user_id: "uuid-2".into(),
            title: "Test Note".into(),
            body: "Hello world".into(),
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&note).unwrap();
        let deserialized: Note = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "uuid-1");
        assert_eq!(deserialized.title, "Test Note");
    }
}
