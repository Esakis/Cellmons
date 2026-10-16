ALTER TABLE unlocked_templates DROP CONSTRAINT unlocked_templates_source_check;
ALTER TABLE unlocked_templates ADD CONSTRAINT unlocked_templates_source_check
    CHECK (source IN ('start','arena','tournament','expedition','junkyard','admin','sacrifice'));

CREATE TABLE reincarnation_history (
    avatar_id uuid NOT NULL REFERENCES avatars(id),
    to_tier smallint NOT NULL CHECK (to_tier BETWEEN 1 AND 3),
    previous_xp integer NOT NULL CHECK (previous_xp >= 0),
    path_id integer NOT NULL CHECK (path_id > 0),
    specialization text,
    gift_id integer,
    content_version bytea NOT NULL CHECK (octet_length(content_version)=32),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (avatar_id,to_tier)
);
CREATE TABLE avatar_sacrifices (
    consumed_avatar uuid PRIMARY KEY REFERENCES avatars(id),
    beneficiary_avatar uuid NOT NULL REFERENCES avatars(id),
    user_id uuid NOT NULL REFERENCES users(id),
    kind text NOT NULL CHECK (kind IN ('shade_create','shade_reincarnate')),
    content_version bytea NOT NULL CHECK (octet_length(content_version)=32),
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK (consumed_avatar <> beneficiary_avatar)
);
CREATE INDEX avatar_sacrifices_owner ON avatar_sacrifices(user_id,created_at);
