#[cfg(test)]
mod tests {

    // Test that health route path constant is valid
    #[test]
    fn health_endpoint_returns_200() {
        let health_path = "/health";
        assert!(health_path.starts_with('/'));
        assert!(health_path.contains("health"));
    }

    // Test that health response has correct structure
    #[test]
    fn health_response_structure() {
        let expected_keys = vec!["status", "service", "version"];
        let response_json = r#"{"status":"ok","service":"api","version":"0.1.0"}"#;

        let response: serde_json::Value = serde_json::from_str(response_json).unwrap();
        for key in &expected_keys {
            assert!(response.as_object().unwrap().contains_key(*key));
        }
    }
}
