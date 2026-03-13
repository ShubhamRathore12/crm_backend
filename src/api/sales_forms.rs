//! Sales Forms API: list, create, get, update, delete forms + submissions

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
        .route("/", get(list_forms).post(create_form))
        .route("/:id", get(get_form).patch(update_form).delete(delete_form))
        .route("/:id/submit", post(submit_form))
        .route("/:id/submissions", get(list_submissions))
}

// ── Request types ──

#[derive(Deserialize)]
pub struct CreateFormRequest {
    pub name: String,
    pub description: Option<String>,
    pub fields_json: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub struct UpdateFormRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub fields_json: Option<serde_json::Value>,
    pub is_active: Option<bool>,
}

#[derive(Deserialize)]
pub struct SubmitFormRequest {
    pub data_json: serde_json::Value,
}

// ── Response types ──

#[derive(Serialize)]
pub struct FormResponse {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub fields_json: serde_json::Value,
    pub is_active: bool,
    pub open_count: i64,
    pub closed_count: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
pub struct SubmissionResponse {
    pub id: Uuid,
    pub form_id: Uuid,
    pub data_json: serde_json::Value,
    pub status: String,
    pub submitted_at: chrono::DateTime<chrono::Utc>,
}

// ── Handlers ──

async fn list_forms(
    State(state): State<AppState>,
) -> Result<Json<Vec<FormResponse>>, AppError> {
    let rows = sqlx::query!(
        r#"SELECT f.id, f.name, f.description, f.fields_json, f.is_active,
                  f.created_at, f.updated_at,
                  COALESCE(SUM(CASE WHEN s.status = 'open' THEN 1 ELSE 0 END), 0) AS "open_count!",
                  COALESCE(SUM(CASE WHEN s.status = 'closed' THEN 1 ELSE 0 END), 0) AS "closed_count!"
           FROM sales_forms f
           LEFT JOIN sales_form_submissions s ON s.form_id = f.id
           GROUP BY f.id
           ORDER BY f.created_at DESC"#
    )
    .fetch_all(&state.pool)
    .await?;

    let forms = rows
        .into_iter()
        .map(|r| FormResponse {
            id: r.id,
            name: r.name,
            description: r.description,
            fields_json: r.fields_json,
            is_active: r.is_active,
            open_count: r.open_count,
            closed_count: r.closed_count,
            created_at: r.created_at,
            updated_at: r.updated_at,
        })
        .collect();

    Ok(Json(forms))
}

async fn create_form(
    State(state): State<AppState>,
    Json(req): Json<CreateFormRequest>,
) -> Result<Json<FormResponse>, AppError> {
    let id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let desc = req.description.unwrap_or_default();
    let fields = req.fields_json.unwrap_or(serde_json::json!([]));

    sqlx::query!(
        r#"INSERT INTO sales_forms (id, name, description, fields_json, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $5)"#,
        id,
        req.name,
        desc,
        fields,
        now,
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(FormResponse {
        id,
        name: req.name,
        description: desc,
        fields_json: fields.clone(),
        is_active: true,
        open_count: 0,
        closed_count: 0,
        created_at: now,
        updated_at: now,
    }))
}

async fn sync_fields_with_definitions(
    pool: &sqlx::PgPool,
    fields: &serde_json::Value,
) -> Result<(), AppError> {
    if let Some(fields_array) = fields.as_array() {
        for field in fields_array {
            let name = field.get("name").and_then(|v| v.as_str());
            let label = field.get("label").and_then(|v| v.as_str());
            let field_type = field.get("type").and_then(|v| v.as_str()).unwrap_or("text");
            let entity_type = field.get("entity_type").and_then(|v| v.as_str()).unwrap_or("lead");
            let sync = field.get("sync_to_db").and_then(|v| v.as_bool()).unwrap_or(false);

            if sync && name.is_some() && label.is_some() {
                sqlx::query!(
                    r#"INSERT INTO field_definitions (entity_type, field_name, label, field_type, updated_at)
                       VALUES ($1, $2, $3, $4, NOW())
                       ON CONFLICT (entity_type, field_name) DO UPDATE SET
                       label = EXCLUDED.label, field_type = EXCLUDED.field_type, updated_at = NOW()"#,
                    entity_type, name.unwrap(), label.unwrap(), field_type
                )
                .execute(pool)
                .await?;
            }
        }
    }
    Ok(())
}

async fn get_form(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<FormResponse>, AppError> {
    let r = sqlx::query!(
        r#"SELECT f.id, f.name, f.description, f.fields_json, f.is_active,
                  f.created_at, f.updated_at,
                  COALESCE(SUM(CASE WHEN s.status = 'open' THEN 1 ELSE 0 END), 0) AS "open_count!",
                  COALESCE(SUM(CASE WHEN s.status = 'closed' THEN 1 ELSE 0 END), 0) AS "closed_count!"
           FROM sales_forms f
           LEFT JOIN sales_form_submissions s ON s.form_id = f.id
           WHERE f.id = $1
           GROUP BY f.id"#,
        id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Form not found".into()))?;

    Ok(Json(FormResponse {
        id: r.id,
        name: r.name,
        description: r.description,
        fields_json: r.fields_json,
        is_active: r.is_active,
        open_count: r.open_count,
        closed_count: r.closed_count,
        created_at: r.created_at,
        updated_at: r.updated_at,
    }))
}

async fn update_form(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateFormRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let now = chrono::Utc::now();
    sqlx::query!(
        r#"UPDATE sales_forms SET
            name        = COALESCE($2, name),
            description = COALESCE($3, description),
            fields_json = COALESCE($4, fields_json),
            is_active   = COALESCE($5, is_active),
            updated_at  = $6
           WHERE id = $1"#,
        id,
        req.name,
        req.description,
        req.fields_json,
        req.is_active,
        now,
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn delete_form(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let r = sqlx::query!("DELETE FROM sales_forms WHERE id = $1", id)
        .execute(&state.pool)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("Form not found".into()));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn submit_form(
    State(state): State<AppState>,
    Path(form_id): Path<Uuid>,
    Json(req): Json<SubmitFormRequest>,
) -> Result<Json<SubmissionResponse>, AppError> {
    // Verify form exists and is active
    let form = sqlx::query!("SELECT is_active FROM sales_forms WHERE id = $1", form_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Form not found".into()))?;
    if !form.is_active {
        return Err(AppError::BadRequest("Form is not active".into()));
    }

    let id = Uuid::new_v4();
    let now = chrono::Utc::now();
    sqlx::query!(
        r#"INSERT INTO sales_form_submissions (id, form_id, data_json, submitted_at)
           VALUES ($1, $2, $3, $4)"#,
        id,
        form_id,
        req.data_json,
        now,
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(SubmissionResponse {
        id,
        form_id,
        data_json: req.data_json,
        status: "open".into(),
        submitted_at: now,
    }))
}

async fn list_submissions(
    State(state): State<AppState>,
    Path(form_id): Path<Uuid>,
) -> Result<Json<Vec<SubmissionResponse>>, AppError> {
    let rows = sqlx::query!(
        r#"SELECT id, form_id, data_json, status, submitted_at
           FROM sales_form_submissions
           WHERE form_id = $1
           ORDER BY submitted_at DESC"#,
        form_id
    )
    .fetch_all(&state.pool)
    .await?;

    let subs = rows
        .into_iter()
        .map(|r| SubmissionResponse {
            id: r.id,
            form_id: r.form_id,
            data_json: r.data_json,
            status: r.status,
            submitted_at: r.submitted_at,
        })
        .collect();

    Ok(Json(subs))
}
