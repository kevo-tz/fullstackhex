# Monitoring Stack Guide

## Overview

FullStackHex includes a monitoring stack with Prometheus and Grafana for tracking application performance, system metrics, and service health.

## Accessing the Monitoring Stack

### Start the Monitoring Stack

```bash
# Using make
make up

# Or using docker compose directly
docker compose -f compose/dev.yml up -d
docker compose -f compose/monitor.yml up -d
```

### Access Points

| Service | URL | Credentials |
|---------|-----|--------------|
| Grafana | http://localhost:3000 | admin / (see `.env` GRAFANA_ADMIN_PASSWORD) |
| Prometheus | http://localhost:9090 | None (read-only) |

## Pre-configured Dashboards

### FullStackHex Overview Dashboard

Located at: `monitoring/grafana/dashboards/overview.json`

**Panels included:**
1. **Service Health Overview** - Shows Up/Down status for all services
2. **Request Rate (RPS)** - Requests per second
3. **p99 Latency (ms)** - 99th percentile response time
4. **Error Rate (%)** - Percentage of 5xx responses
5. **System Metrics** - CPU and Memory usage

**Access:** Grafana → Dashboards → FullStackHex - Overview

### Importing Dashboards

If the dashboard isn't auto-loaded:

1. Go to Grafana → Dashboards → Import
2. Upload `monitoring/grafana/dashboards/overview.json`
3. Select "Prometheus" as datasource
4. Click "Import"

## Key Metrics to Watch

### Application Metrics (from Rust Backend)

| Metric | Description | Target | Alert Threshold |
|--------|-------------|--------|------------------|
| `http_requests_total` | Total HTTP requests | - | - |
| `http_request_duration_ms` | Request latency histogram | p50 < 5ms, p99 < 20ms | p99 > 100ms |
| `http_requests_total{status=~"5.."}` | Server errors | < 1% | > 5% |
| `rust_sidecar_requests_total` | Requests to Python sidecar | - | - |
| `rust_sidecar_duration_ms` | Sidecar latency | p99 < 50ms | p99 > 200ms |

### System Metrics (from Node Exporter)

| Metric | Description | Target | Alert Threshold |
|--------|-------------|--------|------------------|
| `node_cpu_seconds_total` | CPU usage | < 80% | > 90% |
| `node_memory_MemAvailable_bytes` | Available memory | > 20% free | < 10% free |
| `node_disk_read_bytes_total` | Disk I/O | - | Sustained high I/O |
| `node_network_receive_bytes_total` | Network traffic | - | - |

### Database Metrics (configure separately)

| Metric | Description | Target |
|--------|-------------|--------|
| `pg_stat_database_numbackends` | Active connections | < max_connections |
| `pg_stat_database_xact_commit` | Committed transactions | - |
| `pg_stat_database_xact_rollback` | Rolled back transactions | < 1% |

## Setting Up Alerts

### In Grafana

1. Go to Alerting → Alert Rules
2. Click "New Rule"
3. Configure:

**Example: High Error Rate Alert**

```yaml
Name: High Error Rate
Condition: WHEN last() OF query(A, 5m, now) IS ABOVE 5
Folder: FullStackHex Alerts
Evaluation group: critical
```

**Example: High Latency Alert**

```yaml
Name: High p99 Latency
Condition: WHEN last() OF query(A, 5m, now) IS ABOVE 100
Folder: FullStackHex Alerts
Evaluation group: warning
```

### In Prometheus (Alertmanager)

Create `monitoring/alerts.yml`:

```yaml
groups:
  - name: fullstackhex
    rules:
      - alert: HighErrorRate
        expr: |
          sum(rate(http_requests_total{status=~"5.."}[5m])) 
          / sum(rate(http_requests_total[5m])) * 100 > 5
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High error rate detected"
          description: "Error rate is {{ $value }}%"

      - alert: HighLatency
        expr: |
          histogram_quantile(0.99, 
            sum(rate(http_request_duration_ms_bucket[5m])) by (le)
          ) > 100
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High p99 latency"
          description: "p99 latency is {{ $value }}ms"
```

## Exposing Metrics from Rust Backend

### Add Metrics Middleware

In `backend/crates/api/src/main.rs`:

```rust
use prometheus::{register_histogram, register_counter, Histogram, Counter};

// Define metrics
lazy_static! {
    static ref HTTP_REQUESTS: Counter = register_counter!(
        "http_requests_total",
        "Total HTTP requests"
    ).unwrap();

    static ref HTTP_DURATION: Histogram = register_histogram!(
        "http_request_duration_ms",
        "HTTP request duration in milliseconds"
    ).unwrap();
}

// Add middleware to track metrics
app = app.layer(
    tower_http::trace::TraceLayer::new_for_http()
        .on_request(|_req: &http::Request<_>, _span: &tracing::Span| {
            HTTP_REQUESTS.inc();
        })
        .on_response(|_resp: &http::Response<_>, latency: std::time::Duration, _span: &tracing::Span| {
            let ms = latency.as_millis() as f64;
            HTTP_DURATION.observe(ms);
        })
);
```

### Expose `/metrics` Endpoint

```rust
use prometheus::Encoder;

async fn metrics_handler() -> impl IntoResponse {
    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();
    let metrics = prometheus::gather();
    encoder.encode(&metrics, &mut buffer).unwrap();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, encoder.format_type())],
        buffer,
    )
}

// In your router
let app = app.route("/metrics", get(metrics_handler));
```

## Troubleshooting

### Grafana Can't Connect to Prometheus

1. Check Prometheus is running: `curl http://localhost:9090/-/healthy`
2. Verify datasource in Grafana:
   - Go to Configuration → Data Sources → Prometheus
   - Check URL is `http://prometheus:9090` (from Docker network)
   - Click "Save & Test"

### Metrics Not Showing in Prometheus

1. Check Rust backend exposes `/metrics`: `curl http://localhost:8001/metrics`
2. Verify Prometheus scrape config: `monitoring/prometheus.yml`
3. Check Prometheus targets: http://localhost:9090/targets

### Dashboard Panels Empty

1. Verify time range in Grafana (top right)
2. Check if services are running: `make health`
3. Query Prometheus directly: http://localhost:9090/graph

## Related Docs

- [Previous: CI.md](./CI.md) - CI/CD pipeline
- [Next: INFRASTRUCTURE.md](./INFRASTRUCTURE.md) - Infrastructure setup
- [All Docs](./INDEX.md) - Full documentation index
