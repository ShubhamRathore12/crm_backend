-- Sales & Marketing Tasks
CREATE TABLE sales_marketing_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title TEXT NOT NULL,
    description TEXT DEFAULT '',
    status TEXT NOT NULL DEFAULT 'todo'
        CHECK (status IN ('todo', 'in_progress', 'ready_for_launch', 'launched', 'completed')),
    priority TEXT NOT NULL DEFAULT 'medium'
        CHECK (priority IN ('low', 'medium', 'high', 'critical')),
    assignee_id UUID REFERENCES users(id),
    tags TEXT[] DEFAULT '{}',
    start_date DATE,
    end_date DATE,
    estimated_hours REAL DEFAULT 0,
    effort_hours REAL DEFAULT 0,
    category TEXT DEFAULT '',
    department TEXT DEFAULT '',
    parent_task_id UUID REFERENCES sales_marketing_tasks(id) ON DELETE SET NULL,
    created_by UUID REFERENCES users(id),
    updated_by UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_smt_status ON sales_marketing_tasks(status);
CREATE INDEX idx_smt_priority ON sales_marketing_tasks(priority);
CREATE INDEX idx_smt_assignee ON sales_marketing_tasks(assignee_id);
CREATE INDEX idx_smt_parent ON sales_marketing_tasks(parent_task_id);
CREATE INDEX idx_smt_dates ON sales_marketing_tasks(start_date, end_date);
