# 12 — External Services Deployment Plan

This document covers deployment of all wrapped infrastructure services — the FOSS
components that our custom product plane depends on.

## Component dependency inventory

The component catalog (`docs/01-component-catalog.md`) lists 15 infrastructure
components across 6 layers. Our Helm chart (`helm/opengroupware-microservice/`)
deploys all of them via a generic chart with guard conditions.

### Layer 1: Data layer (critical — deploy first)

| Component | K8s Kind | Purpose | PVC | Phase |
|-----------|----------|---------|-----|-------|
| **PostgreSQL** | StatefulSet | Product DB (RLS, tenant data) | Yes (data) | 1 |
| **Redis** | StatefulSet | Filter state, session cache, rate limiting | No (ephemeral) | 1 |
| **MinIO** | StatefulSet | S3-compatible blob storage | Yes (data) | 1 |
| **pgBouncer** | Deployment | Connection pooling for PostgreSQL | No | 1 |

### Layer 2: Mail stack (critical — deploy second)

| Component | K8s Kind | Purpose | PVC | Phase |
|-----------|----------|---------|-----|-------|
| **Stalwart** | Deployment | SMTP/IMAP/JMAP/CalDAV/CardDAV | Yes (data) | 1 |
| **Rspamd** | Deployment | Spam/phishing/scoring policy | No | 1 |
| **ClamAV** | Deployment | Malware scanning | No | 1 |

### Layer 3: Identity (deploy with data layer)

| Component | K8s Kind | Purpose | PVC | Phase |
|-----------|----------|---------|-----|-------|
| **Authentik** | Deployment | OIDC provider for tenant auth | Yes (data) | 1 |
| **LLDAP** | Deployment | LDAP directory compatibility | No | 1 |

### Layer 4: Search

| Component | K8s Kind | Purpose | PVC | Phase |
|-----------|----------|---------|-----|-------|
| **Search (OpenSearch)** | StatefulSet | Full-text search across mail, contacts, calendars | Yes (data) | 1 |
| **Tantivy** | Embedded | Rust embedded search (no separate service) | N/A | — |

### Layer 5: Observability (deploy alongside core)

| Component | K8s Kind | Purpose | PVC | Phase |
|-----------|----------|---------|-----|-------|
| **Prometheus** | StatefulSet | Metrics collection | Yes (data) | 1 |
| **Grafana** | Deployment | Dashboards | No | 1 |
| **Loki** | StatefulSet | Log aggregation | Yes (data) | 1 |
| **OpenTelemetry Collector** | DaemonSet | Trace/metric aggregation | No | 1 |

### Layer 6: Backup (deploy after data layer)

| Component | K8s Kind | Purpose | PVC | Phase |
|-----------|----------|---------|-----|-------|
| **pgBackRest** | CronJob | PostgreSQL backup to MinIO | No | 1 |
| **Restic** | CronJob | Blob backup to MinIO | No | 1 |

### Deferred (not in Phase 1)

| Component | Label | Reason |
|-----------|-------|--------|
| ActiveSync/EAS | Defer | IMAP + CalDAV/CardDAV first |
| EWS/MAPI | Defer | Huge compatibility sink |
| eDiscovery/legal hold | Defer | Defer until core is reliable |
| DMARC receiver | Custom service | Part of job-runner, separate infra |
| Shadow-copy service | Custom service | Part of job-runner, separate infra |

## Deployment order

The data layer must exist before any service that depends on it:

```
Phase 0: Cluster + cert-manager + StorageClass
Phase 1: PostgreSQL + Redis + MinIO + pgBouncer + Prometheus + Grafana + Loki + OTel
Phase 2: Stalwart + Rspamd + ClamAV + Authentik + LLDAP + Search
Phase 3: Our Rust services (admin-api, abuse-api, webmail, etc.)
Phase 4: Ingress + TLS
Phase 5: Backup cron jobs
```

## Per-service deployment patterns

### PostgreSQL

**StatefulSet with PVC.** Single primary for Phase 1. Read replicas for Phase 2+.

