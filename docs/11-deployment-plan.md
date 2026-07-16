# 11 — OpenGroupware Deployment Plan

This document provides the complete deployment plan for OpenGroupware on Kubernetes,
integrating our Helm charts and deployment-configuration kustomize overlays.

## Overview

OpenGroupware deploys as two categories of workloads:

1. **Our services** — Rust binaries built from the workspace crates, deployed as
   standard Deployments via the generic `opengroupware-microservice` Helm chart.
2. **Wrapped external services** — Stalwart Mail Server, Rspamd, MinIO, and Search,
   deployed as sidecar StatefulSets/Deployments controlled through the same Helm chart
   but under the `.Values.external.*` keys.

### Architecture

```
                                    ┌─────────────────────────────┐
                                    │     Ingress + TLS           │
                                    │  (cert-manager + nginx)     │
                                    └──────────┬──────────────────┘
                                               │
          ┌────────────────────────────────────┼────────────────────────────────────┐
          │                                    │                                    │
          ▼                                    ▼                                    ▼
   ┌─────────────┐                   ┌─────────────────┐                ┌─────────────┐
   │  webmail     │                   │  admin-api       │                │  abuse-api  │
   │  (Leptos SSR)│                   │  (Admin REST)    │                │  (Spam/     │
   │  :3000       │                   │  :8080           │                │   Abuse)    │
   └──────┬──────┘                   └────────┬────────┘                └──────┬──────┘
          │                                    │                               │
          │          ┌─────────────────────────┼───────────────────────────┐   │
          │          │                         │                           │   │
          ▼          ▼                         ▼                           ▼   ▼
   ┌──────────────┐ ┌──────────────┐   ┌──────────────┐          ┌──────────────┐
   │  job-runner   │ │ config-      │   │  admin-      │          │  search      │
   │  (async/cron) │ │ compiler     │   │  console     │          │  (OpenSearch)│
   │  :8082        │ │ :8083        │   │  :3001       │          │  :9200       │
   └──────┬───────┘ └──────┬───────┘   └──────────────┘          └──────────────┘
          │                 │
          │                 ▼
          │         ┌──────────────┐
          │         │  Stalwart    │
          │         │  (IMAP/SMTP) │
          │         │  :8080       │
          │         └──────┬───────┘
          │                │
          ▼                ▼
   ┌────────────────────────────────────────────┐
   │              Shared Data Layer             │
   │  ┌──────────┐ ┌───────┐ ┌────────┐ ┌────┐ │
   │  │PostgreSQL│ │Redis  │ │ MinIO  │ │Rsp│ │
   │  │  (Citus) │ │ Cluster│ │ S3     │ │amd│ │
   │  └──────────┘ └───────┘ └────────┘ └────┘ │
   └────────────────────────────────────────────┘
```

## Service inventory

### OpenGroupware services

| Crate | Binary | Port | K8s Kind | Replicas | Resource limits |
|-------|--------|------|----------|----------|-----------------|
| admin-api | admin-api | 8080 | Deployment | 2 | 256Mi/512Mi, 100m/300m |
| abuse-api | abuse-api | 8081 | Deployment | 1 | 128Mi/256Mi, 50m/150m |
| job-runner | job-runner | 8082 | Deployment | 1 | 256Mi/512Mi, 100m/500m |
| config-compiler | config-compiler | 8083 | Deployment | 1 | 256Mi/512Mi, 100m/500m |
| webmail | webmail | 3000 | Deployment | 2 | 128Mi/256Mi, 50m/200m |
| admin-console | admin-console | 3001 | Deployment | 1 | 128Mi/256Mi, 50m/200m |

### Wrapped external services

| Service | K8s Kind | Port | Persistence | Scaling |
|---------|----------|------|-------------|---------|
| Stalwart Mail Server | Deployment | 8080/143/25/8081 | PVC (data) + PostgreSQL | Single primary (Recreate strategy) |
| Rspamd | Deployment | 11334/11332 | ConfigMap only (stateless) | Horizontal |
| MinIO | StatefulSet | 9000/9001 | PVC (data) | Multi-node cluster |
| Search (OpenSearch) | StatefulSet | 9200 | PVC (data) | Multi-node cluster |

