//! Opportunities API: list, create, get, update

use axum::{
    extract::{Path, State},
    routing::{get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{error::AppError, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_opportunities).post(create_opportunity))
        .route("/:id", get(get_opportunity).patch(update_opportunity))
}

#[derive(Deserialize)]
pub struct CreateOpportunityRequest {
    pub lead_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub value: Option<f64>,
    pub currency: Option<String>,
    pub stage: Option<String>,
    pub probability: Option<i32>,
    pub expected_closed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub assigned_to: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct UpdateOpportunityRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub value: Option<f64>,
    pub stage: Option<String>,
    pub probability: Option<i32>,
    pub expected_closed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub assigned_to: Option<Uuid>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct OpportunityResponse {
    pub id: Uuid,
    pub lead_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub value: Option<rust_decimal::Decimal>,
    pub currency: Option<String>,
    pub stage: String,
    pub probability: Option<i32>,
    pub expected_closed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub assigned_to: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

async fn list_opportunities(State(state): State<AppState>) -> Result<Json<Vec<OpportunityResponse>>, AppError> {
    let rows = sqlx::query_as!(
        OpportunityResponse,
        r#"SELECT id, lead_id, title, description, value, currency, stage, probability, expected_closed_at, assigned_to, created_at, updated_at 
           FROM opportunities ORDER BY created_at DESC"#
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn create_opportunity(
    State(state): State<AppState>,
    Json(req): Json<CreateOpportunityRequest>,
) -> Result<Json<OpportunityResponse>, AppError> {
    let id = Uuid::new_v4();
    let value = req.value.map(|v| rust_decimal::Decimal::from_f64_retain(v).unwrap_or_default());
    
    sqlx::query!(
        r#"INSERT INTO opportunities (id, lead_id, title, description, value, currency, stage, probability, expected_closed_at, assigned_to)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
        id,
        req.lead_id,
        req.title,
        req.description,
        value,
        req.currency.unwrap_or_else(|| "USD".into()),
        req.stage.unwrap_or_else(|| "discovery".into()),
        req.probability.unwrap_or(10),
        req.expected_closed_at,
        req.assigned_to
    )
    .execute(&state.pool)
    .await?;

    let row = sqlx::query_as!(OpportunityResponse, "SELECT * FROM opportunities WHERE id = $1", id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row))
}

async fn get_opportunity(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<OpportunityResponse>, AppError> {
    let row = sqlx::query_as!(OpportunityResponse, "SELECT * FROM opportunities WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Opportunity not found".into()))?;
    Ok(Json(row))
}

async fn update_opportunity(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateOpportunityRequest>,
) -> Result<Json<OpportunityResponse>, AppError> {
    let value = req.value.map(|v| rust_decimal::Decimal::from_f64_retain(v).unwrap_or_default());
    
    sqlx::query!(
        r#"UPDATE opportunities SET 
            title = COALESCE($1, title),
            description = COALESCE($2, description),
            value = COALESCE($3, value),
            stage = COALESCE($4, stage),
            probability = COALESCE($5, probability),
            expected_closed_at = COALESCE($6, expected_closed_at),
            assigned_to = COALESCE($7, assigned_to),
            updated_at = NOW()
           WHERE id = $8"#,
        req.title,
        req.description,
        value,
        req.stage,
        req.probability,
        req.expected_closed_at,
        req.assigned_to,
        id
    )
    .execute(&state.pool)
    .await?;

    let row = sqlx::query_as!(OpportunityResponse, "SELECT * FROM opportunities WHERE id = $1", id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row))
}
