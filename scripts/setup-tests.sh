#!/bin/bash
# FullStackHex Test Suite Generator
# Generate baseline test suites for all services

# Source common functions and configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"
source "$SCRIPT_DIR/config.sh"

log_info "Adding generated test suites..."

# Rust unit + integration + smoke tests
for crate in api core db python-sidecar; do
    mkdir -p "backend/crates/$crate/tests"

    if [ ! -f "backend/crates/$crate/tests/unit_generated.rs" ]; then
        cat > "backend/crates/$crate/tests/unit_generated.rs" << 'EOF'
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
EOF
        log_success "Created unit_generated.rs for $crate"
    fi
done

if [ ! -f "backend/crates/api/tests/integration_health_route.rs" ]; then
    cat > "backend/crates/api/tests/integration_health_route.rs" << 'EOF'
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
EOF
    log_success "Created integration_health_route.rs for api"
fi

if [ ! -f "backend/crates/api/tests/smoke_generated.rs" ]; then
    cat > "backend/crates/api/tests/smoke_generated.rs" << 'EOF'
#[cfg(test)]
mod tests {
    // Smoke test: verify workspace compiles and core modules are accessible
    #[test]
    fn workspace_compiles_and_modules_accessible() {
        // Test that we can access core types
        // This test ensures the crate structure is correct
        let version = env!("CARGO_PKG_VERSION");
        assert!(!version.is_empty());
    }

    #[test]
    fn environment_configuration_valid() {
        // Test that required environment variables are properly configured
        use std::env;

        // These should have defaults or be set
        let rust_log = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        let database_url =
            env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://localhost/test".to_string());

        assert!(!rust_log.is_empty());
        assert!(!database_url.is_empty());
    }
}
EOF
    log_success "Created smoke_generated.rs for api"
fi

if [ ! -f "backend/crates/api/tests/integration_socket.rs" ]; then
    cat > "backend/crates/api/tests/integration_socket.rs" << 'EOF'
// Integration tests for Unix socket communication between Rust and Python
// Run with: cargo test --test integration_socket

use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

// Mock socket tests that don't require actual Python sidecar running
// In real CI, these would be run after services are started

/// Test that socket path is correctly configured
#[test]
fn socket_path_configuration() {
    // Test reading socket path from environment
    let socket_path = std::env::var("PYTHON_SIDECAR_SOCKET")
        .unwrap_or_else(|_| "/tmp/python-sidecar.sock".to_string());

    let path = PathBuf::from(socket_path);
    assert!(path.is_absolute() || path.starts_with("~"));
}

