-- F6.1: identity and session storage. IDs are UUIDv7 supplied by the application.
CREATE EXTENSION IF NOT EXISTS citext;

CREATE TABLE users (
    id uuid PRIMARY KEY,
    login citext NOT NULL UNIQUE,
    email citext NOT NULL UNIQUE,
    password_hash text NOT NULL,
    email_verified_at timestamptz,
    role text NOT NULL DEFAULT 'player',
    status text NOT NULL DEFAULT 'active',
    locale text NOT NULL DEFAULT 'pl',
    start_school text NOT NULL,
    challenge_filter text NOT NULL DEFAULT 'all',
    description text NOT NULL DEFAULT '',
    created_at timestamptz NOT NULL DEFAULT now(),
    last_login_at timestamptz,
    deletion_requested_at timestamptz,
    CONSTRAINT users_login_format CHECK (login::text COLLATE "C" ~ '^[A-Za-z0-9_-]{3,20}$'),
    CONSTRAINT users_email_format CHECK (
        char_length(email::text) BETWEEN 3 AND 254
        AND email::text = btrim(email::text)
        AND position('@' IN email::text) > 1
        AND email::text !~ '[[:space:]]'
    ),
    CONSTRAINT users_password_hash_format CHECK (
        char_length(password_hash) BETWEEN 30 AND 1024
        AND starts_with(password_hash, '$argon2id$')
    ),
    CONSTRAINT users_role CHECK (role IN ('player', 'mentor', 'moderator', 'admin')),
    CONSTRAINT users_status CHECK (status IN ('active', 'banned', 'pending_deletion', 'deleted')),
    CONSTRAINT users_locale CHECK (locale IN ('pl', 'en')),
    CONSTRAINT users_start_school CHECK (start_school IN ('combat', 'divine', 'element', 'arcane', 'necro', 'tech')),
    CONSTRAINT users_challenge_filter CHECK (challenge_filter IN ('all', 'level_pm2', 'none')),
    CONSTRAINT users_description_length CHECK (char_length(description) <= 300)
);

CREATE TABLE sessions (
    id_hash bytea PRIMARY KEY,
    kind text NOT NULL,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    rotated_at timestamptz,
    device_label text,
    user_agent text NOT NULL DEFAULT '',
    ip_hash bytea,
    CONSTRAINT sessions_id_hash_length CHECK (octet_length(id_hash) = 32),
    CONSTRAINT sessions_kind CHECK (kind IN ('web', 'app')),
    CONSTRAINT sessions_lifetime CHECK (expires_at > created_at),
    CONSTRAINT sessions_seen_at CHECK (last_seen_at >= created_at),
    CONSTRAINT sessions_rotated_at CHECK (rotated_at IS NULL OR rotated_at >= created_at),
    CONSTRAINT sessions_device_length CHECK (char_length(device_label) <= 100),
    CONSTRAINT sessions_agent_length CHECK (char_length(user_agent) <= 1024),
    CONSTRAINT sessions_ip_hash_length CHECK (ip_hash IS NULL OR octet_length(ip_hash) = 32)
);
CREATE INDEX sessions_user_id_idx ON sessions(user_id);
CREATE INDEX sessions_expires_at_idx ON sessions(expires_at);

CREATE TABLE email_tokens (
    token_hash bytea PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    used_at timestamptz,
    CONSTRAINT email_tokens_hash_length CHECK (octet_length(token_hash) = 32),
    CONSTRAINT email_tokens_kind CHECK (kind IN ('verify', 'reset')),
    CONSTRAINT email_tokens_lifetime CHECK (expires_at > created_at),
    CONSTRAINT email_tokens_used_at CHECK (used_at IS NULL OR used_at >= created_at)
);
CREATE INDEX email_tokens_user_kind_idx ON email_tokens(user_id, kind);
CREATE INDEX email_tokens_expires_at_idx ON email_tokens(expires_at);
