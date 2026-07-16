# 02 — Greenfield Architecture

This is the proposed system architecture for a from-scratch open mailgroupware platform built by gluing FOSS components.

## Architectural principles

1. **Protocol-first**: SMTP, IMAP, JMAP, CalDAV, CardDAV, WebDAV, OIDC, LDAP compatibility.
2. **Replaceable infrastructure**: no component should be impossible to swap.
3. **Product-owned control plane**: the admin model, policy model, abuse workflow, migration, backup, and UX are custom.
4. **Abuse-first delivery**: mail must pass through scoring, authentication, malware scanning, URL checks, and policy before final mailbox delivery.
5. **Restore-first operations**: if backup/restore is not built early, the platform is not production-ready.

## C4-style context diagram

```mermaid
flowchart TB
  classDef actor fill:#fff,stroke:#333,color:#111
  classDef system fill:#e8f0fe,stroke:#1565c0,color:#111
  classDef external fill:#e8f7e8,stroke:#2e7d32,color:#111
  classDef risk fill:#fdeaea,stroke:#b71c1c,color:#111

  User[End user]:::actor
  Admin[Domain admin]:::actor
  Sender[External sender]:::actor
  Recipient[External recipient]:::actor

  Platform[Open Mailgroupware Platform]:::system

  DNS[DNS: MX, SPF, DKIM, DMARC, MTA-STS]:::external
  IdP[External IdP / SSO]:::external
  ObjectStore[S3-compatible storage]:::external
  Monitoring[Monitoring/alerting stack]:::external
  ThreatIntel[DNSBL/URIBL/reputation feeds]:::risk

  Sender -->|SMTP inbound| Platform
  Platform -->|SMTP outbound| Recipient
  User -->|Web/JMAP/IMAP/DAV| Platform
  Admin -->|Admin UI/API| Platform
  Platform --> DNS
  Platform --> IdP
  Platform --> ObjectStore
  Platform --> Monitoring
  Platform --> ThreatIntel
```

## Container/service diagram

```mermaid
flowchart TB
  classDef custom fill:#e8f0fe,stroke:#1565c0,color:#111
  classDef foss fill:#e8f7e8,stroke:#2e7d32,color:#111
  classDef data fill:#f3e5f5,stroke:#6a1b9a,color:#111
  classDef edge fill:#fff7d6,stroke:#f9a825,color:#111

  Internet((Internet))
  Browser[Browser users/admins]
  MailClients[Mail clients]

  subgraph Edge[Edge]
    LB[TLS/load balancer]:::edge
    Proxy[Reverse proxy<br/>Nginx/Traefik/Caddy]:::foss
  end

  subgraph Product[Custom product services]
    WebApp[Webmail/groupware app]:::custom
    AdminAPI[Admin API]:::custom
    ConfigCompiler[Provisioning/config compiler]:::custom
    JobRunner[Jobs: migration, backup, training]:::custom
    AbuseAPI[Abuse/quarantine API]:::custom
  end

  subgraph Identity[Identity]
    IdP[OIDC provider]:::foss
    LDAP[LDAP compatibility]:::foss
  end

  subgraph Mail[Mail services]
    SMTP[SMTP ingress/submission]:::foss
    Filter[Rspamd policy/scoring]:::foss
    AV[ClamAV/clamd]:::foss
    Mailbox[Mailbox backend]:::foss
    DAV[JMAP/CalDAV/CardDAV/WebDAV]:::foss
  end

  subgraph Data[Data]
    ProductDB[(PostgreSQL)]:::data
    Redis[(Redis/Valkey)]:::data
    Blob[(Message/blob/object storage)]:::data
    Search[(Search index)]:::data
    Quarantine[(Quarantine)]:::data
    BackupStore[(Backup repository)]:::data
  end

  Internet --> LB --> Proxy
  Browser --> Proxy
  MailClients --> Proxy

  Proxy --> WebApp
  Proxy --> AdminAPI
  Proxy --> AbuseAPI
  Proxy --> SMTP
  Proxy --> Mailbox
  Proxy --> DAV

  WebApp --> Mailbox
  WebApp --> DAV
  AdminAPI --> ProductDB
  AdminAPI --> ConfigCompiler
  ConfigCompiler --> SMTP
  ConfigCompiler --> Filter
  ConfigCompiler --> Mailbox
  ConfigCompiler --> IdP
  ConfigCompiler --> LDAP

  SMTP --> Filter
  Filter --> AV
  Filter --> Redis
  Filter --> Quarantine
  Filter --> Mailbox
  Mailbox --> Blob
  Mailbox --> Search
  DAV --> ProductDB
  AbuseAPI --> Quarantine
  JobRunner --> ProductDB
  JobRunner --> Mailbox
  JobRunner --> BackupStore
```

