# 13 — Ownership Review & Decision Log

- **Date**: 2026-07-16
- **Status**: Binding. Where this document conflicts with docs 01–12, this document wins until those docs are revised.

This is an opinionated review of the full design set (docs 01–12, ADRs 0004/0011) and the code scaffolding, taken on assuming ownership of the project. The design work is substantial and mostly points in the right direction — protocol-first, FOSS-glued, abuse-first, restore-first are the correct stances. But the docs were written breadth-first: they describe a mature product, not a buildable one, and they defer the two decisions everything else depends on. This document makes those decisions.

---

## The two load-bearing problems

### P1. The product database re-implements Stalwart

Doc 04 defines `MESSAGE`, `FOLDER`, `MESSAGE_PLACEMENT`, `THREAD`, `BLOB`, `ATTACHMENT_DEDUP`, `STORAGE_VOLUME`, `MESSAGE_DELIVERY`, `BOUNCE`, `DSN`. Every one of those is state Stalwart already owns (mail store, blob store with dedup, MTA queue). Mirroring it into Postgres creates two sources of truth for the highest-volume, most-corruption-sensitive data in the system, plus a synchronization pipeline that has to be built, monitored, and repaired forever. It also silently breaks the isolation model: doc 07 requires `tenant_id` on every row and doc 09 shards on it, but the ERD's mail tables don't carry the column.

**Decision (ADR-0005):** The product DB owns *control-plane* state only: tenants, domains, accounts (as directory metadata, not credentials), policies, quotas, quarantine workflow, abuse decisions, audit, jobs, migration state, DNS requirements. Stalwart owns mailboxes, messages, folders, threads, blobs, delivery/queue state, sync. The product reads mail data through Stalwart's APIs (JMAP/management API); it never copies it. Roughly 40% of the doc-04 ERD is deleted by this decision.

### P2. Identity has four candidate sources of truth

Doc 02 says Keycloak/Authentik OIDC. Doc 08 designs a homegrown credential store with TOTP, WebAuthn, push MFA, trusted devices, sessions, and OAuth2/PKCE in the product DB. The Helm/kustomize configs deploy Authentik + LLDAP. The only identity code that exists is a Stalwart-backed stub. Nobody decided where a password lives.

**Decision (ADR-0006 v2, per owner directive):** the in-house **sesame-idam** platform is the directory of record — tenants, users, orgs, roles, credentials (argon2id). Authentik and LLDAP are removed from the deployment profile. The product DB stores **zero credentials** and opengroupware builds **zero MFA code** — doc 08's homegrown TOTP/WebAuthn/session-engine design is cancelled and its requirements transfer to the sesame-idam backlog. Stalwart protocol auth (IMAP/SMTP/DAV) bridges via **Stalwart's SQL directory** over a read-only view of sesame-idam's Postgres (including a new app-password table). Honest cost, stated in the ADR: sesame-idam's OIDC path (RS256/JWKS/auth-code/introspection) and app-passwords must be finished before web SSO and mail-client auth work — sesame-idam hardening is now on the mail MVP's critical path.

---

## Further binding decisions

**D3 — Track B is dead everywhere, not just in ADR-0004.** ADR-0004 picked Stalwart, but doc 05's spikes still prototype both tracks, doc 09 still forks on Track A/B, and doc 02 has the config compiler validating Postfix *and* Dovecot *and* Stalwart schemas. All Track B/Postfix/Dovecot material is historical context only. The config compiler targets exactly two backends: **Stalwart + Rspamd**.

**D4 — Fix the RLS foundation before any query ships.** Two real bugs:
- Doc 07 keys RLS off a session GUC (`current_setting('app.current_tenant_id')`) while doc 02 mandates PgBouncer in *transaction* pooling mode. Session-level `SET` leaks across transaction-pooled connections → cross-tenant reads. Rule: every transaction begins with `SET LOCAL app.current_tenant_id`, enforced by a single sqlx acquisition wrapper that is the *only* way product code gets a connection. No raw pool access.
- Doc 07:83 believes `FORCE ROW LEVEL SECURITY` creates a superuser bypass. It does the opposite — it removes the table owner's implicit bypass. Keep `FORCE` (it's correct for us); platform-admin operations use a dedicated role with explicit `BYPASSRLS`, audited.

**D5 — One search, one scanner path, one Postgres operator.**
- Search: Stalwart's built-in FTS via JMAP. **OpenSearch and the Tantivy wrapper are both cut** from MVP; the dev profile stops deploying OpenSearch. Revisit only if cross-mailbox eDiscovery becomes a committed feature.
- ClamAV: wired through **Rspamd's antivirus module**, not a bespoke client. The deployed ClamAV stays; the product never talks to it directly.
- Postgres HA: doc 09's operators are fabricated (`postgresql.enterprises.databasesystem.com/v1`, `citus-data.com/v1`, `redis.opstreepubliccontainers.com/v1beta1` do not exist). Standardize on **CloudNativePG**. Citus is cut — flat DB + RLS carries us for the tenant counts of Phases 1–2, and after ADR-0005 the high-volume mail tables no longer live in Postgres at all, which removes the main sharding motivation.

