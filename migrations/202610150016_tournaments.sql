CREATE TABLE tournaments (
    id uuid PRIMARY KEY,
    creator uuid NOT NULL REFERENCES users(id),
    name text NOT NULL CHECK(char_length(name) BETWEEN 3 AND 60),
    kind text NOT NULL DEFAULT 'player' CHECK(kind IN ('player','system')),
    status text NOT NULL DEFAULT 'registering' CHECK(status IN ('registering','running','finished','cancelled')),
    terms bytea NOT NULL CHECK(octet_length(terms)<=8192),
    seed bytea NOT NULL CHECK(octet_length(seed)=32),
    bracket bytea CHECK(octet_length(bracket)<=524288),
    balance bigint NOT NULL DEFAULT 0 CHECK(balance>=0),
    revision bigint NOT NULL DEFAULT 0 CHECK(revision>=0),
    scheduler_instance uuid,
    scheduler_until_ms bigint NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    started_at timestamptz,
    finished_at timestamptz,
    CHECK((status='registering' AND bracket IS NULL) OR status<>'registering')
);
CREATE INDEX tournaments_schedule_idx ON tournaments(status,created_at);
CREATE INDEX tournaments_creator_idx ON tournaments(creator,created_at);
CREATE TABLE tournament_entries (
    tournament_id uuid NOT NULL REFERENCES tournaments(id),
    user_id uuid NOT NULL REFERENCES users(id),
    avatar_id uuid NOT NULL REFERENCES avatars(id),
    team_id uuid NOT NULL,
    seat bytea NOT NULL CHECK(octet_length(seat)<=16384),
    fee bigint NOT NULL CHECK(fee>=0),
    status text NOT NULL DEFAULT 'enrolled' CHECK(status IN ('enrolled','left','eliminated','champion')),
    ready_since_ms bigint,
    ready_until_ms bigint NOT NULL DEFAULT 0,
    joined_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY(tournament_id,user_id)
);
CREATE INDEX tournament_entries_user_idx ON tournament_entries(user_id,tournament_id);
CREATE TABLE tournament_requests (
    user_id uuid NOT NULL REFERENCES users(id),
    request_id uuid NOT NULL,
    tournament_id uuid NOT NULL REFERENCES tournaments(id),
    fingerprint bytea NOT NULL CHECK(octet_length(fingerprint)=32),
    PRIMARY KEY(user_id,request_id)
);
CREATE TABLE tournament_fights (
    id uuid PRIMARY KEY,
    tournament_id uuid NOT NULL REFERENCES tournaments(id),
    descriptor bytea NOT NULL CHECK(octet_length(descriptor)<=2048),
    status text NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','active','done')),
    first_fight boolean NOT NULL,
    created_ms bigint NOT NULL,
    deadline_ms bigint NOT NULL,
    score smallint CHECK(score BETWEEN 0 AND 3)
);
CREATE INDEX tournament_fights_pending_idx ON tournament_fights(tournament_id,status);
ALTER TABLE ledger_entries DROP CONSTRAINT ledger_entries_account_check;
ALTER TABLE ledger_entries ADD CONSTRAINT ledger_entries_account_check
    CHECK(account ~ '^(user:[0-9a-f-]{36}|tournament:[0-9a-f-]{36}|system:(faucet|sink))$');

CREATE FUNCTION tournament_check_ledger() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE actual bigint; recorded numeric; tournament_id uuid;
BEGIN
    IF TG_TABLE_NAME='ledger_entries' THEN
        IF NEW.account NOT LIKE 'tournament:%' THEN RETURN NULL; END IF;
        tournament_id:=substring(NEW.account FROM 12)::uuid;
    ELSE
        tournament_id:=NEW.id;
    END IF;
    SELECT balance INTO actual FROM tournaments WHERE id=tournament_id;
    SELECT coalesce(sum(delta),0) INTO recorded FROM ledger_entries WHERE account='tournament:'||tournament_id::text;
    IF actual IS NULL OR actual<>recorded THEN
        RAISE EXCEPTION 'tournament escrow does not match ledger' USING ERRCODE='23514';
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER tournament_balance_matches_ledger AFTER INSERT OR UPDATE ON tournaments
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION tournament_check_ledger();
CREATE CONSTRAINT TRIGGER tournament_ledger_matches_balance AFTER INSERT ON ledger_entries
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION tournament_check_ledger();
