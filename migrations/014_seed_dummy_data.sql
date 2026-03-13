-- Seed dummy data for SMC CRM
-- Run this after the main migrations

-- Create teams first
INSERT INTO teams (id, name, manager_id, created_at) VALUES
('a0000000-0000-0000-0000-000000000001', 'Sales Team Alpha', NULL, NOW()),
('a0000000-0000-0000-0000-000000000002', 'Marketing Team', NULL, NOW()),
('a0000000-0000-0000-0000-000000000003', 'Support Team', NULL, NOW());

-- Create users with password 'password123' (bcrypt hash)
INSERT INTO users (id, name, email, password_hash, role, team_id, status, created_at) VALUES
('b0000000-0000-0000-0000-000000000001', 'Admin User', 'admin@smc.com', '$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/X4aYJGYxMnC6C5.Oy', 'admin', NULL, 'active', NOW()),
('b0000000-0000-0000-0000-000000000002', 'John Agent', 'john@smc.com', '$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/X4aYJGYxMnC6C5.Oy', 'agent', 'a0000000-0000-0000-0000-000000000001', 'active', NOW()),
('b0000000-0000-0000-0000-000000000003', 'Jane Agent', 'jane@smc.com', '$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/X4aYJGYxMnC6C5.Oy', 'agent', 'a0000000-0000-0000-0000-000000000001', 'active', NOW()),
('b0000000-0000-0000-0000-000000000004', 'Mike Sales', 'mike@smc.com', '$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/X4aYJGYxMnC6C5.Oy', 'agent', 'a0000000-0000-0000-0000-000000000002', 'active', NOW()),
('b0000000-0000-0000-0000-000000000005', 'Sarah Support', 'sarah@smc.com', '$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/X4aYJGYxMnC6C5.Oy', 'agent', 'a0000000-0000-0000-0000-000000000003', 'active', NOW());

-- Update team managers
UPDATE teams SET manager_id = 'b0000000-0000-0000-0000-000000000002' WHERE id = 'a0000000-0000-0000-0000-000000000001';
UPDATE teams SET manager_id = 'b0000000-0000-0000-0000-000000000004' WHERE id = 'a0000000-0000-0000-0000-000000000002';
UPDATE teams SET manager_id = 'b0000000-0000-0000-0000-000000000005' WHERE id = 'a0000000-0000-0000-0000-000000000003';

-- Create sample contacts
INSERT INTO contacts (id, ucc_code, name, mobile, email, pan, address, custom_fields, created_at) VALUES
('c0000000-0000-0000-0000-000000000001', 'UCC001', 'Alice Johnson', '+91-9876543210', 'alice@example.com', 'ABCDE1234F', '123 Main St, Mumbai', '{}', NOW()),
('c0000000-0000-0000-0000-000000000002', 'UCC002', 'Bob Smith', '+91-9876543211', 'bob@example.com', 'BCDEF2345F', '456 Oak Ave, Delhi', '{}', NOW()),
('c0000000-0000-0000-0000-000000000003', 'UCC003', 'Carol Williams', '+91-9876543212', 'carol@example.com', 'CDEFG3456F', '789 Pine Rd, Bangalore', '{}', NOW()),
('c0000000-0000-0000-0000-000000000004', 'UCC004', 'David Brown', '+91-9876543213', 'david@example.com', 'DEFGH4567F', '321 Elm St, Chennai', '{}', NOW()),
('c0000000-0000-0000-0000-000000000005', 'UCC005', 'Eva Davis', '+91-9876543214', 'eva@example.com', 'EFGHI5678F', '654 Maple Dr, Hyderabad', '{}', NOW());

-- Create sample leads
INSERT INTO leads (id, contact_id, source, status, stage, assigned_to, product, campaign, custom_fields, created_at, updated_at) VALUES
('d0000000-0000-0000-0000-000000000001', 'c0000000-0000-0000-0000-000000000001', 'website', 'new', 'qualified', 'b0000000-0000-0000-0000-000000000002', 'Premium Plan', 'Q1 Campaign', '{}', NOW(), NOW()),
('d0000000-0000-0000-0000-000000000002', 'c0000000-0000-0000-0000-000000000002', 'referral', 'contacted', 'proposal', 'b0000000-0000-0000-0000-000000000003', 'Enterprise Plan', 'Partner Referral', '{}', NOW(), NOW()),
('d0000000-0000-0000-0000-000000000003', 'c0000000-0000-0000-0000-000000000003', 'social', 'qualified', 'discovery', 'b0000000-0000-0000-0000-000000000002', 'Basic Plan', 'LinkedIn Ads', '{}', NOW(), NOW()),
('d0000000-0000-0000-0000-000000000004', 'c0000000-0000-0000-0000-000000000004', 'email', 'converted', 'negotiation', 'b0000000-0000-0000-0000-000000000004', 'Premium Plan', 'Email Newsletter', '{}', NOW(), NOW()),
('d0000000-0000-0000-0000-000000000005', 'c0000000-0000-0000-0000-000000000005', 'website', 'new', 'qualified', 'b0000000-0000-0000-0000-000000000003', 'Enterprise Plan', 'Google Ads', '{}', NOW(), NOW());

