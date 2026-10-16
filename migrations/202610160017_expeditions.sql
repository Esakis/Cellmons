CREATE TABLE expeditions (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id),
    avatar_id uuid NOT NULL REFERENCES avatars(id),
    difficulty text NOT NULL CHECK(difficulty IN ('walk','expedition','ordeal')),
    content_version bytea NOT NULL CHECK(octet_length(content_version)=32),
    plan bytea NOT NULL CHECK(octet_length(plan)<=262144),
    status text NOT NULL DEFAULT 'active' CHECK(status IN ('active','won','lost','retreated')),
    stage smallint NOT NULL DEFAULT 0 CHECK(stage BETWEEN 0 AND 9),
    xp_accum integer NOT NULL DEFAULT 0 CHECK(xp_accum>=0),
    active_duel uuid,
    revision integer NOT NULL DEFAULT 0 CHECK(revision>=0),
    award bytea CHECK(octet_length(award)<=4096),
    created_at timestamptz NOT NULL DEFAULT now(),
    ended_at timestamptz,
    CHECK((status='active' AND award IS NULL AND ended_at IS NULL) OR
          (status<>'active' AND active_duel IS NULL AND award IS NOT NULL AND ended_at IS NOT NULL))
);
CREATE UNIQUE INDEX expeditions_one_active_idx ON expeditions(user_id) WHERE status='active';
CREATE INDEX expeditions_history_idx ON expeditions(user_id,created_at DESC);
CREATE TABLE expedition_fights (
    duel_id uuid PRIMARY KEY,
    expedition_id uuid NOT NULL REFERENCES expeditions(id),
    stage smallint NOT NULL CHECK(stage BETWEEN 0 AND 8),
    choice smallint NOT NULL CHECK(choice BETWEEN 0 AND 2),
    xp_nominal integer CHECK(xp_nominal>=0),
    winner smallint CHECK(winner BETWEEN 0 AND 2),
    UNIQUE(expedition_id,stage)
);
CREATE TABLE expedition_requests (
    user_id uuid NOT NULL REFERENCES users(id),
    request_id uuid NOT NULL,
    expedition_id uuid NOT NULL REFERENCES expeditions(id),
    fingerprint bytea NOT NULL CHECK(octet_length(fingerprint)=32),
    PRIMARY KEY(user_id,request_id)
);
