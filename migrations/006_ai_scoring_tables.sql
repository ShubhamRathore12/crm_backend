-- AI Scoring and Analytics Tables
-- Lead scoring, conversation analysis, and predictions

-- Lead scores table
CREATE TABLE IF NOT EXISTS lead_scores (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    lead_id UUID NOT NULL REFERENCES leads(id) ON DELETE CASCADE,
    score DECIMAL(5,3) NOT NULL,
    confidence DECIMAL(5,3) NOT NULL,
    factors JSONB NOT NULL DEFAULT '{}',
    prediction JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Unique constraint to ensure one score per lead
ALTER TABLE lead_scores ADD CONSTRAINT unique_lead_score UNIQUE (lead_id);

-- Index for querying high-scoring leads
CREATE INDEX IF NOT EXISTS idx_lead_scores_score ON lead_scores(score DESC);
CREATE INDEX IF NOT EXISTS idx_lead_scores_confidence ON lead_scores(confidence DESC);

-- Conversation analyses table
CREATE TABLE IF NOT EXISTS conversation_analyses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    interaction_id UUID NOT NULL REFERENCES interactions(id) ON DELETE CASCADE,
    sentiment DECIMAL(3,2) NOT NULL, -- -1.0 to 1.0
    engagement_score DECIMAL(5,3) NOT NULL,
    key_topics JSONB NOT NULL DEFAULT '[]',
    intent_detected TEXT NOT NULL,
    next_best_action TEXT NOT NULL,
    response_suggestions JSONB NOT NULL DEFAULT '[]',
    analyzed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Unique constraint for interaction analysis
ALTER TABLE conversation_analyses ADD CONSTRAINT unique_interaction_analysis UNIQUE (interaction_id);

-- Index for sentiment analysis
CREATE INDEX IF NOT EXISTS idx_conversation_analyses_sentiment ON conversation_analyses(sentiment);
CREATE INDEX IF NOT EXISTS idx_conversation_analyses_engagement ON conversation_analyses(engagement_score);

-- Sales predictions table
CREATE TABLE IF NOT EXISTS sales_predictions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    period_days INTEGER NOT NULL,
    predicted_revenue DECIMAL(12,2) NOT NULL,
    predicted_deals INTEGER NOT NULL,
    confidence DECIMAL(5,3) NOT NULL,
    factors JSONB NOT NULL DEFAULT '[]',
    model_version TEXT NOT NULL DEFAULT 'v1.0',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for prediction history
CREATE INDEX IF NOT EXISTS idx_sales_predictions_created ON sales_predictions(created_at DESC);

-- Model retraining logs
CREATE TABLE IF NOT EXISTS model_retraining_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    model_type TEXT NOT NULL,
    status TEXT NOT NULL, -- started, completed, failed
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    error_message TEXT,
    metrics JSONB,
    model_version TEXT
);

-- Index for retraining history
CREATE INDEX IF NOT EXISTS idx_model_retraining_logs_started ON model_retraining_logs(started_at DESC);

