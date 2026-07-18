-- Abuse pipeline schema, slice 2 (docs/03-abuse-pipeline.md, D6 in docs/13).
--
-- Product owns the DECISION + quarantine WORKFLOW, not a replica of Rspamd's
-- per-symbol history (D6): one abuse_decision row per scanned message (verdict
-- + score + top symbols as JSONB), and a quarantine_item workflow row when a
-- message is held. Message bodies stay in the mail plane (Stalwart/quarantine
-- store); we reference them, never copy them (ADR-0005).
--
-- Same RLS contract as the control plane: tenant_id on every row, RLS
-- ENABLED+FORCED, policies keyed off the per-transaction GUCs set by og-db.

CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Shared with the control-plane migrations conceptually, but abuse-api owns
-- its own database/audit_log (separate service, separate migrations set).
CREATE TABLE IF NOT EXISTS audit_log (
    id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    tenant_id   uuid,
    actor       text NOT NULL,
    action      text NOT NULL,
    entity_type text NOT NULL,
    entity_id   text NOT NULL,
    payload     jsonb NOT NULL DEFAULT '{}'::jsonb,
    created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS audit_tenant_idx ON audit_log (tenant_id, created_at);
ALTER TABLE audit_log ENABLE ROW LEVEL SECURITY;
ALTER TABLE audit_log FORCE ROW LEVEL SECURITY;
CREATE POLICY audit_read ON audit_log FOR SELECT
    USING (tenant_id::text = current_setting('app.current_tenant_id', true)
           OR current_setting('app.is_platform_admin', true) = 'true');
CREATE POLICY audit_append ON audit_log FOR INSERT
    WITH CHECK (tenant_id::text = current_setting('app.current_tenant_id', true)
               OR current_setting('app.is_platform_admin', true) = 'true');

-- ---------------------------------------------------------------------------
-- abuse_decision: one row per scanned message (inbound or outbound).
-- ---------------------------------------------------------------------------
CREATE TABLE abuse_decision (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     uuid NOT NULL,
    -- Stalwart message identifier (blob/message id) — reference, not content.
    message_ref   text NOT NULL,
    direction     text NOT NULL DEFAULT 'inbound'
                  CHECK (direction IN ('inbound', 'outbound')),
    recipient     text NOT NULL,
    sender        text NOT NULL DEFAULT '',
    score         real NOT NULL,
    action        text NOT NULL
                  CHECK (action IN ('accept', 'junk', 'quarantine', 'reject', 'discard')),
    verdict       text NOT NULL DEFAULT 'ham'
                  CHECK (verdict IN ('ham', 'spam', 'phishing', 'malware')),
    -- Top symbols only (D6: not the full Rspamd symbol replica).
    symbols       jsonb NOT NULL DEFAULT '[]'::jsonb,
    scanned_at    timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX abuse_decision_tenant_idx ON abuse_decision (tenant_id, scanned_at DESC);
CREATE INDEX abuse_decision_msg_idx ON abuse_decision (tenant_id, message_ref);

ALTER TABLE abuse_decision ENABLE ROW LEVEL SECURITY;
ALTER TABLE abuse_decision FORCE ROW LEVEL SECURITY;
CREATE POLICY abuse_decision_isolation ON abuse_decision
    USING (tenant_id::text = current_setting('app.current_tenant_id', true)
           OR current_setting('app.is_platform_admin', true) = 'true')
    WITH CHECK (tenant_id::text = current_setting('app.current_tenant_id', true)
               OR current_setting('app.is_platform_admin', true) = 'true');

-- ---------------------------------------------------------------------------
-- quarantine_item: workflow state for a held message.
-- ---------------------------------------------------------------------------
CREATE TABLE quarantine_item (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id     uuid NOT NULL,
    decision_id   uuid NOT NULL REFERENCES abuse_decision(id) ON DELETE CASCADE,
    message_ref   text NOT NULL,
    recipient     text NOT NULL,
    sender        text NOT NULL DEFAULT '',
    subject       text NOT NULL DEFAULT '',
    reason        text NOT NULL,
    status        text NOT NULL DEFAULT 'held'
                  CHECK (status IN ('held', 'released', 'deleted')),
    reported_as   text CHECK (reported_as IN ('spam', 'ham')),
    held_at       timestamptz NOT NULL DEFAULT now(),
    resolved_at   timestamptz,
    resolved_by   text,
    UNIQUE (tenant_id, message_ref)
);
CREATE INDEX quarantine_tenant_status_idx ON quarantine_item (tenant_id, status, held_at DESC);

ALTER TABLE quarantine_item ENABLE ROW LEVEL SECURITY;
ALTER TABLE quarantine_item FORCE ROW LEVEL SECURITY;
CREATE POLICY quarantine_isolation ON quarantine_item
    USING (tenant_id::text = current_setting('app.current_tenant_id', true)
           OR current_setting('app.is_platform_admin', true) = 'true')
    WITH CHECK (tenant_id::text = current_setting('app.current_tenant_id', true)
               OR current_setting('app.is_platform_admin', true) = 'true');
