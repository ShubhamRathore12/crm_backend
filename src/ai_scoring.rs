//! AI Lead Scoring and Predictions
//! Machine learning models for lead scoring, sales predictions, and conversation analysis

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::{error::AppError, AppState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeadScore {
    pub lead_id: Uuid,
    pub score: f64,
    pub confidence: f64,
    pub factors: Vec<ScoreFactor>,
    pub prediction: LeadPrediction,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ScoreFactor {
    pub name: String,
    pub value: f64,
    pub weight: f64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeadPrediction {
    pub conversion_probability: f64,
    pub expected_value: f64,
    pub time_to_close_days: i32,
    pub risk_level: String, // low, medium, high
    pub recommended_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationAnalysis {
    pub interaction_id: Uuid,
    pub sentiment: f64, // -1.0 to 1.0
    pub engagement_score: f64,
    pub key_topics: Vec<String>,
    pub intent_detected: String,
    pub next_best_action: String,
    pub response_suggestions: Vec<String>,
    pub analyzed_at: DateTime<Utc>,
}

pub struct AIScoringEngine {
    pub state: AppState,
}

impl AIScoringEngine {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    /// Score a lead based on multiple factors
    pub async fn score_lead(&self, lead_id: Uuid) -> Result<LeadScore, AppError> {
        // Get lead and related data
        let lead_data = self.get_lead_scoring_data(lead_id).await?;
        
        // Calculate individual factor scores
        let factors = self.calculate_score_factors(&lead_data).await;
        
        // Calculate weighted total score
        let total_score: f64 = factors.iter()
            .map(|f| f.value * f.weight)
            .sum();
        
        // Generate predictions
        let prediction = self.generate_lead_prediction(&lead_data, total_score).await;
        
        // Calculate confidence based on data completeness
        let confidence = self.calculate_confidence(&lead_data).await;
        
        Ok(LeadScore {
            lead_id,
            score: total_score,
            confidence,
            factors,
            prediction,
            created_at: Utc::now(),
        })
    }

    /// Analyze conversation for sentiment and insights
    pub async fn analyze_conversation(&self, interaction_id: Uuid) -> Result<ConversationAnalysis, AppError> {
        let conversation_data = self.get_conversation_data(interaction_id).await?;
        
        // Analyze sentiment (simplified - in production use NLP library)
        let sentiment = self.analyze_sentiment(&conversation_data.messages).await;
        
        // Calculate engagement score
        let engagement_score = self.calculate_engagement(&conversation_data).await;
        
        // Extract key topics (simplified keyword extraction)
        let key_topics = self.extract_topics(&conversation_data.messages).await;
        
        // Detect intent
        let intent_detected = self.detect_intent(&conversation_data.messages).await;
        
        // Recommend next best action
        let next_best_action = self.recommend_next_action(&conversation_data, sentiment, engagement_score).await;
        
        // Generate response suggestions
        let response_suggestions = self.generate_response_suggestions(&conversation_data, &intent_detected).await;
        
        Ok(ConversationAnalysis {
            interaction_id,
            sentiment,
            engagement_score,
            key_topics,
            intent_detected,
            next_best_action,
            response_suggestions,
            analyzed_at: Utc::now(),
        })
    }

    /// Get sales predictions for a time period
    pub async fn get_sales_predictions(&self, days_ahead: i32) -> Result<SalesPrediction, AppError> {
        let historical_data = self.get_historical_sales_data(days_ahead * 2).await?;
        
        // Simple linear regression for prediction (in production use ML models)
        let predicted_revenue = self.predict_revenue(&historical_data, days_ahead).await;
        let predicted_deals = self.predict_deal_count(&historical_data, days_ahead).await;
        let confidence = self.calculate_prediction_confidence(&historical_data);
        
        Ok(SalesPrediction {
            period_days: days_ahead,
            predicted_revenue,
            predicted_deals,
            confidence,
            factors: self.get_prediction_factors(&historical_data).await,
        })
    }

    async fn get_lead_scoring_data(&self, lead_id: Uuid) -> Result<LeadScoringData, AppError> {
        // Query lead, contact, interactions, and related data
        let lead = sqlx::query_as!(
            LeadScoringRecord,
            r#"
            SELECT l.id, l.source, l.status, l.stage, l.assigned_to, l.created_at,
                   c.name as contact_name, c.email, c.mobile, c.pan,
                   c.created_at as contact_created_at, l.contact_id
            FROM leads l
            JOIN contacts c ON l.contact_id = c.id
            WHERE l.id = $1
            "#,
            lead_id
        )
        .fetch_one(&self.state.pool)
        .await?;

        let interactions = sqlx::query_as!(
            InteractionData,
            r#"
            SELECT id, channel, subject, status, priority, created_at
            FROM interactions
            WHERE contact_id = $1
            ORDER BY created_at DESC
            LIMIT 10
            "#,
            lead.contact_id
        )
        .fetch_all(&self.state.pool)
        .await?;

        let message_count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM messages m
            JOIN interactions i ON m.interaction_id = i.id
            WHERE i.contact_id = $1
            "#
        )
        .bind(lead.contact_id)
        .fetch_one(&self.state.pool)
        .await
        .unwrap_or(0);

        Ok(LeadScoringData {
            lead,
            interactions,
            message_count,
        })
    }

    async fn calculate_score_factors(&self, data: &LeadScoringData) -> Vec<ScoreFactor> {
        let mut factors = Vec::new();

        // Lead source scoring
        let source_score = match data.lead.source.as_str() {
            "website" => 0.8,
            "referral" => 0.9,
            "cold_email" => 0.3,
            "social_media" => 0.6,
            "phone" => 0.7,
            "event" => 0.8,
            _ => 0.5,
        };
        factors.push(ScoreFactor {
            name: "Lead Source".to_string(),
            value: source_score,
            weight: 0.15,
            description: format!("Source: {}", data.lead.source),
        });

        // Lead age scoring (newer is better)
        let days_since_creation = (Utc::now() - data.lead.created_at).num_seconds() / 86400;
        let age_score = 1.0 - (days_since_creation as f64 / 365.0).min(1.0);
        factors.push(ScoreFactor {
            name: "Lead Freshness".to_string(),
            value: age_score,
            weight: 0.10,
            description: format!("Days old: {}", days_since_creation),
        });

        // Engagement scoring
        let engagement_score = (data.message_count as f64 / 10.0).min(1.0);
        factors.push(ScoreFactor {
            name: "Engagement Level".to_string(),
            value: engagement_score,
            weight: 0.25,
            description: format!("Messages: {}", data.message_count),
        });

        // Status scoring
        let status_score = match data.lead.status.as_str() {
            "new" => 0.8,
            "contacted" => 0.7,
            "qualified" => 0.9,
            "converted" => 1.0,
            "lost" => 0.1,
            _ => 0.5,
        };
        factors.push(ScoreFactor {
            name: "Lead Status".to_string(),
            value: status_score,
            weight: 0.20,
            description: format!("Status: {}", data.lead.status),
        });

        // Interaction count scoring
        let interaction_score = (data.interactions.len() as f64 / 5.0).min(1.0);
        factors.push(ScoreFactor {
            name: "Interaction History".to_string(),
            value: interaction_score,
            weight: 0.15,
            description: format!("Interactions: {}", data.interactions.len()),
        });

        // Assignment scoring (assigned leads are higher priority)
        let assignment_score = if data.lead.assigned_to.is_some() { 0.8 } else { 0.4 };
        factors.push(ScoreFactor {
            name: "Assignment Status".to_string(),
            value: assignment_score,
            weight: 0.15,
            description: if data.lead.assigned_to.is_some() { "Assigned" } else { "Unassigned" }.to_string(),
        });

        factors
    }

    async fn generate_lead_prediction(&self, _data: &LeadScoringData, score: f64) -> LeadPrediction {
        // Conversion probability based on score
        let conversion_probability = score.clamp(0.0, 1.0);
        
        // Expected value (simplified - would use deal size in production)
        let expected_value = conversion_probability * 50000.0; // Assume $50k average deal
        
        // Time to close based on score
        let time_to_close_days = match conversion_probability {
            p if p >= 0.8 => 15,
            p if p >= 0.6 => 30,
            p if p >= 0.4 => 60,
            _ => 90,
        };
        
        // Risk level
        let risk_level = match conversion_probability {
            p if p >= 0.7 => "low".to_string(),
            p if p >= 0.4 => "medium".to_string(),
            _ => "high".to_string(),
        };
        
        // Recommended actions
        let mut recommended_actions = Vec::new();
        if conversion_probability >= 0.7 {
            recommended_actions.push("Prioritize immediate follow-up".to_string());
            recommended_actions.push("Schedule demo call".to_string());
        } else if conversion_probability >= 0.4 {
            recommended_actions.push("Send personalized email".to_string());
            recommended_actions.push("Provide additional resources".to_string());
        } else {
            recommended_actions.push("Nurture with automated campaigns".to_string());
            recommended_actions.push("Re-qualify lead".to_string());
        }
        
        LeadPrediction {
            conversion_probability,
            expected_value,
            time_to_close_days,
            risk_level,
            recommended_actions,
        }
    }

    async fn calculate_confidence(&self, data: &LeadScoringData) -> f64 {
        let mut confidence: f64 = 0.5; // Base confidence
        
        // Higher confidence with more data
        if data.message_count > 0 {
            confidence += 0.2;
        }
        if !data.interactions.is_empty() {
            confidence += 0.2;
        }
        if data.lead.assigned_to.is_some() {
            confidence += 0.1;
        }
        
        confidence.min(1.0)
    }

    async fn analyze_sentiment(&self, messages: &[String]) -> f64 {
        // Simplified sentiment analysis (in production use NLP library)
        let positive_words = vec!["good", "great", "excellent", "interested", "happy", "yes"];
        let negative_words = vec!["bad", "poor", "not", "no", "unhappy", "angry"];
        
        let mut positive_count = 0;
        let mut negative_count = 0;
        
        for message in messages {
            let message_lower = message.to_lowercase();
            for word in &positive_words {
                if message_lower.contains(word) {
                    positive_count += 1;
                }
            }
            for word in &negative_words {
                if message_lower.contains(word) {
                    negative_count += 1;
                }
            }
        }
        
        let total = positive_count + negative_count;
        if total == 0 {
            return 0.0;
        }
        
        (positive_count as f64 - negative_count as f64) / total as f64
    }

    async fn calculate_engagement(&self, data: &ConversationData) -> f64 {
        // Calculate engagement based on message count and response time
        let message_score = (data.messages.len() as f64 / 10.0).min(1.0);
        let response_score = if data.avg_response_time_hours < 24.0 { 1.0 } else { 0.5 };
        
        (message_score + response_score) / 2.0
    }

    async fn extract_topics(&self, messages: &[String]) -> Vec<String> {
        // Simplified topic extraction (in production use NLP)
        let common_topics = vec!["price", "features", "demo", "support", "integration"];
        let mut topics = Vec::new();
        
        for message in messages {
            let message_lower = message.to_lowercase();
            for topic in &common_topics {
                if message_lower.contains(topic) && !topics.contains(&topic.to_string()) {
                    topics.push(topic.to_string());
                }
            }
        }
        
        topics
    }

    async fn detect_intent(&self, messages: &[String]) -> String {
        // Simplified intent detection
        let last_message = messages.last().unwrap_or(&"".to_string()).to_lowercase();
        
        if last_message.contains("price") || last_message.contains("cost") {
            "pricing_inquiry".to_string()
        } else if last_message.contains("demo") || last_message.contains("show") {
            "demo_request".to_string()
        } else if last_message.contains("buy") || last_message.contains("purchase") {
            "purchase_intent".to_string()
        } else if last_message.contains("question") || last_message.contains("help") {
            "support_request".to_string()
        } else {
            "general_inquiry".to_string()
        }
    }

    async fn recommend_next_action(&self, data: &ConversationData, sentiment: f64, engagement: f64) -> String {
        if sentiment > 0.5 && engagement > 0.7 {
            "Schedule follow-up call".to_string()
        } else if sentiment < -0.2 {
            "Send apology and offer support".to_string()
        } else if data.messages.len() < 3 {
            "Provide more information".to_string()
        } else {
            "Ask for next steps".to_string()
        }
    }

    async fn generate_response_suggestions(&self, _data: &ConversationData, intent: &str) -> Vec<String> {
        match intent {
            "pricing_inquiry" => vec![
                "I'd be happy to discuss our pricing options with you.".to_string(),
                "Let me schedule a call to go over our packages.".to_string(),
                "I can send you our pricing sheet right away.".to_string()
            ],
            "demo_request" => vec![
                "I can schedule a personalized demo for you.".to_string(),
                "Would you prefer a live demo or a recorded walkthrough?".to_string(),
                "Let me find a time that works for your schedule.".to_string()
            ],
            "purchase_intent" => vec![
                "Great! Let's discuss the next steps for getting started.".to_string(),
                "I can help you with the onboarding process.".to_string(),
                "Let me connect you with our sales team.".to_string()
            ],
            _ => vec![
                "Thank you for your interest. How can I help you today?".to_string(),
                "I'm here to answer any questions you might have.".to_string(),
                "What would you like to know more about?".to_string()
            ]
        }
    }

    async fn get_conversation_data(&self, interaction_id: Uuid) -> Result<ConversationData, AppError> {
        let messages = sqlx::query_scalar::<_, String>(
            "SELECT content FROM messages WHERE interaction_id = $1 ORDER BY created_at"
        )
        .bind(interaction_id)
        .fetch_all(&self.state.pool)
        .await?;

        let avg_response_time = sqlx::query_scalar::<_, Option<f64>>(
            r#"
            SELECT AVG(EXTRACT(EPOCH FROM (m2.created_at - m1.created_at)) / 3600)
            FROM messages m1
            JOIN messages m2 ON m1.interaction_id = m2.interaction_id 
                AND m2.created_at > m1.created_at
                AND m1.sender != m2.sender
            WHERE m1.interaction_id = $1
            "#
        )
        .bind(interaction_id)
        .fetch_one(&self.state.pool)
        .await
        .unwrap_or(None)
        .unwrap_or(0.0);

        Ok(ConversationData {
            messages,
            avg_response_time_hours: avg_response_time,
        })
    }

    async fn get_historical_sales_data(&self, _days_back: i32) -> Result<Vec<SalesDataPoint>, AppError> {
        // This would query actual sales data in a real implementation
        // For now, return mock data
        Ok(vec![])
    }

    async fn predict_revenue(&self, _data: &[SalesDataPoint], days_ahead: i32) -> f64 {
        // Simplified prediction - in production use ML models
        100000.0 * (days_ahead as f64 / 30.0) // $100k per month
    }

    async fn predict_deal_count(&self, _data: &[SalesDataPoint], days_ahead: i32) -> i32 {
        // Simplified prediction
        (days_ahead / 7) * 5 // 5 deals per week
    }

    async fn calculate_prediction_confidence(&self, data: &[SalesDataPoint]) -> f64 {
        // Calculate confidence based on data quality and quantity
        if data.len() < 10 {
            0.5
        } else if data.len() < 30 {
            0.7
        } else {
            0.85
        }
    }

    async fn get_prediction_factors(&self, _data: &[SalesDataPoint]) -> Vec<String> {
        vec![
            "Historical conversion rates".to_string(),
            "Seasonal trends".to_string(),
            "Pipeline velocity".to_string(),
            "Team performance".to_string(),
        ]
    }
}

#[derive(Debug)]
struct LeadScoringData {
    lead: LeadScoringRecord,
    interactions: Vec<InteractionData>,
    message_count: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct LeadScoringRecord {
    id: Uuid,
    source: String,
    status: String,
    stage: String,
    assigned_to: Option<Uuid>,
    created_at: DateTime<Utc>,
    contact_name: String,
    email: Option<String>,
    mobile: String,
    pan: Option<String>,
    contact_created_at: DateTime<Utc>,
    contact_id: Uuid,
}

#[derive(Debug, sqlx::FromRow)]
struct InteractionData {
    id: Uuid,
    channel: String,
    subject: String,
    status: String,
    priority: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug)]
struct ConversationData {
    messages: Vec<String>,
    avg_response_time_hours: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesPrediction {
    pub period_days: i32,
    pub predicted_revenue: f64,
    pub predicted_deals: i32,
    pub confidence: f64,
    pub factors: Vec<String>,
}

#[derive(Debug)]
struct SalesDataPoint {
    date: DateTime<Utc>,
    revenue: f64,
    deals: i32,
}
