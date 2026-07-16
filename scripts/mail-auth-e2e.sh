#!/bin/bash
# E2E milestone proof: IMAP authentication through the full identity chain.
#
#   IMAP client → Stalwart (SQL directory) → sesame rp_directory views
#                → argon2id PHC hash written by sesame conventions
#
# Spins throwaway containers (network og-e2e): postgres with the sesame
# migration chain + seeded user, and Stalwart configured per ADR-0006 v2 /
# PRD F3. Asserts: correct password authenticates, wrong password is
# rejected, suspended-tenant users cannot authenticate.
#
# Requires: docker, python3 + argon2-cffi (pip install --user argon2-cffi),
# a sibling checkout of sesame-idam (../sesame-idam or $SESAME_REPO).
#
# STATUS (2026-07-16): the DB half is fully proven — see
# scripts/rp-directory-smoke.sh (RP role reads credentials through the
# views, isolation holds). This script additionally stands up Stalwart and
# points its SQL directory at rp_directory. The remaining gap is the exact
# store-settings schema of the pinned Stalwart image
# (docker.io/stalwartlabs/stalwart): its --config parser rejected both TOML
# and our JSON ("missing field @type"), and mounting at the default path
# drops to bootstrap mode. Resolve against the shipped version's docs
# (`stalwart --help`, or extract the bootstrap-generated config) and update
# the config block below. The SQL query shape is already validated.
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SESAME_REPO="${SESAME_REPO:-$HERE/../sesame-idam}"
M="$SESAME_REPO/migrations"
NET=og-e2e
PG=e2e-sesame-pg
SW=e2e-stalwart
PASSWORD="correct-horse-battery"
# NOTE: ghcr.io/stalwart-mail/stalwart-mail does not exist (was in the
# original manifests) — the official image is docker.io/stalwartlabs/stalwart.
STALWART_IMG="${STALWART_IMG:-docker.io/stalwartlabs/stalwart:latest}"

cleanup() { docker rm -f $PG $SW >/dev/null 2>&1; docker network rm $NET >/dev/null 2>&1; }
[ "${1:-}" = "--clean" ] && { cleanup; echo cleaned; exit 0; }
cleanup

echo "== argon2id hash (sesame convention)"
PHC=$(python3 -c "import argon2; print(argon2.PasswordHasher().hash('$PASSWORD'))")
echo "${PHC:0:31}..."

docker network create $NET >/dev/null
docker run -d --name $PG --network $NET -e POSTGRES_PASSWORD=test \
    -e POSTGRES_USER=root -e POSTGRES_DB=sesame -p 15434:5432 postgres:16 >/dev/null
sleep 6

P() { docker exec -i $PG psql -U root -d sesame -v ON_ERROR_STOP=1 -q; }

echo "== apply sesame chain + rp_directory"
echo "CREATE SCHEMA sesame_idam; CREATE ROLE sesame_idam NOLOGIN;" | P || exit 1
P < "$M/rls/20260714180000_sesame_rls_contract_v1.sql" || exit 1
P < "$M/identity-login-service/20260714102157_tenants.sql" || exit 1
P < "$M/identity-user-mgmt-service/20260705235433_users.sql" || exit 1
P < "$M/identity-user-mgmt-service/20260716200000_app_passwords.sql" || exit 1
P < "$M/rls/20260716200001_rp_directory.sql" || exit 1

echo "== seed tenant/user + RP role"
docker exec -i $PG psql -U root -d sesame -v ON_ERROR_STOP=1 -q \
    -v phc="$PHC" <<'SQL' || exit 1
INSERT INTO sesame_idam.tenants (slug, display_name, status, provisioning_mode, created_at, updated_at)
VALUES ('acme', 'Acme', 'active', 'platform', now(), now()),
       ('ghost', 'Ghost', 'suspended', 'platform', now(), now());
INSERT INTO sesame_idam.users (email, password_hash, tenant_id, status, created_at, updated_at)
VALUES ('charles@acme.example', :'phc', 'acme', 'active', now(), now()),
       ('boo@ghost.example',   :'phc', 'ghost', 'active', now(), now());
CREATE ROLE rp_stalwart LOGIN PASSWORD 'rptest';
GRANT rp_directory_read TO rp_stalwart;
SQL

echo "== stalwart config (JSON — this image parses --config as JSON)"
TMP=$(mktemp -d)
python3 - "$TMP/config.json" "$PG" <<'PY'
import json, sys
out, pg = sys.argv[1], sys.argv[2]
cfg = {
    "server": {
        "hostname": "mail.e2e.local",
        "listener": {
            "imap": {"bind": ["0.0.0.0:143"], "protocol": "imap"},
            "http": {"bind": ["0.0.0.0:8080"], "protocol": "http"},
        },
    },
    "storage": {"data": "data", "fts": "data", "blob": "data",
                "lookup": "data", "directory": "sesame"},
    "store": {
        "data": {"@type": "rocksdb", "path": "/data"},
        "sesame-directory": {
            "@type": "postgresql", "host": pg, "port": 5432,
            "database": "sesame", "user": "rp_stalwart", "password": "rptest",
            "query": {
                "name": "SELECT login AS name, 'individual' AS type, secret_phc AS secret, display_name AS description, 0 AS quota FROM rp_directory.users WHERE login = $1",
                "emails": "SELECT login AS email FROM rp_directory.users WHERE login = $1",
            },
        },
    },
    "directory": {
        "sesame": {
            "@type": "sql", "store": "sesame-directory",
            "columns": {"name": "name", "type": "type", "secret": "secret",
                        "description": "description", "quota": "quota"},
        },
    },
    "authentication": {"fallback-admin": {"user": "admin", "secret": "e2e-admin"}},
    "tracer": {"stdout": {"@type": "stdout", "level": "info", "ansi": False}},
}
json.dump(cfg, open(out, "w"), indent=2)
print("wrote", out)
PY

docker run -d --name $SW --network $NET -p 1143:143 -p 1580:8080 \
    -v "$TMP/config.json":/e2e/config.json \
    "$STALWART_IMG" --config /e2e/config.json >/dev/null
sleep 10
docker logs $SW 2>&1 | grep -iE "failed|error|listen|ready|imap|started" | head -6

echo
echo "== TEST 1: correct password must authenticate"
if curl -s -m 15 "imap://127.0.0.1:1143/" --user "charles@acme.example:$PASSWORD" -X "CAPABILITY" >/dev/null; then
    echo "PASS: authenticated via rp_directory"
else
    echo "FAIL: auth rejected (rc=$?)"; docker logs $SW 2>&1 | tail -15
fi

echo "== TEST 2: wrong password must be rejected"
if curl -s -m 15 "imap://127.0.0.1:1143/" --user "charles@acme.example:wrong" -X "CAPABILITY" >/dev/null 2>&1; then
    echo "FAIL: wrong password accepted!"
else
    echo "PASS: rejected"
fi

echo "== TEST 3: suspended-tenant user must be rejected"
if curl -s -m 15 "imap://127.0.0.1:1143/" --user "boo@ghost.example:$PASSWORD" -X "CAPABILITY" >/dev/null 2>&1; then
    echo "FAIL: suspended tenant authenticated!"
else
    echo "PASS: rejected"
fi

echo
echo "done — cleanup with: $0 --clean"
