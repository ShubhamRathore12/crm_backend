//! CTI: inbound, outbound, call-log

use axum::{extract::State, routing::get, routing::post, Json, Router};
use serde::Deserialize;
use crate::{error::AppError, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/inbound", post(cti_inbound))
        .route("/outbound", post(cti_outbound))
        .route("/call-log", get(list_call_logs).post(create_call_log))
}

#[derive(Deserialize)]
pub struct InboundRequest {
    pub from: String,
    pub to: String,
    pub call_id: Option<String>,
}

#[derive(Deserialize)]
pub struct OutboundRequest {
    pub agent_id: String,
    pub to: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct CallLogResponse {
    pub id: Uuid,
    pub call_id: Option<String>,
    pub direction: String,
    pub from_number: String,
    pub to_number: String,
    pub duration_seconds: Option<i32>,
    pub status: String,
    pub agent_id: Option<Uuid>,
    pub contact_id: Option<Uuid>,
    pub lead_id: Option<Uuid>,
    pub recording_url: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct CreateCallLogRequest {
    pub call_id: Option<String>,
    pub direction: String,
    pub from_number: String,
    pub to_number: String,
    pub duration_seconds: Option<i32>,
    pub status: String,
    pub agent_id: Option<Uuid>,
    pub contact_id: Option<Uuid>,
    pub lead_id: Option<Uuid>,
}

async fn cti_inbound(
    State(_state): State<AppState>,
    Json(req): Json<InboundRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let _ = req;
    // TODO: route to agent, open phone workspace, optional lead creation
    Ok(Json(serde_json::json!({ "ok": true, "action": "route" })))
}

async fn cti_outbound(
    State(_state): State<AppState>,
    Json(req): Json<OutboundRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let _ = req;
    // TODO: click-to-dial integration
    Ok(Json(serde_json::json!({ "ok": true, "action": "dial" })))
}

async fn list_call_logs(State(state): State<AppState>) -> Result<Json<Vec<CallLogResponse>>, AppError> {
    let rows = sqlx::query_as!(
        CallLogResponse,
        "SELECT * FROM call_logs ORDER BY created_at DESC LIMIT 100"
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn create_call_log(
    State(state): State<AppState>,
    Json(req): Json<CreateCallLogRequest>,
) -> Result<Json<CallLogResponse>, AppError> {
    let id = Uuid::new_v4();
    sqlx::query!(
        r#"INSERT INTO call_logs (id, call_id, direction, from_number, to_number, duration_seconds, status, agent_id, contact_id, lead_id)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"#,
        id,
        req.call_id,
        req.direction,
        req.from_number,
        req.to_number,
        req.duration_seconds,
        req.status,
        req.agent_id,
        req.contact_id,
        req.lead_id
    )
    .execute(&state.pool)
    .await?;

    let row = sqlx::query_as!(CallLogResponse, "SELECT * FROM call_logs WHERE id = $1", id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row))
}
