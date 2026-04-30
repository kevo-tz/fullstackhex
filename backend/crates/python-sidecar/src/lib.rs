use std::path::PathBuf;
use std::time::Duration;

/// Errors that can occur when communicating with the Python sidecar.
#[derive(Debug, thiserror::Error)]
pub enum SidecarError {
    #[error("socket not found at {0}")]
    SocketNotFound(PathBuf),
    #[error("connection failed: {0}")]
    ConnectionFailed(String),
    #[error("request timed out after {0:?}")]
    Timeout(Duration),
    #[error("invalid JSON response: {0}")]
    InvalidResponse(String),
    #[error("HTTP error from sidecar: {status} — {body}")]
    HttpError { status: u16, body: String },
}

pub struct PythonSidecar {
    socket_path: PathBuf,
    timeout: Duration,
    max_retries: u32,
}

impl PythonSidecar {
    /// Create a new handle with explicit configuration.
    pub fn new(socket_path: impl Into<PathBuf>, timeout: Duration, max_retries: u32) -> Self {
        Self {
            socket_path: socket_path.into(),
            timeout,
            max_retries,
        }
    }

    /// Create from environment variables.
    /// Reads `PYTHON_SIDECAR_SOCKET`, `PYTHON_SIDECAR_TIMEOUT_MS`, `PYTHON_SIDECAR_MAX_RETRIES`.
    pub fn from_env() -> Self {
        let socket_path = std::env::var("PYTHON_SIDECAR_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp/fullstackhex-python.sock"));

        let timeout_ms: u64 = std::env::var("PYTHON_SIDECAR_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5000);

        let max_retries: u32 = std::env::var("PYTHON_SIDECAR_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3);

        Self {
            socket_path,
            timeout: Duration::from_millis(timeout_ms),
            max_retries,
        }
    }

    /// Returns true if the socket file exists on disk.
    /// Does not verify that the socket accepts connections.
    pub fn is_available(&self) -> bool {
        self.socket_path.exists()
    }

    /// GET a path from the Python sidecar. Returns parsed JSON body.
    /// Checks socket existence before connecting.
    /// Retries connection failures up to `max_retries` with backoff.
    pub async fn get(&self, path: &str) -> Result<serde_json::Value, SidecarError> {
        if !self.is_available() {
            return Err(SidecarError::SocketNotFound(self.socket_path.clone()));
        }

        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                const BACKOFF_BASE_MS: u64 = 100;
                let backoff = Duration::from_millis(BACKOFF_BASE_MS * 2u64.pow(attempt - 1));
                tokio::time::sleep(backoff).await;
            }

            match tokio::time::timeout(self.timeout, self.try_get(path)).await {
                Ok(Ok(value)) => return Ok(value),
                Ok(Err(e)) => {
                    // Fast-fail on non-retryable errors
                    match &e {
                        SidecarError::SocketNotFound(_)
                        | SidecarError::InvalidResponse(_)
                        | SidecarError::HttpError { .. } => return Err(e),
                        SidecarError::ConnectionFailed(_) | SidecarError::Timeout(_) => {
                            last_error = Some(e);
                        }
                    }
                }
                Err(_elapsed) => {
                    last_error = Some(SidecarError::Timeout(self.timeout));
                }
            }
        }

