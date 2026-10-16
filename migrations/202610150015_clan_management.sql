-- R-KLA: membership is still the authorization boundary for private rooms.
ALTER TABLE clans ADD COLUMN revision integer NOT NULL DEFAULT 0 CHECK (revision >= 0);
ALTER TABLE clans ADD CONSTRAINT clans_www_length CHECK (char_length(www) <= 200);
ALTER TABLE clans ADD CONSTRAINT clans_email_length CHECK (char_length(email) <= 254);
CREATE UNIQUE INDEX clan_one_master ON clan_members(clan_id) WHERE role = 'master';

CREATE TABLE clan_applications (
    clan_id uuid NOT NULL REFERENCES clans(id),
    user_id uuid NOT NULL REFERENCES users(id),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY(clan_id,user_id)
);
CREATE INDEX clan_applications_user ON clan_applications(user_id,created_at);

CREATE TABLE clan_audit (
    id uuid PRIMARY KEY,
    clan_id uuid NOT NULL REFERENCES clans(id),
    actor_id uuid NOT NULL REFERENCES users(id),
    target_id uuid REFERENCES users(id),
    action text NOT NULL CHECK (action IN
        ('create','apply','withdraw','accept','reject','role','title','edit','leave','kick','transfer','disband')),
    details jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(details) = 'object'),
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX clan_audit_history ON clan_audit(clan_id,id DESC);
CREATE FUNCTION clan_audit_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'clan audit is append only' USING ERRCODE='23514';
END $$;
CREATE TRIGGER clan_audit_immutable BEFORE UPDATE OR DELETE ON clan_audit
FOR EACH ROW EXECUTE FUNCTION clan_audit_immutable();
