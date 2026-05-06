# Examples

Copy-paste patterns for extending FullStackHex. Each example is self-contained.

## Add a new backend API route

### 1. Define the handler (`backend/crates/api/src/routes/hello.rs`)

```rust
use axum::{extract::State, Json};
use serde::Serialize;

#[derive(Serialize)]
pub struct HelloResponse {
    pub message: String,
}

pub async fn hello() -> Json<HelloResponse> {
    Json(HelloResponse {
        message: "Hello from FullStackHex!".to_string(),
    })
}
```

### 2. Register the route (`backend/crates/api/src/lib.rs`)

In `router_with_state()` after existing routes:

```rust
mod routes;

// Inside router_with_state():
let router = router.route("/hello", axum::routing::get(routes::hello::hello));
```

### 3. Test it

```bash
curl http://localhost:8001/hello
# {"message":"Hello from FullStackHex!"}
```

---

## Add a new frontend page

### 1. Create the page (`frontend/src/pages/about.astro`)

```astro
---
import Layout from "../components/Layout.astro";
---

<Layout title="About — FullStackHex">
  <header style="margin-bottom:2rem;text-align:center">
    <h1 style="font-size:1.5rem;font-weight:700;color:#f8fafc">About</h1>
  </header>
  <div class="card" style="max-width:400px;margin:0 auto">
    <p style="color:#94a3b8;line-height:1.6">
      FullStackHex is a Rust + Astro + Python production template.
    </p>
  </div>
</Layout>
```

### 2. Visit the page

```bash
cd frontend && bun run dev
# Open http://localhost:4321/about
```

---

## Add a database migration

### 1. Create the migration

```bash
cd backend
cargo sqlx migrate add create_notes_table
```

### 2. Write the SQL (`backend/migrations/*_create_notes_table.sql`)

```sql
CREATE TABLE IF NOT EXISTS notes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_notes_user_id ON notes(user_id);
```

### 3. Run the migration

```bash
make migrate
```

### 4. Query from Rust

```rust
use sqlx::PgPool;

pub async fn list_notes(pool: &PgPool, user_id: &str) -> Result<Vec<Note>, sqlx::Error> {
    sqlx::query_as!(
        Note,
        "SELECT id, user_id, title, body, created_at, updated_at FROM notes WHERE user_id = $1",
        user_id
    )
    .fetch_all(pool)
    .await
}
```

---

## Add a storage operation

### 1. Upload with the Rust client

```rust
use storage::StorageConfig;
use storage::client;

async fn upload_file(config: &StorageConfig, key: &str, data: Vec<u8>) {
    let client = reqwest::Client::new();
    client::upload(&client, config, key, data, "application/octet-stream")
        .await
        .expect("upload failed");
}
```

### 2. Use multipart for files > 5MB

```rust
use storage::client;

async fn multipart_upload(config: &StorageConfig, key: &str, chunks: Vec<Vec<u8>>) {
    let client = reqwest::Client::new();

    // Initiate
    let upload = client::create_multipart_upload(&client, config, key, "application/octet-stream")
        .await
        .unwrap();

    // Upload parts
    let mut parts = Vec::new();
    for (i, chunk) in chunks.iter().enumerate() {
        let part = client::upload_part(
            &client, config, key, &upload.upload_id,
            (i + 1) as u32, chunk.clone(),
        )
        .await
        .unwrap();
        parts.push(part);
    }

    // Complete
    client::complete_multipart_upload(&client, config, key, &upload.upload_id, &parts)
        .await
        .unwrap();
}
```

### 3. Upload via HTTP (authenticated)

```bash
TOKEN="$(curl -s http://localhost:8001/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"test@example.com","password":"test-pass"}' | jq -r .access_token)"

curl -X PUT http://localhost:8001/storage/my-file.txt \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: text/plain" \
  -d "Hello, storage!"
```

---

## Add a Redis cache entry

```rust
use cache::RedisClient;

async fn cache_user(client: &RedisClient, user_id: &str, data: &User) {
    // Store with 5-minute TTL
    client.cache_set("users", user_id, data, std::time::Duration::from_secs(300))
        .await
        .expect("cache set failed");

    // Retrieve
    let cached: Option<User> = client.cache_get("users", user_id)
        .await
        .expect("cache get failed");
}
```

---

## Add a Grafana dashboard panel

Add to `monitoring/grafana/dashboards/overview.json`:

```json
{
  "title": "Hello Endpoint Rate",
  "type": "timeseries",
  "datasource": { "type": "prometheus", "uid": "${DS_PROMETHEUS}" },
  "targets": [{
    "expr": "sum(rate(http_requests_total{route=\"/hello\"}[5m]))",
    "legendFormat": "hello RPS"
  }],
  "fieldConfig": { "defaults": { "unit": "reqps" } },
  "gridPos": { "h": 8, "w": 12, "x": 0, "y": 0 }
}
```

---

## Call the Python sidecar from Rust

```rust
use python_sidecar::PythonSidecar;
use std::time::Duration;

async fn sidecar_example() {
    let sidecar = PythonSidecar::new(
        "/tmp/fullstackhex-python.sock",
        Duration::from_secs(5),
        3,
    );

    // Health check
    match sidecar.health().await {
        Ok(json) => println!("Sidecar OK: {json}"),
        Err(e) => eprintln!("Sidecar error: {e}"),
    }

    // Custom endpoint
    let data = sidecar.get("/predict?text=hello").await.unwrap();
    println!("Prediction: {data}");
}
```

---

## Write an e2e test

### Bun test (`e2e/my-feature.test.ts`)

```ts
const BACKEND = process.env.VITE_RUST_BACKEND_URL || "http://localhost:8001";

test("GET /hello returns 200", async () => {
  const res = await fetch(`${BACKEND}/hello`);
  expect(res.status).toBe(200);
  const data = await res.json();
  expect(data.message).toContain("Hello");
});
```

### Shell test (`tests/my-feature.sh`)

```bash
#!/bin/bash
set -euo pipefail
BACKEND_URL="${BACKEND_URL:-http://localhost:8001}"

resp=$(curl -sk "$BACKEND_URL/hello")
echo "$resp" | grep -q "Hello" && echo "PASS" || echo "FAIL"
```

---

## Add a CI check

In `.github/workflows/ci.yml`:

```yaml
  my-check:
    name: my-check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      - name: Run my check
        run: ./scripts/my-check.sh
```
