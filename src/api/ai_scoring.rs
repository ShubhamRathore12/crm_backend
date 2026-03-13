//! AI Scoring API endpoints
//! Lead scoring, predictions, and conversation analysis

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{error::AppError, AppState, ai_scoring::AIScoringEngine};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/leads/:id/score", get(score_lead))
        .route("/leads/:id/score", post(rescore_lead))
        .route("/interactions/:id/analyze", get(analyze_conversation))
        .route("/predictions/sales", get(get_sales_predictions))
        .route("/models/retrain", post(retrain_models))
}

#[derive(Deserialize)]
pub struct RescoreRequest {
    pub force_update: Option<bool>,
}

#[derive(Deserialize)]
pub struct SalesPredictionRequest {
    pub days_ahead: i32,
}

#[derive(Serialize)]
pub struct LeadScoreResponse {
    pub lead_id: Uuid,
    pub score: f64,
    pub confidence: f64,
    pub factors: Vec<ScoreFactorResponse>,
    pub prediction: PredictionResponse,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize)]
pub struct ScoreFactorResponse {
    pub name: String,
    pub value: f64,
    pub weight: f64,
    pub description: String,
}

#[derive(Serialize)]
pub struct PredictionResponse {
    pub conversion_probability: f64,
    pub expected_value: f64,
    pub time_to_close_days: i32,
    pub risk_level: String,
    pub recommended_actions: Vec<String>,
}

#[derive(Serialize)]
pub struct ConversationAnalysisResponse {
    pub interaction_id: Uuid,
    pub sentiment: f64,
    pub engagement_score: f64,
    pub key_topics: Vec<String>,
    pub intent_detected: String,
    pub next_best_action: String,
    pub response_suggestions: Vec<String>,
    pub analyzed_at: chrono::DateTime<chrono::Utc>,
}

