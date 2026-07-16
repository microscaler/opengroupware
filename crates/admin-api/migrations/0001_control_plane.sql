-- Control-plane schema, slice 1 (ADR-0005: product DB owns tenancy/policy/
-- audit only — no mail state).
--
-- RLS model (D4, docs/13-ownership-review.md):
--  * Every tenant-scoped table carries tenant_id and has RLS ENABLED+FORCED.
--  * Policies key off current_setting('app.current_tenant_id', true), which
--    is set per-transaction via SET LOCAL (set_config(..., true)) by the
--    admin-api Db wrapper — safe under PgBouncer transaction pooling.
--  * Platform operations set app.is_platform_admin='true' (also SET LOCAL).
--    FORCE means even the table owner obeys policies; there is no implicit
--    superuser path through the app role.

CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ---------------------------------------------------------------------------
-- tenants
-- ---------------------------------------------------------------------------
CREATE TABLE tenants (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    slug        text NOT NULL UNIQUE
                CHECK (slug ~ '^[a-z0-9][a-z0-9-]{1,62}$'),
    name        text NOT NULL,
    plan        text NOT NULL DEFAULT 'standard',
    status      text NOT NULL DEFAULT 'active'
                CHECK (status IN ('active', 'suspended', 'terminated')),
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE tenants ENABLE ROW LEVEL SECURITY;
ALTER TABLE tenants FORCE ROW LEVEL SECURITY;

-- A tenant session sees its own row; platform admin sees all.
CREATE POLICY tenants_isolation ON tenants
    USING (
        id::text = current_setting('app.current_tenant_id', true)
        OR current_setting('app.is_platform_admin', true) = 'true'
    )
    WITH CHECK (current_setting('app.is_platform_admin', true) = 'true');

-- ---------------------------------------------------------------------------
-- domains
-- ---------------------------------------------------------------------------
CREATE TABLE domains (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   uuid NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    fqdn        text NOT NULL UNIQUE
                CHECK (fqdn ~ '^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}$'),
    status      text NOT NULL DEFAULT 'pending_dns'
                CHECK (status IN ('pending_dns', 'active', 'suspended')),
    created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX domains_tenant_idx ON domains (tenant_id);

ALTER TABLE domains ENABLE ROW LEVEL SECURITY;
ALTER TABLE domains FORCE ROW LEVEL SECURITY;

CREATE POLICY domains_isolation ON domains
    USING (
        tenant_id::text = current_setting('app.current_tenant_id', true)
        OR current_setting('app.is_platform_admin', true) = 'true'
    )
    WITH CHECK (
        tenant_id::text = current_setting('app.current_tenant_id', true)
        OR current_setting('app.is_platform_admin', true) = 'true'
    );

-- ---------------------------------------------------------------------------
-- accounts (directory metadata only — credentials live in sesame-idam,
-- mailbox state lives in Stalwart; ADR-0005/0006)
-- ---------------------------------------------------------------------------
CREATE TABLE accounts (
    id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id           uuid NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    domain_id           uuid NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    email               text NOT NULL UNIQUE,
    display_name        text NOT NULL DEFAULT '',
    quota_mb            integer NOT NULL DEFAULT 1024
                        CHECK (quota_mb BETWEEN 1 AND 1048576),
    status              text NOT NULL DEFAULT 'pending_provisioning'
                        CHECK (status IN (
                            'pending_provisioning', 'active',
                            'suspended', 'deleted'
                        )),
    sesame_user_id      uuid,
    stalwart_principal  text,
    created_at          timestamptz NOT NULL DEFAULT now(),
    updated_at          timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX accounts_tenant_idx ON accounts (tenant_id);
CREATE INDEX accounts_domain_idx ON accounts (domain_id);

ALTER TABLE accounts ENABLE ROW LEVEL SECURITY;
ALTER TABLE accounts FORCE ROW LEVEL SECURITY;

CREATE POLICY accounts_isolation ON accounts
    USING (
        tenant_id::text = current_setting('app.current_tenant_id', true)
        OR current_setting('app.is_platform_admin', true) = 'true'
    )
    WITH CHECK (
        tenant_id::text = current_setting('app.current_tenant_id', true)
        OR current_setting('app.is_platform_admin', true) = 'true'
    );

-- ---------------------------------------------------------------------------
-- audit_log (append-only; tenant_id NULL = platform-level event)
-- ---------------------------------------------------------------------------
CREATE TABLE audit_log (
    id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    tenant_id   uuid REFERENCES tenants(id) ON DELETE SET NULL,
    actor       text NOT NULL,
    action      text NOT NULL,
    entity_type text NOT NULL,
    entity_id   text NOT NULL,
    payload     jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX audit_tenant_idx ON audit_log (tenant_id, created_at);

ALTER TABLE audit_log ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_log FORCE ROW LEVEL SECURITY;

-- Tenants read their own trail; only sessions (tenant or platform) may
-- append; nobody updates or deletes (no UPDATE/DELETE policies exist).
CREATE POLICY audit_read ON audit_log FOR SELECT
    USING (
        tenant_id::text = current_setting('app.current_tenant_id', true)
        OR current_setting('app.is_platform_admin', true) = 'true'
    );

CREATE POLICY audit_append ON audit_log FOR INSERT
    WITH CHECK (
        tenant_id::text = current_setting('app.current_tenant_id', true)
        OR current_setting('app.is_platform_admin', true) = 'true'
    );
