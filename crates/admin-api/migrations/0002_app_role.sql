-- Restricted application role (D4, docs/13).
--
-- RLS is only as strong as the connecting role: superusers and BYPASSRLS
-- roles skip policies entirely (found live by the slice-1 smoke test — a
-- superuser connection let tenant B read tenant A's domains). The app must
-- connect as a role that is a member of opengroupware_app and nothing more.
-- admin-api additionally refuses to start on a superuser/BYPASSRLS
-- connection unless OPENGROUPWARE_ALLOW_UNSAFE_DB=1 (dev only).
--
-- Deployment: the bootstrap migration job runs privileged; the service's
-- login user is created per environment (password from secrets) and granted
-- this role:
--   CREATE ROLE og_app LOGIN PASSWORD '...';
--   GRANT opengroupware_app TO og_app;

DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'opengroupware_app') THEN
        CREATE ROLE opengroupware_app NOLOGIN;
    END IF;
END
$$;

GRANT USAGE ON SCHEMA public TO opengroupware_app;
GRANT SELECT, INSERT, UPDATE ON tenants, domains, accounts TO opengroupware_app;
GRANT SELECT, INSERT ON audit_log TO opengroupware_app;
-- audit_log id is GENERATED ALWAYS AS IDENTITY — inserts need the sequence.
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO opengroupware_app;
-- No DELETE anywhere: tenant teardown is a lifecycle workflow (status
-- transitions + export), never a row purge from the API path.
