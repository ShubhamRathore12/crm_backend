//! Integrations: Zapier, Slack, Calendly + Meeting invite (add contact email, send invite)

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::api::messaging::send_one_email;
use crate::{error::AppError, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/meeting-invite", post(send_meeting_invite))
        .route("/connections", get(list_connections).post(create_connection))
        .route("/webhooks/zapier", post(zapier_trigger))
        .route("/slack/notify", post(slack_notify))
        .route("/calendly/link", post(calendly_get_link))
}

// ----- Meeting invite: add person email, send email (e.g. with Calendly link) -----

#[derive(Deserialize)]
pub struct MeetingInviteRequest {
    pub to_email: String,
    pub contact_id: Option<Uuid>,
    pub subject: String,
    pub body: String,
    /// Optional Calendly (or similar) scheduling link to include in the email
    pub calendly_link: Option<String>,
}

#[derive(Serialize)]
pub struct MeetingInviteResponse {
    pub ok: bool,
    pub invite_id: Uuid,
    pub tracking_id: Uuid,
    pub message: String,
}

async fn send_meeting_invite(
    State(state): State<AppState>,
    Json(req): Json<MeetingInviteRequest>,
) -> Result<Json<MeetingInviteResponse>, AppError> {
    let to = req.to_email.trim();
    if to.is_empty() || !to.contains('@') {
        return Err(AppError::BadRequest("Valid to_email is required".into()));
    }
    if req.subject.trim().is_empty() {
        return Err(AppError::BadRequest("Subject is required".into()));
    }

    let body = if let Some(link) = &req.calendly_link {
        format!(
            "{}\n\nSchedule a time: {}",
            req.body.trim(),
            link.trim()
        )
    } else {
        req.body.trim().to_string()
    };

    let tracking_id = send_one_email(
        &state,
        to,
        &req.subject,
        &body,
        Some("meeting_invite"),
        req.contact_id,
    )
    .await
    .map_err(AppError::BadRequest)?;

    let invite_id = Uuid::new_v4();
    let email_send_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM email_sends WHERE tracking_id = $1",
    )
    .bind(tracking_id)
    .fetch_one(&state.pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO meeting_invites (id, contact_id, to_email, subject, calendly_link, email_send_id)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(invite_id)
    .bind(req.contact_id)
    .bind(to)
    .bind(req.subject.trim())
    .bind(req.calendly_link.as_deref())
    .bind(email_send_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(MeetingInviteResponse {
        ok: true,
        invite_id,
        tracking_id,
        message: "Meeting invite email sent. You can track read status in Email sends.",
    }))
}

// ----- Integration connections (Zapier, Slack, Calendly) -----

#[derive(FromRow, Serialize)]
pub struct IntegrationConnectionRow {
    pub id: Uuid,
    pub provider: String,
    pub name: Option<String>,
    pub config: serde_json::Value,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct CreateConnectionRequest {
    pub provider: String, // "zapier" | "slack" | "calendly"
    pub name: Option<String>,
    pub config: serde_json::Value, // { "webhook_url" }, { "webhook_url", "channel" }, { "api_key", "scheduling_link" }
}

async fn list_connections(
    State(state): State<AppState>,
) -> Result<Json<Vec<IntegrationConnectionRow>>, AppError> {
    let rows = sqlx::query_as::<_, IntegrationConnectionRow>(
        r#"
        SELECT id, provider, name, config, is_active, created_at
        FROM integration_connections
        WHERE is_active = true
        ORDER BY provider, created_at
        "#,
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn create_connection(
    State(state): State<AppState>,
    Json(req): Json<CreateConnectionRequest>,
) -> Result<Json<IntegrationConnectionRow>, AppError> {
    let id = Uuid::new_v4();
    let valid = ["zapier", "slack", "calendly"].contains(&req.provider.as_str());
    if !valid {
        return Err(AppError::BadRequest(
            "provider must be one of: zapier, slack, calendly".into(),
        ));
    }
    sqlx::query(
        r#"
        INSERT INTO integration_connections (id, provider, name, config)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(id)
    .bind(&req.provider)
    .bind(req.name.as_deref())
    .bind(&req.config)
    .execute(&state.pool)
    .await?;

    let row = sqlx::query_as::<_, IntegrationConnectionRow>(
        "SELECT id, provider, name, config, is_active, created_at FROM integration_connections WHERE id = $1",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

// ----- Zapier: outbound webhook (trigger from CRM events) -----

#[derive(Deserialize)]
pub struct ZapierTriggerRequest {
    pub event: String, // "lead.created", "interaction.created", etc.
    pub payload: serde_json::Value,
}

async fn zapier_trigger(
    State(state): State<AppState>,
    Json(req): Json<ZapierTriggerRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let webhooks: Vec<(String,)> = sqlx::query_as(
        "SELECT config->>'webhook_url' FROM integration_connections WHERE provider = 'zapier' AND is_active = true AND config->>'webhook_url' IS NOT NULL",
    )
    .fetch_all(&state.pool)
    .await?;

    for (url,) in webhooks {
        if !url.is_empty() {
            let body = serde_json::json!({ "event": req.event, "payload": req.payload });
            let _ = reqwest::Client::new()
                .post(&url)
                .json(&body)
                .send()
                .await;
        }
    }
    Ok(Json(serde_json::json!({ "ok": true, "event": req.event })))
}

// ----- Slack: send notification -----

#[derive(Deserialize)]
pub struct SlackNotifyRequest {
    pub message: String,
    #[serde(default)]
    pub channel: Option<String>,
}

async fn slack_notify(
    State(state): State<AppState>,
    Json(req): Json<SlackNotifyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let row: Option<(String, serde_json::Value)> = sqlx::query_as(
        "SELECT config->>'webhook_url', config FROM integration_connections WHERE provider = 'slack' AND is_active = true LIMIT 1",
    )
    .fetch_optional(&state.pool)
    .await?;

    if let Some((url, _)) = row {
        if !url.is_empty() {
            let body = serde_json::json!({ "text": req.message });
            let _ = reqwest::Client::new()
                .post(&url)
                .json(&body)
                .send()
                .await;
        }
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ----- Calendly: get scheduling link (placeholder – connect Calendly API later) -----

#[derive(Deserialize)]
pub struct CalendlyLinkRequest {
    pub contact_email: Option<String>,
}

async fn calendly_get_link(
    State(state): State<AppState>,
    Json(_req): Json<CalendlyLinkRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let row: Option<(serde_json::Value,)> = sqlx::query_as(
        "SELECT config FROM integration_connections WHERE provider = 'calendly' AND is_active = true LIMIT 1",
    )
    .fetch_optional(&state.pool)
    .await?;

    let link = row
        .and_then(|(c,)| c.get("scheduling_link").and_then(|v| v.as_str()).map(String::from))
        .unwrap_or_else(|| "https://calendly.com/your-link".to_string());
    Ok(Json(serde_json::json!({ "link": link })))
}
