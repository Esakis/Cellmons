-- Managed accounts use the same characters, collections and public profiles as players.
-- The control metadata is private to the server; it is never included in PublicPlayer.
CREATE TABLE bot_accounts (
    user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    roster_key text NOT NULL UNIQUE,
    ai_profile text NOT NULL CHECK (ai_profile IN ('novice', 'apprentice', 'tactician', 'archmage')),
    enabled boolean NOT NULL DEFAULT true,
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK (roster_key ~ '^[a-z0-9_]{3,32}$')
);
