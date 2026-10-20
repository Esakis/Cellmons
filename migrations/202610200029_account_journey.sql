-- Account progress survives character changes, deletion and reincarnation.
ALTER TABLE users ADD COLUMN account_xp bigint NOT NULL DEFAULT 0 CHECK (account_xp >= 0);
ALTER TABLE users ADD COLUMN tutorial_step smallint NOT NULL DEFAULT 0 CHECK (tutorial_step BETWEEN 0 AND 3);

WITH awards AS (
    SELECT first_user AS user_id, 60 + CASE WHEN winner=1 THEN 30 ELSE 0 END AS xp
    FROM matches WHERE kind <> 'training' AND rounds >= 2 AND reason NOT IN ('forfeit','disconnect','timeout')
    UNION ALL
    SELECT second_user, 60 + CASE WHEN winner=2 THEN 30 ELSE 0 END
    FROM matches WHERE kind <> 'training' AND rounds >= 2 AND second_user IS NOT NULL AND reason NOT IN ('forfeit','disconnect','timeout')
), totals AS (
    SELECT user_id, sum(xp) AS xp FROM awards GROUP BY user_id
)
-- Existing players who have finished a real duel retain access to the Arena.
UPDATE users SET account_xp=GREATEST(totals.xp,250), tutorial_step=3
FROM totals WHERE users.id=totals.user_id;
