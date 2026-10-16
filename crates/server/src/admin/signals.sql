WITH recent AS (
    SELECT least(first_user,second_user) AS a,greatest(first_user,second_user) AS b,
        CASE winner WHEN 1 THEN first_user WHEN 2 THEN second_user ELSE NULL END AS winner,rounds
    FROM matches
    WHERE kind='arena' AND second_user IS NOT NULL
        AND created_at >= date_trunc('day',now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'
), repeated AS (
    SELECT a,b,count(*) AS games,count(*) FILTER(WHERE winner=a) AS a_wins,
        count(*) FILTER(WHERE winner=b) AS b_wins,count(*) FILTER(WHERE rounds<=2) AS short
    FROM recent GROUP BY a,b HAVING count(*)>=20
), duel AS (
    SELECT p.user_id,'duels'::text AS kind,least(r.games,2147483647)::integer AS score,
        json_build_object('players',ARRAY[r.a,r.b],'games',r.games,'first_wins',r.a_wins,
            'second_wins',r.b_wins,'short',r.short)::text AS details
    FROM repeated r CROSS JOIN LATERAL unnest(ARRAY[r.a,r.b]) AS p(user_id)
    WHERE greatest(r.a_wins,r.b_wins)*10>=r.games*9 AND r.short*2>=r.games
), shared AS (
    SELECT ip_hash,count(DISTINCT user_id) AS accounts FROM sessions
    WHERE ip_hash IS NOT NULL AND expires_at>now()
    GROUP BY ip_hash HAVING count(DISTINCT user_id)>=4
), accounts AS (
    SELECT s.user_id,'accounts'::text AS kind,max(least(x.accounts,2147483647))::integer AS score,
        json_build_object('shared_network_accounts',max(x.accounts),
            'note','Shared households and mobile networks can be legitimate')::text AS details
    FROM sessions s JOIN shared x ON x.ip_hash=s.ip_hash WHERE s.expires_at>now() GROUP BY s.user_id
), economy AS (
    SELECT substring(e.account FROM 6)::uuid AS user_id,'economy'::text AS kind,
        least(sum(e.delta),2147483647)::integer AS score,
        json_build_object('received',sum(e.delta)::text,'transactions',count(*),
            'note','Includes legitimate system prizes; review the ledger')::text AS details
    FROM ledger_entries e JOIN ledger_transactions t ON t.id=e.tx_id
    WHERE e.account LIKE 'user:%' AND e.delta>0
        AND t.created_at>=date_trunc('day',now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'
    GROUP BY e.account HAVING sum(e.delta)>10000
), trade_records AS (
    SELECT from_user,to_user,convert_from(state,'UTF8')::jsonb AS payload FROM trades
    WHERE status='completed' AND finished_at>=date_trunc('day',now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'
), one_way AS (
    SELECT least(from_user,to_user) AS a,greatest(from_user,to_user) AS b,count(*) AS gifts
    FROM trade_records
    WHERE ((payload->'offers'->0->'cards'='{}'::jsonb AND (payload->'offers'->0->>'ducats')::bigint=0)
        <> (payload->'offers'->1->'cards'='{}'::jsonb AND (payload->'offers'->1->>'ducats')::bigint=0))
    GROUP BY least(from_user,to_user),greatest(from_user,to_user) HAVING count(*)>=5
), trade AS (
    SELECT p.user_id,'trade'::text AS kind,least(o.gifts,2147483647)::integer AS score,
        json_build_object('players',ARRAY[o.a,o.b],'one_way_transfers',o.gifts,
            'note','Gifts can be legitimate; inspect both offers')::text AS details
    FROM one_way o CROSS JOIN LATERAL unnest(ARRAY[o.a,o.b]) AS p(user_id)
), candidates AS (
    SELECT * FROM duel UNION ALL SELECT * FROM accounts UNION ALL SELECT * FROM economy UNION ALL SELECT * FROM trade
)
SELECT DISTINCT ON(user_id,kind) user_id,kind,score,details FROM candidates
ORDER BY user_id,kind,score DESC,details LIMIT 100001