## Helm structure

### Directory layout

```
helm/
├── README.md                           # Usage documentation
├── opengroupware-microservice/         # Main Helm chart
│   ├── Chart.yaml
│   ├── values.yaml                     # Default values (generic)
│   ├── values/                         # Per-service overrides
│   │   ├── admin-api.yaml              # 2 replicas, admin API config
│   │   ├── abuse-api.yaml              # 1 replica, Rspamd integration
│   │   ├── config-compiler.yaml        # 1 replica, config generation
│   │   ├── job-runner.yaml             # 1 replica, async workers
│   │   ├── webmail.yaml                # 2 replicas, Leptos SSR
│   │   └── admin-console.yaml          # 1 replica, Leptos SSR
│   ├── templates/
│   │   ├── _helpers.tpl                # Chart helper templates
│   │   ├── deployment.yaml             # Generic Deployment manifest
│   │   ├── service.yaml                # Generic Service manifest
│   │   ├── configmap.yaml              # Generic ConfigMap manifest
│   │   ├── stalwart-deployment.yaml    # Stalwart with guard condition
│   │   ├── rspamd-deployment.yaml      # Rspamd with guard condition
│   │   ├── minio-statefulset.yaml      # MinIO StatefulSet with guard
│   │   └── search-statefulset.yaml     # Search StatefulSet with guard
└── charts/                             # (optional) Subchart templates
```

### Chart design principles

1. **Generic template** — One `deployment.yaml` template serves all 6 Rust services.
   Service identity is provided entirely through `values/<service>.yaml`.
2. **Guard conditions** — External services use `{{- if and .Values.external .Values.external.<name>.enabled }}`
   so they can be toggled on/off without modifying templates.
3. **Shared helpers** — `_helpers.tpl` provides common labels and naming conventions.
4. **ConfigMap injection** — Each service gets a dedicated ConfigMap from its values file.

### Deploying with Helm

```bash
# Deploy a single service
helm install admin-api ./helm/opengroupware-microservice \
  --namespace opengroupware \
  --create-namespace \
  --values ./helm/opengroupware-microservice/values/admin-api.yaml \
  --set external.stalwart.enabled=true \
  --set external.minio.enabled=true \
  --set external.search.enabled=true \
  --set external.rspamd.enabled=true

# Deploy all services at once
helm install opengroupware ./helm/opengroupware-microservice \
  --namespace opengroupware \
  --create-namespace \
  --values ./helm/opengroupware-microservice/values/admin-api.yaml \
  --values ./helm/opengroupware-microservice/values/abuse-api.yaml \
  --values ./helm/opengroupware-microservice/values/config-compiler.yaml \
  --values ./helm/opengroupware-microservice/values/job-runner.yaml \
  --values ./helm/opengroupware-microservice/values/webmail.yaml \
  --values ./helm/opengroupware-microservice/values/admin-console.yaml \
  --set external.stalwart.enabled=true \
  --set external.minio.enabled=true \
  --set external.search.enabled=true \
  --set external.rspamd.enabled=true

# Verify rendering
helm template opengroupware ./helm/opengroupware-microservice \
  --namespace opengroupware \
  --values ./helm/opengroupware-microservice/values/admin-api.yaml
```

## Deployment-configuration (Kustomize)

Mirrors the hauliage pattern: per-environment overlays with kustomize for secrets,
config, and namespace configuration.

### Directory layout

