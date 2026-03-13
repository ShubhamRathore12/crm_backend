//! Leads API: list, create, get, update, delete

use axum::{
    extract::{Query, Path, State},
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{error::AppError, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_leads).post(create_lead))
        .route("/:id", get(get_lead).patch(update_lead).delete(delete_lead))
}

#[derive(Deserialize)]
pub struct CreateLeadRequest {
    pub contact_id: Uuid,
    pub source: String,
    pub product: Option<String>,
    pub campaign: Option<String>,
    pub custom_fields: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct UpdateLeadRequest {
    pub status: Option<String>,
    pub stage: Option<String>,
    pub assigned_to: Option<Uuid>,
}

#[derive(Serialize)]
pub struct LeadResponse {
    pub id: Uuid,
    pub contact_id: Uuid,
    pub source: String,
    pub status: String,
    pub stage: String,
    pub assigned_to: Option<Uuid>,
    pub product: Option<String>,
    pub campaign: Option<String>,
    pub custom_fields: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

async fn list_leads(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<LeadResponse>>, AppError> {
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let offset = q.offset.unwrap_or(0).max(0);
    
    let pool = state.db.read_pool().await;
    
    let rows = sqlx::query_as!(
        LeadRow,
        r#"SELECT id, contact_id, source, status, stage, assigned_to, product, campaign, custom_fields, created_at, updated_at
           FROM leads ORDER BY created_at DESC LIMIT $1 OFFSET $2"#,
        limit,
        offset
    )
    .fetch_all(pool)
    .await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn create_lead(
    State(state): State<AppState>,
    Json(req): Json<CreateLeadRequest>,
) -> Result<Json<LeadResponse>, AppError> {
    let id = Uuid::new_v4();
    let now = chrono::Utc::now();

    // Auto-assignment
    let engine = crate::assignment_engine::AssignmentEngine::new(state.pool.clone());
    let assigned_to = engine.assign_next_agent("lead").await.ok();

    let pool = state.db.write_pool();
    sqlx::query!(
        r#"INSERT INTO leads (id, contact_id, source, status, stage, product, campaign, custom_fields, assigned_to, created_at, updated_at)
           VALUES ($1, $2, $3, 'new', 'qualified', $4, $5, $6, $7, $8, $8)"#,
        id,
        req.contact_id,
        req.source,
        req.product,
        req.campaign,
        req.custom_fields.unwrap_or_else(|| serde_json::json!({})),
        assigned_to,
        now
    )
    .execute(pool)
    .await?;
    let row = sqlx::query_as!(LeadRow, "SELECT * FROM leads WHERE id = $1", id)
        .fetch_one(pool)
        .await?;
    Ok(Json(row.into()))
}

async fn get_lead(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<LeadResponse>, AppError> {
    let pool = state.db.read_pool().await;
    let row = sqlx::query_as!(LeadRow, "SELECT * FROM leads WHERE id = $1", id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Lead not found".into()))?;
    Ok(Json(row.into()))
}

async fn update_lead(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateLeadRequest>,
) -> Result<Json<LeadResponse>, AppError> {
    // Simplified: in production use dynamic UPDATE
    let _ = (state, id, req);
    let row = sqlx::query_as!(LeadRow, "SELECT * FROM leads WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Lead not found".into()))?;
    Ok(Json(row.into()))
}

async fn delete_lead(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let r = sqlx::query!("DELETE FROM leads WHERE id = $1", id)
        .execute(&state.pool)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("Lead not found".into()));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

struct LeadRow {
    id: Uuid,
    contact_id: Uuid,
    source: String,
    status: String,
    stage: String,
    assigned_to: Option<Uuid>,
    product: Option<String>,
    campaign: Option<String>,
    custom_fields: serde_json::Value,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<LeadRow> for LeadResponse {
    fn from(r: LeadRow) -> Self {
        Self {
            id: r.id,
            contact_id: r.contact_id,
            source: r.source,
            status: r.status,
            stage: r.stage,
            assigned_to: r.assigned_to,
            product: r.product,
            campaign: r.campaign,
            custom_fields: r.custom_fields,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}
