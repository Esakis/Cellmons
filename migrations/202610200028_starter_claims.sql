-- A character's starter collection is granted at most once per account.
CREATE TABLE starter_claims (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    template_id integer NOT NULL CHECK (template_id > 0),
    claimed_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, template_id)
);
-- Preserve existing inventories; recreating an old character must not mint another pack.
INSERT INTO starter_claims(user_id,template_id)
SELECT DISTINCT user_id,template_id FROM avatars;
