ALTER TABLE avatars ADD COLUMN revision integer NOT NULL DEFAULT 0 CHECK(revision>=0);
ALTER TABLE decks ADD COLUMN revision integer NOT NULL DEFAULT 0 CHECK(revision>=0);
ALTER TABLE deck_presets ADD COLUMN revision integer NOT NULL DEFAULT 0 CHECK(revision>=0);
CREATE TABLE arena_progress (
    user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    wins integer NOT NULL DEFAULT 0 CHECK(wins>=0)
);
-- Existing winners already earned progress before this feature was enabled.
INSERT INTO arena_progress(user_id,wins)
SELECT user_id,count(*)::integer FROM (
    SELECT first_user AS user_id FROM matches WHERE kind='arena' AND winner=1
    UNION ALL SELECT second_user FROM matches WHERE kind='arena' AND winner=2 AND second_user IS NOT NULL
) winners GROUP BY user_id;
ALTER TABLE matches
    ADD COLUMN first_unlock integer CHECK(first_unlock>0),
    ADD COLUMN second_unlock integer CHECK(second_unlock>0);
