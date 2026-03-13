//! Interactions API: list, create, update (lifecycle: New → Assigned → In Progress → Resolved → Closed)

use axum::{
    extract::{Query, Path, State},
    routing::{get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{error::AppError, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_interactions).post(create_interaction))
        .route("/:id", get(get_interaction).patch(update_interaction))
}

#[derive(Deserialize)]
pub struct CreateInteractionRequest {
    pub contact_id: Uuid,
    pub channel: String,
    pub subject: String,
    pub priority: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateInteractionRequest {
    pub status: Option<String>,
    pub assigned_to: Option<Uuid>,
}

#[derive(Serialize)]
pub struct InteractionResponse {
    pub id: Uuid,
    pub contact_id: Uuid,
    pub channel: String,
    pub subject: String,
    pub status: String,
    pub priority: Option<String>,
    pub assigned_to: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

async fn list_interactions(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<InteractionResponse>>, AppError> {
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let offset = q.offset.unwrap_or(0).max(0);
    let rows = sqlx::query_as!(
        InteractionRow,
        "SELECT id, contact_id, channel, subject, status, priority, assigned_to, created_at FROM interactions ORDER BY created_at DESC LIMIT $1 OFFSET $2",
        limit,
        offset
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn create_interaction(
    State(state): State<AppState>,
    Json(req): Json<CreateInteractionRequest>,
) -> Result<Json<InteractionResponse>, AppError> {
    let id = Uuid::new_v4();
    let priority = req.priority.as_deref().unwrap_or("medium");
    
    // Auto-assignment
    let engine = crate::assignment_engine::AssignmentEngine::new(state.pool.clone());
    let assigned_to = engine.assign_next_agent("interaction").await.ok();

    sqlx::query!(
        r#"INSERT INTO interactions (id, contact_id, channel, subject, status, priority, assigned_to, created_at)
           VALUES ($1, $2, $3, $4, 'new', $5, $6, NOW())"#,
        id,
        req.contact_id,
        req.channel,
        req.subject,
        priority,
        assigned_to
    )
    .execute(&state.pool)
    .await?;
    let row = sqlx::query_as!(InteractionRow, "SELECT * FROM interactions WHERE id = $1", id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row.into()))
}

async fn get_interaction(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<InteractionResponse>, AppError> {
    let row = sqlx::query_as!(InteractionRow, "SELECT * FROM interactions WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Interaction not found".into()))?;
    Ok(Json(row.into()))
}

async fn update_interaction(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateInteractionRequest>,
) -> Result<Json<InteractionResponse>, AppError> {
    if let Some(status) = &req.status {
        sqlx::query!("UPDATE interactions SET status = $1 WHERE id = $2", status, id)
            .execute(&state.pool)
            .await?;
    }
    if let Some(assigned_to) = req.assigned_to {
        sqlx::query!("UPDATE interactions SET assigned_to = $1 WHERE id = $2", assigned_to, id)
            .execute(&state.pool)
            .await?;
    }
    let row = sqlx::query_as!(InteractionRow, "SELECT * FROM interactions WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Interaction not found".into()))?;
    Ok(Json(row.into()))
}

struct InteractionRow {
    id: Uuid,
    contact_id: Uuid,
    channel: String,
    subject: String,
    status: String,
    priority: Option<String>,
    assigned_to: Option<Uuid>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl From<InteractionRow> for InteractionResponse {
    fn from(r: InteractionRow) -> Self {
        Self {
            id: r.id,
            contact_id: r.contact_id,
            channel: r.channel,
            subject: r.subject,
            status: r.status,
            priority: r.priority,
            assigned_to: r.assigned_to,
            created_at: r.created_at,
        }
    }
}
