//! Sales & Marketing Tasks API: list, create, get, update, delete

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
        .route("/tasks", get(list_tasks).post(create_task))
        .route("/tasks/:id", get(get_task).patch(update_task).delete(delete_task))
}

// ── Request types ──

#[derive(Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub assignee_id: Option<Uuid>,
    pub tags: Option<Vec<String>>,
    pub start_date: Option<chrono::NaiveDate>,
    pub end_date: Option<chrono::NaiveDate>,
    pub estimated_hours: Option<f32>,
    pub effort_hours: Option<f32>,
    pub category: Option<String>,
    pub department: Option<String>,
    pub parent_task_id: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub assignee_id: Option<Uuid>,
    pub tags: Option<Vec<String>>,
    pub start_date: Option<chrono::NaiveDate>,
    pub end_date: Option<chrono::NaiveDate>,
    pub estimated_hours: Option<f32>,
    pub effort_hours: Option<f32>,
    pub category: Option<String>,
    pub department: Option<String>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub assignee_id: Option<Uuid>,
}

// ── Response types ──

#[derive(Serialize)]
pub struct TaskResponse {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub assignee_id: Option<Uuid>,
    pub tags: Vec<String>,
    pub start_date: Option<chrono::NaiveDate>,
    pub end_date: Option<chrono::NaiveDate>,
    pub estimated_hours: f32,
    pub effort_hours: f32,
    pub category: String,
    pub department: String,
    pub parent_task_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// ── Handlers ──

async fn list_tasks(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<TaskResponse>>, AppError> {
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let offset = q.offset.unwrap_or(0).max(0);

    let rows = sqlx::query_as!(
        TaskRow,
        r#"SELECT id, title, description, status, priority, assignee_id,
                  tags, start_date, end_date, estimated_hours, effort_hours,
                  category, department, parent_task_id, created_at, updated_at
           FROM sales_marketing_tasks
           WHERE ($3::TEXT IS NULL OR status = $3)
             AND ($4::TEXT IS NULL OR priority = $4)
             AND ($5::UUID IS NULL OR assignee_id = $5)
           ORDER BY created_at DESC
           LIMIT $1 OFFSET $2"#,
        limit,
        offset,
        q.status.as_deref(),
        q.priority.as_deref(),
        q.assignee_id,
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn create_task(
    State(state): State<AppState>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<Json<TaskResponse>, AppError> {
    let id = Uuid::new_v4();
    let now = chrono::Utc::now();
    let status = req.status.unwrap_or_else(|| "todo".into());
    let priority = req.priority.unwrap_or_else(|| "medium".into());
    let description = req.description.unwrap_or_default();
    let tags = req.tags.unwrap_or_default();
    let category = req.category.unwrap_or_default();
    let department = req.department.unwrap_or_default();

    sqlx::query!(
        r#"INSERT INTO sales_marketing_tasks
           (id, title, description, status, priority, assignee_id, tags,
            start_date, end_date, estimated_hours, effort_hours,
            category, department, parent_task_id, created_at, updated_at)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$15)"#,
        id,
        req.title,
        description,
        status,
        priority,
        req.assignee_id,
        &tags,
        req.start_date,
        req.end_date,
        req.estimated_hours.unwrap_or(0.0),
        req.effort_hours.unwrap_or(0.0),
        category,
        department,
        req.parent_task_id,
        now,
    )
    .execute(&state.pool)
    .await?;

    let row = sqlx::query_as!(
        TaskRow,
        "SELECT id, title, description, status, priority, assignee_id, tags, start_date, end_date, estimated_hours, effort_hours, category, department, parent_task_id, created_at, updated_at FROM sales_marketing_tasks WHERE id = $1",
        id
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row.into()))
}

async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TaskResponse>, AppError> {
    let row = sqlx::query_as!(
        TaskRow,
        "SELECT id, title, description, status, priority, assignee_id, tags, start_date, end_date, estimated_hours, effort_hours, category, department, parent_task_id, created_at, updated_at FROM sales_marketing_tasks WHERE id = $1",
        id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Task not found".into()))?;
    Ok(Json(row.into()))
}

async fn update_task(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateTaskRequest>,
) -> Result<Json<TaskResponse>, AppError> {
    let now = chrono::Utc::now();
    sqlx::query!(
        r#"UPDATE sales_marketing_tasks SET
            title       = COALESCE($2, title),
            description = COALESCE($3, description),
            status      = COALESCE($4, status),
            priority    = COALESCE($5, priority),
            assignee_id = COALESCE($6, assignee_id),
            tags        = COALESCE($7, tags),
            start_date  = COALESCE($8, start_date),
            end_date    = COALESCE($9, end_date),
            estimated_hours = COALESCE($10, estimated_hours),
            effort_hours    = COALESCE($11, effort_hours),
            category    = COALESCE($12, category),
            department  = COALESCE($13, department),
            updated_at  = $14
           WHERE id = $1"#,
        id,
        req.title,
        req.description,
        req.status,
        req.priority,
        req.assignee_id,
        req.tags.as_deref(),
        req.start_date,
        req.end_date,
        req.estimated_hours,
        req.effort_hours,
        req.category,
        req.department,
        now,
    )
    .execute(&state.pool)
    .await?;

    let row = sqlx::query_as!(
        TaskRow,
        "SELECT id, title, description, status, priority, assignee_id, tags, start_date, end_date, estimated_hours, effort_hours, category, department, parent_task_id, created_at, updated_at FROM sales_marketing_tasks WHERE id = $1",
        id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Task not found".into()))?;
    Ok(Json(row.into()))
}

async fn delete_task(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let r = sqlx::query!("DELETE FROM sales_marketing_tasks WHERE id = $1", id)
        .execute(&state.pool)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("Task not found".into()));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ── Internal row type ──

struct TaskRow {
    id: Uuid,
    title: String,
    description: String,
    status: String,
    priority: String,
    assignee_id: Option<Uuid>,
    tags: Vec<String>,
    start_date: Option<chrono::NaiveDate>,
    end_date: Option<chrono::NaiveDate>,
    estimated_hours: f32,
    effort_hours: f32,
    category: String,
    department: String,
    parent_task_id: Option<Uuid>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<TaskRow> for TaskResponse {
    fn from(r: TaskRow) -> Self {
        Self {
            id: r.id,
            title: r.title,
            description: r.description,
            status: r.status,
            priority: r.priority,
            assignee_id: r.assignee_id,
            tags: r.tags,
            start_date: r.start_date,
            end_date: r.end_date,
            estimated_hours: r.estimated_hours,
            effort_hours: r.effort_hours,
            category: r.category,
            department: r.department,
            parent_task_id: r.parent_task_id,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}
