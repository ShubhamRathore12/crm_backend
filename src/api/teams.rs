//! Teams (Groups) API: creation and member management

use axum::{
    extract::{Path, State},
    routing::{get, post, delete},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{error::AppError, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_teams).post(create_team))
        .route("/:id", get(get_team).delete(delete_team))
        .route("/:id/members", get(list_members).post(add_member))
        .route("/:id/members/:user_id", delete(remove_member))
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TeamResponse {
    pub id: Uuid,
    pub name: String,
    pub manager_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct CreateTeamRequest {
    pub name: String,
    pub manager_id: Option<Uuid>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct TeamMember {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub role: String,
}

async fn list_teams(
    State(state): State<AppState>,
) -> Result<Json<Vec<TeamResponse>>, AppError> {
    let teams = sqlx::query_as!(
        TeamResponse,
        "SELECT id, name, manager_id, created_at FROM teams ORDER BY created_at DESC"
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(teams))
}

async fn create_team(
    State(state): State<AppState>,
    Json(req): Json<CreateTeamRequest>,
) -> Result<Json<TeamResponse>, AppError> {
    let id = Uuid::new_v4();
    let team = sqlx::query_as!(
        TeamResponse,
        r#"INSERT INTO teams (id, name, manager_id, created_at)
           VALUES ($1, $2, $3, NOW())
           RETURNING id, name, manager_id, created_at"#,
        id,
        req.name,
        req.manager_id
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(team))
}

async fn get_team(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TeamResponse>, AppError> {
    let team = sqlx::query_as!(
        TeamResponse,
        "SELECT id, name, manager_id, created_at FROM teams WHERE id = $1",
        id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Team".into()))?;
    Ok(Json(team))
}

async fn delete_team(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Check for members first? Or just clear team_id for members
    sqlx::query!("UPDATE users SET team_id = NULL WHERE team_id = $1", id)
        .execute(&state.pool)
        .await?;

    sqlx::query!("DELETE FROM teams WHERE id = $1", id)
        .execute(&state.pool)
        .await?;
    
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn list_members(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<TeamMember>>, AppError> {
    let members = sqlx::query_as!(
        TeamMember,
        "SELECT id, name, email, role FROM users WHERE team_id = $1",
        id
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(members))
}

#[derive(Deserialize)]
pub struct AddMemberRequest {
    pub user_id: Uuid,
}

async fn add_member(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<AddMemberRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    sqlx::query!("UPDATE users SET team_id = $1 WHERE id = $2", id, req.user_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn remove_member(
    State(state): State<AppState>,
    Path((id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, AppError> {
    sqlx::query!("UPDATE users SET team_id = NULL WHERE id = $1", user_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
