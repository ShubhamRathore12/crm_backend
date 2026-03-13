-- Scalability + Archiving (6 months retention in hot tables)
--
-- Strategy:
-- - Keep recent 6 months in primary tables (fast queries, small indexes)
-- - Move older data to *_archive tables using archive_old_data()
-- - Archive tables have no FKs to keep moves simple and fast

-- Helpful indexes for high-volume tables
CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_messages_interaction_id ON messages(interaction_id);
CREATE INDEX IF NOT EXISTS idx_escalations_created_at ON escalations(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_lead_history_timestamp ON lead_history(timestamp DESC);

-- Archive tables (mirror columns + archived_at)

CREATE TABLE IF NOT EXISTS leads_archive (
    id UUID,
    contact_id UUID,
    source TEXT,
    status TEXT,
    stage TEXT,
    assigned_to UUID,
    product TEXT,
    campaign TEXT,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_leads_archive_created ON leads_archive(created_at DESC);

CREATE TABLE IF NOT EXISTS lead_history_archive (
    id UUID,
    lead_id UUID,
    status TEXT,
    changed_by UUID,
    notes TEXT,
    timestamp TIMESTAMPTZ,
    archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_lead_history_archive_ts ON lead_history_archive(timestamp DESC);

CREATE TABLE IF NOT EXISTS interactions_archive (
    id UUID,
    contact_id UUID,
    channel TEXT,
    subject TEXT,
    status TEXT,
    priority TEXT,
    assigned_to UUID,
    created_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_interactions_archive_created ON interactions_archive(created_at DESC);

CREATE TABLE IF NOT EXISTS messages_archive (
    id UUID,
    interaction_id UUID,
    sender TEXT,
    content TEXT,
    channel TEXT,
    created_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_messages_archive_created ON messages_archive(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_messages_archive_interaction ON messages_archive(interaction_id);

CREATE TABLE IF NOT EXISTS escalations_archive (
    id UUID,
    interaction_id UUID,
    level INT,
    assigned_to UUID,
    deadline TIMESTAMPTZ,
    status TEXT,
    created_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_escalations_archive_created ON escalations_archive(created_at DESC);

CREATE TABLE IF NOT EXISTS workflow_runs_archive (
    id UUID,
    workflow_id UUID,
    entity_id UUID,
    entity_type TEXT,
    status TEXT,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_workflow_runs_archive_started ON workflow_runs_archive(started_at DESC);

CREATE TABLE IF NOT EXISTS email_sends_archive (
    id UUID,
    tracking_id UUID,
    to_email TEXT,
    subject TEXT,
    entity_type TEXT,
    entity_id UUID,
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_email_sends_archive_created ON email_sends_archive(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_email_sends_archive_tracking ON email_sends_archive(tracking_id);

CREATE TABLE IF NOT EXISTS meeting_invites_archive (
    id UUID,
    contact_id UUID,
    to_email TEXT,
    subject TEXT,
    calendly_link TEXT,
    email_send_id UUID,
    created_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_meeting_invites_archive_created ON meeting_invites_archive(created_at DESC);

CREATE TABLE IF NOT EXISTS bulk_email_campaigns_archive (
    id UUID,
    subject TEXT,
    body TEXT,
    total_count INT,
    sent_count INT,
    failed_count INT,
    status TEXT,
    created_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_bulk_email_campaigns_archive_created ON bulk_email_campaigns_archive(created_at DESC);

CREATE TABLE IF NOT EXISTS bulk_email_recipients_archive (
    id UUID,
    campaign_id UUID,
    email TEXT,
    status TEXT,
    error_message TEXT,
    sent_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_bulk_email_recipients_archive_campaign ON bulk_email_recipients_archive(campaign_id);

-- Archive function (moves in batches; call repeatedly or via cron)
CREATE OR REPLACE FUNCTION archive_old_data(p_months INT DEFAULT 6, p_batch INT DEFAULT 5000)
RETURNS JSONB
LANGUAGE plpgsql
AS $$
DECLARE
  cutoff TIMESTAMPTZ := NOW() - (p_months::text || ' months')::interval;
  moved_leads INT := 0;
  moved_interactions INT := 0;
  moved_messages INT := 0;
  moved_escalations INT := 0;
  moved_workflow_runs INT := 0;
  moved_email_sends INT := 0;
  moved_meeting_invites INT := 0;
  moved_bulk_campaigns INT := 0;
  moved_bulk_recipients INT := 0;
  moved_lead_history INT := 0;
BEGIN
  -- Leads + lead_history (by lead created_at)
  WITH lead_ids AS (
      SELECT id FROM leads WHERE created_at < cutoff ORDER BY created_at ASC LIMIT p_batch
  ),
  moved_hist AS (
      DELETE FROM lead_history h
      USING lead_ids i
      WHERE h.lead_id = i.id
      RETURNING h.*
  ),
  moved_leads_cte AS (
      DELETE FROM leads l
      USING lead_ids i
      WHERE l.id = i.id
      RETURNING l.*
  )
  INSERT INTO lead_history_archive (id, lead_id, status, changed_by, notes, timestamp)
  SELECT id, lead_id, status, changed_by, notes, timestamp FROM moved_hist;

  GET DIAGNOSTICS moved_lead_history = ROW_COUNT;

  INSERT INTO leads_archive (id, contact_id, source, status, stage, assigned_to, product, campaign, created_at, updated_at)
  SELECT id, contact_id, source, status, stage, assigned_to, product, campaign, created_at, updated_at FROM moved_leads_cte;

  GET DIAGNOSTICS moved_leads = ROW_COUNT;

  -- Interactions + messages + escalations (by interaction created_at)
  WITH interaction_ids AS (
      SELECT id FROM interactions WHERE created_at < cutoff ORDER BY created_at ASC LIMIT p_batch
  ),
  moved_msgs AS (
      DELETE FROM messages m
      USING interaction_ids i
      WHERE m.interaction_id = i.id
      RETURNING m.*
  ),
  moved_escalations_cte AS (
      DELETE FROM escalations e
      USING interaction_ids i
      WHERE e.interaction_id = i.id
      RETURNING e.*
  ),
  moved_interactions_cte AS (
      DELETE FROM interactions it
      USING interaction_ids i
      WHERE it.id = i.id
      RETURNING it.*
  )
  INSERT INTO messages_archive (id, interaction_id, sender, content, channel, created_at)
  SELECT id, interaction_id, sender, content, channel, created_at FROM moved_msgs;

  GET DIAGNOSTICS moved_messages = ROW_COUNT;

  INSERT INTO escalations_archive (id, interaction_id, level, assigned_to, deadline, status, created_at)
  SELECT id, interaction_id, level, assigned_to, deadline, status, created_at FROM moved_escalations_cte;

  GET DIAGNOSTICS moved_escalations = ROW_COUNT;

  INSERT INTO interactions_archive (id, contact_id, channel, subject, status, priority, assigned_to, created_at)
  SELECT id, contact_id, channel, subject, status, priority, assigned_to, created_at FROM moved_interactions_cte;

  GET DIAGNOSTICS moved_interactions = ROW_COUNT;

  -- Bulk email campaigns + recipients (by campaign created_at)
  WITH campaign_ids AS (
      SELECT id FROM bulk_email_campaigns WHERE created_at < cutoff ORDER BY created_at ASC LIMIT p_batch
  ),
  moved_recipients AS (
      DELETE FROM bulk_email_recipients r
      USING campaign_ids c
      WHERE r.campaign_id = c.id
      RETURNING r.*
  ),
  moved_campaigns AS (
      DELETE FROM bulk_email_campaigns c
      USING campaign_ids i
      WHERE c.id = i.id
      RETURNING c.*
  )
  INSERT INTO bulk_email_recipients_archive (id, campaign_id, email, status, error_message, sent_at, created_at)
  SELECT id, campaign_id, email, status, error_message, sent_at, created_at FROM moved_recipients;

  GET DIAGNOSTICS moved_bulk_recipients = ROW_COUNT;

  INSERT INTO bulk_email_campaigns_archive (id, subject, body, total_count, sent_count, failed_count, status, created_at)
  SELECT id, subject, body, total_count, sent_count, failed_count, status, created_at FROM moved_campaigns;

  GET DIAGNOSTICS moved_bulk_campaigns = ROW_COUNT;

  -- Meeting invites (by created_at)
  WITH moved AS (
      DELETE FROM meeting_invites WHERE created_at < cutoff RETURNING *
  )
  INSERT INTO meeting_invites_archive (id, contact_id, to_email, subject, calendly_link, email_send_id, created_at)
  SELECT id, contact_id, to_email, subject, calendly_link, email_send_id, created_at FROM moved;

  GET DIAGNOSTICS moved_meeting_invites = ROW_COUNT;

  -- Email sends (by created_at)
  WITH moved AS (
      DELETE FROM email_sends WHERE created_at < cutoff RETURNING *
  )
  INSERT INTO email_sends_archive (id, tracking_id, to_email, subject, entity_type, entity_id, read_at, created_at)
  SELECT id, tracking_id, to_email, subject, entity_type, entity_id, read_at, created_at FROM moved;

  GET DIAGNOSTICS moved_email_sends = ROW_COUNT;

  -- Workflow runs (by started_at)
  WITH moved AS (
      DELETE FROM workflow_runs WHERE started_at < cutoff RETURNING *
  )
  INSERT INTO workflow_runs_archive (id, workflow_id, entity_id, entity_type, status, started_at, completed_at)
  SELECT id, workflow_id, entity_id, entity_type, status, started_at, completed_at FROM moved;

  GET DIAGNOSTICS moved_workflow_runs = ROW_COUNT;

  RETURN jsonb_build_object(
      'cutoff', cutoff,
      'moved', jsonb_build_object(
          'leads', moved_leads,
          'lead_history', moved_lead_history,
          'interactions', moved_interactions,
          'messages', moved_messages,
          'escalations', moved_escalations,
          'bulk_campaigns', moved_bulk_campaigns,
          'bulk_recipients', moved_bulk_recipients,
          'meeting_invites', moved_meeting_invites,
          'email_sends', moved_email_sends,
          'workflow_runs', moved_workflow_runs
      )
  );
END;
$$;

