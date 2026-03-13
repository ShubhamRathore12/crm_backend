//! Maintenance API: archiving and system health

use ax_sessions::Session; // Placeholder if needed
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use crate::{error::AppError, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/archive/run", post(trigger_archiving))
        .route("/health/db", get(db_health_check))
}

async fn db_health_check(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let primary_status = match sqlx::query("SELECT 1").execute(state.db.primary()).await {
        Ok(_) => "online",
        Err(_) => "offline",
    };

    let secondary_status = if let Some(sec) = state.db.secondary() {
        match sqlx::query("SELECT 1").execute(sec).await {
            Ok(_) => "online",
            Err(_) => "offline",
        }
    } else {
        "not_configured"
    };

    Ok(Json(serde_json::json!({
        "primary": primary_status,
        "secondary": secondary_status,
        "mode": if primary_status == "online" { "normal" } else { "failover" }
    })))
}

async fn trigger_archiving(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Call the database function
    let result = sqlx::query_scalar!(
        "SELECT archive_maintenance($1)",
        5000 // batch size
    )
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(result.unwrap_or_else(|| serde_json::json!({ "error": "archiving failed" }))))
}
