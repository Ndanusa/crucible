-- Add audit_logs table for audit logging service
CREATE TABLE IF NOT EXISTS audit_logs (
    id SERIAL PRIMARY KEY,
    event_type TEXT NOT NULL,
    user_id TEXT,
    details JSONB NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
