# 01 — Component Catalog

This document maps the main components needed for a greenfield Zimbra-like alternative.

Labels:

- **FOSS commodity**: use as-is or lightly configure.
- **Custom product**: should be built from scratch because this is where the product differentiation lives.
- **Optional commercial**: defer until the FOSS/product core works.
- **Replaceable**: select one implementation, but keep the boundary abstract.

## Component status map

| Layer | Component | Candidate implementation | Label | Notes |
|---|---|---|---|---|
| Admin/control plane | Admin UI/API | Custom app | Custom product | Domain, user, policy, queue, abuse, backup, and migration surface. |
| Provisioning | Config compiler | Custom service | Custom product | Converts product model into Postfix/Rspamd/Dovecot/Stalwart/etc. config. |
| Web UX | Webmail/calendar/contacts | Custom web app | Custom product | Product-defining experience. Do not copy legacy Zimbra UX. |
| Identity | OIDC | Keycloak, Authentik, Zitadel CE, external IdP | FOSS/replaceable | Prefer OIDC-first; keep LDAP compatibility for mail components. |
| Directory compatibility | LDAP | OpenLDAP, LLDAP, custom read-only LDAP adapter | FOSS/replaceable | Useful for legacy components and enterprise integrations. |
| SMTP ingress | MTA | Postfix, Stalwart SMTP, Haraka, OpenSMTPD | FOSS/replaceable | Do not rewrite early. |
| SMTP submission | MSA | Postfix or Stalwart SMTP | FOSS/replaceable | Needs auth, rate limits, outbound abuse controls. |
| Abuse engine | Spam/phishing scoring | Rspamd | FOSS commodity | Best default center of gravity for policy/scoring. |
| Malware scan | AV daemon | ClamAV/clamd | FOSS commodity | Integrate through Rspamd or MTA filter path. |
| Policy state | Cache/state | Redis or Valkey | FOSS commodity | Rspamd state, rate limits, greylisting, reputation. |
| Mailbox | Store/backend | Stalwart, Dovecot, Cyrus | FOSS/replaceable | Biggest architecture decision. |
| Mail client protocol | IMAP | Dovecot/Cyrus/Stalwart | FOSS commodity | Required for broad compatibility. |
| Modern app protocol | JMAP | Stalwart or dedicated gateway | FOSS/replaceable | Better basis for a modern web app than IMAP. |
| Filters | Sieve/ManageSieve | Dovecot/Cyrus/Stalwart Sieve | FOSS commodity | User mail rules and server-side filtering. |
| Calendar | CalDAV/JMAP Calendar | Stalwart, Radicale, DAViCal, SabreDAV, custom | FOSS/replaceable | Needs scheduling, invites, free/busy, sharing. |
| Contacts | CardDAV/JMAP Contacts | Stalwart, Radicale, SabreDAV, custom | FOSS/replaceable | Needs address books, GAL, sharing. |
| Search | Search engine | backend-native, OpenSearch, Xapian, Tantivy, Meilisearch | FOSS/replaceable | Keep indexing boundary isolated. |
| Blob storage | Message/attachment storage | filesystem, S3-compatible object store | FOSS/replaceable | Design for S3-compatible storage from the start. |
| Product DB | Metadata/control DB | PostgreSQL | FOSS commodity | Tenants, domains, policies, jobs, audit, quarantine metadata. |
| Observability | Metrics/logs/traces | Prometheus, Grafana, Loki, OpenTelemetry | FOSS commodity | Required for serious operations. |
| Backup | Backup/restore | Restic/Borg/snapshots/custom orchestrator | FOSS/custom | Productize restore flows early. |
| Migration | Import/export | imapsync, custom Zimbra TGZ parser, DAV import | FOSS/custom | Critical wedge for adoption. |
| Mobile enterprise | ActiveSync/EAS | z-push, commercial bridge, custom later | Optional commercial | Defer. IMAP + CalDAV/CardDAV first. |
| Outlook enterprise | EWS/MAPI | commercial bridge/custom later | Optional commercial | Defer. Huge compatibility sink. |
| Compliance | Legal hold/eDiscovery | custom archive service later | Optional commercial | Defer until core is reliable. |
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
  classDef replaceable fill:#fff7d6,stroke:#f9a825,color:#111
  classDef data fill:#f3e5f5,stroke:#6a1b9a,color:#111
  classDef optional fill:#fdeaea,stroke:#b71c1c,color:#111

  Admin[Admin UI/API<br/>Custom]:::custom
  Web[Webmail/groupware UX<br/>Custom]:::custom
  Provisioner[Config compiler<br/>Custom]:::custom
  AbuseUI[Abuse/quarantine UX<br/>Custom]:::custom
  ProductDB[(PostgreSQL<br/>Product DB)]:::data

  OIDC[OIDC provider<br/>Keycloak/AuthentiK/external]:::replaceable
  LDAP[LDAP adapter<br/>OpenLDAP/LLDAP/custom]:::replaceable

  MTA[SMTP/MTA<br/>Postfix/Stalwart/Haraka]:::replaceable
  Rspamd[Rspamd<br/>spam/phishing/policy]:::foss
  ClamAV[ClamAV/clamd<br/>malware scan]:::foss
  Redis[(Redis/Valkey<br/>filter state)]:::data
  Mailbox[Mailbox backend<br/>Stalwart/Dovecot/Cyrus]:::replaceable
  DAV[DAV/JMAP groupware<br/>Stalwart/Radicale/custom]:::replaceable
  Search[(Search index)]:::data
  Blob[(Blob/object storage)]:::data

  ActiveSync[ActiveSync/EWS/MAPI<br/>later]:::optional
  Archive[eDiscovery/legal hold<br/>later]:::optional

  Admin --> ProductDB
  Admin --> Provisioner
  Admin --> AbuseUI
  Provisioner --> OIDC
  Provisioner --> LDAP
  Provisioner --> MTA
  Provisioner --> Rspamd
  Provisioner --> Mailbox
  Provisioner --> DAV

  Web --> Mailbox
  Web --> DAV
  Web --> Search

  MTA --> Rspamd
  Rspamd --> ClamAV
  Rspamd --> Redis
  Rspamd --> Mailbox
  Mailbox --> Search
  Mailbox --> Blob
  Mailbox --> ActiveSync
  Mailbox --> Archive
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
      Postfix or Stalwart SMTP
      Rspamd
      ClamAV
      Dovecot/Cyrus/Stalwart
      Keycloak/OIDC
      OpenLDAP/LLDAP adapter
      PostgreSQL
      Redis/Valkey
      Object storage
      Prometheus/Grafana/Loki
    Defer or commercialize
      ActiveSync
      EWS/MAPI/Outlook connector
      eDiscovery/legal hold
      Advanced archival storage tiering
      Managed enterprise support
      HA automation
```

## Recommended candidate stacks

### Track A — Integrated backend

```mermaid
flowchart TB
  Product[Custom product plane] --> Stalwart[Stalwart mail/collaboration server]
  Product --> Rspamd[Rspamd if used externally or for custom policy]
  Stalwart --> Storage[(Storage/search/auth backends)]
  Stalwart --> Protocols[JMAP/IMAP/POP3/SMTP/CalDAV/CardDAV/WebDAV]
```

Best when the priority is a modern backend and fewer moving parts.

### Track B — Composable Unix stack

```mermaid
flowchart TB
  Product[Custom product plane] --> Postfix[Postfix]
  Product --> Dovecot[Dovecot or Cyrus]
  Product --> DAV[CalDAV/CardDAV service]
  Postfix --> Rspamd[Rspamd]
  Rspamd --> ClamAV[ClamAV]
  Rspamd --> Dovecot
  Dovecot --> Storage[(Mail storage)]
  DAV --> DB[(Groupware DB)]
```

Best when the priority is conservative components, operational familiarity, and easy replacement of individual services.

