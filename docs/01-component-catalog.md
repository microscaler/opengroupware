# 01 — Component Catalog

This document maps the main components needed for a greenfield Zimbra-like alternative.

Labels:

- **FOSS commodity**: use as-is or lightly configure.
- **Custom product**: should be built from scratch because this is where the product differentiation lives.
- **Optional commercial**: defer until the FOSS/product core works.
- **Replaceable**: select one implementation, but keep the boundary abstract.

## Component status map

| Layer | Component | Implementation | Label | Notes |
|---|---|---|---|---|
| Admin/control plane | Admin UI/API | Custom app | Custom product | Domain, user, policy, queue, abuse, backup, and migration surface. |
| Provisioning | Config compiler | Custom service | Custom product | Converts product model into Stalwart/Rspamd config. |
| Web UX | Webmail/calendar/contacts | Custom web app | Custom product | Product-defining experience. Do not copy legacy Zimbra UX. |
| Identity | OIDC | Authentik | FOSS/replaceable | SaaS-native multi-tenancy. |
| Directory compatibility | LDAP | LLDAP | FOSS/replaceable | Lightweight Rust LDAP server. |
| SMTP ingress | MTA | Stalwart SMTP | FOSS/commodity | Track A integrated backend. |
| Abuse engine | Spam/phishing scoring | Rspamd | FOSS/commodity | Multi-tenant capable via Redis key prefixing. |
| Malware scan | AV daemon | ClamAV/clamd | FOSS/commodity | Stateless — no multi-tenant concerns. |
| Policy state | Cache/state | Redis | FOSS/commodity | ACL-based key prefixing for tenant isolation. |
| Mailbox | Store/backend | Stalwart | FOSS/commodity | Track A integrated backend — IMAP, JMAP, CalDAV, CardDAV all included. |
| Filters | Sieve/ManageSieve | Stalwart Sieve | FOSS/commodity | Tenant-isolated via parent backend. |
| Calendar | CalDAV/JMAP Calendar | Stalwart | FOSS/commodity | Built into Stalwart stack. |
| Contacts | CardDAV/JMAP Contacts | Stalwart | FOSS/commodity | Built into Stalwart stack. |
| Search | Search engine | Tantivy | FOSS/commodity | Embedded Rust library — no separate service. Tenant-scoped index files. |
| Blob storage | Message/attachment storage | MinIO | FOSS/commodity | S3-compatible. Prefix + IAM for tenant isolation. |
| Product DB | Metadata/control DB | PostgreSQL | FOSS/commodity | RLS for tenant isolation. |
| Observability | Metrics/logs/traces | Prometheus, Grafana, Loki, OpenTelemetry | FOSS/commodity | Required for serious operations. |
| Backup | Backup/restore | pgBackRest + Restic | FOSS/custom | Per-tenant schema restore via pgBackRest; blob via Restic. |
| Migration | Import/export | imapsync + custom orchestrator | FOSS/custom | IMAP-based migration with checkpoint/resume. |
| Mobile enterprise | ActiveSync/EAS | Defer | Defer | IMAP + CalDAV/CardDAV first. |
| Outlook enterprise | EWS/MAPI | Defer | Defer | Huge compatibility sink. |
| Compliance | Legal hold/eDiscovery | Defer | Defer | Defer until core is reliable. |
|| DMARC reporting | Aggregate/forensic reports | Rspamd + custom receiver service | FOSS/commercial | Receive and store DMARC reports for deliverability analysis. |
|| DMARC auto-remediation | Auto-apply DMARC policy | Rspamd policy integration | FOSS commodity | Reject/quarantine based on DMARC fail — no manual config needed. |
|| Outbound shadow-copy | Enterprise security BCC | Custom service or Rspamd BCC | FOSS/commercial | Shadow-copy outbound messages for security audit. |
|| Threat intelligence | Blocklists, feed integration | Custom service + threat intel sources | FOSS/commercial | Store and apply threat intel to abuse pipeline. |
|| Quarantine digest | Daily/weekly user notifications | Custom notification service | FOSS/custom | Users receive digest of quarantined messages. |

## Component dependency graph

