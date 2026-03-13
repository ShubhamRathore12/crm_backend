//! Messaging: SMS, Email, WhatsApp send endpoints + Bulk Email + Email read tracking

use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::{error::AppError, AppState};

// 1x1 transparent GIF for open-tracking pixel
const TRACKING_PIXEL: &[u8] = &[
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0xff, 0xff,
    0xff, 0x00, 0x00, 0x00, 0x21, 0xf9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2c, 0x00, 0x00,
    0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3b,
];

pub fn sms_routes() -> Router<AppState> {
    Router::new().route("/send", post(send_sms))
}

pub fn email_routes() -> Router<AppState> {
    Router::new()
        .route("/send", post(send_email))
        .route("/bulk", get(list_bulk_campaigns).post(send_bulk_email))
        .route("/bulk/:id", get(get_bulk_campaign))
        .route("/sends", get(list_email_sends))
        .route("/open/:tracking_id", get(email_open_tracking_pixel))
}

pub fn whatsapp_routes() -> Router<AppState> {
    Router::new().route("/send", post(send_whatsapp))
}

// ----- Single send (existing) -----

#[derive(Deserialize)]
pub struct SendSmsRequest {
    pub to: String,
    pub message: String,
}

async fn send_sms(
    State(_state): State<AppState>,
    Json(req): Json<SendSmsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let _ = req;
    // TODO: integrate SMS provider (Twilio, etc.)
    Ok(Json(serde_json::json!({ "ok": true, "channel": "sms" })))
}

#[derive(Deserialize)]
pub struct SendEmailRequest {
    pub to: String,
    pub subject: String,
    pub body: String,
    #[serde(default)]
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
}

/// Sends a single email and records it in email_sends for read tracking.
/// Injects a tracking pixel into body so when the recipient opens the email, read_at is set.
pub async fn send_one_email(
    state: &AppState,
    to: &str,
    subject: &str,
    body: &str,
    entity_type: Option<&str>,
    entity_id: Option<Uuid>,
) -> Result<Uuid, String> {
    let tracking_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO email_sends (tracking_id, to_email, subject, entity_type, entity_id)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(tracking_id)
    .bind(to)
    .bind(subject)
    .bind(entity_type)
    .bind(entity_id)
    .execute(&state.pool)
    .await
    .map_err(|e| e.to_string())?;

    // Inject tracking pixel: when recipient opens email, client requests this URL and we set read_at
    let base = state.config.api_base_url.trim_end_matches('/');
    let open_url = format!("{}/email/open/{}", base, tracking_id);
    let body_with_pixel = format!(
        "{}<br/><img src=\"{}\" width=\"1\" height=\"1\" alt=\"\" style=\"display:none\" />",
        body, open_url
    );

    // TODO: integrate email (SMTP / SendGrid) - send body_with_pixel
    let _ = body_with_pixel;
    Ok(tracking_id)
}

async fn send_email(
    State(state): State<AppState>,
    Json(req): Json<SendEmailRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let entity_type = req.entity_type.as_deref();
    let tracking_id = send_one_email(
        &state,
        &req.to,
        &req.subject,
        &req.body,
        entity_type,
        req.entity_id,
    )
    .await
    .map_err(AppError::BadRequest)?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "channel": "email",
        "tracking_id": tracking_id.to_string()
    })))
}

#[derive(Deserialize)]
pub struct SendWhatsAppRequest {
    pub to: String,
    pub message: String,
}

async fn send_whatsapp(
    State(_state): State<AppState>,
    Json(req): Json<SendWhatsAppRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let _ = req;
    // TODO: integrate WhatsApp Business API
    Ok(Json(serde_json::json!({ "ok": true, "channel": "whatsapp" })))
}

// ----- Bulk email -----

fn is_valid_email(s: &str) -> bool {
    let s = s.trim();
    !s.is_empty() && s.contains('@') && s.len() <= 255
}

#[derive(Deserialize)]
pub struct SendBulkEmailRequest {
    /// List of recipient email addresses
    pub to: Vec<String>,
    pub subject: String,
    pub body: String,
}

#[derive(Serialize)]
pub struct BulkEmailSummary {
    pub campaign_id: Uuid,
    pub total: i32,
    pub sent: i32,
    pub failed: i32,
    pub errors: Vec<BulkEmailError>,
}

#[derive(Serialize)]
pub struct BulkEmailError {
    pub email: String,
    pub message: String,
}

#[derive(FromRow, Serialize)]
pub struct BulkEmailCampaignRow {
    pub id: Uuid,
    pub subject: String,
    pub body: String,
    pub total_count: i32,
    pub sent_count: i32,
    pub failed_count: i32,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(FromRow, Serialize)]
pub struct BulkEmailRecipientRow {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub email: String,
    pub status: String,
    pub error_message: Option<String>,
    pub sent_at: Option<chrono::DateTime<chrono::Utc>>,
}

