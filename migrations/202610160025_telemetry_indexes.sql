CREATE INDEX matches_created_at_idx ON matches(created_at);
CREATE INDEX ledger_transactions_created_at_idx ON ledger_transactions(created_at);
CREATE INDEX sessions_last_seen_idx ON sessions(last_seen_at);
