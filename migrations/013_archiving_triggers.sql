-- Advanced Archiving Triggers & Maintenance

-- 1. Create missing archive tables for new entities
CREATE TABLE IF NOT EXISTS opportunities_archive (
    id UUID,
    lead_id UUID,
    name TEXT,
    amount DECIMAL,
    stage TEXT,
    probability INT,
    expected_close_date DATE,
    assigned_to UUID,
    custom_fields JSONB,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_opportunities_archive_created ON opportunities_archive(created_at DESC);

CREATE TABLE IF NOT EXISTS attachments_archive (
    id UUID,
    entity_type TEXT,
    entity_id UUID,
    file_name TEXT,
    file_path TEXT,
    file_type TEXT,
    file_size INT,
    uploaded_by UUID,
    created_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_attachments_archive_entity ON attachments_archive(entity_type, entity_id);

-- 2. Update existing archive tables for custom fields
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name='leads_archive' AND column_name='custom_fields') THEN
        ALTER TABLE leads_archive ADD COLUMN custom_fields JSONB;
    END IF;
END $$;

-- 3. Enhanced Archiving Function with selective logic
-- Handles 1-month for campaigns and 6-month for resolved items
CREATE OR REPLACE FUNCTION archive_maintenance(p_batch INT DEFAULT 5000)
RETURNS JSONB
LANGUAGE plpgsql
AS $$
DECLARE
  cutoff_6m TIMESTAMPTZ := NOW() - INTERVAL '6 months';
  cutoff_1m TIMESTAMPTZ := NOW() - INTERVAL '1 month';
  moved_leads INT := 0;
  moved_interactions INT := 0;
  moved_campaigns INT := 0;
  moved_opportunities INT := 0;
BEGIN
  -- A. Archive Bulk Campaigns (> 1 month)
  WITH campaign_ids AS (
      SELECT id FROM bulk_email_campaigns 
      WHERE created_at < cutoff_1m 
      AND status IN ('completed', 'failed')
      ORDER BY created_at ASC LIMIT p_batch
  ),
  moved_recipients AS (
      DELETE FROM bulk_email_recipients r
      USING campaign_ids c
      WHERE r.campaign_id = c.id
      RETURNING r.*
  ),
  moved_camps AS (
      DELETE FROM bulk_email_campaigns c
      USING campaign_ids i
      WHERE c.id = i.id
      RETURNING c.*
  )
  INSERT INTO bulk_email_campaigns_archive (id, subject, body, total_count, sent_count, failed_count, status, created_at)
  SELECT id, subject, body, total_count, sent_count, failed_count, status, created_at FROM moved_camps;
  
  GET DIAGNOSTICS moved_campaigns = ROW_COUNT;

  -- B. Archive Resolved Leads (> 6 months)
  WITH lead_ids AS (
      SELECT id FROM leads 
      WHERE (status = 'closed' OR status = 'resolved' OR status = 'junk')
      AND updated_at < cutoff_6m 
      ORDER BY updated_at ASC LIMIT p_batch
  ),
  moved_leads_cte AS (
      DELETE FROM leads l
      USING lead_ids i
      WHERE l.id = i.id
      RETURNING l.*
  )
  INSERT INTO leads_archive (id, contact_id, source, status, stage, assigned_to, product, campaign, custom_fields, created_at, updated_at)
  SELECT id, contact_id, source, status, stage, assigned_to, product, campaign, custom_fields, created_at, updated_at FROM moved_leads_cte;

  GET DIAGNOSTICS moved_leads = ROW_COUNT;

  -- C. Archive Resolved Interactions (> 6 months)
  WITH interaction_ids AS (
      SELECT id FROM interactions 
      WHERE (status = 'resolved' OR status = 'completed')
      AND created_at < cutoff_6m 
      ORDER BY created_at ASC LIMIT p_batch
  ),
  moved_interactions_cte AS (
      DELETE FROM interactions it
      USING interaction_ids i
      WHERE it.id = i.id
      RETURNING it.*
  )
  INSERT INTO interactions_archive (id, contact_id, channel, subject, status, priority, assigned_to, created_at)
  SELECT id, contact_id, channel, subject, status, priority, assigned_to, created_at FROM moved_interactions_cte;

  GET DIAGNOSTICS moved_interactions = ROW_COUNT;

  -- D. Archive Won/Lost Opportunities (> 6 months)
  WITH opp_ids AS (
      SELECT id FROM opportunities 
      WHERE (stage = 'closed won' OR stage = 'closed lost')
      AND updated_at < cutoff_6m 
      ORDER BY updated_at ASC LIMIT p_batch
  ),
  moved_opps_cte AS (
      DELETE FROM opportunities o
      USING opp_ids i
      WHERE o.id = i.id
      RETURNING o.*
  )
  INSERT INTO opportunities_archive (id, lead_id, name, amount, stage, probability, expected_close_date, assigned_to, custom_fields, created_at, updated_at)
  SELECT id, lead_id, name, amount, stage, probability, expected_close_date, assigned_to, custom_fields, created_at, updated_at FROM moved_opps_cte;

  GET DIAGNOSTICS moved_opportunities = ROW_COUNT;

  RETURN jsonb_build_object(
      'timestamp', NOW(),
      'summary', jsonb_build_object(
          'campaigns_archived', moved_campaigns,
          'leads_archived', moved_leads,
          'interactions_archived', moved_interactions,
          'opportunities_archived', moved_opportunities
      )
  );
END;
$$;
