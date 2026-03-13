//! Contacts API: list, create, get, update, bulk import

use axum::{
    extract::{Query, Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{error::AppError, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_contacts).post(create_contact))
        .route("/import", post(import_contacts))
        .route("/:id", get(get_contact))
}

#[derive(Deserialize)]
pub struct CreateContactRequest {
    pub ucc_code: String,
    pub name: String,
    pub mobile: String,
    pub email: Option<String>,
    pub pan: Option<String>,
    pub address: Option<String>,
    pub custom_fields: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct ContactResponse {
    pub id: Uuid,
    pub ucc_code: String,
    pub name: String,
    pub mobile: String,
    pub email: Option<String>,
    pub pan: Option<String>,
    pub address: Option<String>,
    pub custom_fields: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

async fn list_contacts(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<ContactResponse>>, AppError> {
    let limit = q.limit.unwrap_or(100).clamp(1, 1000);
    let offset = q.offset.unwrap_or(0).max(0);

    let pool = state.db.read_pool().await;
    let rows = sqlx::query_as!(
        ContactRow,
        "SELECT * FROM contacts ORDER BY name ASC LIMIT $1 OFFSET $2",
        limit,
        offset
    )
    .fetch_all(pool)
    .await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn create_contact(
    State(state): State<AppState>,
    Json(req): Json<CreateContactRequest>,
) -> Result<Json<ContactResponse>, AppError> {
    let id = Uuid::new_v4();
    let pool = state.db.write_pool();
    sqlx::query!(
        r#"INSERT INTO contacts (id, ucc_code, name, mobile, email, pan, address, custom_fields, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())"#,
        id,
        req.ucc_code,
        req.name,
        req.mobile,
        req.email,
        req.pan,
        req.address,
        req.custom_fields.unwrap_or_else(|| serde_json::json!({}))
    )
    .execute(pool)
    .await?;
    let row = sqlx::query_as!(ContactRow, "SELECT * FROM contacts WHERE id = $1", id)
        .fetch_one(pool)
        .await?;
    Ok(Json(row.into()))
}

async fn get_contact(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ContactResponse>, AppError> {
    let pool = state.db.read_pool().await;
    let row = sqlx::query_as!(ContactRow, "SELECT * FROM contacts WHERE id = $1", id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Contact not found".into()))?;
    Ok(Json(row.into()))
}

#[derive(Deserialize)]
pub struct ImportContactsRequest {
    pub contacts: Vec<CreateContactRequest>,
}

async fn import_contacts(
    State(state): State<AppState>,
    Json(req): Json<ImportContactsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let now = chrono::Utc::now();
    let mut imported = 0u32;
    for c in req.contacts {
        let id = Uuid::new_v4();
        let _ = sqlx::query!(
            r#"INSERT INTO contacts (id, ucc_code, name, mobile, email, pan, address, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               ON CONFLICT (ucc_code) DO NOTHING"#,
            id,
            c.ucc_code,
            c.name,
            c.mobile,
            c.email,
            c.pan,
            c.address,
            now
        )
        .execute(&state.pool)
        .await?;
        imported += 1;
    }
    Ok(Json(serde_json::json!({ "imported": imported })))
}

struct ContactRow {
    pub id: Uuid,
    pub ucc_code: String,
    pub name: String,
    pub mobile: String,
    pub email: Option<String>,
    pub pan: Option<String>,
    pub address: Option<String>,
    pub custom_fields: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<ContactRow> for ContactResponse {
    fn from(r: ContactRow) -> Self {
        Self {
            id: r.id,
            ucc_code: r.ucc_code,
            name: r.name,
            mobile: r.mobile,
            email: r.email,
            pan: r.pan,
            address: r.address,
            custom_fields: r.custom_fields,
            created_at: r.created_at,
        }
    }
}
