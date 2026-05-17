//! Note CRUD routes.
//!
//! Implements a complete CRUD lifecycle with Postgres-backed storage,
//! user-scoped authorization (user_id), and standard REST patterns.

use crate::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use domain::error::ApiError;
use domain::{CreateNoteInput, Note, UpdateNoteInput};
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
) -> Result<impl IntoResponse, ApiError> {
    let pool = state.db_pool()?;

    let page = params.page.max(1);
    let limit = params.per_page.clamp(1, 100);
    let offset = page.saturating_sub(1).saturating_mul(limit);

    let rows = sqlx::query_as::<_, (String, String, String, String, String, i64)>(
        r#"
        SELECT id::text, title, body, created_at::text, updated_at::text,
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
    .map_err(|e| super::db_err(e, "failed to list notes"))?;

    let total = rows.first().map(|r| r.5).unwrap_or(0);
    let items: Vec<Note> = rows
        .into_iter()
        .map(|r| Note {
            id: r.0,
            user_id: auth.user_id.clone(),
            title: r.1,
            body: r.2,
            created_at: r.3,
            updated_at: r.4,
        })
        .collect();
    Ok((
        StatusCode::OK,
        Json(PaginatedNotes {
            items,
            total,
            page,
            per_page: limit,
        }),
    ))
}

/// Create a new note.
pub async fn create_note(
    auth: auth::middleware::AuthUser,
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateNoteInput>,
) -> Result<impl IntoResponse, ApiError> {
    let pool = state.db_pool()?;

    if input.title.trim().is_empty() {
        return Err(ApiError::ValidationError("title is required".into()));
    }
    if input.title.len() > 255 {
        return Err(ApiError::ValidationError(
            "title must be 255 characters or fewer".into(),
        ));
    }
    if input.body.len() > 100_000 {
        return Err(ApiError::ValidationError(
            "body must be 100KB or fewer".into(),
        ));
    }

    let r = sqlx::query_as::<_, (String, String, String, String, String, String)>(
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
    .map_err(|e| super::db_err(e, "failed to create note"))?;

    let note = Note {
        id: r.0,
        user_id: r.1,
        title: r.2,
        body: r.3,
        created_at: r.4,
        updated_at: r.5,
    };
    ::metrics::counter!("notes_created_total").increment(1);
    Ok((StatusCode::CREATED, Json(note)))
}

/// Get a single note by ID.
pub async fn get_note(
    auth: auth::middleware::AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ApiError> {
    let _ =
        Uuid::parse_str(&id).map_err(|_| ApiError::ValidationError("invalid note id".into()))?;
    let pool = state.db_pool()?;

    let r = sqlx::query_as::<_, (String, String, String, String, String, String)>(
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
    .map_err(|e| super::db_err(e, "failed to get note"))?
    .ok_or_else(|| ApiError::NotFound("note not found".into()))?;

    let note = Note {
        id: r.0,
        user_id: r.1,
        title: r.2,
        body: r.3,
        created_at: r.4,
        updated_at: r.5,
    };
    Ok((StatusCode::OK, Json(note)))
}

/// Update an existing note (full replacement of title and body).
pub async fn update_note(
    auth: auth::middleware::AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(input): Json<UpdateNoteInput>,
) -> Result<impl IntoResponse, ApiError> {
    let _ =
        Uuid::parse_str(&id).map_err(|_| ApiError::ValidationError("invalid note id".into()))?;
    let pool = state.db_pool()?;

    if input.title.trim().is_empty() {
        return Err(ApiError::ValidationError("title is required".into()));
    }
    if input.title.len() > 255 {
        return Err(ApiError::ValidationError(
            "title must be 255 characters or fewer".into(),
        ));
    }
    if input.body.len() > 100_000 {
        return Err(ApiError::ValidationError(
            "body must be 100KB or fewer".into(),
        ));
    }

    let r = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        r#"
        UPDATE notes
        SET title = $1, body = $2, updated_at = NOW()
        WHERE id = $3::uuid AND user_id = $4::uuid
        RETURNING id::text, user_id::text, title, body, created_at::text, updated_at::text
        "#,
    )
    .bind(&input.title)
    .bind(&input.body)
    .bind(&id)
    .bind(&auth.user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| super::db_err(e, "failed to update note"))?
    .ok_or_else(|| ApiError::NotFound("note not found".into()))?;

    let note = Note {
        id: r.0,
        user_id: r.1,
        title: r.2,
        body: r.3,
        created_at: r.4,
        updated_at: r.5,
    };
    Ok((StatusCode::OK, Json(note)))
}

/// Delete a note by ID.
pub async fn delete_note(
    auth: auth::middleware::AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ApiError> {
    let _ =
        Uuid::parse_str(&id).map_err(|_| ApiError::ValidationError("invalid note id".into()))?;
    let pool = state.db_pool()?;

    let res = sqlx::query("DELETE FROM notes WHERE id = $1::uuid AND user_id = $2::uuid")
        .bind(&id)
        .bind(&auth.user_id)
        .execute(pool)
        .await
        .map_err(|e| super::db_err(e, "failed to delete note"))?;

    if res.rows_affected() > 0 {
        ::metrics::counter!("notes_deleted_total").increment(1);
        Ok((StatusCode::OK, "{\"status\":\"deleted\"}"))
    } else {
        Err(ApiError::NotFound("note not found".into()))
    }
}
