-- Offers are immutable to the client after completion; only server actors write this journal.
CREATE TABLE trades (
    id uuid PRIMARY KEY,
    from_user uuid NOT NULL REFERENCES users(id),
    to_user uuid NOT NULL REFERENCES users(id),
    status text NOT NULL CHECK (status IN ('invited','offering','completed','cancelled','expired')),
    revision bigint NOT NULL DEFAULT 0 CHECK (revision >= 0),
    state bytea NOT NULL CHECK (octet_length(state) BETWEEN 1 AND 65536),
    owner_instance uuid NOT NULL,
    lease_until timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz,
    CHECK (from_user <> to_user),
    CHECK ((status IN ('completed','cancelled','expired')) = (finished_at IS NOT NULL))
);
CREATE UNIQUE INDEX trades_pending_recipient ON trades(to_user) WHERE status IN ('invited','offering');
CREATE INDEX trades_participant_history ON trades(from_user,id DESC);
CREATE INDEX trades_recipient_history ON trades(to_user,id DESC);
CREATE INDEX trades_recovery ON trades(lease_until) WHERE status IN ('invited','offering');