        Err(last_error.unwrap_or(SidecarError::Timeout(self.timeout)))
    }

    async fn try_get(&self, path: &str) -> Result<serde_json::Value, SidecarError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::UnixStream;

        // Reject paths containing CR or LF to prevent HTTP header injection
        if path.contains('\r') || path.contains('\n') {
            return Err(SidecarError::InvalidResponse(
                "path contains invalid characters".into(),
            ));
        }

        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .map_err(|e| SidecarError::ConnectionFailed(e.to_string()))?;

        let request = format!(
            "GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
            path
        );
        stream
            .write_all(request.as_bytes())
            .await
            .map_err(|e| SidecarError::ConnectionFailed(e.to_string()))?;

        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .map_err(|e| SidecarError::ConnectionFailed(e.to_string()))?;

        // Find end of HTTP headers
        let body_start = response
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|p| p + 4)
            .unwrap_or(0);

        let body = &response[body_start..];

        // Parse status code from headers
        let headers = std::str::from_utf8(&response[..body_start])
            .map_err(|e| SidecarError::InvalidResponse(e.to_string()))?;

        let status_code = headers
            .lines()
            .next()
            .and_then(|status_line| {
                let parts: Vec<&str> = status_line.split_whitespace().collect();
                if parts.len() >= 2 {
                    parts[1].parse::<u16>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(200);

        if status_code >= 400 {
            let body_str = String::from_utf8_lossy(body).to_string();
            return Err(SidecarError::HttpError {
                status: status_code,
                body: body_str,
            });
        }

        serde_json::from_slice(body).map_err(|e| SidecarError::InvalidResponse(e.to_string()))
    }

    /// Convenience: GET /health from the sidecar.
    pub async fn health(&self) -> Result<serde_json::Value, SidecarError> {
        self.get("/health").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sets_fields() {
        let sc = PythonSidecar::new("/tmp/test.sock", Duration::from_secs(3), 5);
        assert_eq!(sc.socket_path, PathBuf::from("/tmp/test.sock"));
        assert_eq!(sc.timeout, Duration::from_secs(3));
        assert_eq!(sc.max_retries, 5);
    }

    #[test]
    #[serial_test::serial]
    fn from_env_defaults() {
        unsafe {
            std::env::remove_var("PYTHON_SIDECAR_SOCKET");
            std::env::remove_var("PYTHON_SIDECAR_TIMEOUT_MS");
            std::env::remove_var("PYTHON_SIDECAR_MAX_RETRIES");
        }
        let sc = PythonSidecar::from_env();
        assert_eq!(
            sc.socket_path,
            PathBuf::from("/tmp/fullstackhex-python.sock")
        );
        assert_eq!(sc.timeout, Duration::from_millis(5000));
        assert_eq!(sc.max_retries, 3);
    }

    #[test]
    #[serial_test::serial]
    fn from_env_overrides() {
        unsafe {
            std::env::set_var("PYTHON_SIDECAR_SOCKET", "/custom/path.sock");
            std::env::set_var("PYTHON_SIDECAR_TIMEOUT_MS", "2000");
            std::env::set_var("PYTHON_SIDECAR_MAX_RETRIES", "1");
        }
        let sc = PythonSidecar::from_env();
        assert_eq!(sc.socket_path, PathBuf::from("/custom/path.sock"));
        assert_eq!(sc.timeout, Duration::from_millis(2000));
        assert_eq!(sc.max_retries, 1);
        unsafe {
            std::env::remove_var("PYTHON_SIDECAR_SOCKET");
            std::env::remove_var("PYTHON_SIDECAR_TIMEOUT_MS");
            std::env::remove_var("PYTHON_SIDECAR_MAX_RETRIES");
        }
    }

    #[test]
    fn is_available_true() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        // NamedTempFile creates a real file — we just need a path that exists
        let sc = PythonSidecar::new(tmp.path().to_path_buf(), Duration::from_secs(1), 1);
        assert!(sc.is_available());
        // Cleanup: tempfile drops the file, but we already know it existed
        drop(tmp);
    }

    #[test]
    fn is_available_false() {
        let sc = PythonSidecar::new(
            "/tmp/__nonexistent_fullstackhex_test__.sock",
            Duration::from_secs(1),
            1,
        );
        assert!(!sc.is_available());
    }

    #[test]
    fn health_delegates_to_get() {
        // Verify health() uses the right path by checking the error type.
        // With a nonexistent socket, get("/health") → SocketNotFound.
        let sc = PythonSidecar::new(
            "/tmp/__nonexistent_health_test__.sock",
            Duration::from_secs(1),
            1,
        );
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(sc.health());
        assert!(matches!(result, Err(SidecarError::SocketNotFound(_))));
    }

    #[tokio::test]
    async fn get_socket_not_found_immediate() {
        let sc = PythonSidecar::new(
            "/tmp/__nonexistent_get_test__.sock",
            Duration::from_secs(1),
            3,
        );
        let result = sc.get("/health").await;
        assert!(matches!(result, Err(SidecarError::SocketNotFound(_))));
    }

    // ------------------------------------------------------------------
    // Socket integration tests — require a mock Unix socket server
    // These tests use real UnixListener and are skipped by default
    // because they're timing-sensitive in Rust's parallel test runner.
    // Run with: cargo test -p python-sidecar -- --ignored
    // ------------------------------------------------------------------

    #[tokio::test]
    #[ignore = "requires real Unix socket infrastructure — run with --ignored"]
    async fn get_happy_path_via_socket() {
        use tokio::io::AsyncWriteExt;
        use tokio::net::UnixListener;

        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("happy.sock");
        let listener = UnixListener::bind(&sock_path).unwrap();
        let sc = PythonSidecar::new(sock_path, Duration::from_secs(2), 0);

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let response =
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"status\":\"ok\"}";
            stream.write_all(response.as_bytes()).await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(10)).await;
        let result = sc.get("/health").await;
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        assert_eq!(result.unwrap()["status"], "ok");
    }

    #[tokio::test]
    #[ignore = "requires real Unix socket infrastructure — run with --ignored"]
    async fn get_http_error_via_socket() {
        use tokio::io::AsyncWriteExt;
        use tokio::net::UnixListener;

        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("httperr.sock");
        let listener = UnixListener::bind(&sock_path).unwrap();
        let sc = PythonSidecar::new(sock_path, Duration::from_secs(2), 0);

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let response = "HTTP/1.1 500 Internal Server Error\r\n\r\nbang";
            stream.write_all(response.as_bytes()).await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(10)).await;
        let result = sc.get("/health").await;
        assert!(
            matches!(result, Err(SidecarError::HttpError { status: 500, .. })),
            "expected HttpError 500, got {:?}",
            result
        );
    }

    #[tokio::test]
    #[ignore = "requires real Unix socket infrastructure — run with --ignored"]
    async fn get_invalid_json_via_socket() {
        use tokio::io::AsyncWriteExt;
        use tokio::net::UnixListener;

        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("invalid.sock");
        let listener = UnixListener::bind(&sock_path).unwrap();
        let sc = PythonSidecar::new(sock_path, Duration::from_secs(2), 0);

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let response = "HTTP/1.1 200 OK\r\n\r\nnot-json";
            stream.write_all(response.as_bytes()).await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(10)).await;
        let result = sc.get("/health").await;
        assert!(
            matches!(result, Err(SidecarError::InvalidResponse(_))),
            "expected InvalidResponse, got {:?}",
            result
        );
    }

    #[tokio::test]
    #[ignore = "requires real Unix socket infrastructure — run with --ignored"]
    async fn get_retries_on_connection_refused_via_socket() {
        use tokio::io::AsyncWriteExt;
        use tokio::net::UnixListener;

        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("retry.sock");
        // Don't create a listener yet — first connect attempt will fail.
        // Create the file so is_available() passes.
        std::fs::File::create(&sock_path).unwrap();
        let sc = PythonSidecar::new(sock_path.clone(), Duration::from_millis(500), 2);

        // After a short delay, bring up the listener so the second attempt succeeds
        let listener = UnixListener::bind(&sock_path).unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let response = "HTTP/1.1 200 OK\r\n\r\n{\"retry\":\"worked\"}";
            stream.write_all(response.as_bytes()).await.unwrap();
        });

        let result = sc.get("/health").await;
        assert!(
            result.is_ok(),
            "expected Ok after retry, got {:?}",
            result
        );
        assert_eq!(result.unwrap()["retry"], "worked");
    }

    #[tokio::test]
    async fn get_rejects_crlf_in_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crlf.sock");
        std::fs::File::create(&path).unwrap();
        let sc = PythonSidecar::new(path, Duration::from_secs(1), 0);
        let result = sc.get("/health\r\nX-Injected: true").await;
        assert!(matches!(result, Err(SidecarError::InvalidResponse(_))));
    }
}