```
deployment-configuration/
├── profiles/
│   └── dev/
│       └── opengroupware/
│           ├── kustomization.yaml          # Top-level: runtime + bootstrap + services
│           ├── runtime/
│           │   ├── kustomization.yaml
│           │   └── secrets.yaml            # Database, JWT, API keys
│           ├── bootstrap/
│           │   ├── kustomization.yaml
│           │   └── migration-job.yaml      # DB migration job
│           ├── services/
│           │   ├── kustomization.yaml
│           │   ├── admin-api.yaml          # Per-service Deployment + Service
│           │   ├── abuse-api.yaml
│           │   ├── config-compiler.yaml
│           │   ├── job-runner.yaml
│           │   ├── webmail.yaml
│           │   └── admin-console.yaml
│           ├── external-services/
│           │   └── kustomization.yaml      # References helm subcharts
│           └── ingress/
│               ├── kustomization.yaml
│               ├── tls.yaml                # cert-manager Certificate
│               └── ingress.yaml            # Ingress rules
└── ... (staging, production profiles would follow)
```

### Per-service manifest pattern

Each service in `services/<name>.yaml` defines both a Deployment and Service:

```yaml
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: admin-api
  namespace: opengroupware
  labels:
    app: admin-api
spec:
  replicas: 2
  strategy:
    type: RollingUpdate
  selector:
    matchLabels:
      app: admin-api
  template:
    spec:
      containers:
        - name: admin-api
          image: ghcr.io/microscaler/opengroupware/admin-api:0.1.0
          imagePullPolicy: IfNotPresent
          command: ["/app/admin-api"]
          ports:
            - name: http
              containerPort: 8080
          env:
            - name: POD_NAMESPACE
              valueFrom:
                fieldRef:
                  fieldPath: metadata.namespace
            - name: RUST_LOG
              value: "info"
          livenessProbe:
            httpGet:
              path: /health
              port: http
          readinessProbe:
            httpGet:
              path: /health
              port: http
          resources:
            requests:
              memory: "256Mi"
              cpu: "100m"
            limits:
              memory: "512Mi"
              cpu: "300m"
          volumeMounts:
            - name: config
              mountPath: /etc/opengroupware
              readOnly: true
            - name: logs
              mountPath: /var/log/opengroupware
            - name: tmp
              mountPath: /tmp
      volumes:
        - name: config
          configMap:
            name: admin-api-config
        - name: logs
          emptyDir: {}
        - name: tmp
          emptyDir:
            medium: Memory
            sizeLimit: 64Mi
---
apiVersion: v1
kind: Service
metadata:
  name: admin-api
  namespace: opengroupware
  annotations:
    prometheus.io/scrape: "true"
    prometheus.io/port: "8080"
    prometheus.io/path: "/metrics"
spec:
  type: ClusterIP
  ports:
    - port: 8080
      targetPort: 8080
      protocol: TCP
      name: http
  selector:
    app: admin-api
```

### Secrets

All secrets are Kubernetes `Opaque` Secrets. **Default values must be changed in production:**

| Secret Name | Purpose | Fields |
|-------------|---------|--------|
| `opengroupware-database-credentials` | PostgreSQL connection | DB_HOST, DB_PORT, DB_NAME, DB_USER, DB_PASSWORD |
| `opengroupware-jwt-secret` | JWT signing | JWT_SECRET |
| `opengroupware-rspamd-key` | Rspamd controller | RSPAMD_PASSWORD |
| `stalwart-database-credentials` | Stalwart PostgreSQL | username, password |
| `stalwart-admin-credentials` | Stalwart admin | password |
| `minio-credentials` | MinIO storage | access-key, secret-key |
| `rspamd-credentials` | Rspamd controller | password |

For production, replace plain Kubernetes Secrets with a secrets manager
(HashiCorp Vault, AWS Secrets Manager, GCP Secret Manager) with CSI driver injection.

### Ingress

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: opengroupware-ingress
  annotations:
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
    cert-manager.io/cluster-issuer: letsencrypt-prod
spec:
  tls:
    - hosts:
        - CHANGE_ME.mail.example.com
        - CHANGE_ME-admin.example.com
      secretName: opengroupware-tls-secret
  rules:
    - host: CHANGE_ME.mail.example.com
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: webmail
                port:
                  number: 3000
    - host: CHANGE_ME-admin.example.com
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: admin-console
                port:
                  number: 3001
          - path: /api
            pathType: Prefix
            backend:
              service:
                name: admin-api
                port:
                  number: 8080
