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
}