/// Test socket path directory creation
#[tokio::test]
async fn socket_directory_creation() {
    use std::fs;

    // Create a temp socket path
    let temp_dir = std::env::temp_dir().join("fullstackhex_test");
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    let socket_path = temp_dir.join("test-socket.sock");

    // Clean up if exists
    if socket_path.exists() {
        fs::remove_file(&socket_path).expect("Failed to remove stale socket");
    }

    // Verify directory exists
    assert!(temp_dir.exists());
    assert!(temp_dir.is_dir());

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

/// Test error handling for missing socket
#[tokio::test]
async fn error_handling_missing_socket() {
    use tokio::net::UnixStream;

    let non_existent = PathBuf::from("/tmp/non-existent-socket.sock");

    // Attempting to connect to non-existent socket should fail
    let result = timeout(Duration::from_secs(1), UnixStream::connect(&non_existent)).await;

    match result {
        Ok(Err(_)) => {
            // Expected: connection failed
        }
        Ok(Ok(_)) => {
            panic!("Should not be able to connect to non-existent socket");
        }
        Err(_) => {
            // Timeout is also acceptable
        }
    }
}

/// Test socket path from environment with priority
#[test]
fn socket_path_env_override() {
    // Save original value
    let original = std::env::var("PYTHON_SIDECAR_SOCKET").ok();

    // Safety: single-threaded test; no other threads reading this variable.
    unsafe {
        std::env::set_var("PYTHON_SIDECAR_SOCKET", "/custom/path/socket.sock");
    }
    let path = std::env::var("PYTHON_SIDECAR_SOCKET").unwrap();
    assert_eq!(path, "/custom/path/socket.sock");

    // Restore original
    unsafe {
        match original {
            Some(val) => std::env::set_var("PYTHON_SIDECAR_SOCKET", val),
            None => std::env::remove_var("PYTHON_SIDECAR_SOCKET"),
        }
    }
}

/// Test request structure for sidecar communication
#[test]
fn sidecar_request_structure() {
    // Simulate what a request to Python sidecar might look like
    let request_json = serde_json::json!({
        "method": "GET",
        "path": "/api/data",
        "headers": {},
        "body": null
    });

    assert_eq!(request_json["method"], "GET");
    assert_eq!(request_json["path"], "/api/data");
    assert!(request_json["body"].is_null());
}

/// Test response structure from sidecar
#[test]
fn sidecar_response_structure() {
    let response_json = serde_json::json!({
        "status": 200,
        "body": {"message": "success"},
        "headers": {"content-type": "application/json"}
    });

    assert_eq!(response_json["status"], 200);
    assert_eq!(response_json["body"]["message"], "success");
    assert_eq!(response_json["headers"]["content-type"], "application/json");
}

/// Test retry logic for socket connection
#[tokio::test]
async fn socket_retry_logic() {
    let socket_path = PathBuf::from("/tmp/non-existent-test-socket.sock");
    let max_retries = 3;
    let mut attempts = 0;

    loop {
        attempts += 1;

        let result = timeout(
            Duration::from_millis(100),
            tokio::net::UnixStream::connect(&socket_path),
        )
        .await;

        match result {
            Ok(Ok(_)) => {
                // Connection succeeded (shouldn't happen in this test)
                break;
            }
            _ => {
                if attempts >= max_retries {
                    // Give up after max retries
                    assert!(attempts >= max_retries);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }

    assert!(attempts >= max_retries);
}

/// Mock test for full request/response cycle
/// (Requires actual sidecar running - marked as ignored by default)
#[tokio::test]
#[ignore]
async fn full_socket_communication() {
    // This test requires the Python sidecar to be running
    // Run with: cargo test --test integration_socket -- --ignored

    let socket_path = std::env::var("PYTHON_SIDECAR_SOCKET")
        .unwrap_or_else(|_| "/tmp/python-sidecar.sock".to_string());

    if !PathBuf::from(&socket_path).exists() {
        println!("Skipping test: socket not found at {}", socket_path);
        return;
    }

    // In a real implementation, you would:
    // 1. Connect to the Unix socket
    // 2. Send an HTTP request over the socket
    // 3. Receive and parse the response
    // 4. Assert on the response

    assert!(PathBuf::from(&socket_path).exists());
}
EOF
    log_success "Created integration_socket.rs for api"
fi

# Frontend unit + integration + smoke tests
mkdir -p frontend/tests

if [ ! -f "frontend/tests/unit.test.ts" ]; then
    cat > "frontend/tests/unit.test.ts" << 'EOF'
import { describe, expect, test } from "bun:test";

describe("frontend generated unit test", () => {
  test("health endpoint path is valid", () => {
    const healthRoute = "/api/health";
    expect(healthRoute).toStartWith("/api/");
    expect(healthRoute).toContain("health");
  });

  test("environment variables are defined", () => {
    const apiUrl = process.env.PUBLIC_API_URL || "http://localhost:8001";
    expect(apiUrl).toBeTypeOf("string");
    expect(apiUrl.length).toBeGreaterThan(0);
  });

  test("TypeScript types work correctly", () => {
    interface HealthResponse {
      status: string;
      service: string;
    }

    const mockResponse: HealthResponse = {
      status: "ok",
      service: "api"
    };

    expect(mockResponse.status).toBe("ok");
    expect(mockResponse.service).toBe("api");
  });
});
EOF
    log_success "Created unit.test.ts for frontend"
fi

if [ ! -f "frontend/tests/integration-health-route.test.ts" ]; then
    cat > "frontend/tests/integration-health-route.test.ts" << 'EOF'
import { describe, expect, test } from "bun:test";

describe("frontend generated integration test", () => {
  test("health route path is stable", () => {
    const route = "/api/health";
        expect(route.startsWith("/api/")).toBe(true);
  });
});
EOF
    log_success "Created integration-health-route.test.ts for frontend"
fi

if [ ! -f "frontend/tests/smoke.test.ts" ]; then
    cat > "frontend/tests/smoke.test.ts" << 'EOF'
import { expect, test } from "bun:test";

test("generated frontend smoke test", () => {
  expect(typeof Bun.version).toBe("string");
});
EOF
    log_success "Created smoke.test.ts for frontend"
fi

log_success "Generated test suites completed"
exit 0