async fn score_lead(
    State(state): State<AppState>,
    Path(lead_id): Path<Uuid>,
) -> Result<Json<LeadScoreResponse>, AppError> {
    let engine = AIScoringEngine::new(state);
    let score = engine.score_lead(lead_id).await?;
    
    // Save score to database
    sqlx::query!(
        r#"
        INSERT INTO lead_scores (lead_id, score, confidence, factors, prediction, created_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (lead_id) DO UPDATE SET
            score = EXCLUDED.score,
            confidence = EXCLUDED.confidence,
            factors = EXCLUDED.factors,
            prediction = EXCLUDED.prediction,
            created_at = EXCLUDED.created_at
        "#,
        score.lead_id,
        score.score,
        score.confidence,
        serde_json::to_value(&score.factors).unwrap_or(serde_json::Value::Null),
        serde_json::to_value(&score.prediction).unwrap_or(serde_json::Value::Null),
        score.created_at
    )
    .execute(&engine.state.pool)
    .await?;

    Ok(Json(LeadScoreResponse {
        lead_id: score.lead_id,
        score: score.score,
        confidence: score.confidence,
        factors: score.factors.into_iter().map(Into::into).collect(),
        prediction: PredictionResponse {
            conversion_probability: score.prediction.conversion_probability,
            expected_value: score.prediction.expected_value,
            time_to_close_days: score.prediction.time_to_close_days,
            risk_level: score.prediction.risk_level,
            recommended_actions: score.prediction.recommended_actions,
        },
        created_at: score.created_at,
    }))
}

async fn rescore_lead(
    State(state): State<AppState>,
    Path(lead_id): Path<Uuid>,
    Json(_req): Json<RescoreRequest>,
) -> Result<Json<LeadScoreResponse>, AppError> {
    // Force recalculation by deleting existing score first
    sqlx::query!("DELETE FROM lead_scores WHERE lead_id = $1", lead_id)
        .execute(&state.pool)
        .await?;

    // Generate new score
    let engine = AIScoringEngine::new(state);
    let score = engine.score_lead(lead_id).await?;
    
    // Save new score
    sqlx::query!(
        r#"
        INSERT INTO lead_scores (lead_id, score, confidence, factors, prediction, created_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        score.lead_id,
        score.score,
        score.confidence,
        serde_json::to_value(&score.factors).unwrap_or(serde_json::Value::Null),
        serde_json::to_value(&score.prediction).unwrap_or(serde_json::Value::Null),
        score.created_at
    )
    .execute(&engine.state.pool)
    .await?;

    Ok(Json(LeadScoreResponse {
        lead_id: score.lead_id,
        score: score.score,
        confidence: score.confidence,
        factors: score.factors.into_iter().map(Into::into).collect(),
        prediction: PredictionResponse {
            conversion_probability: score.prediction.conversion_probability,
            expected_value: score.prediction.expected_value,
            time_to_close_days: score.prediction.time_to_close_days,
            risk_level: score.prediction.risk_level,
            recommended_actions: score.prediction.recommended_actions,
        },
        created_at: score.created_at,
    }))
}

async fn analyze_conversation(
    State(state): State<AppState>,
    Path(interaction_id): Path<Uuid>,
) -> Result<Json<ConversationAnalysisResponse>, AppError> {
    let engine = AIScoringEngine::new(state);
    let analysis = engine.analyze_conversation(interaction_id).await?;
    
    // Save analysis to database
    sqlx::query!(
        r#"
        INSERT INTO conversation_analyses (
            interaction_id, sentiment, engagement_score, key_topics, 
            intent_detected, next_best_action, response_suggestions, analyzed_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (interaction_id) DO UPDATE SET
            sentiment = EXCLUDED.sentiment,
            engagement_score = EXCLUDED.engagement_score,
            key_topics = EXCLUDED.key_topics,
            intent_detected = EXCLUDED.intent_detected,
            next_best_action = EXCLUDED.next_best_action,
            response_suggestions = EXCLUDED.response_suggestions,
            analyzed_at = EXCLUDED.analyzed_at
        "#,
        analysis.interaction_id,
        analysis.sentiment,
        analysis.engagement_score,
        serde_json::to_value(&analysis.key_topics).unwrap_or(serde_json::Value::Null),
        analysis.intent_detected,
        analysis.next_best_action,
        serde_json::to_value(&analysis.response_suggestions).unwrap_or(serde_json::Value::Null),
        analysis.analyzed_at
    )
    .execute(&engine.state.pool)
    .await?;

    Ok(Json(ConversationAnalysisResponse {
        interaction_id: analysis.interaction_id,
        sentiment: analysis.sentiment,
        engagement_score: analysis.engagement_score,
        key_topics: analysis.key_topics,
        intent_detected: analysis.intent_detected,
        next_best_action: analysis.next_best_action,
        response_suggestions: analysis.response_suggestions,
        analyzed_at: analysis.analyzed_at,
    }))
}

async fn get_sales_predictions(
    State(state): State<AppState>,
    Json(req): Json<SalesPredictionRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let engine = AIScoringEngine::new(state);
    let prediction = engine.get_sales_predictions(req.days_ahead).await?;
    
    Ok(Json(serde_json::json!({
        "period_days": prediction.period_days,
        "predicted_revenue": prediction.predicted_revenue,
        "predicted_deals": prediction.predicted_deals,
        "confidence": prediction.confidence,
        "factors": prediction.factors,
        "generated_at": chrono::Utc::now()
    })))
}

async fn retrain_models(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    // In a real implementation, this would trigger ML model retraining
    // For now, just return a success message
    
    // Log the retraining request
    sqlx::query!(
        "INSERT INTO model_retraining_logs (model_type, status, started_at) VALUES ($1, $2, $3)",
        "lead_scoring",
        "started",
        chrono::Utc::now()
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({
        "message": "Model retraining initiated",
        "status": "started",
        "estimated_completion": "30 minutes"
    })))
}

impl From<crate::ai_scoring::ScoreFactor> for ScoreFactorResponse {
    fn from(factor: crate::ai_scoring::ScoreFactor) -> Self {
        Self {
            name: factor.name,
            value: factor.value,
            weight: factor.weight,
            description: factor.description,
        }
    }
}
