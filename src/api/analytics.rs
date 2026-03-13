//! Analytics API: aggregated metrics for dashboards

use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use crate::{error::AppError, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/leads", get(get_lead_stats))
        .route("/interactions", get(get_interaction_stats))
        .route("/opportunities", get(get_opportunity_stats))
        .route("/overall", get(get_overall_stats))
}

#[derive(Serialize)]
pub struct StatItem {
    pub label: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct ValueStatItem {
    pub label: String,
    pub count: i64,
    pub value: f64,
}

#[derive(Serialize)]
pub struct TimeSeriesPoint {
    pub label: String,
    pub value: i64,
}

#[derive(Serialize)]
pub struct LeadStats {
    pub total: i64,
    pub by_status: Vec<StatItem>,
    pub by_source: Vec<StatItem>,
    pub growth: Vec<TimeSeriesPoint>,
}

#[derive(Serialize)]
pub struct InteractionStats {
    pub total: i64,
    pub by_channel: Vec<StatItem>,
    pub by_priority: Vec<StatItem>,
}

#[derive(Serialize)]
pub struct OpportunityStats {
    pub total: i64,
    pub total_value: f64,
    pub by_stage: Vec<ValueStatItem>,
}

#[derive(Serialize)]
pub struct OverallStats {
    pub leads: i64,
    pub interactions: i64,
    pub opportunities: i64,
    pub tasks: i64,
}

async fn get_lead_stats(
    State(state): State<AppState>,
) -> Result<Json<LeadStats>, AppError> {
    let total = sqlx::query_scalar!("SELECT COUNT(*) FROM leads")
        .fetch_one(&state.pool)
        .await?
        .unwrap_or(0);

    let by_status = sqlx::query_as!(
        StatItem,
        "SELECT status as label, COUNT(*) as count FROM leads GROUP BY status"
    )
    .fetch_all(&state.pool)
    .await?;

    let by_source = sqlx::query_as!(
        StatItem,
        "SELECT source as label, COUNT(*) as count FROM leads GROUP BY source"
    )
    .fetch_all(&state.pool)
    .await?;

    let growth = sqlx::query_as!(
        TimeSeriesPoint,
        r#"SELECT TO_CHAR(created_at, 'YYYY-MM') as label, COUNT(*) as value
           FROM leads
           WHERE created_at > NOW() - INTERVAL '6 months'
           GROUP BY label
           ORDER BY label ASC"#
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(LeadStats { total, by_status, by_source, growth }))
}

async fn get_interaction_stats(
    State(state): State<AppState>,
) -> Result<Json<InteractionStats>, AppError> {
    let total = sqlx::query_scalar!("SELECT COUNT(*) FROM interactions")
        .fetch_one(&state.pool)
        .await?
        .unwrap_or(0);

    let by_channel = sqlx::query_as!(
        StatItem,
        "SELECT channel as label, COUNT(*) as count FROM interactions GROUP BY channel"
    )
    .fetch_all(&state.pool)
    .await?;

    let by_priority = sqlx::query_as!(
        StatItem,
        "SELECT priority as label, COUNT(*) as count FROM interactions GROUP BY priority"
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(InteractionStats { total, by_channel, by_priority }))
}

async fn get_opportunity_stats(
    State(state): State<AppState>,
) -> Result<Json<OpportunityStats>, AppError> {
    let stats = sqlx::query!(
        "SELECT COUNT(*) as count, SUM(value) as total_value FROM opportunities"
    )
    .fetch_one(&state.pool)
    .await?;

    let by_stage = sqlx::query_as!(
        ValueStatItem,
        r#"SELECT stage as label, COUNT(*) as count, SUM(value)::FLOAT8 as "value!"
           FROM opportunities
           GROUP BY stage"#
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(OpportunityStats {
        total: stats.count.unwrap_or(0),
        total_value: stats.total_value.map(|v| v.to_string().parse().unwrap_or(0.0)).unwrap_or(0.0),
        by_stage
    }))
}

async fn get_overall_stats(
    State(state): State<AppState>,
) -> Result<Json<OverallStats>, AppError> {
    let leads = sqlx::query_scalar!("SELECT COUNT(*) FROM leads").fetch_one(&state.pool).await?.unwrap_or(0);
    let interactions = sqlx::query_scalar!("SELECT COUNT(*) FROM interactions").fetch_one(&state.pool).await?.unwrap_or(0);
    let opportunities = sqlx::query_scalar!("SELECT COUNT(*) FROM opportunities").fetch_one(&state.pool).await?.unwrap_or(0);
    let tasks = sqlx::query_scalar!("SELECT COUNT(*) FROM sales_marketing_tasks").fetch_one(&state.pool).await?.unwrap_or(0);

    Ok(Json(OverallStats { leads, interactions, opportunities, tasks }))
}