-- AI feature flags and configuration
CREATE TABLE IF NOT EXISTS ai_configuration (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    feature_name TEXT NOT NULL UNIQUE,
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    configuration JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Insert default AI configuration
INSERT INTO ai_configuration (feature_name, configuration) VALUES
('lead_scoring', '{"auto_score": true, "min_confidence": 0.7, "update_frequency": "daily"}'),
('conversation_analysis', '{"auto_analyze": true, "sentiment_threshold": 0.1}'),
('sales_predictions', '{"enabled": true, "default_period_days": 30}'),
('response_suggestions', '{"enabled": true, "max_suggestions": 3}')
ON CONFLICT (feature_name) DO NOTHING;

-- Lead scoring factors configuration
CREATE TABLE IF NOT EXISTS lead_scoring_factors (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    factor_name TEXT NOT NULL UNIQUE,
    weight DECIMAL(5,3) NOT NULL DEFAULT 1.0,
    is_active BOOLEAN NOT NULL DEFAULT true,
    configuration JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Insert default scoring factors
INSERT INTO lead_scoring_factors (factor_name, weight, configuration) VALUES
('lead_source', 0.15, '{"website": 0.8, "referral": 0.9, "cold_email": 0.3}'),
('lead_freshness', 0.10, '{"max_age_days": 365}'),
('engagement_level', 0.25, '{"max_messages": 10}'),
('lead_status', 0.20, '{"new": 0.8, "qualified": 0.9, "converted": 1.0}'),
('interaction_history', 0.15, '{"max_interactions": 5}'),
('assignment_status', 0.15, '{"assigned": 0.8, "unassigned": 0.4}')
ON CONFLICT (factor_name) DO NOTHING;

-- Create a view for high-priority leads
CREATE OR REPLACE VIEW high_priority_leads AS
SELECT 
    l.id,
    l.contact_id,
    c.name as contact_name,
    c.email,
    c.mobile,
    l.source,
    l.status,
    l.stage,
    ls.score,
    ls.confidence,
    ls.prediction,
    l.assigned_to,
    l.created_at
FROM leads l
JOIN contacts c ON l.contact_id = c.id
JOIN lead_scores ls ON l.id = ls.lead_id
WHERE ls.score >= 0.7 AND ls.confidence >= 0.6
ORDER BY ls.score DESC, l.created_at DESC;

-- Create a view for conversation insights
CREATE OR REPLACE VIEW conversation_insights AS
SELECT 
    ca.interaction_id,
    i.contact_id,
    c.name as contact_name,
    i.channel,
    i.subject,
    ca.sentiment,
    ca.engagement_score,
    ca.key_topics,
    ca.intent_detected,
    ca.next_best_action,
    ca.response_suggestions,
    ca.analyzed_at
FROM conversation_analyses ca
JOIN interactions i ON ca.interaction_id = i.id
JOIN contacts c ON i.contact_id = c.id
ORDER BY ca.analyzed_at DESC;

-- Create a view for AI performance metrics
CREATE OR REPLACE VIEW ai_performance_metrics AS
SELECT 
    'lead_scoring' as metric_type,
    COUNT(*) as total_scores,
    AVG(score) as avg_score,
    AVG(confidence) as avg_confidence,
    COUNT(CASE WHEN score >= 0.8 THEN 1 END) as high_score_count,
    DATE(created_at) as metric_date
FROM lead_scores
GROUP BY DATE(created_at)

UNION ALL

SELECT 
    'conversation_analysis' as metric_type,
    COUNT(*) as total_analyses,
    AVG(sentiment) as avg_sentiment,
    AVG(engagement_score) as avg_engagement,
    COUNT(CASE WHEN sentiment > 0.5 THEN 1 END) as positive_sentiment_count,
    DATE(analyzed_at) as metric_date
FROM conversation_analyses
GROUP BY DATE(analyzed_at);

-- Grant necessary permissions (adjust as needed for your setup)
-- GRANT SELECT, INSERT, UPDATE, DELETE ON lead_scores TO crm_user;
-- GRANT SELECT, INSERT, UPDATE, DELETE ON conversation_analyses TO crm_user;
-- GRANT SELECT, INSERT, UPDATE, DELETE ON sales_predictions TO crm_user;
-- GRANT SELECT, INSERT, UPDATE, DELETE ON model_retraining_logs TO crm_user;
-- GRANT SELECT, INSERT, UPDATE, DELETE ON ai_configuration TO crm_user;
-- GRANT SELECT, INSERT, UPDATE, DELETE ON lead_scoring_factors TO crm_user;
-- GRANT SELECT ON high_priority_leads TO crm_user;
-- GRANT SELECT ON conversation_insights TO crm_user;
-- GRANT SELECT ON ai_performance_metrics TO crm_user;
