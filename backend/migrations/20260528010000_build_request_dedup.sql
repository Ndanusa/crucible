CREATE TABLE IF NOT EXISTS build_request_dedup (
    request_id UUID PRIMARY KEY,
    fingerprint TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    request_payload JSONB NOT NULL,
    response_payload JSONB,
    duplicate_count BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT build_request_dedup_status_check
        CHECK (status IN ('accepted', 'completed', 'failed'))
);

CREATE INDEX IF NOT EXISTS idx_build_request_dedup_expires_at
    ON build_request_dedup (expires_at);

CREATE INDEX IF NOT EXISTS idx_build_request_dedup_status
    ON build_request_dedup (status);
