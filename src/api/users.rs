//! User Management API: List, Create, Update, Delete

use axum::{
    extract::{Path, State},
    routing::{get, post, patch, delete},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{error::AppError, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_users))
        .route("/", post(create_user))
        .route("/:id", patch(update_user))
        .route("/:id", delete(delete_user))
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub role: String,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub name: String,
    pub email: String,
    pub role: String,
}

#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub name: Option<String>,
    pub email: Option<String>,
    pub role: Option<String>,
    pub status: Option<String>,
}

async fn list_users(
    State(state): State<AppState>,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    let users = sqlx::query_as!(
        UserResponse,
        r#"
        SELECT id, name, email, role, status, created_at
        FROM users
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(users))
}

async fn create_user(
    State(state): State<AppState>,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    // Default password for new users - in a real app, we'd send an invite email
    let password_hash = bcrypt::hash("Welcome123!", bcrypt::DEFAULT_COST).map_err(|e| AppError::Internal(e.to_string()))?;
    
    let user = sqlx::query_as!(
        UserResponse,
        r#"
        INSERT INTO users (name, email, password_hash, role, status)
        VALUES ($1, $2, $3, $4, 'active')
        RETURNING id, name, email, role, status, created_at
        "#,
        req.name,
        req.email,
        password_hash,
        req.role
    )
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(user))
}

async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    let user = sqlx::query_as!(
        UserResponse,
        r#"
        UPDATE users
        SET 
            name = COALESCE($1, name),
            email = COALESCE($2, email),
            role = COALESCE($3, role),
            status = COALESCE($4, status)
        WHERE id = $5
        RETURNING id, name, email, role, status, created_at
        "#,
        req.name,
        req.email,
        req.role,
        req.status,
        id
    )
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(user))
}

async fn delete_user(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Soft delete: set status to 'deleted'
    sqlx::query!(
        "UPDATE users SET status = 'deleted' WHERE id = $1",
        id
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}
