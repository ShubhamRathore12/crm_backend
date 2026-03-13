-- Opportunities and Media Storage

-- Opportunities (Linked to Leads)
CREATE TABLE IF NOT EXISTS opportunities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    lead_id UUID NOT NULL REFERENCES leads(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT,
    value DECIMAL(12, 2) DEFAULT 0.00,
    currency TEXT DEFAULT 'USD',
    stage TEXT NOT NULL DEFAULT 'discovery', -- discovery, proposal, negotiation, won, lost
    probability INT DEFAULT 10, -- 0 to 100
    expected_closed_at TIMESTAMPTZ,
    assigned_to UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_opportunities_lead ON opportunities(lead_id);
CREATE INDEX idx_opportunities_stage ON opportunities(stage);
CREATE INDEX idx_opportunities_assigned ON opportunities(assigned_to);

-- Generic Attachments (Polymorphism using entity_type and entity_id)
CREATE TABLE IF NOT EXISTS attachments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    file_name TEXT NOT NULL,
    file_type TEXT NOT NULL, -- mime type
    file_size INT NOT NULL,  -- in bytes
    file_path TEXT NOT NULL, -- local path or S3 key
    entity_type TEXT NOT NULL, -- 'lead', 'contact', 'interaction', 'opportunity', 'task'
    entity_id UUID NOT NULL,
    uploaded_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_attachments_entity ON attachments(entity_type, entity_id);

-- Bulk Upload Tracking
CREATE TABLE IF NOT EXISTS bulk_uploads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    file_name TEXT NOT NULL,
    entity_type TEXT NOT NULL, -- 'contact', 'lead'
    status TEXT NOT NULL DEFAULT 'pending', -- pending, processing, completed, failed
    total_rows INT DEFAULT 0,
    processed_rows INT DEFAULT 0,
    failed_rows INT DEFAULT 0,
    error_log TEXT,
    created_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX idx_bulk_uploads_status ON bulk_uploads(status);
CREATE INDEX idx_bulk_uploads_creator ON bulk_uploads(created_by);
