# ADR-0006 (v2): Identity — sesame-idam as directory of record

- **Status**: Accepted (v2 supersedes v1 same-day)
- **Date**: 2026-07-16
- **v1 → v2**: v1 selected LLDAP + Authentik. Owner directive (Charles,
  2026-07-16): use the in-house **sesame-idam** platform
  (`microscaler/sesame-idam`, `microscaler/sesame-idam-client`) instead.
  Authentik/LLDAP are removed from the deployment profile.

## Context

sesame-idam is a multi-tenant, OpenAPI-first Rust IDAM (PropelAuth-style)
on the microscaler stack (may/BRRTRouter/Lifeguard, Postgres + RLS, Redis).
Assessment (2026-07-16) of what exists today:

**Solid:** tenant model with `UNIQUE(tenant_id, email)` and RLS zero-bleed
tests; argon2id password hashing; orgs/roles/permissions with inheritance;
RFC 8693 token exchange (~40 unit tests); session service design; helm/k8s
deployment scaffolding.

**Not production-ready yet:** JWTs signed with a placeholder (no real
RS256/JWKS); auth-code issuance and client_credentials incomplete; no token
introspection; TOTP MFA is a stub; no WebAuthn; 241 TODO markers; tree may
not currently compile. `sesame-idam-client` is a typed SDK that does **not**
verify JWTs (trusts edge validation).

**Structural gaps for a mail platform:** no LDAP server, and no per-user
app-password concept. Stalwart authenticates IMAP/SMTP/DAV against an
internal, LDAP, SQL, or OIDC directory — sesame-idam currently satisfies
none of these in production form.

## Decision

1. **sesame-idam is the directory of record** for tenants, users, orgs,
   roles, and credentials. The opengroupware product DB stores zero
   credentials; admin-api provisions users via the sesame-idam API using a
   client-credentials integration secret (`sesame-idam-integration`).
2. **Web SSO** (webmail, admin-console, abuse console) uses sesame-idam
   OIDC once its RS256/JWKS/auth-code path is finished. Until then, product
   web surfaces are considered blocked on sesame-idam's OIDC milestone —
   we do not build an interim homegrown login.
3. **Stalwart protocol auth bridges via Stalwart's SQL directory** pointed
   at a dedicated read-only view/schema exposing (tenant, login, argon2id
   secret, app-passwords, quota). Rationale: sesame-idam already stores
   argon2id PHC hashes in Postgres and Stalwart's SQL directory verifies
   standard hash formats — this is the shortest credible bridge and avoids
   writing an LDAP server. Revisit OIDC-directory once sesame-idam
   introspection exists.
4. **App passwords become a sesame-idam feature** (new table + issue/revoke
   API, argon2id-hashed, per-user, scoped "mail"), surfaced to Stalwart
   through the same SQL view. Doc 04's one-time-use model remains rejected.
5. **MFA (TOTP/WebAuthn) is delivered inside sesame-idam**, not in
   opengroupware. Its absence today is accepted MVP risk, sequenced on the
   sesame-idam roadmap.
6. **No Authentik, no LLDAP, no homegrown auth in opengroupware.** Doc 08's
   credential/MFA/session engine remains cancelled; its requirements
   transfer to the sesame-idam backlog.

## Working rule (owner directive, 2026-07-16)

**Boy-scout rule:** when sesame-idam lacks something opengroupware needs,
the opengroupware owner writes a PRD in the sesame-idam repo and builds the
feature there, framed as a general SaaS-IDAM capability (never a
mail-specific hack). Other sesame agents pause while this happens.
opengroupware serves as sesame's dogfooding harness, surfacing edge cases.
First PRD: `sesame-idam/docs/PRD-OPENGROUPWARE-RELYING-PARTY.md` (OIDC
completion, app passwords, trusted-RP directory bridge, client-side JWT
verification, compile fixes).

## Work this creates (cross-repo)

- sesame-idam: finish RS256 signing + JWKS + auth-code + introspection;
  app-password API; Stalwart SQL-directory view + migration; fix
  compile-blocking issues (`auth_token.rs` async-in-sync call, arity bug).
- opengroupware: `IdentityProvider` trait implemented against sesame-idam
  API; service-kit middleware validating sesame JWTs via JWKS (the client
  SDK explicitly does not verify — we must); deployment reference to
  sesame-idam's own helm release.

## Consequences

- Full-stack control and dogfooding of the in-house IDAM; no third-party
  identity containers.
- The mail MVP's critical path now includes sesame-idam hardening. This is
  the single largest schedule risk accepted in this ADR, taken consciously
  in exchange for owning the identity product.
- The microscaler runtime (may coroutines) differs from opengroupware's
  tokio stack; integration is API-level only — no shared runtime code
  except the client SDK where it fits.