- **Image:** `docker.io/postgres:16-alpine` or operator (Citus)
- **Replicas:** 1 (Phase 1), 3+ with streaming replication (Phase 2)
- **PVC:** 50Gi SSD (Phase 1), grows with tenant count
- **Secrets:** `postgresql-credentials` (username, password, database name)
- **Ports:** 5432 — PostgreSQL wire protocol

Phase 2 uses Citus operator for logical sharding across worker nodes.

### Redis

**StatefulSet with emptyDir.** Ephemeral state for Phase 1. Redis Cluster for Phase 2.

- **Image:** `docker.io/redis:7-alpine`
- **Replicas:** 1 (Phase 1 sentinel), 6 (Phase 2 cluster: 3 masters + 3 replicas)
- **PVC:** None in Phase 1 (RDB persistence to disk), dedicated volumes in Phase 2
- **Secrets:** `redis-password` (AUTH password)
- **Ports:** 6379 — Redis protocol, 26379 — sentinel

Key prefix convention: `tenant:{id}:resource:{id}:{attr}`

### MinIO

**StatefulSet with PVC.** Erasure-coded from Phase 1 if multiple replicas.

- **Image:** `docker.io/minio/minio:latest`
- **Replicas:** 1 (Phase 1 single-node), 4+ (Phase 2 erasure coding)
- **PVC:** 50Gi (Phase 1), per-node PVCs for erasure coding (Phase 2)
- **Secrets:** `minio-credentials` (access-key, secret-key)
- **Ports:** 9000 — S3 API, 9001 — web console
- **Bucket:** `opengroupware` (tenant-isolated via IAM policies)

### pgBouncer

**Deployment.** Thin connection pooler in front of PostgreSQL.

- **Image:** `docker.io/deitch/pgbouncer:latest` or `edoburu/pgbouncer`
- **Replicas:** 1 (Phase 1), 3 active-passive (Phase 2)
- **PVC:** None
- **Secrets:** reads from `postgresql-credentials`
- **Ports:** 6432 — pooled PostgreSQL

### ClamAV

**Deployment.** Stateless virus scanner. Rspamd calls it via socket or TCP.

- **Image:** `docker.io/clamav/clamav:latest`
- **Replicas:** 1 (Phase 1), scale horizontally behind load balancer (Phase 2)
- **PVC:** None (fresh clamd config each restart)
- **Ports:** 3310 — clamd socket/TCP

### Authentik

**Deployment.** OIDC provider for tenant identity. Stateful.

- **Image:** `ghcr.io/goauthentik/server:2024`
- **Replicas:** 1 (Phase 1), 3+ active-active (Phase 2)
- **PVC:** 10Gi (database files)
- **Secrets:** `authentik-secret-key` (encryption key), `postgresql-credentials`
- **Ports:** 9000 — HTTP API, 9443 — HTTPS

### LLDAP

**Deployment.** Lightweight Rust LDAP server. Minimal footprint.

- **Image:** `docker.io/greenweb/lldap:latest`
- **Replicas:** 1
- **PVC:** 5Gi (SQLite file)
- **Secrets:** `lldap-secret-key`
- **Ports:** 389 — LDAP, 636 — LDAPS

### Prometheus

**StatefulSet with PVC.** Metrics collection for all services.

- **Image:** `docker.io/prom/prometheus:latest`
- **Replicas:** 1 (Phase 1), 2+ HA (Phase 2)
- **PVC:** 50Gi (metrics retention: 15d default)
- **Secrets:** none
- **Ports:** 9090 — HTTP API

Scrape targets: all OpenGroupware services (`prometheus.io/scrape: "true"` annotations), Node Exporter, PostgreSQL exporter, Redis exporter, MinIO exporter.

### Grafana

**Deployment.** Dashboards for infrastructure and application metrics.

- **Image:** `docker.io/grafana/grafana:latest`
- **Replicas:** 1 (Phase 1), 2+ HA (Phase 2)
- **PVC:** 10Gi (dashboards, provisioning config)
- **Secrets:** `grafana-admin-credentials`
- **Ports:** 3000 — HTTP

Datasources: Prometheus (metrics), Loki (logs).

### Loki

**StatefulSet with PVC.** Log aggregation for all services.

