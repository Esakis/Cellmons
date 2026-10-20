ALTER TABLE unlocked_templates DROP CONSTRAINT unlocked_templates_source_check;
ALTER TABLE unlocked_templates ADD CONSTRAINT unlocked_templates_source_check
    CHECK (source IN ('start','arena','tournament','expedition','junkyard','admin','sacrifice','recruitment'));

-- The account and milestone form the durable idempotency key.
-- A NULL template is a commemorative seal for an already complete collection.
CREATE TABLE recruitment_claims (
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    milestone smallint NOT NULL CHECK (milestone IN (5,8,12,16,20,24,28)),
    template_id integer CHECK (template_id > 0),
    avatar_id uuid REFERENCES avatars(id) ON DELETE SET NULL,
    claimed_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, milestone),
    CHECK (template_id IS NOT NULL OR avatar_id IS NULL)
);
