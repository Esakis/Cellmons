-- Run as game_migrator AFTER migrations (including future deployments).
-- Migrations must have been run as game_migrator so that it owns the tables.
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO game_app;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO game_readonly;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO game_app;
GRANT SELECT ON ALL SEQUENCES IN SCHEMA public TO game_readonly;

-- The application may check readiness but must never rewrite migration history.
REVOKE INSERT, UPDATE, DELETE, TRUNCATE ON public._sqlx_migrations FROM game_app;
GRANT SELECT ON public._sqlx_migrations TO game_app;

-- Apply defaults only after _sqlx_migrations exists; future game tables inherit DML grants.
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO game_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO game_readonly;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT USAGE, SELECT ON SEQUENCES TO game_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON SEQUENCES TO game_readonly;