```mermaid
flowchart LR
  classDef custom fill:#e8f0fe,stroke:#1565c0,color:#111
  classDef foss fill:#e8f7e8,stroke:#2e7d32,color:#111
  classDef data fill:#f3e5f5,stroke:#6a1b9a,color:#111
  classDef optional fill:#fdeaea,stroke:#b71c1c,color:#111

  Admin[Admin UI/API<br/>Custom]:::custom
  Web[Webmail/groupware UX<br/>Custom]:::custom
  Provisioner[Config compiler<br/>Custom]:::custom
  AbuseUI[Abuse/quarantine UX<br/>Custom]:::custom
  ProductDB[(PostgreSQL<br/>Product DB)]:::data

  OIDC[Authentik<br/>OIDC provider]:::foss
  LDAP[LLDAP<br/>LDAP adapter]:::foss

  Stalwart[Stalwart<br/>SMTP/IMAP/JMAP/CalDAV]:::foss
  Rspamd[Rspamd<br/>spam/phishing/policy]:::foss
  ClamAV[ClamAV/clamd<br/>malware scan]:::foss
  Redis[(Redis<br/>filter state)]:::data
  MinIO[MinIO<br/>S3-compatible blob]:::foss
  Search[(Tantivy<br/>search index)]:::data

  ActiveSync[ActiveSync/EWS/MAPI<br/>later]:::optional
  Archive[eDiscovery/legal hold<br/>later]:::optional

  Admin --> ProductDB
  Admin --> Provisioner
  Admin --> AbuseUI
  Provisioner --> OIDC
  Provisioner --> LDAP
  Provisioner --> Stalwart
  Provisioner --> Rspamd
  Provisioner --> MinIO

  Web --> Stalwart
  Web --> Search

  Stalwart --> Rspamd
  Rspamd --> ClamAV
  Rspamd --> Redis
  Rspamd --> Stalwart
  Stalwart --> Search
  Stalwart --> MinIO
  Stalwart --> ActiveSync
  Stalwart --> Archive
```

## Build/glue/buy boundaries

```mermaid
mindmap
  root((Open Mailgroupware))
    Build from scratch
      Admin UI/API
      Product data model
      Config compiler
      Webmail UX
      Abuse dashboard
      Migration workflows
      Backup/restore UX
      Tenant billing hooks if needed
    Glue FOSS components
      Stalwart (SMTP/IMAP/JMAP/CalDAV/CardDAV)
      Rspamd
      ClamAV
      Authentik (OIDC)
      LLDAP (LDAP)
      PostgreSQL
      Redis
      MinIO (S3-compatible)
      Tantivy
      Prometheus/Grafana/Loki/pgBackRest/Restic
    Defer or commercialize
      ActiveSync
      EWS/MAPI/Outlook connector
      eDiscovery/legal hold
      Advanced archival storage tiering
      Managed enterprise support
      HA automation
```

## Selected stack (Track A + K8s)

```mermaid
flowchart TB
  Product[Custom product plane] --> Stalwart[Stalwart mail/collaboration server]
  Product --> Rspamd[Rspamd policy engine]
  Stalwart --> Storage[(MinIO S3 + PostgreSQL + Tantivy)]
  Stalwart --> Protocols[JMAP/IMAP/SMTP/CalDAV/CardDAV]
  Product --> Redis[(Redis filter state)]
  Product --> OIDC[Authentik OIDC]

  Stalwart --> Rspamd
  Rspamd --> ClamAV[ClamAV]
  Rspamd --> Redis
```

Stalwart is the integrated backend — it provides SMTP, IMAP, JMAP, CalDAV,
CardDAV, and Sieve in a single Rust codebase with native multi-tenancy. The
custom product plane sits above it: admin API, config compiler, web UI, abuse
console, migration tool, and backup controller. All infrastructure (Stalwart,
Rspamd, ClamAV, Redis, MinIO, PostgreSQL, Authentik, Tantivy) is deployed via
Helm charts in Kubernetes, managed with GitOps (ArgoCD or Flux).

Track B (Postfix/Dovecot) remains documented for reference but is superseded
by Track A for the reasons in the component audit.

