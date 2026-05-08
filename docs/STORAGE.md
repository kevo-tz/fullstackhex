# Storage

S3-compatible object storage via \`storage/\`. Backed by RustFS (local) or any S3-compatible endpoint (MinIO, AWS S3, Cloudflare R2).

## Configuration

| Env Var | Default | Description |
|---------|---------|-------------|
| `STORAGE_ENDPOINT` | — | S3 endpoint. Example: `http://localhost:9000` |
| `STORAGE_PUBLIC_ENDPOINT` | — | Public endpoint for presigned URLs. |
| `STORAGE_ACCESS_KEY` | — | S3 access key. |
| `STORAGE_SECRET_KEY` | — | S3 secret key. |
| `STORAGE_BUCKET` | — | Bucket name. |
| `STORAGE_REGION` | `us-east-1` | Bucket region. |
| `STORAGE_AUTO_CREATE_BUCKET` | `false` | Create bucket on startup if missing. |

Storage is optional — when `STORAGE_ENDPOINT` is unset, storage endpoints return 503.

## RustFS (Local Development)

For local development without an S3 server, set `STORAGE_ENDPOINT=rustfs` to use the built-in RustFS backend. Files are stored on local disk.

## Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/storage/upload` | Required | Upload a file. |
| GET | `/storage/download/{key}` | Required | Download a file. |
| GET | `/storage/presign` | Required | Generate a presigned URL. |
| DELETE | `/storage/delete/{key}` | Required | Delete a file. |
| GET | `/storage/list` | Required | List objects with optional prefix. |

## Presigned URLs

Generate time-limited download URLs without exposing storage credentials. Pass `method` (`GET` default) and `expiry_secs` (default 3600) in the request body:

```json
{
  "key": "path/to/file.txt",
  "method": "GET",
  "expiry_secs": 3600
}
```

Files are stored under `users/{user_id}/` to isolate per-user data.

## Streaming

Upload and download stream data directly — no buffering of entire files in memory. This prevents OOM with large files.

## Multipart Upload

For files larger than 5 MB, use multipart upload:
- `POST /storage/multipart/initiate` — start multipart upload.
- `POST /storage/multipart/part` — upload a part.
- `POST /storage/multipart/complete` — finalize and assemble.

## Size Limits

- Maximum single upload: 100 MB
- Download size limit: 100 MB (prevents OOM from unbounded S3 responses)
- Multipart parts: 5 MB minimum, 5 GB maximum per part

## URL Safety

Object keys and prefixes are URL-encoded via `url::Url` to prevent panics from invalid characters and query-parameter injection in list operations.
