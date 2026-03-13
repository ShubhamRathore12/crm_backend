//! Inbound Email API: webhooks and manual processing

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{error::AppError, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/webhook", post(inbound_webhook))
        .route("/list", get(list_inbound))
        .route("/:id", get(get_inbound_detail))
}

#[derive(Deserialize)]
pub struct InboundEmailRequest {
    pub from: String,
    pub to: String,
    pub subject: String,
    pub body: String,
    pub external_id: Option<String>,
}

#[derive(Serialize)]
pub struct InboundEmailResponse {
    pub id: Uuid,
    pub contact_id: Option<Uuid>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub subject: String,
    pub status: String,
    pub priority: String,
    pub assigned_to: Option<Uuid>,
    pub latest_message: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
pub struct InboundEmailDetail {
    pub id: Uuid,
    pub contact_id: Option<Uuid>,
    pub subject: String,
    pub status: String,
    pub messages: Vec<MessageDetail>,
}

#[derive(Serialize)]
pub struct MessageDetail {
    pub id: Uuid,
    pub sender: String,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

async fn inbound_webhook(
    State(state): State<AppState>,
    Json(req): Json<InboundEmailRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // 1. Find contact by email
    let contact = sqlx::query!(
        "SELECT id FROM contacts WHERE email = $1",
        req.from
    )
    .fetch_optional(&state.pool)
    .await?;

    let contact_id = if let Some(c) = contact {
        c.id
    } else {
        Uuid::nil()
    };

    // 2. Create an interaction
    let interaction_id = Uuid::new_v4();
    
    let engine = crate::assignment_engine::AssignmentEngine::new(state.pool.clone());
    let assigned_to = engine.assign_next_agent("interaction").await.ok();

    sqlx::query!(
        r#"INSERT INTO interactions (id, contact_id, channel, subject, status, priority, assigned_to, created_at)
           VALUES ($1, $2, 'email', $3, 'new', 'medium', $4, NOW())"#,
        interaction_id,
        if contact_id.is_nil() { None } else { Some(contact_id) },
        req.subject,
        assigned_to
    )
    .execute(&state.pool)
    .await?;

    // 3. Save the actual message content
    sqlx::query!(
        r#"INSERT INTO messages (id, interaction_id, sender, content, channel, created_at)
           VALUES ($1, $2, $3, $4, 'email', NOW())"#,
        Uuid::new_v4(),
        interaction_id,
        req.from,
        req.body
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({ "ok": true, "interaction_id": interaction_id })))
}

async fn list_inbound(
    State(state): State<AppState>,
) -> Result<Json<Vec<InboundEmailResponse>>, AppError> {
    let rows = sqlx::query!(
        r#"SELECT 
            i.id, i.contact_id, i.subject, i.status, i.priority, i.assigned_to, i.created_at,
            c.name as contact_name, c.email as contact_email,
            (SELECT content FROM messages m WHERE m.interaction_id = i.id ORDER BY m.created_at DESC LIMIT 1) as latest_message
           FROM interactions i
           LEFT JOIN contacts c ON c.id = i.contact_id
           WHERE i.channel = 'email'
           ORDER BY i.created_at DESC"#
    )
    .fetch_all(&state.pool)
    .await?;

    let emails = rows.into_iter().map(|r| InboundEmailResponse {
        id: r.id,
        contact_id: r.contact_id,
        contact_name: r.contact_name,
        contact_email: r.contact_email,
        subject: r.subject,
        status: r.status,
        priority: r.priority.unwrap_or_else(|| "medium".into()),
        assigned_to: r.assigned_to,
        latest_message: r.latest_message,
        created_at: r.created_at,
    }).collect();

    Ok(Json(emails))
}

async fn get_inbound_detail(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<InboundEmailDetail>, AppError> {
    let interaction = sqlx::query!(
        "SELECT id, contact_id, subject, status FROM interactions WHERE id = $1",
        id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Inbound email not found".into()))?;

    let messages = sqlx::query_as!(
        MessageDetail,
        "SELECT id, sender, content, created_at FROM messages WHERE interaction_id = $1 ORDER BY created_at ASC",
        id
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(InboundEmailDetail {
        id: interaction.id,
        contact_id: interaction.contact_id,
        subject: interaction.subject,
        status: interaction.status,
        messages,
    }))
}