## Inbound mail delivery sequence

```mermaid
sequenceDiagram
  autonumber
  participant Sender as External sender MTA
  participant MX as SMTP ingress
  participant Rspamd as Rspamd
  participant Clam as ClamAV/clamd
  participant Policy as Product policy API
  participant Quar as Quarantine
  participant Store as Mailbox store
  participant User as User mailbox

  Sender->>MX: SMTP connection
  MX->>MX: Cheap connection checks<br/>TLS, HELO, postscreen, rate limits
  MX->>Rspamd: Scan envelope + headers + body
  Rspamd->>Rspamd: SPF/DKIM/DMARC/ARC checks
  Rspamd->>Rspamd: Bayes/neural/fuzzy/URL checks
  Rspamd->>Clam: Scan attachments/body streams
  Clam-->>Rspamd: Malware verdict
  Rspamd->>Policy: Tenant/user/domain policy lookup
  Policy-->>Rspamd: Thresholds, allow/deny, VIP rules

  alt Reject
    Rspamd-->>MX: reject
    MX-->>Sender: SMTP reject
  else Quarantine
    Rspamd->>Quar: Store message + symbols + evidence
    Rspamd-->>MX: accept and quarantine
  else Deliver with tag/header
    Rspamd-->>MX: add headers/score/action
    MX->>Store: LMTP/internal delivery
    Store->>User: Message visible in mailbox
  end
```

## Outbound mail sequence

Outbound filtering matters as much as inbound filtering. A compromised mailbox can destroy domain reputation quickly.

```mermaid
sequenceDiagram
  autonumber
  participant User as Authenticated user/client
  participant Submit as SMTP submission
  participant Auth as Auth/OIDC/LDAP
  participant Policy as Rate-limit/policy service
  participant Rspamd as Rspamd outbound scan
  participant DKIM as DKIM signer
  participant Queue as Outbound queue
  participant Remote as Remote MX
  participant SecOps as Abuse console

  User->>Submit: Submit message
  Submit->>Auth: Verify auth/session
  Auth-->>Submit: Account/domain identity
  Submit->>Policy: Check rate, geo, device, reputation

  alt Suspicious account behavior
    Policy-->>Submit: throttle or hold
    Submit->>SecOps: Create incident
  else Allowed
    Policy-->>Submit: allowed
    Submit->>Rspamd: Scan outbound content and recipients
    Rspamd-->>Submit: score/action
    Submit->>DKIM: Sign message
    DKIM->>Queue: enqueue
    Queue->>Remote: SMTP delivery
  end
```

## Control plane flow

```mermaid
flowchart LR
  classDef custom fill:#e8f0fe,stroke:#1565c0,color:#111
  classDef foss fill:#e8f7e8,stroke:#2e7d32,color:#111
  classDef data fill:#f3e5f5,stroke:#6a1b9a,color:#111

  Admin[Admin changes domain/user/policy]:::custom
  API[Admin API]:::custom
  DB[(Product DB)]:::data
  Compiler[Config compiler]:::custom
  Render[Render desired configs]:::custom
  Validate[Validate and diff]:::custom
  Apply[Apply with staged rollout]:::custom
  Services[Postfix/Rspamd/Dovecot/Stalwart/etc.]:::foss
  Audit[(Audit log)]:::data

  Admin --> API --> DB
  API --> Audit
  DB --> Compiler --> Render --> Validate --> Apply --> Services
  Apply --> Audit
```

## Configuration ownership model

```mermaid
flowchart TB
  Desired[(Desired state<br/>product DB)]
  Compiler[Config compiler]
  Generated[Generated service configs]
  Runtime[Runtime services]
  Drift[Drift detector]
  Alert[Admin alert]

  Desired --> Compiler --> Generated --> Runtime
  Runtime --> Drift
  Desired --> Drift
  Drift -->|drift detected| Alert
  Drift -->|safe reconcile| Compiler
```

The platform should avoid hand-edited production configs. Admin changes should produce desired state; the compiler renders service-specific configs; drift detection tells the operator when reality diverges.

## Deployment shapes

### Single-node MVP

```mermaid
flowchart TB
  Node[Single VM/bare metal node]
  Node --> Proxy[Proxy/TLS]
  Node --> SMTP[SMTP]
  Node --> Rspamd[Rspamd]
  Node --> ClamAV[ClamAV]
  Node --> Mailbox[Mailbox]
  Node --> DB[PostgreSQL]
  Node --> Redis[Redis/Valkey]
  Node --> Blob[Local blob store]
  Node --> Backup[Restic/Borg to remote target]
```

