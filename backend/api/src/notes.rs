//! Note CRUD routes.
//!
//! Implements a complete CRUD lifecycle with Postgres-backed storage,
//! user-scoped authorization (user_id), and standard REST patterns.

use crate::AppState;
use crate::DbStatus;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use domain::CreateNoteInput;
use domain::Note;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

/// Pagination query parameters.
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    20
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 20,
        }
    }
}

/// Returns `None` if the database is not configured (caller returns 503).
/// Returns `Some(pool)` if the database is connected.
fn pool_from_state(state: &AppState) -> Option<&sqlx::PgPool> {
    match &state.db {
        DbStatus::Connected(pool) => Some(pool),
        _ => None,
    }
}

/// Paginated response wrapper.
#[derive(serde::Serialize)]
pub struct PaginatedNotes {
    pub items: Vec<Note>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

/// List notes for the authenticated user.
pub async fn list_notes(
    auth: auth::middleware::AuthUser,
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> impl IntoResponse {
    let pool = match pool_from_state(&state) {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error":"database not configured"})),
            )
                .into_response();
        }
    };

    let page = params.page.max(1);
    let limit = params.per_page.clamp(1, 100);
    let offset = page.saturating_sub(1).saturating_mul(limit);

    match sqlx::query_as::<_, (String, String, String, String, i64)>(
        r#"
        SELECT id::text, title, created_at::text, updated_at::text,
               COUNT(*) OVER()::bigint AS total_count
        FROM notes
        WHERE user_id = $1::uuid
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(&auth.user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => {
            let total = rows.first().map(|r| r.4).unwrap_or(0);
            let items: Vec<Note> = rows
                .into_iter()
                .map(|r| Note {
                    id: r.0,
                    user_id: String::new(),
                    title: r.1,
                    body: String::new(),
                    created_at: r.2,
                    updated_at: r.3,
                })
                .collect();
            (
                StatusCode::OK,
                Json(PaginatedNotes {
                    items,
                    total,
                    page,
                    per_page: limit,
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to list notes");
            ::metrics::counter!("notes_query_errors_total").increment(1);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error":"failed to list notes"})),
            )
                .into_response()
        }
    }
}

/// Create a new note.
pub async fn create_note(
    auth: auth::middleware::AuthUser,
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateNoteInput>,
) -> impl IntoResponse {
    let pool = match pool_from_state(&state) {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error":"database not configured"})),
            )
                .into_response();
        }
    };

    if input.title.trim().is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error":"title is required"})),
        )
            .into_response();
    }
    if input.title.len() > 255 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error":"title must be 255 characters or fewer"})),
        )
            .into_response();
    }
    if input.body.len() > 100_000 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error":"body must be 100KB or fewer"})),
        )
            .into_response();
    }

    match sqlx::query_as::<_, (String, String, String, String, String, String)>(
        r#"
        INSERT INTO notes (user_id, title, body)
        VALUES ($1::uuid, $2, $3)
        RETURNING id::text, user_id::text, title, body, created_at::text, updated_at::text
        "#,
    )
    .bind(&auth.user_id)
    .bind(&input.title)
    .bind(&input.body)
    .fetch_one(pool)
    .await
    {
        Ok(r) => {
            let note = Note {
                id: r.0,
                user_id: r.1,
                title: r.2,
                body: r.3,
                created_at: r.4,
                updated_at: r.5,
            };
            ::metrics::counter!("notes_created_total").increment(1);
            (StatusCode::CREATED, Json(note)).into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to create note");
            ::metrics::counter!("notes_query_errors_total").increment(1);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error":"failed to create note"})),
            )
                .into_response()
        }
    }
}

/// Get a single note by ID.
pub async fn get_note(
    auth: auth::middleware::AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if Uuid::parse_str(&id).is_err() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"invalid note id"})),
        )
            .into_response();
    }
    let pool = match pool_from_state(&state) {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error":"database not configured"})),
            )
                .into_response();
        }
    };

    match sqlx::query_as::<_, (String, String, String, String, String, String)>(
        r#"
        SELECT id::text, user_id::text, title, body, created_at::text, updated_at::text
        FROM notes
        WHERE id = $1::uuid AND user_id = $2::uuid
        "#,
    )
    .bind(&id)
    .bind(&auth.user_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(r)) => {
            let note = Note {
                id: r.0,
                user_id: r.1,
                title: r.2,
                body: r.3,
                created_at: r.4,
                updated_at: r.5,
            };
            (StatusCode::OK, Json(note)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"note not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "failed to get note");
            ::metrics::counter!("notes_query_errors_total").increment(1);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error":"failed to get note"})),
            )
                .into_response()
        }
    }
}

/// Delete a note by ID.
pub async fn delete_note(
    auth: auth::middleware::AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    if Uuid::parse_str(&id).is_err() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":"invalid note id"})),
        )
            .into_response();
    }
    let pool = match pool_from_state(&state) {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error":"database not configured"})),
            )
                .into_response();
        }
    };

    match sqlx::query("DELETE FROM notes WHERE id = $1::uuid AND user_id = $2::uuid")
        .bind(&id)
        .bind(&auth.user_id)
        .execute(pool)
        .await
    {
        Ok(res) if res.rows_affected() > 0 => {
            ::metrics::counter!("notes_deleted_total").increment(1);
            (StatusCode::OK, "{\"status\":\"deleted\"}").into_response()
        }
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"note not found"})),
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "failed to delete note");
            ::metrics::counter!("notes_query_errors_total").increment(1);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error":"failed to delete note"})),
            )
                .into_response()
        }
    }
}
