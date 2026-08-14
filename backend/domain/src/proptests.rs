use proptest::prelude::*;

use crate::{CreateNoteInput, FeatureFlags, Note};

proptest! {
    /// FeatureFlags serde round-trip: arbitrary flag combinations survive
    /// serialization and deserialization without data loss.
    #[test]
    fn feature_flags_serde_roundtrip(maintenance: bool) {
        let flags = FeatureFlags {
            maintenance_mode: maintenance,
        };
        let json = serde_json::to_string(&flags).unwrap();
        let deserialized: FeatureFlags = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, flags);
    }

    /// Note serde round-trip: arbitrary strings for id/user_id/title/body and
    /// arbitrary timestamps for created_at/updated_at survive serialization and
    /// deserialization without data loss.
    /// Each string field capped at 4096 chars to keep test runtime bounded.
    #[test]
    fn note_serde_roundtrip(
        id in "[a-zA-Z0-9_ ]{0,4096}",
        user_id in "[a-zA-Z0-9_ ]{0,4096}",
        title in "[a-zA-Z0-9_ ]{0,4096}",
        body in "[a-zA-Z0-9_ ]{0,4096}",
        created_at_secs in -10_000_000_000i64..10_000_000_000i64,
        updated_at_secs in -10_000_000_000i64..10_000_000_000i64,
    ) {
        let created_at = chrono::DateTime::from_timestamp(created_at_secs, 0).unwrap();
        let updated_at = chrono::DateTime::from_timestamp(updated_at_secs, 0).unwrap();
        let note = Note { id: id.clone(), user_id: user_id.clone(), title: title.clone(), body: body.clone(), created_at, updated_at };
        let json = serde_json::to_string(&note).unwrap();
        let deserialized: Note = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, id);
        assert_eq!(deserialized.user_id, user_id);
        assert_eq!(deserialized.title, title);
        assert_eq!(deserialized.body, body);
        assert_eq!(deserialized.created_at, created_at);
        assert_eq!(deserialized.updated_at, updated_at);
    }

    /// CreateNoteInput deserialization never panics on arbitrary title/body strings.
    #[test]
    fn create_note_input_deser_never_panics(title: String, body: String) {
        prop_assume!(title.len() <= 4096);
        prop_assume!(body.len() <= 4096);
        let json = format!("{{\"title\":{},\"body\":{}}}", serde_json::to_string(&title).unwrap(), serde_json::to_string(&body).unwrap());
        let deserialized: CreateNoteInput = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.title, title);
        assert_eq!(deserialized.body, body);
    }
}
