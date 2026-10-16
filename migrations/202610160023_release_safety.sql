-- Expand only. Legacy frozen activities belong to the last pre-migration engine/content.
ALTER TABLE tournaments
    ADD COLUMN engine_version text NOT NULL DEFAULT '0.2.0',
    ADD COLUMN content_version bytea NOT NULL DEFAULT decode('2fe5309b47546cda1df29067283d34e881f6c61e58be132e127cc5dfb8225701','hex')
        CHECK(octet_length(content_version)=32);
-- New writers must always pin their own versions explicitly.
ALTER TABLE tournaments ALTER COLUMN engine_version DROP DEFAULT,
    ALTER COLUMN content_version DROP DEFAULT;
ALTER TABLE server_controls ADD COLUMN deployment_draining boolean NOT NULL DEFAULT false;
ALTER TABLE expeditions ADD COLUMN engine_version text NOT NULL DEFAULT '0.2.0';
ALTER TABLE expeditions ALTER COLUMN engine_version DROP DEFAULT;

CREATE TABLE ws_tickets (
    nonce_hash bytea PRIMARY KEY CHECK(octet_length(nonce_hash)=32),
    session_hash bytea NOT NULL REFERENCES sessions(id_hash) ON DELETE CASCADE,
    expires_at timestamptz NOT NULL
);
CREATE INDEX ws_tickets_expiry_idx ON ws_tickets(expires_at);
