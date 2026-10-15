-- F6.4: persistent social data; local guests never create rows.
CREATE TABLE friends (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    friend_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY(user_id,friend_id),
    CHECK (user_id <> friend_id)
);
CREATE INDEX friends_reverse_idx ON friends(friend_id);
CREATE TABLE ignores (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ignored_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY(user_id,ignored_id),
    CHECK (user_id <> ignored_id)
);
CREATE INDEX ignores_reverse_idx ON ignores(ignored_id);
CREATE TABLE mail (
    id uuid PRIMARY KEY,
    from_user uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    to_user uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    body text NOT NULL CHECK (char_length(btrim(body)) BETWEEN 1 AND 1000),
    created_at timestamptz NOT NULL DEFAULT now(),
    read_at timestamptz,
    deleted_by_sender boolean NOT NULL DEFAULT false,
    deleted_by_recipient boolean NOT NULL DEFAULT false,
    CHECK (from_user <> to_user)
);
CREATE INDEX mail_inbox_idx ON mail(to_user,id DESC) WHERE NOT deleted_by_recipient;
CREATE INDEX mail_sent_idx ON mail(from_user,id DESC) WHERE NOT deleted_by_sender;
-- Membership is the authorization boundary for private clan rooms/chat. Clan management is F11.
CREATE TABLE clans (
    id uuid PRIMARY KEY,
    name citext NOT NULL UNIQUE CHECK (char_length(name::text) BETWEEN 3 AND 40),
    status text NOT NULL DEFAULT 'founding' CHECK (status IN ('founding','active','disbanded')),
    description text NOT NULL DEFAULT '' CHECK (char_length(description)<=1000),
    motd text NOT NULL DEFAULT '' CHECK (char_length(motd)<=300),
    www text NOT NULL DEFAULT '', email text NOT NULL DEFAULT '',
    created_at timestamptz NOT NULL DEFAULT now(), founded_at timestamptz
);
CREATE TABLE clan_members (
    clan_id uuid NOT NULL REFERENCES clans(id) ON DELETE CASCADE,
    user_id uuid NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    role text NOT NULL DEFAULT 'member' CHECK (role IN ('master','vice','scribe','member')),
    title text NOT NULL DEFAULT '' CHECK (char_length(title)<=64),
    joined_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY(clan_id,user_id)
);
CREATE TABLE chat_log (
    id uuid PRIMARY KEY,
    room text NOT NULL,
    from_user uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    to_user uuid REFERENCES users(id) ON DELETE CASCADE,
    clan_id uuid REFERENCES clans(id) ON DELETE CASCADE,
    text text NOT NULL CHECK (char_length(btrim(text)) BETWEEN 1 AND 300),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX chat_log_retention_idx ON chat_log(created_at);
