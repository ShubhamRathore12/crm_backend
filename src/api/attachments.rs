//! Attachments API: upload, download, list, delete

use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{error::AppError, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_attachments))
        .route("/upload", post(upload_attachment))
        .route("/:id", get(get_attachment).delete(delete_attachment))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AttachmentResponse {
    pub id: Uuid,
    pub file_name: String,
    pub file_type: String,
    pub file_size: i32,
    pub file_path: String,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub uploaded_by: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

async fn list_attachments(State(state): State<AppState>) -> Result<Json<Vec<AttachmentResponse>>, AppError> {
    let rows = sqlx::query_as!(
        AttachmentResponse,
        "SELECT * FROM attachments ORDER BY created_at DESC"
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn upload_attachment(
    State(_state): State<AppState>,
    Json(_req): Json<serde_json::Value>, // Placeholder for multipart
) -> Result<Json<AttachmentResponse>, AppError> {
    // TODO: Implement multipart upload and storage
    Err(AppError::NotImplemented("Attachment upload is a placeholder".into()))
}

async fn get_attachment(
    State(_state): State<AppState>,
    Path(_id): Path<Uuid>,
) -> Result<Json<AttachmentResponse>, AppError> {
    Err(AppError::NotImplemented("Attachment retrieval is a placeholder".into()))
}

async fn delete_attachment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let r = sqlx::query!("DELETE FROM attachments WHERE id = $1", id)
        .execute(&state.pool)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("Attachment not found".into()));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}
