-- Per-tenant abuse policy (spam-score action thresholds).
--
-- Desired state owned by the control plane (ADR-0003); config-compiler renders
-- these into Rspamd `settings` per tenant. One row per tenant; absence means
-- "use the platform defaults" (the compiler COALESCEs to the same values these
-- columns default to, so a tenant with no row and a tenant at defaults render
-- identically).
--
-- Thresholds are Rspamd action scores: a message scoring >= reject is
-- rejected, >= add_header is marked, >= greylist is greylisted. Ordering is
-- enforced so a policy can never invert (greylist < add_header < reject).

CREATE TABLE abuse_policy (
    tenant_id   uuid PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    reject      double precision NOT NULL DEFAULT 15.0 CHECK (reject > 0),
    add_header  double precision NOT NULL DEFAULT 6.0  CHECK (add_header > 0),
    greylist    double precision NOT NULL DEFAULT 4.0  CHECK (greylist > 0),
    updated_at  timestamptz NOT NULL DEFAULT now(),
    CHECK (greylist < add_header AND add_header < reject)
);

ALTER TABLE abuse_policy ENABLE ROW LEVEL SECURITY;
ALTER TABLE abuse_policy FORCE ROW LEVEL SECURITY;

CREATE POLICY abuse_policy_isolation ON abuse_policy
    USING (
        tenant_id::text = current_setting('app.current_tenant_id', true)
        OR current_setting('app.is_platform_admin', true) = 'true'
    )
    WITH CHECK (
        tenant_id::text = current_setting('app.current_tenant_id', true)
        OR current_setting('app.is_platform_admin', true) = 'true'
    );

-- Same restricted role the rest of the control plane uses (0002).
GRANT SELECT, INSERT, UPDATE ON abuse_policy TO opengroupware_app;