-- Create sample interactions
INSERT INTO interactions (id, contact_id, channel, subject, status, priority, assigned_to, created_at) VALUES
('e0000000-0000-0000-0000-000000000001', 'c0000000-0000-0000-0000-000000000001', 'email', 'Product inquiry', 'new', 'high', 'b0000000-0000-0000-0000-000000000002', NOW()),
('e0000000-0000-0000-0000-000000000002', 'c0000000-0000-0000-0000-000000000002', 'phone', 'Follow-up call', 'in_progress', 'medium', 'b0000000-0000-0000-0000-000000000003', NOW()),
('e0000000-0000-0000-0000-000000000003', 'c0000000-0000-0000-0000-000000000003', 'whatsapp', 'Demo request', 'resolved', 'low', 'b0000000-0000-0000-0000-000000000002', NOW());

-- Create sample opportunities
INSERT INTO opportunities (id, lead_id, title, description, value, currency, stage, probability, expected_closed_at, assigned_to, created_at, updated_at) VALUES
('f0000000-0000-0000-0000-000000000001', 'd0000000-0000-0000-0000-000000000001', 'Enterprise Deal - Alice', 'Annual enterprise contract', '50000', 'INR', 'proposal', 60, '2026-06-30', 'b0000000-0000-0000-0000-000000000002', NOW(), NOW()),
('f0000000-0000-0000-0000-000000000002', 'd0000000-0000-0000-0000-000000000004', 'Premium Upgrade - David', 'Upgrade from basic to premium', '12000', 'INR', 'negotiation', 80, '2026-04-15', 'b0000000-0000-0000-0000-000000000004', NOW(), NOW());

SELECT 'Dummy data seeded successfully!' as status;

-- Create lead history
INSERT INTO lead_history (id, lead_id, status, changed_by, notes, timestamp) VALUES
('h0000000-0000-0000-0000-000000000001', 'd0000000-0000-0000-0000-000000000001', 'new', 'b0000000-0000-0000-0000-000000000002', 'Lead created from website', NOW()),
('h0000000-0000-0000-0000-000000000002', 'd0000000-0000-0000-0000-000000000002', 'contacted', 'b0000000-0000-0000-0000-000000000003', 'Initial contact made', NOW() - INTERVAL '2 days'),
('h0000000-0000-0000-0000-000000000003', 'd0000000-0000-0000-0000-000000000004', 'converted', 'b0000000-0000-0000-0000-000000000004', 'Lead converted to customer', NOW() - INTERVAL '5 days');

-- Messages for interactions
INSERT INTO messages (id, interaction_id, sender, content, channel, created_at) VALUES
('m0000000-0000-0000-0000-000000000001', 'e0000000-0000-0000-0000-000000000001', 'John Agent', 'Hi Alice, thanks for reaching out!', 'email', NOW()),
('m0000000-0000-0000-0000-000000000002', 'e0000000-0000-0000-0000-000000000001', 'Alice Johnson', 'Can you tell me more about pricing?', 'email', NOW() + INTERVAL '1 hour'),
('m0000000-0000-0000-0000-000000000003', 'e0000000-0000-0000-0000-000000000002', 'Jane Agent', 'Bob, let me schedule a call with you', 'phone', NOW());

-- Escalations
INSERT INTO escalations (id, interaction_id, escalated_by, escalated_to, reason, created_at) VALUES
('esc00000-0000-0000-0000-000000000001', 'e0000000-0000-0000-0000-000000000001', 'b0000000-0000-0000-0000-000000000002', 'b0000000-0000-0000-0000-000000000001', 'High priority customer', NOW());

-- Tasks
INSERT INTO tasks (id, title, description, entity_type, entity_id, assigned_to, status, priority, due_date, created_at) VALUES
('t0000000-0000-0000-0000-000000000001', 'Follow up with Alice', 'Send proposal to Alice Johnson', 'lead', 'd0000000-0000-0000-0000-000000000001', 'b0000000-0000-0000-0000-000000000002', 'pending', 'high', NOW() + INTERVAL '2 days', NOW()),
('t0000000-0000-0000-0000-000000000002', 'Schedule demo for Bob', 'Schedule product demo', 'lead', 'd0000000-0000-0000-0000-000000000002', 'b0000000-0000-0000-0000-000000000003', 'in_progress', 'medium', NOW() + INTERVAL '1 day', NOW()),
('t0000000-0000-0000-0000-000000000003', 'Close Carol deal', 'Finalize contract with Carol', 'opportunity', 'f0000000-0000-0000-0000-000000000001', 'b0000000-0000-0000-0000-000000000002', 'pending', 'critical', NOW() + INTERVAL '5 days', NOW());