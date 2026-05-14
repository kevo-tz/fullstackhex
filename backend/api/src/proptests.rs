use proptest::prelude::*;

proptest! {
    /// LiveEvent serde round-trip: serializing and deserializing must
    /// produce the original value.
    #[test]
    fn live_event_roundtrip(service: String, status: String, detail: Option<String>) {
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
}