**D6 — Abuse pipeline stores decisions, not a symbol replica.** The quarantine workflow, explainable verdicts, release/feedback loop, and outbound scanning (doc 03) remain the product's core differentiator — that survives intact. But persisting every Rspamd symbol per message duplicates Rspamd's own history at millions of rows/day. We store one `abuse_decision` row per message (verdict, score, top symbols as JSONB, action) and link to Rspamd history for forensics. The MVP-6 wishlist (lookalike detection, QR-phishing, impossible travel, DMARC auto-remediation) is explicitly post-MVP.

**D7 — Scope amputations (pre-MVP).** Cut from the design until an MVP exists and a customer asks: S/MIME + PGP server-side crypto, read receipts, resource-calendar working-hours modeling, four-level delegation, tenant merge, per-tenant data residency, multi-region, cross-region S3 replication, per-tenant IOPS, service mesh (pick **one** mTLS story later; not two competing ones), Postgres "TDE" (doesn't exist in community PG — use volume encryption), per-tenant Postfix queue paths (not a real feature, and Postfix is gone anyway). Doc 04's "one-time-use" app passwords are unimplementable for IMAP (clients reconnect constantly); doc 08's reusable 90-day model stands.

**D8 — Performance targets are withdrawn until benchmarked.** The published numbers are internally inconsistent (backup restore 100GB/h in doc 02 vs 400GB/h in doc 09). Targets return when the mailbox-backend spike produces measurements.

---

## Code review verdict (state as of 2026-07-16)

`types` is the one genuinely good crate: coherent domain model, consistent async provider traits, `ProviderContext` threaded for tenant checks, proper `thiserror` taxonomy. Everything else is scaffold or stub: wrappers make zero HTTP calls (reqwest is declared and unused, several methods `todo!()` and would panic), four services are one-line libs with no binaries while their k8s manifests reference images and `/health` probes that cannot exist, the Leptos crates lack the `ssr`/`hydrate` feature split so they likely don't compile, both frontends bind `127.0.0.1` (unreachable behind a k8s Service), the strict workspace lints (`unwrap_used`, `panic = forbid`) are inherited by **no crate**, and Prometheus annotations hardcode port 8080 for services listening on five different ports.

Fix list (execution order): (1) enforce `[lints] workspace = true` in every crate and purge `todo!()`/`unwrap()` in favor of `ProviderError`; (2) make the whole workspace compile, including Leptos feature flags; (3) split the 27-method `MailProvider` god-trait into `MailProvider` / `CalendarProvider` / `ContactsProvider` / `SieveProvider`; (4) shared `service-kit` crate providing axum bootstrap, `/health`, `/metrics`, `0.0.0.0` binding, tracing — used by every service so the manifests stop describing fiction; (5) real Stalwart management-API client as the first wrapper with an integration test against a containerized Stalwart.

## Execution status (2026-07-16, end of first ownership session)

Landed and verified: workspace green (all crates compile, clippy clean,
tests pass); slice 1 provisioning live-proven with RLS isolation
(privileged-connection guard added after the smoke test caught superuser
bypass); secrets sops-encrypted with Flux decryption; Tilt + systemd dev
loop on ms02 (port 10852). Sesame side (boy-scout rule): token-exchange
module resurrected (35→89 tests), EdDSA signing wired (placeholder
signatures eliminated), client_credentials grant implemented,
app_passwords + rp_directory bridge migrated and smoke-tested, and
opengroupware's admin-api now provisions accounts into sesame with
audited activation. Stalwart's config rewritten to a real config.toml
consuming rp_directory (verify query schema on first rollout).

Open items: sesame RFC 7662 introspection + auth-code/PKCE (needs the
OpenAPI codegen loop), app-password issue/revoke API (F2 endpoints),
service-kit JWKS verification middleware, rp_stalwart role + secret per
environment, Stalwart config validation on a live cluster, quarantine
slice 2.

## Path out of conceptual stage

The roadmap's own dependency graph says it: product DB/schema → admin API → config compiler → provision domain. The first vertical slice is **tenant/domain/account provisioning**: `POST /tenants` → Postgres (RLS on, `SET LOCAL` wrapper) → provision into sesame-idam + Stalwart via real API calls → audit row → visible in admin-console. That slice forces every hard decision above to be real code: the data-ownership boundary, the identity chain, the RLS pattern, the wrapper layer, and health/metrics. Abuse quarantine is slice 2; webmail read-path via JMAP is slice 3.

| # | Decision | Supersedes |
|---|----------|-----------|
| ADR-0005 | Product DB = control plane only; Stalwart owns mail state | doc 04 mail tables, doc 09 Citus sharding of `message` |
| ADR-0006 v2 | sesame-idam directory of record; Stalwart SQL-directory bridge; no homegrown auth in opengroupware | doc 08 §MFA/sessions/OAuth engine; ADR-0006 v1 (LLDAP+Authentik) |
| D3 | Track B removed from all active docs/spikes | doc 05 spikes 1/3, doc 09 forks, doc 02 compiler scope |
| D4 | `SET LOCAL` tenant GUC via single sqlx wrapper; BYPASSRLS admin role | doc 07 RLS SQL |
| D5 | Stalwart FTS; ClamAV via Rspamd; CloudNativePG; no Citus | doc 09 operators, doc 12 OpenSearch, tantivy wrapper |
| D6 | Store abuse decisions, not symbol replicas; MVP-6 wishlist deferred | doc 03 §evidence model |
| D7 | Pre-MVP scope amputations | docs 02/04/08 features listed above |
| D8 | Performance targets withdrawn pending benchmarks | doc 02:581, doc 09:1037 |
