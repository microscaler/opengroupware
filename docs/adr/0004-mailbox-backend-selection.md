# ADR-0004: Mailbox backend selection — Track A (Stalwart)

- **Status**: Accepted
- **Date**: 2026-07-16
- **Supersedes**: ADR-0004 (pending)

## Context

The product needs a mailbox backend that provides SMTP, IMAP, JMAP, CalDAV,
CardDAV, and Sieve. Two tracks were evaluated:

1. **Track A**: Stalwart — integrated Rust backend providing all protocols.
2. **Track B**: Postfix + Dovecot + separate DAV — composable Unix stack.

## Decision

**Track A: Stalwart** is selected as the integrated backend.

## Rationale

1. **Multi-tenancy is native.** Stalwart has a built-in tenant model with
   per-tenant policies, quotas, domains, and users. Track B requires the config
   compiler to generate tenant-scoped configuration for every component, and
   tenant leakage risks are higher (wrong SQL query, wrong namespace, wrong
   filesystem path).

2. **Fewer moving parts.** Stalwart is one Rust binary that provides SMTP, IMAP,
   JMAP, CalDAV, CardDAV, and Sieve. Track B requires Postfix, Dovecot, Cyrus
   (or Radicale), and multiple configuration files.

3. **JMAP support.** Only Stalwart provides native JMAP. JMAP Push is a superior
   basis for a modern webmail client over IMAP IDLE + SSE.

4. **Rust codebase.** The product plane is being written in Rust. Stalwart is
   also Rust, so the provider interfaces, config rendering, and operational
   patterns share a language and mindset.

5. **Config simplicity.** Track A requires config for one service (Stalwart) +
   Rspamd. Track B requires config for Postfix + Dovecot + DAV service + Rspamd,
   each with their own configuration format and reload semantics.

6. **Operational simplicity for K8s.** Deploying one container (Stalwart) + one
   sidecar (Rspamd) is simpler than managing four separate services with
   interdependencies.

## Consequences

- **Younger codebase.** Stalwart is 2023+, not 25+ years old. Community is
  growing but smaller. Risk is acceptable given the multi-tenancy advantage.
- **Vendor lock-in (reduced).** The config compiler abstracts Stalwart-specific
  config, but the protocol layer (JMAP, CalDAV) is standardized. Switching the
  backend later would require re-rendering the config, not changing the product
  APIs.
- **Provider interface design.** The MailProvider trait must model Stalwart's
  native tenant model (tenant_id, domain_id, user_id) rather than Postfix's
  virtual-domain abstraction.
- **Docker images.** Stalwart publishes official Docker images, simplifying K8s
  deployment.

## Alternatives considered

| Alternative | Why not |
|-------------|---------|
| Postfix + Dovecot | More components, no native multi-tenancy, no JMAP, more config |
| Postfix + Cyrus | Older, more complex ACL model, no JMAP |
| Haraka + custom DAV | Node.js ecosystem, no mailbox store, all custom |

## Related decisions

- ADR-0001: Protocol-first architecture (JMAP is a required protocol)
- ADR-0002: Rspamd as abuse engine (works with Stalwart)
- ADR-0011: Kubernetes deployment (Stalwart has Docker images)
