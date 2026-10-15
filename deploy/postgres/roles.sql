-- Run once as the database administrator in the application's database, BEFORE migrations.
-- No passwords are embedded: configure login credentials separately (e.g. psql \password).
-- These role names belong to this application; use a dedicated PostgreSQL cluster.
DO $roles$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'game_migrator') THEN
        CREATE ROLE game_migrator LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION;
    END IF;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'game_app') THEN
        CREATE ROLE game_app LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION;
    END IF;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'game_readonly') THEN
        CREATE ROLE game_readonly LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION;
    END IF;
    -- citext is a trusted extension; installing it requires CREATE on this database.
    EXECUTE format('GRANT CONNECT, CREATE ON DATABASE %I TO game_migrator', current_database());
    EXECUTE format('GRANT CONNECT ON DATABASE %I TO game_app, game_readonly', current_database());
END
$roles$;

REVOKE CREATE ON SCHEMA public FROM PUBLIC;
REVOKE ALL ON SCHEMA public FROM game_app, game_readonly;
GRANT USAGE, CREATE ON SCHEMA public TO game_migrator;
GRANT USAGE ON SCHEMA public TO game_app, game_readonly;
