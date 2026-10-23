-- Keep the pre-migration characters, including old gifts and allocated points.
-- Before this release the first two tutorial screens granted no XP.
UPDATE users SET account_xp=account_xp+100 WHERE tutorial_step IN (1,2);
CREATE TABLE legacy_progression (
    avatar_id uuid PRIMARY KEY REFERENCES avatars(id) ON DELETE CASCADE,
    profile jsonb NOT NULL,
    migrated_at timestamptz NOT NULL DEFAULT now()
);
INSERT INTO legacy_progression SELECT id,to_jsonb(a),now() FROM avatars a WHERE tier>0;

CREATE TABLE avatar_advancement (
    avatar_id uuid PRIMARY KEY REFERENCES avatars(id) ON DELETE CASCADE,
    permanent_points smallint NOT NULL DEFAULT 0 CHECK(permanent_points BETWEEN 0 AND 6),
    permanent_unspent smallint NOT NULL DEFAULT 0 CHECK(permanent_unspent BETWEEN 0 AND permanent_points)
);
INSERT INTO avatar_advancement(avatar_id,permanent_points,permanent_unspent)
SELECT id,least(tier,2)*3,
    least(dev_points_unspent,CASE tier WHEN 1 THEN 2 WHEN 2 THEN 4 ELSE 6 END)
    + CASE tier WHEN 1 THEN 1 WHEN 2 THEN 2 ELSE 0 END
FROM avatars WHERE tier>0;
UPDATE avatars SET dev_points_unspent=greatest(0,dev_points_unspent-
    CASE tier WHEN 1 THEN 2 WHEN 2 THEN 4 ELSE 6 END) WHERE tier>0;

CREATE TABLE class_trials (
    avatar_id uuid NOT NULL REFERENCES avatars(id) ON DELETE CASCADE,
    tier smallint NOT NULL CHECK(tier IN (0,1)),
    wins smallint NOT NULL DEFAULT 0 CHECK(wins BETWEEN 0 AND 3),
    PRIMARY KEY(avatar_id,tier)
);
-- Never remove old XP; grant only newly crossed ordinary level milestones.
WITH levels AS (
    SELECT a.id,a.level AS previous,greatest(a.level,
        (SELECT count(*) FROM unnest(ARRAY[0,100,220,360,520,700,900,1120,1360,1620,1900,2200,2520,2860,3220,3620,4080,4620,5260,6000]) n
         WHERE n::bigint * CASE a.tier WHEN 0 THEN 2 WHEN 1 THEN 3 ELSE 4 END / 2 <= a.xp))::smallint AS current
    FROM avatars a WHERE a.deleted_at IS NULL AND a.template_id IN (1,2,3,4,5,11,13,32)
)
UPDATE avatars a SET level=l.current,revision=revision+1,dev_points_unspent=dev_points_unspent+
    (SELECT coalesce(sum(points),0) FROM (VALUES(5,1),(10,2),(15,1),(20,2)) p(level,points) WHERE p.level>l.previous AND p.level<=l.current)
FROM levels l WHERE a.id=l.id AND l.current>l.previous;
CREATE TABLE path_packs (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    path_id integer NOT NULL CHECK(path_id>0),
    cards smallint[] NOT NULL,
    claimed_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY(user_id,path_id)
);
-- The old deck is always retained, even when all ten user preset slots are occupied.
CREATE TABLE incarnation_decks (
    avatar_id uuid NOT NULL REFERENCES avatars(id) ON DELETE CASCADE,
    tier smallint NOT NULL CHECK(tier IN (1,2)),
    cards smallint[] NOT NULL,
    PRIMARY KEY(avatar_id,tier)
);
CREATE TABLE class_contracts (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    template_id integer NOT NULL CHECK(template_id>0),
    fragments smallint NOT NULL DEFAULT 0 CHECK(fragments BETWEEN 0 AND 30),
    active boolean NOT NULL DEFAULT false,
    claimed_at timestamptz,
    avatar_id uuid REFERENCES avatars(id) ON DELETE SET NULL,
    PRIMARY KEY(user_id,template_id),
    CHECK(claimed_at IS NULL OR (fragments=30 AND NOT active))
);
CREATE UNIQUE INDEX one_active_contract ON class_contracts(user_id) WHERE active;
ALTER TABLE unlocked_templates DROP CONSTRAINT unlocked_templates_source_check;
ALTER TABLE unlocked_templates ADD CONSTRAINT unlocked_templates_source_check
    CHECK(source IN ('start','arena','tournament','expedition','junkyard','admin','sacrifice','recruitment','contract'));
CREATE TABLE card_mastery (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    card_id smallint NOT NULL CHECK(card_id>0),
    xp smallint NOT NULL DEFAULT 0 CHECK(xp BETWEEN 0 AND 200),
    PRIMARY KEY(user_id,card_id)
);
CREATE TABLE daily_quests (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    day date NOT NULL,
    fights smallint NOT NULL DEFAULT 0 CHECK(fights BETWEEN 0 AND 3),
    rewarded boolean NOT NULL DEFAULT false,
    PRIMARY KEY(user_id,day)
);
ALTER TABLE duel_journal DROP CONSTRAINT duel_journal_kind_check;
ALTER TABLE duel_journal ADD CONSTRAINT duel_journal_kind_check CHECK(kind IN ('arena','ranked','training','tournament','expedition'));
ALTER TABLE ledger_transactions DROP CONSTRAINT ledger_transactions_reason_check;
ALTER TABLE ledger_transactions ADD CONSTRAINT ledger_transactions_reason_check
    CHECK(reason IN ('start','daily_win','win','expedition','tournament_fee','tournament_prize','booster','trade','junkyard','admin','daily_quest','card_mastery'));