async fn send_bulk_email(
    State(state): State<AppState>,
    Json(req): Json<SendBulkEmailRequest>,
) -> Result<Json<BulkEmailSummary>, AppError> {
    let to: Vec<String> = req
        .to
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| is_valid_email(s))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    if to.is_empty() {
        return Err(AppError::BadRequest(
            "At least one valid email address is required".into(),
        ));
    }
    if req.subject.trim().is_empty() {
        return Err(AppError::BadRequest("Subject is required".into()));
    }

    let total = to.len() as i32;
    let campaign_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO bulk_email_campaigns (id, subject, body, total_count, status)
        VALUES ($1, $2, $3, $4, 'sending')
        "#,
    )
    .bind(campaign_id)
    .bind(req.subject.trim())
    .bind(req.body.trim())
    .bind(total)
    .execute(&state.pool)
    .await?;

    for email in &to {
        sqlx::query(
            r#"
            INSERT INTO bulk_email_recipients (campaign_id, email, status)
            VALUES ($1, $2, 'pending')
            "#,
        )
        .bind(campaign_id)
        .bind(email)
        .execute(&state.pool)
        .await?;
    }

    let mut sent_count = 0i32;
    let mut failed_count = 0i32;
    let mut errors: Vec<BulkEmailError> = Vec::new();

    let rows: Vec<(Uuid, String)> = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id, email FROM bulk_email_recipients WHERE campaign_id = $1 ORDER BY created_at",
    )
    .bind(campaign_id)
    .fetch_all(&state.pool)
    .await?;

    for (recipient_id, email) in rows {
        match send_one_email(
            &state,
            &email,
            &req.subject,
            &req.body,
            Some("bulk"),
            Some(campaign_id),
        )
        .await
        {
            Ok(_tracking_id) => {
                sent_count += 1;
                let now = chrono::Utc::now();
                sqlx::query(
                    "UPDATE bulk_email_recipients SET status = 'sent', sent_at = $1 WHERE id = $2",
                )
                .bind(now)
                .bind(recipient_id)
                .execute(&state.pool)
                .await?;
            }
            Err(e) => {
                failed_count += 1;
                errors.push(BulkEmailError {
                    email: email.clone(),
                    message: e.clone(),
                });
                sqlx::query(
                    "UPDATE bulk_email_recipients SET status = 'failed', error_message = $1 WHERE id = $2",
                )
                .bind(&e)
                .bind(recipient_id)
                .execute(&state.pool)
                .await?;
            }
        }
    }

    let status = if failed_count == 0 { "completed" } else { "completed" };
    sqlx::query(
        r#"
        UPDATE bulk_email_campaigns
        SET sent_count = $1, failed_count = $2, status = $3
        WHERE id = $4
        "#,
    )
    .bind(sent_count)
    .bind(failed_count)
    .bind(status)
    .bind(campaign_id)
    .execute(&state.pool)
    .await?;

    Ok(Json(BulkEmailSummary {
        campaign_id,
        total,
        sent: sent_count,
        failed: failed_count,
        errors,
    }))
}

async fn list_bulk_campaigns(
    State(state): State<AppState>,
) -> Result<Json<Vec<BulkEmailCampaignRow>>, AppError> {
    let rows = sqlx::query_as::<_, BulkEmailCampaignRow>(
        r#"
        SELECT id, subject, body, total_count, sent_count, failed_count, status, created_at
        FROM bulk_email_campaigns
        ORDER BY created_at DESC
        LIMIT 100
        "#,
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[derive(Serialize)]
pub struct BulkEmailCampaignDetail {
    pub campaign: BulkEmailCampaignRow,
    pub recipients: Vec<BulkEmailRecipientRow>,
}

async fn get_bulk_campaign(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<BulkEmailCampaignDetail>, AppError> {
    let campaign = sqlx::query_as::<_, BulkEmailCampaignRow>(
        r#"
        SELECT id, subject, body, total_count, sent_count, failed_count, status, created_at
        FROM bulk_email_campaigns WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Bulk email campaign".into()))?;

    let recipients = sqlx::query_as::<_, BulkEmailRecipientRow>(
        r#"
        SELECT id, campaign_id, email, status, error_message, sent_at
        FROM bulk_email_recipients WHERE campaign_id = $1 ORDER BY created_at
        "#,
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(BulkEmailCampaignDetail {
        campaign,
        recipients,
    }))
}

// ----- Email read tracking -----

#[derive(FromRow, Serialize)]
pub struct EmailSendRow {
    pub id: Uuid,
    pub tracking_id: Uuid,
    pub to_email: String,
    pub subject: String,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub read_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

async fn list_email_sends(
    State(state): State<AppState>,
    Query(params): Query<ListEmailSendsQuery>,
) -> Result<Json<Vec<EmailSendRow>>, AppError> {
    let limit = params.limit.unwrap_or(50).min(100);
    let rows = if let (Some(et), Some(eid)) = (params.entity_type, params.entity_id) {
        sqlx::query_as::<_, EmailSendRow>(
            r#"
            SELECT id, tracking_id, to_email, subject, entity_type, entity_id, read_at, created_at
            FROM email_sends
            WHERE entity_type = $1 AND entity_id = $2
            ORDER BY created_at DESC
            LIMIT $3
            "#,
        )
        .bind(et)
        .bind(eid)
        .bind(limit as i64)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query_as::<_, EmailSendRow>(
            r#"
            SELECT id, tracking_id, to_email, subject, entity_type, entity_id, read_at, created_at
            FROM email_sends
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit as i64)
        .fetch_all(&state.pool)
        .await?
    };
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct ListEmailSendsQuery {
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub limit: Option<u32>,
}

/// Tracking pixel: when recipient opens email, client requests this URL; we set read_at and return 1x1 GIF.
async fn email_open_tracking_pixel(
    State(state): State<AppState>,
    Path(tracking_id): Path<Uuid>,
) -> impl IntoResponse {
    let _ = sqlx::query("UPDATE email_sends SET read_at = $1 WHERE tracking_id = $2 AND read_at IS NULL")
        .bind(chrono::Utc::now())
        .bind(tracking_id)
        .execute(&state.pool)
        .await;

    (
        [(header::CONTENT_TYPE, "image/gif")],
        TRACKING_PIXEL,
    )
}