### Small cluster

```mermaid
flowchart TB
  Edge[Edge nodes<br/>proxy + SMTP ingress]
  App[App/control nodes<br/>web + admin + jobs]
  Mail[Mailbox nodes]
  Data[Data layer<br/>PostgreSQL + Redis + object storage]
  Obs[Observability]

  Edge --> App
  Edge --> Mail
  App --> Data
  Mail --> Data
  Edge --> Obs
  App --> Obs
  Mail --> Obs
```

### Enterprise later

```mermaid
flowchart TB
  MultiTenant[Multi-tenant control plane]
  RegionalIngress[Regional SMTP ingress]
  HAStore[Highly available mailbox/object storage]
  Archive[Immutable archive/legal hold]
  SSO[Enterprise SSO/SCIM]
  Compliance[Audit/eDiscovery]

  MultiTenant --> RegionalIngress
  MultiTenant --> HAStore
  MultiTenant --> Archive
  MultiTenant --> SSO
  MultiTenant --> Compliance
```

## Multi-tenancy isolation model

All tenants share PostgreSQL, Redis, blob storage, and search index. Isolation is
enforced at the application layer via **tenant-scoped queries** and **row-level
security (RLS)** on PostgreSQL.

```mermaid
flowchart TB
  classDef custom fill:#e8f0fe,stroke:#1565c0,color:#111
  classDef foss fill:#e8f7e8,stroke:#2e7d32,color:#111
  classDef data fill:#f3e5f5,stroke:#6a1b9a,color:#111

  TenantA[Tenant A]:::custom
  TenantB[Tenant B]:::custom
  TenantC[Tenant C]:::custom

  TenantA --> Proxy[API gateway / Proxy]:::foss
  TenantB --> Proxy
  TenantC --> Proxy

  Proxy --> Auth[OIDC auth / Tenant resolver]:::foss
  Auth --> TenantA_Context[Tenant A context]
  Auth --> TenantB_Context[Tenant B context]
  Auth --> TenantC_Context[Tenant C context]

  TenantA_Context --> PG[(PostgreSQL)]:::data
  TenantB_Context --> PG
  TenantC_Context --> PG

  TenantA_Context --> Redis[(Redis / Valkey)]:::data
  TenantB_Context --> Redis
  TenantC_Context --> Redis

  PG --> PG_RLS[RLS policies<br/>tenant_id = current_tenant]:::foss
  PG --> PG_Partition[Schema partition by tenant<br/>optional for data-heavy tenants]:::foss

  Redis --> Redis_TenantKeys[Tenant-scoped keys<br/>tenant:A:..., tenant:B:... ]:::foss

  Proxy --> Blob[S3-compatible storage]:::data
  Blob --> Blob_Prefix[Prefix isolation<br/>tenant-A/... tenant-B/... ]:::foss
  Blob --> Blob_IAM[IAM bucket policies]:::foss
```

### Isolation layers

| Layer | Mechanism | Enforced by |
|-------|-----------|-------------|
| Query isolation | `tenant_id` FK on every row, RLS on PostgreSQL | Product DB + RLS policies |
| Cache isolation | Redis keys prefixed by tenant (`tenant:{id}:...`) | Config compiler |
| Storage isolation | S3 prefixes (`tenant-{id}/`) + IAM policies | Blob backend |
| Search isolation | Tenant-scoped search index partitions | Config compiler |
| SMTP isolation | Per-tenant connection limits, queue partitioning | MTA config |
| Resource quotas | `TENANT_RESOURCE_QUOTA` enforced at API layer | Admin API |

### Tenant lifecycle

```mermaid
flowchart LR
  A[Create tenant] --> B[Create PG schema partition]
  B --> C[Allocate S3 prefix]
  C --> D[Create RLS policies]
  D --> E[Create Redis key namespace]
  E --> F[Apply initial config]
  F --> G[Active]
  G --> H[Overage warning]
  G --> I[Overage action]
  G --> J[Suspend]
  J --> K[Grace period]
  K --> L[Terminate + data export]
```

### Policy

- Default: single PostgreSQL database with RLS on all tenant-scoped tables.
- Data-heavy tenants (10K+ accounts): schema-per-tenant optional via config compiler.
- Redis: keyspace isolation via prefix convention + config compiler enforced at write time.
- S3: one bucket, prefix-based isolation with per-tenant IAM policies (future).
- Search: tenant-scoped queries with index-level filtering (no cross-tenant visibility).

