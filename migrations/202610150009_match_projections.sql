-- Completed replays must survive later content/engine releases without replaying old code.
CREATE TABLE match_projections (
    match_id uuid PRIMARY KEY REFERENCES matches(id) ON DELETE CASCADE,
    replay bytea NOT NULL CHECK(octet_length(replay)<=262144),
    first_snapshot bytea NOT NULL CHECK(octet_length(first_snapshot)<=262144),
    second_snapshot bytea NOT NULL CHECK(octet_length(second_snapshot)<=262144)
);
