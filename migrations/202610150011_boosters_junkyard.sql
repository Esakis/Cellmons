CREATE TABLE idempotency_keys (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key uuid NOT NULL,
    operation text NOT NULL,
    request_hash bytea NOT NULL CHECK(octet_length(request_hash)=32),
    result bytea NOT NULL CHECK(octet_length(result) BETWEEN 1 AND 1048576),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY(user_id,key)
);
CREATE TABLE boosters (
    id uuid PRIMARY KEY,
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind text NOT NULL,
    status text NOT NULL DEFAULT 'unopened' CHECK(status IN ('unopened','opened')),
    price bigint NOT NULL CHECK(price>0),
    pools bytea NOT NULL CHECK(octet_length(pools) BETWEEN 1 AND 65536),
    seed bytea NOT NULL CHECK(octet_length(seed)=32),
    content_version bytea NOT NULL CHECK(octet_length(content_version)=32),
    contents smallint[],
    purchased_at timestamptz NOT NULL DEFAULT now(),
    opened_at timestamptz,
    purchase_key uuid NOT NULL,
    UNIQUE(user_id,purchase_key),
    CHECK((status='opened')=(contents IS NOT NULL AND opened_at IS NOT NULL)),
    CHECK(contents IS NULL OR (cardinality(contents) BETWEEN 1 AND 20 AND 0<ALL(contents)))
);
CREATE INDEX boosters_owner_idx ON boosters(user_id,status,id DESC);
CREATE TABLE junkyard (
    user_id uuid PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    points bigint NOT NULL DEFAULT 0 CHECK(points>=0),
    rewards_granted bigint NOT NULL DEFAULT 0 CHECK(rewards_granted>=0)
);