## High availability and disaster recovery

### RPO/RTO targets

| Component | RPO | RTO | Strategy |
|-----------|-----|-----|----------|
| Product DB (PostgreSQL) | 1 min | 30s | Streaming replication + automated failover |
| Redis/Valkey | 0 loss | 60s | Redis Sentinel or Redis Cluster |
| Mailbox backend | 5 min | 5 min | Shared storage or hot standby |
| Blob/object storage | 15 min | 15 min | S3 cross-region replication |
| Search index | 5 min | 10 min | Near-real-time replication |

### Small cluster — with HA

```mermaid
flowchart TB
  classDef edge fill:#fff7d6,stroke:#f9a825,color:#111
  classDef custom fill:#e8f0fe,stroke:#1565c0,color:#111
  classDef foss fill:#e8f7e8,stroke:#2e7d32,color:#111
  classDef data fill:#f3e5f5,stroke:#6a1b9a,color:#111
  classDef ha fill:#ffe8e8,stroke:#c62828,color:#111

  Internet((Internet))
  Browser[Browser users/admins]
  MailClients[Mail clients]

  subgraph Edge[Edge nodes<br/>proxy + SMTP ingress]
    LB[Load balancer<br/>HA pair]:::ha
    Proxy[Reverse proxy<br/>Nginx/Traefik/Caddy<br/>HA pair]:::foss
  end

  subgraph App[App/control nodes]
    App1[Webmail + Admin API<br/>Node 1]:::custom
    App2[Webmail + Admin API<br/>Node 2]:::custom
    ConfigCompiler[Config compiler<br/>Node 1 + Node 2]:::custom
    JobRunner[Jobs: migration, backup, training]:::custom
  end

  subgraph Mail[Mail services]
    SMTP1[SMTP/MSA Node 1]:::foss
    SMTP2[SMTP/MSA Node 2]:::foss
    Rspamd1[Rspamd Node 1]:::foss
    Rspamd2[Rspamd Node 2]:::foss
    ClamAV1[ClamAV Node 1]:::foss
    ClamAV2[ClamAV Node 2]:::foss
    Mailbox1[Mailbox Node 1]:::foss
    Mailbox2[Mailbox Node 2]:::foss
  end

  subgraph Data[Data layer]
    PG1[(PostgreSQL Primary<br/>Node 1)]:::data
    PG2[(PostgreSQL Standby<br/>Node 2)]:::data
    PG1 -.->|Streaming<br/>replication| PG2
    PG2 -.->|Auto-failover<br/>pg_auto_failover| PG1

    Redis1[(Redis Sentinel<br/>3 nodes)]:::ha
    Redis2[Redis Sentinel<br/>monitor]:::ha

    Blob[(S3-compatible<br/>cross-region replication)]:::data

    Search1[(Search index<br/>replica)]:::data
    Search2[(Search index<br/>replica)]:::data
  end

  subgraph Backup[Backup/DR]
    Restic[Restic/Borg<br/>remote target]:::foss
    DR[DR region<br/>warm standby]:::ha
  end

  Browser --> Proxy
  MailClients --> Proxy
  Internet --> LB --> Proxy
  Proxy --> SMTP1
  Proxy --> SMTP2

  App1 --> PG1
  App2 --> PG1

  ConfigCompiler --> SMTP1
  ConfigCompiler --> SMTP2
  ConfigCompiler --> Mailbox1
  ConfigCompiler --> Mailbox2
  ConfigCompiler --> PG1

  SMTP1 --> Rspamd1
  SMTP2 --> Rspamd2
  Rspamd1 --> ClamAV1
  Rspamd2 --> ClamAV2
  Rspamd1 --> Redis1
  Rspamd2 --> Redis1
  Rspamd1 --> Mailbox1
  Rspamd2 --> Mailbox2

  Mailbox1 --> Blob
  Mailbox2 --> Blob
  Mailbox1 --> Search1
  Mailbox2 --> Search1

  PG2 --> Restic
  Restic --> DR
```

### Upgrade strategy

- Config compiler supports **staged rollout**: renders configs for one node at a time,
  validates health, then proceeds to next.
- Rolling restart of mail services: SMTP → Rspamd → Mailbox (in order).
- PostgreSQL failover tested quarterly; runbook stored with DR region.

## Operational model

### Health checks

Every service exposes `/health` (liveness) and `/ready` (readiness) endpoints.
Liveness = process is alive; readiness = dependencies (DB, cache, downstream
services) are responsive. Load balancer uses readiness to drain traffic before
shutdown.

