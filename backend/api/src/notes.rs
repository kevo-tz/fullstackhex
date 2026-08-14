//! Note CRUD routes.
//!
//! Implements a complete CRUD lifecycle with Postgres-backed storage,
//! user-scoped authorization (user_id), and standard REST patterns.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use chrono::{DateTime, Utc};
use domain::error::ApiError;
use domain::{CreateNoteInput, Note, UpdateNoteInput};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

/// Pagination query parameters.
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    /// Opaque keyset cursor; omit for the first page.
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default = "default_per_page")]
    pub per_page: i64,
}

fn default_per_page() -> i64 {
    20
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            cursor: None,
            per_page: 20,
        }
    }
}

/// Paginated response wrapper for keyset pagination.
#[derive(serde::Serialize)]
pub struct PaginatedNotes {
    pub items: Vec<Note>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub per_page: i64,
}

/// Opaque keyset cursor: base64url (no padding) of "{created_at_epoch_micros}:{id}".
fn encode_cursor(created_at: DateTime<Utc>, id: &str) -> String {
    let raw = format!("{}:{}", created_at.timestamp_micros(), id);
    URL_SAFE_NO_PAD.encode(raw.as_bytes())
}

/// Decode a keyset cursor; garbage input yields None (treated as no cursor).
fn decode_cursor(cursor: &str) -> Option<(i64, String)> {
    let raw = String::from_utf8(URL_SAFE_NO_PAD.decode(cursor).ok()?).ok()?;
    let (micros, id) = raw.split_once(':')?;
    Some((micros.parse().ok()?, id.to_string()))
}

/// List notes for the authenticated user, keyset-paginated.
pub async fn list_notes(
    auth: auth::middleware::AuthUser,
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, ApiError> {
    let pool = state.db_pool()?;

    let limit = params.per_page.clamp(1, 100);

    // Keyset cursor decodes to (created_at, id); garbage cursors fall back to
    // the first page instead of erroring.
    let cursor: Option<(DateTime<Utc>, String)> = params
        .cursor
        .as_deref()
        .and_then(decode_cursor)
        .and_then(|(micros, id)| {
            DateTime::from_timestamp_micros(micros).zip(Uuid::parse_str(&id).ok().map(|_| id))
        });

    // List UI only renders id/title/created_at — skip the body column to avoid
    // transferring full note content. Note still serializes body:"" for the
    // shared `Note` contract; the detail endpoint returns the real body.
    let mut sql = String::from(
        "SELECT id::text, title, created_at, updated_at \
         FROM notes WHERE user_id = $1::uuid",
    );
    let limit_param = if cursor.is_some() {
        sql.push_str(" AND (created_at, id) < ($2::timestamptz, $3::uuid)");
        "$4"
    } else {
        "$2"
    };
    sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ");
    sql.push_str(limit_param);

    let mut query = sqlx::query_as::<_, (String, String, DateTime<Utc>, DateTime<Utc>)>(&sql)
        .bind(&auth.user_id);
    if let Some((ts, id)) = cursor {
        query = query.bind(ts).bind(id);
    }
    let rows = query
        .bind(limit + 1)
        .fetch_all(pool)
        .await
        .map_err(|e| super::db_err(e, "failed to list notes"))?;

    // Fetch one extra row to detect another page, then drop it.
    let has_more = rows.len() > limit as usize;
    let items: Vec<Note> = rows
        .into_iter()
        .take(limit as usize)
        .map(|r| Note {
            id: r.0,
            user_id: auth.user_id.clone(),
            title: r.1,
            body: String::new(),
            created_at: r.2,
            updated_at: r.3,
        })
        .collect();
    let next_cursor = if has_more {
        items.last().map(|n| encode_cursor(n.created_at, &n.id))
    } else {
        None
    };
    Ok((
        StatusCode::OK,
        Json(PaginatedNotes {
            items,
            next_cursor,
            has_more,
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

    let r = sqlx::query_as::<_, (String, String, String, String, DateTime<Utc>, DateTime<Utc>)>(
        r#"
        INSERT INTO notes (user_id, title, body)
        VALUES ($1::uuid, $2, $3)
        RETURNING id::text, user_id::text, title, body, created_at, updated_at
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

    let r = sqlx::query_as::<_, (String, String, String, String, DateTime<Utc>, DateTime<Utc>)>(
        r#"
        SELECT id::text, user_id::text, title, body, created_at, updated_at
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

    let r = sqlx::query_as::<_, (String, String, String, String, DateTime<Utc>, DateTime<Utc>)>(
        r#"
        UPDATE notes
        SET title = $1, body = $2, updated_at = NOW()
        WHERE id = $3::uuid AND user_id = $4::uuid
        RETURNING id::text, user_id::text, title, body, created_at, updated_at
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
        Ok((StatusCode::NO_CONTENT, ""))
    } else {
        Err(ApiError::NotFound("note not found".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_roundtrip() {
        let ts = DateTime::parse_from_rfc3339("2026-05-27T10:00:00.123456Z")
            .unwrap()
            .with_timezone(&Utc);
        let id = "550e8400-e29b-41d4-a716-446655440000";
        let cursor = encode_cursor(ts, id);
        let decoded = decode_cursor(&cursor).unwrap();
        assert_eq!(decoded.0, ts.timestamp_micros());
        assert_eq!(decoded.1, id);
    }

    #[test]
    fn cursor_rejects_garbage() {
        assert!(decode_cursor("not-base64!!").is_none());
        assert!(decode_cursor("aGVsbG8").is_none());
        assert!(decode_cursor("").is_none());
    }
}
