# ADR-0011: Deployment — Kubernetes, Helm, GitOps

- **Status**: Accepted
- **Date**: 2026-07-16

## Context

The product must support multi-tenant production deployments with:
- Horizontal scaling per component
- Zero-downtime upgrades
- Automated failover
- Infrastructure-as-code

## Decision

**Kubernetes** as the deployment target. **Helm** for package management. **ArgoCD**
(or Flux) for GitOps — all manifests and Helm values live in Git, ArgoCD syncs
to clusters.

## Rationale

1. **Sharding.** Kubernetes natively supports horizontal scaling via Deployment
   replicas, StatefulSet for stateful services, and HPA/VPA for auto-scaling.
   This is how we shard PostgreSQL (Citus), Redis (Cluster), search (multi-node),
   and stateless services (admin API, web UI).

2. **Config compiler as operator.** The config compiler becomes a K8s operator that
   watches CustomResourceDefinitions (TenantConfig, DomainConfig, PolicyProfile)
   and renders service configs. This is a natural fit for K8s's declarative model.

3. **GitOps discipline.** All infrastructure state (Helm values, K8s manifests,
   operator CRDs) lives in Git. Changes go through PR review. ArgoCD detects
   drift and reconciles. No hand-edited production configs — exactly the principle
   from ADR-0003.

4. **Multi-region.** K8s supports multi-region via cluster federation.
   Kubernetes Deployment/StatefulSet patterns translate directly to multi-region
   sharding (each region is a K8s cluster).

5. **Helm for templating.** Helm charts parameterize deployment per environment
   (dev, staging, production). Tenant-specific values (S3 endpoint, DB credentials)
   are injected via ArgoCD sync waves or sealed-secrets.

6. **Operator pattern for custom services.** The config compiler, admin API,
   migration tool, and backup controller are all custom Rust services deployed
   as Deployments with operator-style reconciliation loops.

## Consequences

- **Team learning curve.** If the team is not familiar with K8s, there's an
  onboarding cost. Mitigation: start with a single-node K3s cluster for
  development.
- **Helm chart development.** Each component (Stalwart, Rspamd, Redis, MinIO,
  PostgreSQL, our custom services) needs a Helm chart. Reuse community charts
  where available (bitnami, jetstack/cert-manager) and write custom charts for
  product services.
- **GitOps workflow.** All changes go through PR → merge → ArgoCD auto-sync.
  Emergency fixes require a hotfix PR. No `kubectl edit` in production.
- **Testing complexity.** E2E tests must run against a real K8s cluster (kind,
  k3s, or minikube). Local development uses k3d or k3s.

## Architecture

```
Git repository (GitOps)
  ├── infra/
  │   ├── helm/
  │   │   ├── stalwart/          # Stalwart chart
  │   │   ├── rspamd/            # Rspamd chart
  │   │   ├── redis/             # Redis (bitnami)
  │   │   ├── minio/             # MinIO (bitnami)
  │   │   ├── postgresql/        # PostgreSQL (bitnami)
  │   │   ├── authentik/         # Authentik (jetstack)
  │   │   ├── argocd/            # ArgoCD (argoproj)
  │   │   └── opengroupware/     # Product services
  │   │       ├── admin-api/
  │   │       ├── config-compiler/
  │   │       ├── webmail/
  │   │       └── abuse-console/
  │   └── k8s/
  │       ├── namespaces/        # Namespace definitions
  │       ├── network-policies/  # K8s NetworkPolicy
  │       ├── rbac/              # RBAC bindings
  │       ├── cert-manager/      # Certificates + Issuers
  │       └── storage/           # StorageClass + PVC templates
  ├── services/                  # Rust service crates (build → Docker)
  ├── packages/                  # SDK, config-schema
  └── docs/
```

## Related decisions

- ADR-0001: Protocol-first (K8s services speak standard protocols)
- ADR-0003: Product-owned desired state (config compiler operator)
- ADR-0007: Multi-tenant isolation (K8s NetworkPolicy enforces it)
