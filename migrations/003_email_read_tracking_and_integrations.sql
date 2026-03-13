-- Email read tracking: each sent email gets a tracking_id; when recipient opens,
-- GET /email/open/:tracking_id is hit (tracking pixel) and read_at is set.

CREATE TABLE email_sends (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tracking_id UUID NOT NULL UNIQUE,
    to_email TEXT NOT NULL,
    subject TEXT NOT NULL,
    entity_type TEXT,  -- 'contact' | 'lead' | 'interaction' | 'meeting_invite' | 'bulk'
    entity_id UUID,
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_email_sends_tracking ON email_sends(tracking_id);
CREATE INDEX idx_email_sends_to_email ON email_sends(to_email);
CREATE INDEX idx_email_sends_entity ON email_sends(entity_type, entity_id);
CREATE INDEX idx_email_sends_created ON email_sends(created_at DESC);

-- Integrations: Zapier (outbound webhooks), Slack, Calendly, etc.

CREATE TABLE integration_connections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider TEXT NOT NULL,  -- 'zapier' | 'slack' | 'calendly'
    name TEXT,
    config JSONB NOT NULL DEFAULT '{}',  -- webhook_url, channel_id, access_token, etc.
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_integration_connections_provider ON integration_connections(provider);

-- Meeting invites: log when we send a meeting invite (e.g. with Calendly link) to a contact

CREATE TABLE meeting_invites (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    contact_id UUID REFERENCES contacts(id),
    to_email TEXT NOT NULL,
    subject TEXT NOT NULL,
    calendly_link TEXT,
    email_send_id UUID REFERENCES email_sends(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_meeting_invites_contact ON meeting_invites(contact_id);
CREATE INDEX idx_meeting_invites_created ON meeting_invites(created_at DESC);