- **Image:** `docker.io/grafana/loki:latest` (single-binary mode for Phase 1)
- **Replicas:** 1 (Phase 1), 3+ (Phase 2)
- **PVC:** 50Gi (logs)
- **Secrets:** none
- **Ports:** 3100 — HTTP API

### OpenTelemetry Collector

**DaemonSet.** Runs on every node. Collects traces and metrics.

- **Image:** `docker.io/otel/opentelemetry-collector-contrib:latest`
- **Replicas:** DaemonSet (one per node)
- **PVC:** None
- **Secrets:** none
- **Ports:** 4317 — gRPC, 4318 — HTTP

### pgBackRest

**CronJob.** PostgreSQL backup to MinIO.

- **Image:** `docker.io/dimfeld/pgbackrest:latest`
- **Schedule:** Daily incremental, weekly full
- **PVC:** None (runs in pod, mounts MinIO as volume)
- **Secrets:** `postgresql-credentials`, `minio-credentials`
- **Dependencies:** PostgreSQL, MinIO

### Restic

**CronJob.** Blob backup (MinIO objects) to external storage.

- **Image:** `docker.io/restic/restic:latest`
- **Schedule:** Daily
- **PVC:** None
- **Secrets:** `restic-repository-password`
- **Dependencies:** MinIO (source), S3-compatible backup target

## Resource allocation (all infrastructure)

| Component | Memory requests | Memory limits | CPU requests | CPU limits | PVC |
|-----------|----------------|---------------|--------------|------------|-----|
| **Data layer** | | | | | |
| PostgreSQL | 1Gi | 4Gi | 500m | 2000m | 50Gi |
| Redis | 256Mi | 512Mi | 100m | 500m | — |
| MinIO | 512Mi | 1Gi | 200m | 500m | 50Gi |
| pgBouncer | 32Mi | 64Mi | 25m | 50m | — |
| **Mail stack** | | | | | |
| Stalwart | 512Mi | 2Gi | 200m | 1000m | — |
| Rspamd | 256Mi | 512Mi | 100m | 500m | — |
| ClamAV | 128Mi | 256Mi | 50m | 200m | — |
| **Identity** | | | | | |
| Authentik | 512Mi | 1Gi | 200m | 500m | 10Gi |
| LLDAP | 32Mi | 64Mi | 25m | 50m | 5Gi |
| **Search** | | | | | |
| Search (OpenSearch) | 512Mi | 2Gi | 200m | 1000m | 50Gi |
| **Observability** | | | | | |
| Prometheus | 512Mi | 2Gi | 200m | 1000m | 50Gi |
| Grafana | 128Mi | 256Mi | 50m | 200m | 10Gi |
| Loki | 256Mi | 512Mi | 100m | 500m | 50Gi |
| OTel Collector | 64Mi | 128Mi | 25m | 100m | — |
| **Subtotal** | **4.5Gi** | **16Gi** | **1.975** | **8.15** | **215Gi** |
| **Our services** | **1.25Gi** | **2.5Gi** | **450m** | **1.85** | **0Gi** |
| **TOTAL Phase 1** | **5.75Gi** | **18.5Gi** | **2.425** | **10** | **215Gi** |

## Secrets inventory

| Secret Name | Namespace | Components using it | Fields |
|-------------|-----------|-------------------|--------|
| `postgresql-credentials` | opengroupware | PostgreSQL, pgBouncer, Stalwart, Authentik, pgBackRest | username, password, database |
| `redis-password` | opengroupware | Redis, Rspamd, Stalwart | password |
| `minio-credentials` | opengroupware | MinIO, Stalwart (storage), pgBackRest | access-key, secret-key |
| `authentik-secret-key` | opengroupware | Authentik | secret-key, password |
| `lldap-secret-key` | opengroupware | LLDAP | secret-key |
| `grafana-admin-credentials` | opengroupware | Grafana | username, password |
| `rspamd-credentials` | opengroupware | Rspamd | password |
| `stalwart-admin-credentials` | opengroupware | Stalwart | password |
| `restic-repository-password` | opengroupware | Restic, pgBackRest | password |

For production: replace Kubernetes Secrets with HashiCorp Vault, AWS Secrets Manager,
or GCP Secret Manager with CSI driver injection.
