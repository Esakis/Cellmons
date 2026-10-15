-- F7: private durable input journal; never exposed directly to a browser.
CREATE TABLE duel_journal (
    id uuid PRIMARY KEY,
    kind text NOT NULL CHECK (kind IN ('arena','training','tournament','expedition')),
    status text NOT NULL DEFAULT 'active' CHECK (status IN ('active','finished')),
    engine_version text NOT NULL,
    content_version bytea NOT NULL CHECK (octet_length(content_version)=32),
    data bytea NOT NULL CHECK (octet_length(data) BETWEEN 1 AND 1048576),
    revision bigint NOT NULL DEFAULT 0 CHECK (revision>=0),
    owner_instance uuid NOT NULL,
    lease_until timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX duel_journal_recovery_idx ON duel_journal(lease_until) WHERE status='active';
CREATE TABLE duel_locks (
    user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    duel_id uuid NOT NULL REFERENCES duel_journal(id) ON DELETE CASCADE
);
CREATE INDEX duel_locks_duel_idx ON duel_locks(duel_id);
CREATE TABLE duel_journal_rounds (
    duel_id uuid NOT NULL REFERENCES duel_journal(id) ON DELETE CASCADE,
    round smallint NOT NULL CHECK(round BETWEEN 1 AND 9),
    moves bytea NOT NULL,
    state_hash bytea NOT NULL CHECK(octet_length(state_hash)=32),
    PRIMARY KEY(duel_id,round)
);
CREATE TABLE matches (
    id uuid PRIMARY KEY REFERENCES duel_journal(id),
    kind text NOT NULL,
    first_user uuid NOT NULL REFERENCES users(id),
    second_user uuid REFERENCES users(id),
    first_avatar uuid NOT NULL REFERENCES avatars(id),
    second_avatar uuid REFERENCES avatars(id),
    winner smallint NOT NULL CHECK(winner BETWEEN 0 AND 2),
    reason text NOT NULL,
    rounds smallint NOT NULL CHECK(rounds BETWEEN 0 AND 9),
    first_rating_delta integer NOT NULL DEFAULT 0,
    second_rating_delta integer NOT NULL DEFAULT 0,
    first_xp integer NOT NULL DEFAULT 0 CHECK(first_xp>=0),
    second_xp integer NOT NULL DEFAULT 0 CHECK(second_xp>=0),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX matches_first_history_idx ON matches(first_user,id DESC);
CREATE INDEX matches_second_history_idx ON matches(second_user,id DESC);
CREATE TABLE rating_history (
    match_id uuid NOT NULL REFERENCES matches(id),
    user_id uuid NOT NULL REFERENCES users(id),
    before integer NOT NULL CHECK(before>=1),
    after integer NOT NULL CHECK(after>=1),
    PRIMARY KEY(match_id,user_id)
);
CREATE TABLE player_stats (
    user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    games integer NOT NULL DEFAULT 0 CHECK(games>=0),
    wins integer NOT NULL DEFAULT 0 CHECK(wins>=0),
    draws integer NOT NULL DEFAULT 0 CHECK(draws>=0),
    losses integer NOT NULL DEFAULT 0 CHECK(losses>=0),
    disconnects integer NOT NULL DEFAULT 0 CHECK(disconnects>=0),
    CHECK(games=wins+draws+losses)
);
