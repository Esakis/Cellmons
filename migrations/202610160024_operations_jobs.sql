CREATE TABLE operation_jobs (
    name text PRIMARY KEY CHECK(name IN ('maintenance','backup','restore-drill')),
    started_at timestamptz NOT NULL,
    finished_at timestamptz,
    last_success_at timestamptz,
    success boolean NOT NULL DEFAULT false,
    summary jsonb NOT NULL DEFAULT '{}'::jsonb CHECK(octet_length(summary::text)<=8192)
);
CREATE INDEX admin_sessions_expiry_idx ON admin_sessions(expires_at);
