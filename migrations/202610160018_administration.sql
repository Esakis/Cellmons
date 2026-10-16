ALTER TABLE users ADD COLUMN muted_until timestamptz, ADD COLUMN banned_until timestamptz;
CREATE TABLE admin_totp (
    user_id uuid PRIMARY KEY REFERENCES users(id),
    secret_cipher bytea CHECK(octet_length(secret_cipher)=48),
    pending_cipher bytea CHECK(octet_length(pending_cipher)=48),
    pending_until_ms bigint NOT NULL DEFAULT 0,
    last_step bigint NOT NULL DEFAULT -1 CHECK(last_step>=-1),
    revision integer NOT NULL DEFAULT 0 CHECK(revision>=0)
);
CREATE TABLE admin_sessions (
    session_hash bytea PRIMARY KEY REFERENCES sessions(id_hash) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES users(id),
    totp_revision integer NOT NULL CHECK(totp_revision>=0),
    ip_hash bytea NOT NULL CHECK(octet_length(ip_hash)=32),
    expires_at timestamptz NOT NULL
);
CREATE INDEX admin_sessions_user_idx ON admin_sessions(user_id);
CREATE TABLE admin_audit (
    id uuid PRIMARY KEY,
    actor uuid REFERENCES users(id),
    action text NOT NULL CHECK(char_length(action) BETWEEN 1 AND 40),
    target uuid,
    request_id uuid,
    reason text NOT NULL DEFAULT '' CHECK(char_length(reason)<=2000),
    details bytea NOT NULL CHECK(octet_length(details)<=32768),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX admin_audit_recent_idx ON admin_audit(created_at DESC);
CREATE FUNCTION admin_audit_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'administration audit is append only' USING ERRCODE='23514'; END $$;
CREATE TRIGGER admin_audit_immutable BEFORE UPDATE OR DELETE ON admin_audit
FOR EACH ROW EXECUTE FUNCTION admin_audit_immutable();
CREATE TABLE moderation_reports (
    id uuid PRIMARY KEY,
    reporter uuid NOT NULL REFERENCES users(id),
    target_kind text NOT NULL CHECK(target_kind IN ('chat','player','clan')),
    target_id uuid NOT NULL,
    reason text NOT NULL CHECK(char_length(reason) BETWEEN 5 AND 2000),
    context bytea NOT NULL CHECK(octet_length(context)<=32768),
    status text NOT NULL DEFAULT 'open' CHECK(status IN ('open','reviewed','rejected')),
    resolved_by uuid REFERENCES users(id),
    resolution text NOT NULL DEFAULT '' CHECK(char_length(resolution)<=2000),
    resolved_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX moderation_reports_open_idx ON moderation_reports(status,created_at DESC);
CREATE TABLE word_filters (
    id uuid PRIMARY KEY,
    phrase text NOT NULL UNIQUE CHECK(char_length(phrase) BETWEEN 2 AND 80),
    enabled boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE announcements (
    id uuid PRIMARY KEY,
    author uuid NOT NULL REFERENCES users(id),
    body text NOT NULL CHECK(char_length(body) BETWEEN 1 AND 1000),
    enabled boolean NOT NULL DEFAULT true,
    expires_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE server_controls (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
    maintenance boolean NOT NULL DEFAULT false,
    notice text NOT NULL DEFAULT '' CHECK(char_length(notice)<=1000),
    revision integer NOT NULL DEFAULT 0 CHECK(revision>=0)
);
INSERT INTO server_controls(singleton) VALUES (true);
CREATE TABLE user_notices (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id),
    kind text NOT NULL CHECK(kind IN ('warning','ban','unban','mute','unmute','report','role')),
    body text NOT NULL CHECK(char_length(body) BETWEEN 1 AND 2000),
    read_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX user_notices_user_idx ON user_notices(user_id,created_at DESC);
CREATE TABLE abuse_signals (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id),
    kind text NOT NULL CHECK(kind IN ('duels','accounts','trade','economy','automation')),
    day date NOT NULL,
    score integer NOT NULL CHECK(score>0),
    details bytea NOT NULL CHECK(octet_length(details)<=32768),
    reviewed boolean NOT NULL DEFAULT false,
    updated_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(user_id,kind,day)
);
CREATE TABLE admin_requests (
    actor uuid NOT NULL REFERENCES users(id),
    request_id uuid NOT NULL,
    fingerprint bytea NOT NULL CHECK(octet_length(fingerprint)=32),
    PRIMARY KEY(actor,request_id)
);
ALTER TABLE tournaments ADD COLUMN system_reward bytea CHECK(octet_length(system_reward)<=4096);
