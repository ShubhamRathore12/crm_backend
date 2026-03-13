-- Bulk email campaigns: track each send and per-recipient status

CREATE TABLE bulk_email_campaigns (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    subject TEXT NOT NULL,
    body TEXT NOT NULL,
    total_count INT NOT NULL DEFAULT 0,
    sent_count INT NOT NULL DEFAULT 0,
    failed_count INT NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'pending',  -- pending | sending | completed | failed
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE bulk_email_recipients (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    campaign_id UUID NOT NULL REFERENCES bulk_email_campaigns(id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',  -- pending | sent | failed
    error_message TEXT,
    sent_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_bulk_email_recipients_campaign ON bulk_email_recipients(campaign_id);
CREATE INDEX idx_bulk_email_campaigns_created ON bulk_email_campaigns(created_at DESC);
