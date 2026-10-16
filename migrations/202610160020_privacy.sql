-- Identity may be removed while immutable game settlements retain their UUID references.
ALTER TABLE users ALTER COLUMN email DROP NOT NULL;
ALTER TABLE users ADD CONSTRAINT users_live_email CHECK(status='deleted' OR email IS NOT NULL);
ALTER TABLE users DROP CONSTRAINT users_login_format;
UPDATE users SET login='deleted_'||replace(id::text,'-',''),email=NULL,
    password_hash='$argon2id$erased-unrecoverable-account' WHERE status='deleted';
ALTER TABLE users ADD CONSTRAINT users_login_format CHECK(
    (status='deleted' AND login::text='deleted_'||replace(id::text,'-','')) OR
    (status<>'deleted' AND login::text COLLATE "C" ~ '^[A-Za-z0-9_-]{3,20}$')
);
CREATE INDEX users_deletion_due_idx ON users(deletion_requested_at,id) WHERE status='pending_deletion';
ALTER TABLE users ADD COLUMN deletion_previous_status text CHECK(deletion_previous_status IN ('active','banned'));
CREATE TABLE privacy_events (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id),
    event text NOT NULL CHECK(event IN ('requested','restored','erased')),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX privacy_events_user_idx ON privacy_events(user_id,created_at);
