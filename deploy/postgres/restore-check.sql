SET default_transaction_read_only=on;
SELECT json_build_object(
  'migrations',(SELECT count(*) FROM _sqlx_migrations WHERE success),
  'users',(SELECT count(*) FROM users),
  'matches',(SELECT count(*) FROM matches),
  'ledger_entries',(SELECT count(*) FROM ledger_entries),
  'unbalanced_transactions',(SELECT count(*) FROM (SELECT t.id FROM ledger_transactions t LEFT JOIN ledger_entries e ON e.tx_id=t.id GROUP BY t.id HAVING count(e.account)<2 OR coalesce(sum(e.delta),0)<>0) bad),
  'incorrect_wallets',(SELECT count(*) FROM (SELECT coalesce(w.balance,0) actual,coalesce(l.recorded,0) recorded FROM wallets w FULL JOIN (SELECT account,sum(delta) recorded FROM ledger_entries WHERE account LIKE 'user:%' GROUP BY account) l ON l.account='user:'||w.user_id::text) balances WHERE actual<>recorded),
  'incorrect_escrows',(SELECT count(*) FROM (SELECT coalesce(t.balance,0) actual,coalesce(l.recorded,0) recorded FROM tournaments t FULL JOIN (SELECT account,sum(delta) recorded FROM ledger_entries WHERE account LIKE 'tournament:%' GROUP BY account) l ON l.account='tournament:'||t.id::text) balances WHERE actual<>recorded)
);
