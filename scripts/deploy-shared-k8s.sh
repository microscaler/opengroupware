#!/bin/bash
# Deploy the opengroupware control plane to the shared-k8s cluster.
# Idempotent; safe to re-run. Steps are gated by --step for control.
#
#   ./deploy-shared-k8s.sh image     # build + push admin-api
#   ./deploy-shared-k8s.sh db        # create DB + roles in shared postgres
#   ./deploy-shared-k8s.sh apply     # ns, secret, migration job, admin-api
#   ./deploy-shared-k8s.sh verify    # rollout status + in-cluster provision test
#   ./deploy-shared-k8s.sh all
set -uo pipefail

export KUBECONFIG="${KUBECONFIG:-$HOME/Workspace/microscaler/shared-k8s-cluster/kubeconfig/shared-k8s.yaml}"
REPO="$HOME/Workspace/microscaler/opengroupware"
REG=10.177.76.220:5000
IMG="$REG/opengroupware/admin-api:dev-2"
NS=opengroupware
# Shared postgres (data namespace). Superuser creds discovered at db step.
PG_SVC="postgres.data.svc.cluster.local"
K() { kubectl "$@"; }

step_image() {
  echo "== build admin-api image from warm binary"
  rm -rf /tmp/ogimg && mkdir -p /tmp/ogimg/target/debug
  cp "$REPO/target/debug/admin-api" /tmp/ogimg/target/debug/
  cp "$REPO/docker/Dockerfile.prebuilt" /tmp/ogimg/
  ( cd /tmp/ogimg && docker build -f Dockerfile.prebuilt \
      --build-arg CRATE=admin-api --build-arg PROFILE=debug -t "$IMG" . )
  docker push "$IMG"
}

step_db() {
  echo "== ensure opengroupware DB + roles in shared postgres"
  local su pwmig pwapp
  su=$(K -n data get secret postgres-credentials -o jsonpath='{.data.postgres-password}' | base64 -d)
  # Generate app/migrator passwords once and persist for the secret step.
  mkdir -p /tmp/ogsecret
  [ -f /tmp/ogsecret/mig ] || openssl rand -hex 20 > /tmp/ogsecret/mig
  [ -f /tmp/ogsecret/app ] || openssl rand -hex 20 > /tmp/ogsecret/app
  pwmig=$(cat /tmp/ogsecret/mig); pwapp=$(cat /tmp/ogsecret/app)

  # Run DDL from a throwaway psql pod against pgpool (routes to primary).
  K -n data delete pod ogpsql --ignore-not-found >/dev/null 2>&1
  K -n data run ogpsql --image=postgres:16 --restart=Never \
    --env=PGPASSWORD="$su" --command -- sleep 300
  K -n data wait --for=condition=ready pod/ogpsql --timeout=60s
  PS() { K -n data exec -i ogpsql -- psql -h postgres.data.svc.cluster.local -U postgres "$@"; }
  PS -tc "SELECT 1 FROM pg_database WHERE datname='opengroupware'" | grep -q 1 \
    || PS -c "CREATE DATABASE opengroupware"
  PS -v ON_ERROR_STOP=1 <<SQL
DO \$do\$ BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname='og_migrator') THEN
    CREATE ROLE og_migrator LOGIN PASSWORD '$pwmig';
  ELSE ALTER ROLE og_migrator PASSWORD '$pwmig'; END IF;
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname='og_app') THEN
    CREATE ROLE og_app LOGIN PASSWORD '$pwapp';
  ELSE ALTER ROLE og_app PASSWORD '$pwapp'; END IF;
END \$do\$;
GRANT ALL ON DATABASE opengroupware TO og_migrator;
SQL
  K -n data exec ogpsql -- psql -h postgres.data.svc.cluster.local -U postgres \
    -d opengroupware -c "GRANT ALL ON SCHEMA public TO og_migrator;"
  K -n data delete pod ogpsql --ignore-not-found >/dev/null 2>&1
  echo "db ready"
}

# Generate DB credentials as a k8s Secret with random values. Never
# committed; the manifest references it via secretKeyRef.
step_secret() {
  K create namespace "$NS" --dry-run=client -o yaml | K apply -f -
  local su app
  su=$(openssl rand -hex 24); app=$(openssl rand -hex 24)
  K -n "$NS" create secret generic og-db-credentials \
    --from-literal=superuser-password="$su" \
    --from-literal=app-password="$app" \
    --dry-run=client -o yaml | K apply -f -
  echo "og-db-credentials secret created/rotated"
}

step_apply() {
  echo "== namespace + secret + self-contained workloads (own postgres)"
  step_secret
  K apply -f "$REPO/scripts/k8s/admin-api-deploy.yaml"
}

# After migration creates the opengroupware_app NOLOGIN role + table grants,
# make the og_app login role (created by initdb) a member so the restricted
# app can read/write under RLS.
step_grant() {
  K -n "$NS" wait --for=condition=complete job/og-migrate --timeout=180s
  local pod
  pod=$(K -n "$NS" get pods -l app=og-postgres -o name | head -1)
  K -n "$NS" exec "${pod#pod/}" -- psql -U postgres -d opengroupware \
    -c "GRANT opengroupware_app TO og_app;"
  K -n "$NS" rollout restart deploy/admin-api
  echo "membership granted"
}

step_verify() {
  echo "== migration job"
  K -n "$NS" wait --for=condition=complete job/og-migrate --timeout=120s || \
    K -n "$NS" logs job/og-migrate --tail=30
  echo "== admin-api rollout"
  K -n "$NS" rollout status deploy/admin-api --timeout=120s
  K -n "$NS" get pods
  echo "== in-cluster provision smoke"
  K -n "$NS" run og-smoke --rm -i --restart=Never --image=curlimages/curl:8.11.0 -- \
    sh -c "curl -s -X POST http://admin-api:8080/api/v1/tenants -H 'content-type: application/json' -H 'x-actor: deploy-smoke' -d '{\"slug\":\"acme\",\"name\":\"Acme\"}'; echo; curl -s http://admin-api:8080/health; echo"
}

case "${1:-all}" in
  image) step_image ;;
  db) step_db ;;
  secret) step_secret ;;
  apply) step_apply ;;
  verify) step_verify ;;
  grant) step_grant ;;
  all) step_image && step_apply && step_grant && step_verify ;;
  *) echo "unknown step: $1"; exit 1 ;;
esac
