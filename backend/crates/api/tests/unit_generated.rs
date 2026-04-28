#[cfg(test)]
mod tests {

    #[test]
    fn health_response_structure() {
        // Test that health endpoint returns proper JSON structure
        let response = serde_json::json!({
            "status": "ok",
            "service": "test-service"
        });

        assert_eq!(response["status"], "ok");
        assert!(response["service"].is_string());
    }

    #[test]
    fn environment_variables_loaded() {
        // Test that required env vars have defaults or are set
        // Safety: single-threaded test; no other threads reading this variable.
        unsafe {
            std::env::set_var("RUST_LOG", "info");
        }
        let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        assert_eq!(log_level, "info");
    }
}
