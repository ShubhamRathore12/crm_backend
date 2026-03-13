//! Bulk Uploads API: list, get, create (kick off)

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{error::AppError, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_bulk_uploads).post(create_bulk_upload))
        .route("/:id", get(get_bulk_upload))
}

#[derive(Deserialize)]
pub struct CreateBulkUploadRequest {
    pub file_name: String,
    pub entity_type: String, // 'contact', 'lead'
    pub created_by: Option<Uuid>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct BulkUploadResponse {
    pub id: Uuid,
    pub file_name: String,
    pub entity_type: String,
    pub status: String,
    pub total_rows: Option<i32>,
    pub processed_rows: Option<i32>,
    pub failed_rows: Option<i32>,
    pub error_log: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

async fn list_bulk_uploads(State(state): State<AppState>) -> Result<Json<Vec<BulkUploadResponse>>, AppError> {
    let rows = sqlx::query_as!(
        BulkUploadResponse,
        "SELECT * FROM bulk_uploads ORDER BY created_at DESC"
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn create_bulk_upload(
    State(state): State<AppState>,
    Json(req): Json<CreateBulkUploadRequest>,
) -> Result<Json<BulkUploadResponse>, AppError> {
    let id = Uuid::new_v4();
    sqlx::query!(
        r#"INSERT INTO bulk_uploads (id, file_name, entity_type, status, created_by)
           VALUES ($1, $2, $3, 'pending', $4)"#,
        id,
        req.file_name,
        req.entity_type,
        req.created_by
    )
    .execute(&state.pool)
    .await?;

    let row = sqlx::query_as!(BulkUploadResponse, "SELECT * FROM bulk_uploads WHERE id = $1", id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row))
}

async fn get_bulk_upload(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<BulkUploadResponse>, AppError> {
    let row = sqlx::query_as!(BulkUploadResponse, "SELECT * FROM bulk_uploads WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Bulk upload not found".into()))?;
    Ok(Json(row))
}
