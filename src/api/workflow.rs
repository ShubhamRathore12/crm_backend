//! Workflow engine: list, create, update, run

use axum::{
    extract::{Path, State},
    routing::{get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{error::AppError, AppState, workflow_engine::{WorkflowEngine, WorkflowContext}};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_workflows).post(create_workflow))
        .route("/run", post(run_workflow))
        .route("/:id", get(get_workflow).patch(update_workflow))
}

#[derive(Deserialize)]
pub struct CreateWorkflowRequest {
    pub name: String,
    pub trigger: String,
    pub definition_json: serde_json::Value,
}

#[derive(Deserialize)]
pub struct UpdateWorkflowRequest {
    pub name: Option<String>,
    pub definition_json: Option<serde_json::Value>,
    pub active: Option<bool>,
}

#[derive(Deserialize)]
pub struct RunWorkflowRequest {
    pub workflow_id: Uuid,
    pub entity_id: Uuid,
    pub entity_type: String,
    pub trigger_data: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct WorkflowResponse {
    pub id: Uuid,
    pub name: String,
    pub trigger: String,
    pub definition_json: serde_json::Value,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

async fn list_workflows(State(state): State<AppState>) -> Result<Json<Vec<WorkflowResponse>>, AppError> {
    let rows = sqlx::query_as!(
        WorkflowRow,
        "SELECT id, name, trigger, definition_json, active, created_at FROM workflows ORDER BY created_at DESC"
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

async fn create_workflow(
    State(state): State<AppState>,
    Json(req): Json<CreateWorkflowRequest>,
) -> Result<Json<WorkflowResponse>, AppError> {
    let id = Uuid::new_v4();
    let def = serde_json::to_value(&req.definition_json).unwrap_or(serde_json::Value::Null);
    sqlx::query!(
        r#"INSERT INTO workflows (id, name, trigger, definition_json, active) VALUES ($1, $2, $3, $4, true)"#,
        id,
        req.name,
        req.trigger,
        def
    )
    .execute(&state.pool)
    .await?;
    let row = sqlx::query_as!(WorkflowRow, "SELECT * FROM workflows WHERE id = $1", id)
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(row.into()))
}

async fn get_workflow(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<WorkflowResponse>, AppError> {
    let row = sqlx::query_as!(WorkflowRow, "SELECT * FROM workflows WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Workflow not found".into()))?;
    Ok(Json(row.into()))
}

async fn update_workflow(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateWorkflowRequest>,
) -> Result<Json<WorkflowResponse>, AppError> {
    if let Some(name) = &req.name {
        sqlx::query!("UPDATE workflows SET name = $1 WHERE id = $2", name, id)
            .execute(&state.pool)
            .await?;
    }
    if let Some(ref def) = req.definition_json {
        let d = serde_json::to_value(def).unwrap_or(serde_json::Value::Null);
        sqlx::query!("UPDATE workflows SET definition_json = $1 WHERE id = $2", d, id)
            .execute(&state.pool)
            .await?;
    }
    if let Some(active) = req.active {
        sqlx::query!("UPDATE workflows SET active = $1 WHERE id = $2", active, id)
            .execute(&state.pool)
            .await?;
    }
    let row = sqlx::query_as!(WorkflowRow, "SELECT * FROM workflows WHERE id = $1", id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("Workflow not found".into()))?;
    Ok(Json(row.into()))
}

async fn run_workflow(
    State(state): State<AppState>,
    Json(req): Json<RunWorkflowRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let run_id = Uuid::new_v4();
    
    // Create workflow run record
    sqlx::query!(
        r#"INSERT INTO workflow_runs (id, workflow_id, entity_id, entity_type, status, started_at)
           VALUES ($1, $2, $3, $4, 'running', NOW())"#,
        run_id,
        req.workflow_id,
        req.entity_id,
        req.entity_type
    )
    .execute(&state.pool)
    .await?;

    // Create workflow context and execute
    let context = WorkflowContext {
        entity_id: req.entity_id,
        entity_type: req.entity_type.clone(),
        trigger_data: req.trigger_data.unwrap_or_default(),
        variables: std::collections::HashMap::new(),
        current_node: None,
        history: vec![],
    };

    let engine = WorkflowEngine::new(state.clone());
    
    // Execute workflow asynchronously
    let workflow_id = req.workflow_id;
    let pool = state.pool.clone();
    tokio::spawn(async move {
        match engine.execute_workflow(workflow_id, context).await {
            Ok(result) => {
                // Update workflow run status
                let status = if matches!(result.status, crate::workflow_engine::ExecutionStatus::Success) {
                    "completed"
                } else {
                    "failed"
                };
                
                sqlx::query!(
                    "UPDATE workflow_runs SET status = $1, completed_at = NOW() WHERE id = $2",
                    status,
                    run_id
                )
                .execute(&pool)
                .await
                .ok();
            }
            Err(e) => {
                tracing::error!("Workflow execution failed: {}", e);
                sqlx::query!(
                    "UPDATE workflow_runs SET status = 'failed', completed_at = NOW() WHERE id = $1",
                    run_id
                )
                .execute(&pool)
                .await
                .ok();
            }
        }
    });

    Ok(Json(serde_json::json!({ 
        "run_id": run_id, 
        "status": "running",
        "message": "Workflow execution started"
    })))
}

struct WorkflowRow {
    id: Uuid,
    name: String,
    trigger: String,
    definition_json: serde_json::Value,
    active: bool,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl From<WorkflowRow> for WorkflowResponse {
    fn from(r: WorkflowRow) -> Self {
        Self {
            id: r.id,
            name: r.name,
            trigger: r.trigger,
            definition_json: r.definition_json,
            active: r.active,
            created_at: r.created_at,
        }
    }
}