### Logging and tracing

All services log structured JSON with correlation IDs (X-Correlation-Id header
propagated through the stack). OpenTelemetry trace context embedded in logs.
Log aggregation: Loki + Promtail or OpenTelemetry collector → Loki.

### Configuration schema validation

The config compiler validates against **JSON Schema definitions** for each service
(Postfix, Rspamd, Dovecot, Stalwart). Before applying a new config:

1. Compiler renders desired-state → generated config
2. JSON Schema validator checks structural correctness
3. Dry-run validation against live service (syntax check)
4. If validation passes: apply to target node(s) in staged order
5. Post-apply health check confirms service is accepting traffic

### Secrets management

| Secret type | Storage | Rotation |
|-------------|---------|----------|
| PostgreSQL passwords | HashiCorp Vault / SSM Parameter Store | Config compiler injects at runtime |
| DKIM private keys | Vault / Kubernetes Secrets | Quarterly + domain transfer |
| SMTP submission auth | LDAP/OIDC (external) | N/A |
| TLS certificates | ACME (Let's Encrypt) / Vault | Auto-renewal every 90 days |
| API keys (admin) | Vault / Kubernetes Secrets | On-rotate, 90-day expiry |
| Rspamd secrets | Vault / env vars (K8s) | Quarterly |

### Internal service-to-service auth

mTLS with mutual certificate exchange between services. Each service has a
service identity certificate. Internal API calls require valid mTLS client cert.
Config compiler provisions certificates to each service at bootstrap.

## Protocol coverage

### Required (MVP)

| Protocol | Purpose | Implementation |
|----------|---------|----------------|
| SMTP (ESMTP) | Ingress/submission | Postfix / Stalwart |
| SMTPUTF8 | RFC 6531 — non-ASCII addresses | Enabled at MTA level |
| IMAP4rev1 | Mail access | Dovecot / Stalwart |
| IMAP IDLE | Push notification (real-time) | Backend-implemented |
| JMAP | Modern web app API | Stalwart or dedicated gateway |
| CalDAV | Calendar | Stalwart / Radicale |
| CardDAV | Contacts | Stalwart / Radicale |
| WebDAV | Files/documents | Stalwart / Radicale |
| ManageSieve | User-side rules | Dovecot / Stalwart |
| OIDC | Authentication | Keycloak / Authentik |

### Should have (post-MVP)

| Protocol | Purpose | Notes |
|----------|---------|-------|
| POP3 | Archival clients | Low demand but standard |
| S/MIME | Message-level encryption | Client + server certificate store |
| PGP/MIME | Message-level encryption | OpenPGP integration |
| IMAP MOVE / UIDPLUS | Efficient client sync | Dovecot supports natively |
| IMAP QRESYNC | Fast reconnection sync | Reduces bandwidth |
| SMTP 8BITMIME | Binary attachments | Required by RFC 6152 |
| EAI / SMTPUTF8 | Full RFC 6531 compliance | Non-ASCII addresses |
| JMAP Push | Real-time webmail updates | JMAP-native push |
| OAuth 2.0 / PKCE | Third-party app auth | For web API integrations |

### Deferred

| Protocol | Purpose | Notes |
|----------|---------|-------|
| ActiveSync (EAS) | Mobile enterprise | Defer to IMAP+CalDAV first |
| EWS / MAPI | Outlook connector | Huge compatibility sink |
| XMPP / Chat | Real-time messaging | Out of scope for MVP |
| WebRTC / Video | Video calling | Third-party integration |

### Webmail push strategy

If using JMAP: JMAP Push is native (server-initiated events).
If using IMAP: IMAP IDLE from backend → Server-Sent Events (SSE) or WebSocket
to browser. JMAP Push preferred for new development.

## Performance targets

| Metric | Target | Notes |
|--------|--------|-------|
| SMTP throughput | 10K msg/min per node | Sustained, peak 2x |
| Concurrent SMTP connections | 5K per node | Rate-limited by config compiler |
| IMAP concurrent connections | 10K per node | Per mailbox backend |
| Search latency | < 200ms p95 | Per-tenant index |
| API response time | < 500ms p95 | Admin API |
| Webmail load time | < 2s first paint | With 10K messages |
| Backup restore | 100GB/hour | Per backup repository |

### Connection pooling

PostgreSQL connections use **PgBouncer** in transaction pooling mode. Admin API
and config compiler connect via PgBouncer pool (max 500 connections per primary).
Read replicas use separate PgBouncer pools for read-heavy admin queries.

