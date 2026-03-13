//! Field Definitions API: list, create, update, delete

use axum::{
    extract::{Path, State},
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{error::AppError, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_fields).post(create_field))
        .route("/:id", patch(update_field).delete(delete_field))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct FieldDefinitionResponse {
    pub id: Uuid,
    pub entity_type: String,
    pub field_name: String,
    pub label: String,
    pub field_type: String,
    pub options: Option<serde_json::Value>,
    pub is_required: Option<bool>,
    pub is_system: Option<bool>,
    pub display_order: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct CreateFieldRequest {
    pub entity_type: String,
    pub field_name: String,
    pub label: String,
    pub field_type: String,
    pub options: Option<serde_json::Value>,
    pub is_required: Option<bool>,
    pub display_order: Option<i32>,
}

async fn list_fields(State(state): State<AppState>) -> Result<Json<Vec<FieldDefinitionResponse>>, AppError> {
    let rows = sqlx::query_as!(
        FieldDefinitionResponse,
        "SELECT * FROM field_definitions ORDER BY entity_type, display_order"
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn create_field(
    State(state): State<AppState>,
    Json(req): Json<CreateFieldRequest>,
) -> Result<Json<FieldDefinitionResponse>, AppError> {
    let id = Uuid::new_v4();
    sqlx::query!(
        r#"INSERT INTO field_definitions (id, entity_type, field_name, label, field_type, options, is_required, display_order)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
        id,
        req.entity_type,
        req.field_name,
        req.label,
        req.field_type,
        req.options,
        req.is_required.unwrap_or(false),
        req.display_order.unwrap_or(0)
    )
    .execute(&state.pool)
    .await?;

    let row = sqlx::query_as!(FieldDefinitionResponse, "SELECT * FROM field_definitions WHERE id = $1", id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row))
}

async fn update_field(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<serde_json::Value>, // Simplified
) -> Result<Json<FieldDefinitionResponse>, AppError> {
    let _ = (state, id, req);
    Err(AppError::NotImplemented("Field update is a placeholder".into()))
}

async fn delete_field(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    sqlx::query!("DELETE FROM field_definitions WHERE id = $1 AND is_system = false", id)
        .execute(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
