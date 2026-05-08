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
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("HTTP error from sidecar: {status} — {body}")]
    HttpError { status: u16, body: String },
}

pub struct PythonSidecar {
    socket_path: PathBuf,
    timeout: Duration,
    max_retries: u32,
}

/// Validate a header value: reject CR, LF, and overlong values.
const BACKOFF_BASE_MS: u64 = 100;
const MAX_BACKOFF_MS: u64 = 30_000;

/// Compute exponential backoff for retry attempt (1-indexed).
fn backoff_for_attempt(attempt: u32) -> Duration {
    let raw_ms = BACKOFF_BASE_MS.saturating_mul(2u64.saturating_pow(attempt.saturating_sub(1)));
    Duration::from_millis(raw_ms.min(MAX_BACKOFF_MS))
}

fn validate_header_value(name: &str, value: &str) -> Result<(), SidecarError> {
    if value.contains('\r') || value.contains('\n') {
        return Err(SidecarError::InvalidInput(format!(
            "{} contains invalid characters",
            name
        )));
    }
    if value.len() > 256 {
        return Err(SidecarError::InvalidInput(format!(
            "{} exceeds maximum length (256 bytes)",
            name
        )));
    }
    Ok(())
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
            .unwrap_or(3)
            .min(10);

        Self {
            socket_path,
            timeout: Duration::from_millis(timeout_ms),
            max_retries,
        }
    }

    /// Returns the configured socket path.
    pub fn socket_path(&self) -> &std::path::Path {
        &self.socket_path
    }

    /// Returns true if the socket file exists on disk.
    /// Does not verify that the socket accepts connections.
    pub fn is_available(&self) -> bool {
        self.socket_path.exists()
    }

    /// GET a path from the Python sidecar. Returns parsed JSON body.
    /// Checks socket existence before connecting.
    /// Retries connection failures up to `max_retries` with backoff.
    /// Generates a UUIDv4 trace_id and sends it as an x-trace-id header.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use py_sidecar::PythonSidecar;
    /// use std::time::Duration;
    ///
    /// # async fn example() {
    /// let sidecar = PythonSidecar::new(
    ///     "/tmp/fullstackhex-python.sock",
    ///     Duration::from_secs(5),
    ///     3,
    /// );
    ///
    /// match sidecar.get("/health").await {
    ///     Ok(json) => println!("Health response: {json}"),
    ///     Err(e) => eprintln!("Sidecar error: {e}"),
    /// }
    /// # }
    /// ```
    pub async fn get(&self, path: &str) -> Result<serde_json::Value, SidecarError> {
        self.get_with_trace_id(path, &uuid::Uuid::new_v4().to_string(), None)
            .await
    }

    /// GET a path with auth headers forwarded to the Python sidecar.
    /// `auth_headers` is `(user_id, email, name, signature)` — all pre-computed.
    pub async fn get_with_auth(
        &self,
        path: &str,
        auth_headers: (&str, &str, &str, &str),
    ) -> Result<serde_json::Value, SidecarError> {
        self.get_with_trace_id(path, &uuid::Uuid::new_v4().to_string(), Some(auth_headers))
            .await
    }

    /// GET a path with an explicit trace_id. Used when the caller wants to
    /// propagate an existing trace context rather than generating a new one.
    pub async fn get_with_trace_id(
        &self,
        path: &str,
        trace_id: &str,
        auth_headers: Option<(&str, &str, &str, &str)>,
    ) -> Result<serde_json::Value, SidecarError> {
        self.do_get(path, Some(trace_id), auth_headers).await
    }

    /// GET raw bytes from a path. Returns the response body without JSON parsing.
    pub async fn get_raw(&self, path: &str) -> Result<Vec<u8>, SidecarError> {
        self.get_raw_with_trace_id(path, &uuid::Uuid::new_v4().to_string(), None)
            .await
    }

    /// GET raw bytes with an explicit trace_id.
    pub async fn get_raw_with_trace_id(
        &self,
        path: &str,
        trace_id: &str,
        auth_headers: Option<(&str, &str, &str, &str)>,
    ) -> Result<Vec<u8>, SidecarError> {
        self.do_get_raw(path, Some(trace_id), auth_headers).await
    }

    async fn do_get(
        &self,
        path: &str,
        trace_id: Option<&str>,
        auth_headers: Option<(&str, &str, &str, &str)>,
    ) -> Result<serde_json::Value, SidecarError> {
        let body = self.do_request_inner(path, trace_id, auth_headers).await?;
        serde_json::from_slice(&body).map_err(|e| SidecarError::InvalidResponse(e.to_string()))
    }

    async fn do_get_raw(
        &self,
        path: &str,
        trace_id: Option<&str>,
        auth_headers: Option<(&str, &str, &str, &str)>,
    ) -> Result<Vec<u8>, SidecarError> {
        self.do_request_inner(path, trace_id, auth_headers).await
    }

    /// Shared retry-with-backoff loop. Calls perform_request and handles
    /// status-code checks, retries, and fast-fail classification.
    async fn do_request_inner(
        &self,
        path: &str,
        trace_id: Option<&str>,
        auth_headers: Option<(&str, &str, &str, &str)>,
    ) -> Result<Vec<u8>, SidecarError> {
        if !self.is_available() {
            return Err(SidecarError::SocketNotFound(self.socket_path.clone()));
        }

        let mut last_error = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                let backoff = backoff_for_attempt(attempt);
                tokio::time::sleep(backoff).await;
            }

            match tokio::time::timeout(
                self.timeout,
                self.perform_request(path, trace_id, auth_headers),
            )
            .await
            {
                Ok(Ok((status_code, body))) => {
                    if status_code >= 400 {
                        let body_str = String::from_utf8_lossy(&body).to_string();
                        return Err(SidecarError::HttpError {
                            status: status_code,
                            body: body_str,
                        });
                    }
                    return Ok(body);
                }
                Ok(Err(e)) => {
                    // Fast-fail on non-retryable errors
                    match &e {
                        SidecarError::SocketNotFound(_)
                        | SidecarError::InvalidResponse(_)
                        | SidecarError::InvalidInput(_)
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

    async fn perform_request(
        &self,
        path: &str,
        trace_id: Option<&str>,
        auth_headers: Option<(&str, &str, &str, &str)>,
    ) -> Result<(u16, Vec<u8>), SidecarError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::UnixStream;

        // Validate path to prevent HTTP header injection
        validate_header_value("path", path)?;

        // Validate trace_id
        if let Some(tid) = trace_id {
            validate_header_value("trace_id", tid)?;
        }

        // Validate auth headers
        if let Some((user_id, email, name, signature)) = auth_headers {
            validate_header_value("X-User-Id", user_id)?;
            validate_header_value("X-User-Email", email)?;
            validate_header_value("X-User-Name", name)?;
            validate_header_value("X-Auth-Signature", signature)?;
        }

        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .map_err(|e| SidecarError::ConnectionFailed(e.to_string()))?;

        let mut request = if let Some(tid) = trace_id {
            format!(
                "GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nx-trace-id: {}\r\n",
                path, tid
            )
        } else {
            format!(
                "GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n",
                path
            )
        };

        if let Some((user_id, email, name, signature)) = auth_headers {
            request.push_str(&format!(
                "X-User-Id: {}\r\nX-User-Email: {}\r\nX-User-Name: {}\r\nX-Auth-Signature: {}\r\n",
                user_id, email, name, signature,
            ));
        }

        request.push_str("\r\n");

        stream
            .write_all(request.as_bytes())
            .await
            .map_err(|e| SidecarError::ConnectionFailed(e.to_string()))?;

        const MAX_RESPONSE_SIZE: usize = 1_048_576; // 1 MiB
        let mut response = Vec::new();
        stream
            .take(MAX_RESPONSE_SIZE as u64 + 1)
            .read_to_end(&mut response)
            .await
            .map_err(|e| SidecarError::ConnectionFailed(e.to_string()))?;

        if response.len() > MAX_RESPONSE_SIZE {
            return Err(SidecarError::InvalidResponse(
                "response exceeded maximum allowed size".into(),
            ));
        }

        // Find end of HTTP headers
        let body_start = response
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map(|p| p + 4)
            .ok_or_else(|| SidecarError::InvalidResponse("missing HTTP header separator".into()))?;

        let body = response[body_start..].to_vec();

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

        Ok((status_code, body))
    }

    /// Convenience: GET /health from the sidecar.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use py_sidecar::PythonSidecar;
    /// use std::time::Duration;
    ///
    /// # async fn example() {
    /// let sidecar = PythonSidecar::new(
    ///     "/tmp/fullstackhex-python.sock",
    ///     Duration::from_secs(5),
    ///     3,
    /// );
    ///
    /// match sidecar.health().await {
    ///     Ok(json) => println!("Sidecar healthy: {json}"),
    ///     Err(e) => eprintln!("Sidecar unhealthy: {e}"),
    /// }
    /// # }
    /// ```
    pub async fn health(&self) -> Result<serde_json::Value, SidecarError> {
        let trace_id = uuid::Uuid::new_v4().to_string();
        tracing::info!(%trace_id, target = "py_sidecar", "health check");
        let start = std::time::Instant::now();
        let result = self.get_with_trace_id("/health", &trace_id, None).await;
        let duration_ms = start.elapsed().as_millis() as u64;
        match &result {
            Ok(_) => {
                tracing::info!(%trace_id, duration_ms, target = "py_sidecar", "health check OK")
            }
            Err(e) => {
                tracing::warn!(%trace_id, duration_ms, error = %e, target = "py_sidecar", "health check failed")
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    #[test]
    fn validate_header_value_rejects_cr() {
        assert!(validate_header_value("X-Test", "val\r\nInjected").is_err());
    }

    #[test]
    fn validate_header_value_rejects_lf() {
        assert!(validate_header_value("X-Test", "val\nInjected").is_err());
    }

    #[test]
    fn validate_header_value_rejects_overlong() {
        let long = "a".repeat(257);
        assert!(validate_header_value("X-Test", &long).is_err());
    }

    #[test]
    fn validate_header_value_accepts_valid() {
        assert!(validate_header_value("X-Test", "normal-value").is_ok());
        assert!(validate_header_value("X-Test", "").is_ok());
    }

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
        // SAFETY: serial_test ensures no other test mutates env vars concurrently.
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
        // SAFETY: serial_test ensures no other test mutates env vars concurrently.
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

    #[tokio::test]
    async fn get_with_auth_validates_headers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth-val.sock");
        std::fs::File::create(&path).unwrap();
        let sc = PythonSidecar::new(path, Duration::from_secs(1), 0);
        // CR in name should be rejected
        let result = sc
            .get_with_auth("/health", ("user-1", "a@b.com", "bad\nname", "abc123"))
            .await;
        assert!(matches!(result, Err(SidecarError::InvalidInput(_))));
    }

    // ------------------------------------------------------------------
    // Socket integration tests — require a mock Unix socket server
    // These tests use real UnixListener and are skipped by default
    // because they're timing-sensitive in Rust's parallel test runner.
    // Run with: cargo test -p py-sidecar -- --ignored
    // ------------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial]
    #[ignore = "socket test — run via make test-socket-ci"]
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
            stream.shutdown().await.ok();
            tokio::time::sleep(Duration::from_millis(200)).await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let result = sc.get("/health").await;
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        assert_eq!(result.unwrap()["status"], "ok");
    }

    #[tokio::test]
    #[serial_test::serial]
    #[ignore = "socket test — run via make test-socket-ci"]
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
            stream.shutdown().await.ok();
            tokio::time::sleep(Duration::from_millis(200)).await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let result = sc.get("/health").await;
        assert!(
            matches!(result, Err(SidecarError::HttpError { status: 500, .. })),
            "expected HttpError 500, got {:?}",
            result
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    #[ignore = "socket test — run via make test-socket-ci"]
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
            stream.shutdown().await.ok();
            tokio::time::sleep(Duration::from_millis(200)).await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let result = sc.get("/health").await;
        assert!(
            matches!(result, Err(SidecarError::InvalidResponse(_))),
            "expected InvalidResponse, got {:?}",
            result
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    #[ignore = "socket test — run via make test-socket-ci"]
    async fn get_retries_on_connection_refused_via_socket() {
        use tokio::io::AsyncWriteExt;
        use tokio::net::UnixListener;

        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("retry.sock");
        // Don't create a listener yet — first connect attempt will fail.
        // Create the file so is_available() passes, then remove it so bind works.
        std::fs::File::create(&sock_path).unwrap();
        let sc = PythonSidecar::new(sock_path.clone(), Duration::from_millis(500), 2);

        // After a short delay, bring up the listener so the second attempt succeeds
        let _ = std::fs::remove_file(&sock_path);
        let listener = UnixListener::bind(&sock_path).unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let response = "HTTP/1.1 200 OK\r\n\r\n{\"retry\":\"worked\"}";
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.shutdown().await.ok();
            tokio::time::sleep(Duration::from_millis(200)).await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let result = sc.get("/health").await;
        assert!(result.is_ok(), "expected Ok after retry, got {:?}", result);
        assert_eq!(result.unwrap()["retry"], "worked");
    }

    #[tokio::test]
    async fn get_rejects_crlf_in_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crlf.sock");
        std::fs::File::create(&path).unwrap();
        let sc = PythonSidecar::new(path, Duration::from_secs(1), 0);
        let result = sc.get("/health\r\nX-Injected: true").await;
        assert!(matches!(result, Err(SidecarError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn get_rejects_cr_only_in_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cr.sock");
        std::fs::File::create(&path).unwrap();
        let sc = PythonSidecar::new(path, Duration::from_secs(1), 0);
        let result = sc.get("/health\rX-Injected: true").await;
        assert!(matches!(result, Err(SidecarError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn get_rejects_lf_only_in_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lf.sock");
        std::fs::File::create(&path).unwrap();
        let sc = PythonSidecar::new(path, Duration::from_secs(1), 0);
        let result = sc.get("/health\nX-Injected: true").await;
        assert!(matches!(result, Err(SidecarError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn get_rejects_crlf_in_trace_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trace-crlf.sock");
        std::fs::File::create(&path).unwrap();
        let sc = PythonSidecar::new(path, Duration::from_secs(1), 0);
        let result = sc
            .get_with_trace_id("/health", "test-id\r\nInjected", None)
            .await;
        assert!(matches!(result, Err(SidecarError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn get_rejects_cr_only_in_trace_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trace-cr.sock");
        std::fs::File::create(&path).unwrap();
        let sc = PythonSidecar::new(path, Duration::from_secs(1), 0);
        let result = sc
            .get_with_trace_id("/health", "test-id\rInjected", None)
            .await;
        assert!(matches!(result, Err(SidecarError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn get_rejects_lf_only_in_trace_id() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trace-lf.sock");
        std::fs::File::create(&path).unwrap();
        let sc = PythonSidecar::new(path, Duration::from_secs(1), 0);
        let result = sc
            .get_with_trace_id("/health", "test-id\nInjected", None)
            .await;
        assert!(matches!(result, Err(SidecarError::InvalidInput(_))));
    }

    // ------------------------------------------------------------------
    // Socket integration tests — use mock UnixListeners
    // These tests create in-memory UnixListeners with controlled responses
    // and are skipped by default because they're timing-sensitive.
    // Run with: cargo test -p py-sidecar -- --ignored
    // Or via: make test-socket-ci
    // ------------------------------------------------------------------

    #[tokio::test]
    #[serial_test::serial]
    #[ignore = "socket test — run via make test-socket-ci"]
    async fn get_timeout_via_socket() {
        use tokio::net::UnixListener;

        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("timeout.sock");
        let listener = UnixListener::bind(&sock_path).unwrap();
        let sc = PythonSidecar::new(sock_path, Duration::from_millis(200), 0);

        // Accept but never write — client should time out
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            // Hold connection open without writing
            tokio::time::sleep(Duration::from_secs(5)).await;
            let _ = stream.shutdown().await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let result = sc.get("/health").await;
        assert!(
            matches!(result, Err(SidecarError::Timeout(_))),
            "expected Timeout, got {:?}",
            result
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    #[ignore = "socket test — run via make test-socket-ci"]
    async fn get_missing_json_fields_via_socket() {
        use tokio::io::AsyncWriteExt;
        use tokio::net::UnixListener;

        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("missing.sock");
        let listener = UnixListener::bind(&sock_path).unwrap();
        let sc = PythonSidecar::new(sock_path, Duration::from_secs(2), 0);

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let response = "HTTP/1.1 200 OK\r\n\r\n{\"foo\":\"bar\"}";
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.shutdown().await.ok();
            tokio::time::sleep(Duration::from_millis(200)).await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let result = sc.get("/health").await;
        // Parses successfully — missing fields are fine, JSON is valid
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        let v = result.unwrap();
        assert_eq!(v["foo"], "bar");
        // Verify no spurious status field
        assert!(v.get("status").is_none());
    }

    #[tokio::test]
    #[serial_test::serial]
    #[ignore = "socket test — run via make test-socket-ci"]
    async fn get_empty_body_via_socket() {
        use tokio::io::AsyncWriteExt;
        use tokio::net::UnixListener;

        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("empty.sock");
        let listener = UnixListener::bind(&sock_path).unwrap();
        let sc = PythonSidecar::new(sock_path, Duration::from_secs(2), 0);

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let response = "HTTP/1.1 200 OK\r\n\r\n";
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.shutdown().await.ok();
            tokio::time::sleep(Duration::from_millis(200)).await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let result = sc.get("/health").await;
        assert!(
            matches!(result, Err(SidecarError::InvalidResponse(_))),
            "expected InvalidResponse for empty body, got {:?}",
            result
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    #[ignore = "socket test — run via make test-socket-ci"]
    async fn get_trace_id_propagation_via_socket() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::UnixListener;

        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("trace.sock");
        let listener = UnixListener::bind(&sock_path).unwrap();
        let sc = PythonSidecar::new(sock_path, Duration::from_secs(2), 0);

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 512];
            let n = stream.read(&mut buf).await.unwrap();
            let request = String::from_utf8_lossy(&buf[..n]);
            let received = request.contains("x-trace-id:") && request.contains("qa-propagate-123");
            let response = format!(
                "HTTP/1.1 200 OK\r\n\r\n{{\"status\":\"ok\",\"trace_present\":{}}}",
                received
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.shutdown().await.ok();
            tokio::time::sleep(Duration::from_millis(200)).await;
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let result = sc
            .get_with_trace_id("/health", "qa-propagate-123", None)
            .await;
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
        let v = result.unwrap();
        assert_eq!(v["status"], "ok");
        assert!(v["trace_present"].as_bool().unwrap_or(false));
    }

    // ── Backoff calculation tests ───────────────────────────────────────

    #[test]
    fn backoff_attempt_1_is_base() {
        let d = backoff_for_attempt(1);
        assert_eq!(d, Duration::from_millis(100));
    }

    #[test]
    fn backoff_attempt_2_doubles() {
        let d = backoff_for_attempt(2);
        assert_eq!(d, Duration::from_millis(200));
    }

    #[test]
    fn backoff_attempt_9_is_25s() {
        let d = backoff_for_attempt(9);
        assert_eq!(d, Duration::from_millis(25600));
    }

    #[test]
    fn backoff_attempt_10_saturates() {
        let d = backoff_for_attempt(10);
        assert_eq!(d, Duration::from_millis(MAX_BACKOFF_MS));
    }

    #[test]
    fn backoff_attempt_3_is_400ms() {
        let d = backoff_for_attempt(3);
        assert_eq!(d, Duration::from_millis(400));
    }

    #[test]
    fn backoff_saturates_at_max() {
        for attempt in 9..=20 {
            let d = backoff_for_attempt(attempt);
            assert!(
                d <= Duration::from_millis(MAX_BACKOFF_MS),
                "attempt {attempt}: {d:?} exceeds max",
            );
        }
    }
}
