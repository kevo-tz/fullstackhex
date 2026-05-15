use proptest::prelude::*;

proptest! {
    /// LiveEvent serde round-trip for all variants: serializing and
    /// deserializing must produce the original value.
    #[test]
    fn live_event_health_update_roundtrip(service: String, status: String, detail: Option<String>) {
        let event = crate::live::LiveEvent::HealthUpdate {
            service: service.clone(),
            status: status.clone(),
            detail: detail.clone(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: crate::live::LiveEvent = serde_json::from_str(&json).unwrap();
        match deserialized {
            crate::live::LiveEvent::HealthUpdate { service: s, status: st, detail: d } => {
                assert_eq!(s, service);
                assert_eq!(st, status);
                assert_eq!(d, detail);
            }
            _ => panic!("expected HealthUpdate"),
        }
    }

    /// AuthEvent serde round-trip with arbitrary kind and email.
    #[test]
    fn live_event_auth_event_roundtrip(kind: String, email: Option<String>) {
        let event = crate::live::LiveEvent::AuthEvent {
            kind: kind.clone(),
            email: email.clone(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: crate::live::LiveEvent = serde_json::from_str(&json).unwrap();
        match deserialized {
            crate::live::LiveEvent::AuthEvent { kind: k, email: e } => {
                assert_eq!(k, kind);
                assert_eq!(e, email);
            }
            _ => panic!("expected AuthEvent"),
        }
    }

    /// ConnectionStatus serde round-trip with arbitrary status string.
    #[test]
    fn live_event_connection_status_roundtrip(status: String) {
        let event = crate::live::LiveEvent::ConnectionStatus {
            status: status.clone(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: crate::live::LiveEvent = serde_json::from_str(&json).unwrap();
        match deserialized {
            crate::live::LiveEvent::ConnectionStatus { status: s } => {
                assert_eq!(s, status);
            }
            _ => panic!("expected ConnectionStatus"),
        }
    }

    /// Random byte strings must not panic LiveEvent deserialization.
    #[test]
    fn live_event_random_bytes_never_panics(bytes: Vec<u8>) {
        prop_assume!(bytes.len() <= 4096);
        let _ = serde_json::from_slice::<crate::live::LiveEvent>(&bytes);
    }

    /// Invalid JSON with missing type field must not panic.
    #[test]
    fn live_event_missing_type_never_panics(data: String) {
        prop_assume!(data.len() <= 512);
        let json = format!("{{\"data\": {}}}", serde_json::to_string(&data).unwrap());
        let _ = serde_json::from_str::<crate::live::LiveEvent>(&json);
    }

    /// Invalid JSON with unknown type value must not panic.
    #[test]
    fn live_event_unknown_type_never_panics(typ: String) {
        prop_assume!(!typ.is_empty() && typ.len() <= 64);
        prop_assume!(typ != "health_update" && typ != "auth_event" && typ != "connection_status");
        let json = format!("{{\"type\": \"{}\", \"data\": {{}}}}", typ);
        let _ = serde_json::from_str::<crate::live::LiveEvent>(&json);
    }
}
