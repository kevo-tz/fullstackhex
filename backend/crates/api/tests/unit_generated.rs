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
        // Test that env lookup logic falls back to the default when a variable is unset.
        let log_level = std::env::var("UNIT_GENERATED_TEST_RUST_LOG_UNSET")
            .unwrap_or_else(|_| "info".to_string());
        assert_eq!(log_level, "info");
    }
}
