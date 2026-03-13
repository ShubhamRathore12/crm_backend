-- Workflow Engine Enhancements
-- Add missing tables for workflow execution and task management

-- Tasks table for workflow-created tasks
CREATE TABLE IF NOT EXISTS tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title TEXT NOT NULL,
    description TEXT,
    assigned_to UUID REFERENCES users(id),
    entity_id UUID NOT NULL,
    entity_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    priority TEXT DEFAULT 'medium',
    due_date TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_tasks_assigned ON tasks(assigned_to);
CREATE INDEX IF NOT EXISTS idx_tasks_entity ON tasks(entity_id, entity_type);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);

-- Workflow node execution history for detailed tracking
CREATE TABLE IF NOT EXISTS workflow_node_executions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_run_id UUID NOT NULL REFERENCES workflow_runs(id),
    node_id TEXT NOT NULL,
    node_type TEXT NOT NULL,
    status TEXT NOT NULL,
    result JSONB,
    error_message TEXT,
    executed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_workflow_node_executions_run ON workflow_node_executions(workflow_run_id);

-- Workflow triggers configuration
CREATE TABLE IF NOT EXISTS workflow_triggers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_id UUID NOT NULL REFERENCES workflows(id),
    trigger_type TEXT NOT NULL,
    trigger_config JSONB NOT NULL DEFAULT '{}',
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_workflow_triggers_workflow ON workflow_triggers(workflow_id);
CREATE INDEX IF NOT EXISTS idx_workflow_triggers_type ON workflow_triggers(trigger_type);

-- Workflow schedules for time-based triggers
CREATE TABLE IF NOT EXISTS workflow_schedules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_id UUID NOT NULL REFERENCES workflows(id),
    schedule_type TEXT NOT NULL, -- 'cron', 'interval', 'once'
    schedule_config JSONB NOT NULL DEFAULT '{}',
    next_run TIMESTAMPTZ,
    active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_run TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_workflow_schedules_next_run ON workflow_schedules(next_run);
CREATE INDEX IF NOT EXISTS idx_workflow_schedules_workflow ON workflow_schedules(workflow_id);

-- Add tenant_id support for multi-tenancy
ALTER TABLE workflows ADD COLUMN IF NOT EXISTS tenant_id UUID;
ALTER TABLE workflow_runs ADD COLUMN IF NOT EXISTS tenant_id UUID;
ALTER TABLE tasks ADD COLUMN IF NOT EXISTS tenant_id UUID;
ALTER TABLE workflow_triggers ADD COLUMN IF NOT EXISTS tenant_id UUID;
ALTER TABLE workflow_schedules ADD COLUMN IF NOT EXISTS tenant_id UUID;

-- Add workflow categories for better organization
ALTER TABLE workflows ADD COLUMN IF NOT EXISTS category TEXT DEFAULT 'general';

-- Add workflow priority
ALTER TABLE workflows ADD COLUMN IF NOT EXISTS priority TEXT DEFAULT 'medium';

-- Add workflow versioning
ALTER TABLE workflows ADD COLUMN IF NOT EXISTS version INTEGER DEFAULT 1;

-- Add workflow description
ALTER TABLE workflows ADD COLUMN IF NOT EXISTS description TEXT;

-- Create workflow execution statistics view
CREATE OR REPLACE VIEW workflow_stats AS
SELECT 
    w.id,
    w.name,
    w.category,
    w.active,
    COUNT(wr.id) as total_runs,
    COUNT(CASE WHEN wr.status = 'completed' THEN 1 END) as successful_runs,
    COUNT(CASE WHEN wr.status = 'failed' THEN 1 END) as failed_runs,
    AVG(EXTRACT(EPOCH FROM (wr.completed_at - wr.started_at))) as avg_execution_time_seconds
FROM workflows w
LEFT JOIN workflow_runs wr ON w.id = wr.workflow_id
GROUP BY w.id, w.name, w.category, w.active;

-- Insert some default workflow triggers
INSERT INTO workflow_triggers (workflow_id, trigger_type, trigger_config) 
SELECT 
    id,
    'entity_created',
    '{"entity_type": "lead"}'::jsonb
FROM workflows 
WHERE NOT EXISTS (
    SELECT 1 FROM workflow_triggers wt 
    WHERE wt.workflow_id = workflows.id AND wt.trigger_type = 'entity_created'
);

-- Grant necessary permissions (adjust as needed for your setup)
-- GRANT SELECT, INSERT, UPDATE, DELETE ON tasks TO crm_user;
-- GRANT SELECT, INSERT, UPDATE, DELETE ON workflow_node_executions TO crm_user;
-- GRANT SELECT, INSERT, UPDATE, DELETE ON workflow_triggers TO crm_user;
-- GRANT SELECT, INSERT, UPDATE, DELETE ON workflow_schedules TO crm_user;
-- GRANT SELECT ON workflow_stats TO crm_user;
