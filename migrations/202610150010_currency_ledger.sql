CREATE TABLE wallets (
    user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    balance bigint NOT NULL DEFAULT 0 CHECK(balance>=0),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE ledger_transactions (
    id uuid PRIMARY KEY,
    reason text NOT NULL CHECK(reason IN ('start','daily_win','win','expedition','tournament_fee','tournament_prize','booster','trade','junkyard','admin')),
    ref_id uuid,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE ledger_entries (
    tx_id uuid NOT NULL REFERENCES ledger_transactions(id),
    account text NOT NULL CHECK(account ~ '^(user:[0-9a-f-]{36}|system:(faucet|sink))$'),
    delta bigint NOT NULL CHECK(delta<>0),
    PRIMARY KEY(tx_id,account)
);
CREATE INDEX ledger_entries_account_idx ON ledger_entries(account,tx_id);
CREATE TABLE daily_counters (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    day date NOT NULL,
    first_win_bonus boolean NOT NULL DEFAULT false,
    win_ducats integer NOT NULL DEFAULT 0 CHECK(win_ducats>=0),
    PRIMARY KEY(user_id,day)
);
ALTER TABLE matches
    ADD COLUMN first_ducats integer NOT NULL DEFAULT 0 CHECK(first_ducats>=0),
    ADD COLUMN second_ducats integer NOT NULL DEFAULT 0 CHECK(second_ducats>=0);

-- Existing registered players receive the same initial grant as new accounts.
INSERT INTO wallets(user_id,balance) SELECT id,40 FROM users WHERE status<>'deleted';
INSERT INTO ledger_transactions(id,reason,ref_id) SELECT user_id,'start',user_id FROM wallets;
INSERT INTO ledger_entries(tx_id,account,delta)
SELECT user_id,'user:'||user_id::text,40 FROM wallets
UNION ALL SELECT user_id,'system:faucet',-40 FROM wallets;

CREATE FUNCTION ledger_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'ledger history is append only' USING ERRCODE='23514';
END $$;
CREATE TRIGGER ledger_entries_immutable BEFORE UPDATE OR DELETE ON ledger_entries
FOR EACH ROW EXECUTE FUNCTION ledger_immutable();
CREATE TRIGGER ledger_transactions_immutable BEFORE UPDATE OR DELETE ON ledger_transactions
FOR EACH ROW EXECUTE FUNCTION ledger_immutable();

CREATE FUNCTION ledger_check_transaction() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE total numeric; actual bigint; recorded numeric; owner_id uuid;
BEGIN
    SELECT sum(delta) INTO total FROM ledger_entries WHERE tx_id=NEW.tx_id;
    IF total<>0 THEN RAISE EXCEPTION 'unbalanced ledger transaction' USING ERRCODE='23514'; END IF;
    IF NEW.account LIKE 'user:%' THEN
        owner_id:=substring(NEW.account FROM 6)::uuid;
        SELECT balance INTO actual FROM wallets WHERE user_id=owner_id;
        SELECT sum(delta) INTO recorded FROM ledger_entries WHERE account=NEW.account;
        IF actual IS NULL OR actual<>recorded THEN
            RAISE EXCEPTION 'wallet does not match ledger' USING ERRCODE='23514';
        END IF;
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER ledger_transaction_balanced AFTER INSERT ON ledger_entries
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION ledger_check_transaction();

CREATE FUNCTION wallet_check_ledger() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE actual bigint; recorded numeric;
BEGIN
    -- Read the final row, not an intermediate NEW value from this transaction.
    SELECT balance INTO actual FROM wallets WHERE user_id=NEW.user_id;
    IF actual IS NULL THEN RETURN NULL; END IF;
    SELECT coalesce(sum(delta),0) INTO recorded FROM ledger_entries WHERE account='user:'||NEW.user_id::text;
    IF actual<>recorded THEN RAISE EXCEPTION 'wallet does not match ledger' USING ERRCODE='23514'; END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER wallet_matches_ledger AFTER INSERT OR UPDATE ON wallets
DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION wallet_check_ledger();