```

## Deployment sequence

### 0. Prerequisites

- Kubernetes 1.28+ cluster
- cert-manager installed (for TLS)
- Container registry (ghcr.io or private registry) with built images
- PostgreSQL 16+ cluster (external or operator-managed)
- Redis 7+ cluster (external or operator-managed)

### 1. Build and push images

```bash
# Build all Rust binaries
cargo build --release --workspace

# Build Docker images for each crate
for crate in admin-api abuse-api job-runner config-compiler webmail admin-console; do
  docker build -t ghcr.io/microscaler/opengroupware/${crate}:0.1.0 crates/${crate}
  docker push ghcr.io/microscaler/opengroupware/${crate}:0.1.0
done

# External service images (use published releases)
# stalwart-mail:latest from ghcr.io/stalwart-mail/stalwart-mail
# rspamd:latest from docker.io/rspamd/rspamd
# minio:latest from docker.io/minio/minio
# opensearch:2.11 from docker.io/opensearchproject/opensearch
```

### 2. Deploy data layer first

The data layer (PostgreSQL, Redis, MinIO, Search) must exist before deploying
the OpenGroupware services.

**With Kustomize (dev profile):**
```bash
kustomize build deployment-configuration/profiles/dev/opengroupware/runtime | kubectl apply -f -
kustomize build deployment-configuration/profiles/dev/opengroupware/external-services | kubectl apply -f -
```

**With Helm:**
```bash
# Set external service values and deploy
helm install opengroupware-external ./helm/opengroupware-microservice \
  --namespace opengroupware \
  --set external.stalwart.enabled=true \
  --set external.minio.enabled=true \
  --set external.search.enabled=true \
  --set external.rspamd.enabled=true \
  --set external.minio.persistence.enabled=true \
  --set external.search.persistence.enabled=true \
  --set external.minio.persistence.size=50Gi \
  --set external.search.persistence.size=50Gi
```

### 3. Run database migration

```bash
kubectl apply -f deployment-configuration/profiles/dev/opengroupware/bootstrap/
```

### 4. Deploy application services

```bash
# With Kustomize
kustomize build deployment-configuration/profiles/dev/opengroupware/core/services | kubectl apply -f -

# With Helm (individual installs)
for svc in admin-api abuse-api job-runner config-compiler webmail admin-console; do
  helm upgrade --install $svc ./helm/opengroupware-microservice \
    --namespace opengroupware \
    --values ./helm/opengroupware-microservice/values/${svc}.yaml \
    --set service.name=${svc}
