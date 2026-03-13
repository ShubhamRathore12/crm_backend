-- Assignment Tracking for Round Robin
CREATE TABLE IF NOT EXISTS assignment_tracking (
    entity_type TEXT PRIMARY KEY, -- 'lead', 'interaction'
    last_agent_id UUID REFERENCES users(id),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO assignment_tracking (entity_type) VALUES ('lead'), ('interaction') ON CONFLICT DO NOTHING;
