# Monitoring Stack Guide

## Overview

FullStackHex includes a monitoring stack with Prometheus and Grafana for tracking application performance, system metrics, and service health.

## Accessing the Monitoring Stack

### Start the Monitoring Stack

```bash
# Start dev infrastructure
docker compose -f compose/dev.yml up -d

# Start monitoring stack
docker compose -f compose/monitor.yml up -d
```

### Access Points

| Service | URL | Credentials |
|---------|-----|--------------|
| Grafana | http://localhost:3000 | admin / (see `.env` GRAFANA_ADMIN_PASSWORD) |
| Prometheus | http://localhost:9090 | None (read-only) |

## Pre-configured Dashboards

All dashboards are auto-provisioned from \`compose/monitoring/grafana/dashboards/`.

### FullStackHex Overview Dashboard

Located at: \`compose/monitoring/grafana/dashboards/overview.json`

**Panels included:**
1. **Service Health Overview** - Shows Up/Down status for all services
2. **Request Rate (RPS)** - Requests per second
3. **p99 Latency (s)** - 99th percentile response time
4. **Error Rate (%)** - Percentage of 5xx responses
5. **System Metrics** - CPU and Memory usage

### Auth Dashboard

Located at: \`compose/monitoring/grafana/dashboards/auth.json`

**Panels included:** Login success/failure rates, registration activity, token issuance and refresh counts, active sessions, custom auth request rate (`auth_requests_total`), auth error rate by type (`auth_errors_total`), auth p50/p99 latency (`auth_latency_seconds`), auth errors cumulative, OAuth callbacks by provider.

### Database Dashboard

Located at: \`compose/monitoring/grafana/dashboards/database.json`

**Panels:** Active connections, query rate, cache hit ratio, slow queries.

### Python Sidecar Dashboard

Located at: \`compose/monitoring/grafana/dashboards/python.json`

**Panels:** Request rate, error rate, p99 latency, socket health.

### Infrastructure Dashboard

Located at: \`compose/monitoring/grafana/dashboards/infra.json`

**Panels:** CPU usage, memory usage, disk I/O, nginx request rate.

### SLO Dashboard

Located at: \`compose/monitoring/grafana/dashboards/slo.json`

**Panels:** Error rate vs SLO, p99 latency vs SLO, uptime %, error budget burn.

> **Note:** The SLO dashboard may show empty panels for fresh templates with no traffic. This is expected — data appears once requests start flowing.

### Importing Dashboards

If the dashboard isn't auto-loaded:

1. Go to Grafana → Dashboards → Import
2. Upload \`compose/monitoring/grafana/dashboards/overview.json`
3. Select "Prometheus" as datasource
4. Click "Import"

## Key Metrics to Watch

### Application Metrics (from Rust Backend)

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `http_requests_total` | Counter | `method`, `route`, `status` | Total HTTP requests |
| `http_request_duration_seconds` | Histogram | `method`, `route` | Request latency (seconds) |
| `db_pool_connections` | Gauge | `state` (`idle` / `used`) | DB connection pool size |
| `auth_requests_total` | Counter | `method`, `path` | Auth request count by endpoint |
| `auth_latency_seconds` | Histogram | `method`, `path` | Auth request latency (custom buckets 1ms–5s) |
| `auth_errors_total` | Counter | `error_type`, `status` | Auth errors by type and HTTP status |
| `token_refresh_total` | Counter | `status` | Token refresh events |

### Application Metrics (from Python Sidecar)

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `python_requests_total` | Counter | `method`, `endpoint`, `status` | Total Python requests |
| `python_request_duration_seconds` | Histogram | `method`, `endpoint` | Python request latency (seconds) |

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

Alert rules are defined in \`compose/monitoring/alerts.yml\` and loaded by Prometheus via `rule_files` in `prometheus.yml`.

> **Note:** Alert rules are commented out by default (opt-in). Uncomment the `groups` block in \`compose/monitoring/alerts.yml\` to enable alerting.

Example rules included:

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

## Metrics Architecture

Metrics are collected via the `metrics` + `metrics-exporter-prometheus` crates.

### Security

In production, the `/metrics` endpoint is restricted to the internal Docker network (`172.20.0.0/16`) via nginx. External access returns 403. Prometheus scrapes from within the Docker network, so this works transparently.

### How it works

1. **Tower middleware** (\`backend/api/src/metrics.rs::track_metrics\`) records every request:
   - `http_requests_total` counter with `method`, `route`, `status` labels
   - `http_request_duration_seconds` histogram with custom buckets
2. **Auth middleware** (\`backend/auth/src/metrics.rs::track_auth_metrics\`) records auth-specific metrics on auth routes:
   - `auth_requests_total` counter with `method`, `path` labels
   - `auth_latency_seconds` histogram with custom buckets (1ms–5s)
   - `auth_errors_total` counter with `error_type`, `status` labels on 4xx/5xx
   - `token_refresh_total` counter with `status` label
3. **Background task** updates `db_pool_connections` gauge every 15s
4. **`/metrics` endpoint** renders all metrics in Prometheus text format
5. **`/metrics/python` endpoint** proxies Python sidecar metrics over the Unix socket
6. **Prometheus** scrapes both endpoints every 5-15s
7. **Grafana** displays the data via 6 pre-configured dashboards

### Route label normalization

To prevent cardinality explosion, the middleware normalizes paths:
- `/health`, `/health/db`, `/health/python` → exact match
- Everything else → `unknown`

Add new routes to `normalize_route()` in \`backend/api/src/metrics.rs\` to track them correctly.

## Troubleshooting

### Grafana Can't Connect to Prometheus

1. Check Prometheus is running: `curl http://localhost:9090/-/healthy`
2. Verify datasource in Grafana:
   - Go to Configuration → Data Sources → Prometheus
   - Check URL is `http://prometheus:9090` (from Docker network)
   - Click "Save & Test"

### Metrics Not Showing in Prometheus

1. Check Rust backend exposes `/metrics`: `curl http://localhost:8001/metrics`
2. Verify Prometheus scrape config: \`compose/monitoring/prometheus.yml\`
3. Check Prometheus targets: http://localhost:9090/targets

### Dashboard Panels Empty

1. Verify time range in Grafana (top right)
2. Check if services are running: `make status`
3. Query Prometheus directly: http://localhost:9090/graph

## Related Docs

- [Previous: CI.md](./CI.md) - CI/CD pipeline
- [Next: INFRASTRUCTURE.md](./INFRASTRUCTURE.md) - Infrastructure setup
- [All Docs](./INDEX.md) - Full documentation index
