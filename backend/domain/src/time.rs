use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Returns the current Unix timestamp in seconds, or 0 if the system clock is
/// before the Unix epoch (e.g. NTP misconfiguration, VM clock skew).
///
/// Unlike `SystemTime::now().duration_since(UNIX_EPOCH).unwrap()`, this never
/// panics — a broken clock produces a zero timestamp instead of crashing the
/// server.
pub fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

/// Returns the current Unix timestamp in milliseconds, or 0 if the system
/// clock is before the Unix epoch.
pub fn unix_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_timestamp_secs_is_reasonable() {
        let ts = unix_timestamp_secs();
        assert!(ts > 1_700_000_000, "timestamp should be after 2023");
    }

    #[test]
    fn unix_timestamp_ms_is_reasonable() {
        let ts = unix_timestamp_ms();
        assert!(ts > 1_700_000_000_000, "timestamp should be after 2023");
    }
}
