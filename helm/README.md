# OpenGroupware Helm Charts

Kubernetes deployment configuration for OpenGroupware. Mirrors the hauliage pattern of generic Helm chart + per-service values files + kustomize overlays for environment-specific configuration.

## Directory structure

```
helm/
├── opengroupware-microservice/      # Generic Helm chart for all services
│   ├── Chart.yaml                   # Chart metadata
│   ├── values.yaml                  # Default values
│   ├── values/                      # Per-service values overrides
│   │   ├── admin-api.yaml
│   │   ├── abuse-api.yaml
│   │   ├── config-compiler.yaml
│   │   ├── job-runner.yaml
│   │   ├── webmail.yaml
│   │   └── admin-console.yaml
│   ├── templates/                   # Kubernetes manifests
│   │   ├── deployment.yaml          # Generic Deployment template
│   │   ├── service.yaml             # Generic Service template
│   │   ├── configmap.yaml           # Generic ConfigMap template
│   │   └── external/                # Wrapped external services
│   │       ├── stalwart-deployment.yaml
│   │       ├── rspamd-deployment.yaml
│   │       ├── minio-statefulset.yaml
│   │       └── search-statefulset.yaml
└── README.md                        # This file

deployment-configuration/
├── profiles/
│   └── dev/
│       └── opengroupware/           # Kustomize overlays
│           ├── kustomization.yaml   # Core: runtime, bootstrap, services
│           ├── core/
│           │   ├── bootstrap/       # Migration jobs, init containers
│           │   ├── runtime/         # Shared secrets, config
│           │   └── services/        # Per-service Deployment + Service
│           ├── external-services/   # Wrapped external services
│           └── ingress/             # TLS + ingress rules
```

## Deploying

### 1. Build images

```bash
# Build and push all Rust binaries
cargo build --release --workspace

# Build Docker images (using Dockerfile from each crate or a shared multi-stage)
docker build -t ghcr.io/microscaler/opengroupware/admin-api:0.1.0 crates/admin-api
docker build -t ghcr.io/microscaler/opengroupware/abuse-api:0.1.0 crates/abuse-api
# ... repeat for each binary
```

### 2. Deploy with Helm

```bash
# Deploy admin-api to dev namespace
helm install admin-api ./helm/opengroupware-microservice \
  --namespace opengroupware \
  --create-namespace \
  --values helm/opengroupware-microservice/values/admin-api.yaml

# Deploy all microservices
for svc in admin-api abuse-api job-runner config-compiler webmail admin-console; do
  helm install "$svc" ./helm/opengroupware-microservice \
    --namespace opengroupware \
    --create-namespace \
    --values "helm/opengroupware-microservice/values/${svc}.yaml" \
    --set "service.name=${svc}"
done
```

### 3. Deploy with Kustomize (dev profile)

```bash
# Deploy core services with kustomize
kustomize build deployment-configuration/profiles/dev/opengroupware | kubectl apply -f -

# Deploy external services
kustomize build deployment-configuration/profiles/dev/opengroupware/external-services | kubectl apply -f -

# Deploy ingress
kustomize build deployment-configuration/profiles/dev/opengroupware/ingress | kubectl apply -f -
```

## Service overview

| Service | Port | Description | Replicas |
|---------|------|-------------|----------|
| admin-api | 8080 | Admin REST API, tenant/account management | 2 |
| abuse-api | 8081 | Rspamd integration, abuse detection | 1 |
| job-runner | 8082 | Async job execution, cron workers | 1 |
| config-compiler | 8083 | Config generation, RLS policy compiler | 1 |
| webmail | 3000 | Leptos SSR webmail frontend | 2 |
| admin-console | 3001 | Leptos SSR admin console | 1 |

## External services (wrapped)

| Service | Description | Persistence |
|---------|-------------|-------------|
| Stalwart | Mail server, IMAP/SMTP, JMAP | PVC (data) + shared PostgreSQL |
| Rspamd | Spam/virus detection | ConfigMap only (stateless) |
| MinIO | S3-compatible blob storage | PVC (data) |
| Search | OpenSearch/Tantivy search index | PVC (data) |

## Secrets management

All secrets are stored as Kubernetes `Opaque` Secrets in the `opengroupware` namespace. **Change defaults before production:**

- `opengroupware-database-credentials` — PostgreSQL connection credentials
- `opengroupware-jwt-secret` — JWT signing key
- `opengroupware-rspamd-key` — Rspamd controller password
- `stalwart-database-credentials` — Stalwart PostgreSQL credentials
- `stalwart-admin-credentials` — Stalwart admin password
- `minio-credentials` — MinIO access/secret keys
- `rspamd-credentials` — Rspamd controller password

For production, use a secrets manager (HashiCorp Vault, AWS Secrets Manager, GCP Secret Manager) with CSI driver injection instead of plain Kubernetes Secrets.