done
```

### 5. Deploy ingress

```bash
kustomize build deployment-configuration/profiles/dev/opengroupware/ingress | kubectl apply -f -
```

## Wrapped external services — deployment plan

### Stalwart Mail Server

**Deployment strategy: Recreate (single primary)**

Stalwart does not support multi-node active-active clustering. Deploy as a
single Deployment with `strategy.type: Recreate`. For high availability:

- **Read scale:** Multiple replicas behind a load balancer (IMAP/JMAP reads)
- **Write scale:** Single primary handles all writes
- **Storage:** Shared S3 (MinIO) for blob storage
- **Metadata:** Shared PostgreSQL for account/domain data

**Dependencies:**
- PostgreSQL (shared across all services)
- MinIO (S3-compatible storage)
- Redis (cache/sessions)

**Ports exposed:**
- 8080 — HTTP/JMAP admin
- 143 — IMAP
- 25 — SMTP
- 8081 — JMAP API

### Rspamd

**Deployment strategy: Horizontal pool**

Rspamd is stateless (except for shared Redis state). Deploy multiple replicas:

**Dependencies:**
- Redis (shared learning pool, rate limiting)
- Rspamd web controller password (stored in secret)

**Ports exposed:**
- 11334 — HTTP web interface
- 11332 — Milter protocol (for SMTP integration)

### MinIO

**Deployment strategy: StatefulSet with PVC**

MinIO requires persistent storage for bucket data. Deploy as a StatefulSet:

**Dependencies:**
- StorageClass (SSD-backed preferred)
- MinIO credentials (access-key + secret-key in secret)

**Ports exposed:**
- 9000 — S3 API
- 9001 — Web console

**Scaling:**
- Phase 1: Single node (single PVC)
- Phase 2: Multi-node Erasure Coding (4+ nodes, each with own PVC)
- Use `minio/headless` service for discovery

### Search (OpenSearch)

**Deployment strategy: StatefulSet with PVC**

OpenSearch requires persistent storage for index data. Deploy as a StatefulSet:

**Dependencies:**
- StorageClass (SSD-backed, high IOPS)
- Minimum 3 nodes for quorum

**Ports exposed:**
- 9200 — REST API
- 9300 — Node-to-node (not exposed in our config)

**Scaling:**
- Phase 1: 1 node (single PVC, no HA)
- Phase 2: 3 nodes (quorum, each with own PVC)
- Phase 3: 5+ nodes (data nodes + master-only nodes)

## Monitoring and observability

### Health checks

All services expose a `/health` endpoint for liveness and readiness probes.

### Prometheus metrics

All services expose `/metrics` on port 9090 (internal) or the service port.
Service annotations enable Prometheus scraping:

```yaml
annotations:
  prometheus.io/scrape: "true"
  prometheus.io/port: "9090"
  prometheus.io/path: "/metrics"
```

### Resource allocation summary

| Service | Memory requests | Memory limits | CPU requests | CPU limits |
|---------|----------------|---------------|--------------|------------|
| admin-api | 256Mi | 512Mi | 100m | 300m |
| abuse-api | 128Mi | 256Mi | 50m | 150m |
| job-runner | 256Mi | 512Mi | 100m | 500m |
| config-compiler | 256Mi | 512Mi | 100m | 500m |
| webmail | 128Mi | 256Mi | 50m | 200m |
| admin-console | 128Mi | 256Mi | 50m | 200m |
| **Subtotal** | **1.25Gi** | **2.5Gi** | **450m** | **1.85** |
| Stalwart | 512Mi | 2Gi | 200m | 1000m |
| Rspamd | 256Mi | 512Mi | 100m | 500m |
| MinIO | 512Mi | 1Gi | 200m | 500m |
| Search | 512Mi | 2Gi | 200m | 1000m |
| **External subtotal** | **1.75Gi** | **5.5Gi** | **700m** | **3.0** |
| **TOTAL** | **3.0Gi** | **8.0Gi** | **1150m** | **4.85** |

## Rollout strategy

1. **Blue/Green** — Deploy new version to parallel namespace, switch ingress
2. **RollingUpdate** — Default strategy for all Deployments (maxSurge=25%, maxUnavailable=25%)
3. **Recreate** — Used for Stalwart (single primary, no concurrent writes)
4. **StatefulSet** — Used for MinIO/Search (ordered rollout, PVC affinity)

## Disaster recovery

- **Database backup:** PostgreSQL `pg_basebackup` + WAL archiving to MinIO
- **Config backup:** `git push` of deployment-configuration/ to version control
- **TLS cert rotation:** cert-manager handles automatically via Let's Encrypt
- **Secret rotation:** Manual rotation of Kubernetes Secrets (or Vault integration)

## Multi-environment support

To support staging/production, copy `profiles/dev/opengroupware/` to
`profiles/staging/opengroupware/` or `profiles/production/opengroupware/`
and adjust:

- `namespace` in kustomization.yaml
- `replicas` in service manifests
- `resources` limits
- Ingress hostnames and TLS certs
- Secret values (credentials, JWT keys)

The Helm chart structure supports the same pattern — each environment gets its
own values file that overrides the chart defaults.
