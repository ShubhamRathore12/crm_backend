-- Dynamic Fields and CTI Call Logs

-- Field Definitions (for UI-driven form building)
CREATE TABLE IF NOT EXISTS field_definitions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type TEXT NOT NULL, -- 'lead', 'contact', 'opportunity'
    field_name TEXT NOT NULL,  -- the key in JSONB
    label TEXT NOT NULL,       -- UI label
    field_type TEXT NOT NULL,  -- 'text', 'number', 'date', 'select', 'boolean'
    options JSONB,             -- for select/radio
    is_required BOOLEAN DEFAULT false,
    is_system BOOLEAN DEFAULT false, -- system fields cannot be deleted
    display_order INT DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(entity_type, field_name)
);

-- CTI Call Logs
CREATE TABLE IF NOT EXISTS call_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    call_id TEXT UNIQUE,
    direction TEXT NOT NULL, -- 'inbound', 'outbound'
    from_number TEXT NOT NULL,
    to_number TEXT NOT NULL,
    duration_seconds INT,
    status TEXT NOT NULL, -- 'missed', 'completed', 'busy'
    agent_id UUID REFERENCES users(id),
    contact_id UUID REFERENCES contacts(id),
    lead_id UUID REFERENCES leads(id),
    recording_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_call_logs_agent ON call_logs(agent_id);
CREATE INDEX idx_call_logs_contact ON call_logs(contact_id);
CREATE INDEX idx_call_logs_created ON call_logs(created_at DESC);

-- Add custom_fields to core tables
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name='leads' AND column_name='custom_fields') THEN
        ALTER TABLE leads ADD COLUMN custom_fields JSONB NOT NULL DEFAULT '{}';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name='contacts' AND column_name='custom_fields') THEN
        ALTER TABLE contacts ADD COLUMN custom_fields JSONB NOT NULL DEFAULT '{}';
    END IF;
END $$;
