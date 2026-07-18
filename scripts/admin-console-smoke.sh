#!/bin/bash
# Live smoke for admin-console: seed control-plane data, run admin-api +
# admin-console, and assert the rendered HTML contains real tenants, a tenant's
# domains/accounts, and its abuse-policy thresholds.
#
# Portable: runs on the ms02 build host. Isolated CARGO_TARGET_DIR.
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO"
export PATH="$HOME/.cargo/bin:/usr/local/bin:/opt/homebrew/bin:$PATH"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/jr_target}"
BIN="$CARGO_TARGET_DIR/debug"

C=console-pg
SU_PW=$(openssl rand -hex 8); APP_PW=$(openssl rand -hex 8)
DB_PORT=15438; API_PORT=18080; UI_PORT=18090
API=http://127.0.0.1:$API_PORT; UI=http://127.0.0.1:$UI_PORT
T1=aaaaaaaa-0000-0000-0000-000000000001
D1=bbbbbbbb-0000-0000-0000-000000000001

cleanup() { pkill -x admin-console 2>/dev/null; pkill -x admin-api 2>/dev/null; docker rm -f $C >/dev/null 2>&1; }
cleanup

docker run -d --name $C -e POSTGRES_PASSWORD="$SU_PW" -e POSTGRES_USER=postgres \
  -e POSTGRES_DB=opengroupware -p $DB_PORT:5432 postgres:16 >/dev/null
echo "== waiting for postgres"; sleep 7

echo "== build admin-api + admin-console"
cargo build -q -p admin-api -p admin-console 2>&1 | tail -3

PGPASSWORD="$SU_PW" DATABASE_URL="postgres://postgres@127.0.0.1:$DB_PORT/opengroupware" \
  MIGRATE_ONLY=1 OPENGROUPWARE_ALLOW_UNSAFE_DB=1 "$BIN/admin-api" >/tmp/ac-mig.log 2>&1
grep -q "migrations applied" /tmp/ac-mig.log && echo "migrate: ok" || { echo "migrate: FAIL"; cat /tmp/ac-mig.log; }

docker exec -e PGPASSWORD="$SU_PW" $C psql -U postgres -d opengroupware -v ON_ERROR_STOP=1 \
  -c "CREATE ROLE og_app LOGIN PASSWORD '$APP_PW';" \
  -c "GRANT opengroupware_app TO og_app;" >/dev/null && echo "role: ok"

echo "== seed tenants/domains/accounts"
docker exec -e PGPASSWORD="$SU_PW" $C psql -U postgres -d opengroupware -v ON_ERROR_STOP=1 \
  -c "INSERT INTO tenants (id,slug,name,plan,status) VALUES
       ('$T1','acme','Acme','standard','active'),
       ('aaaaaaaa-0000-0000-0000-000000000002','globex','Globex','standard','active');" \
  -c "INSERT INTO domains (id,tenant_id,fqdn,status) VALUES
       ('$D1','$T1','acme.example','active'),
       ('bbbbbbbb-0000-0000-0000-000000000002','$T1','old.acme.example','suspended');" 2>/tmp/ac-seed.log >/dev/null && echo "seed t/d: ok" || sed -n '1,3p' /tmp/ac-seed.log
docker exec -e PGPASSWORD="$SU_PW" $C psql -U postgres -d opengroupware -v ON_ERROR_STOP=1 \
  -c "INSERT INTO accounts (tenant_id,domain_id,email,display_name,quota_mb,status) VALUES
       ('$T1','$D1','alice@acme.example','Alice',1024,'active'),
       ('$T1','$D1','bob@acme.example','Bob',2048,'active');" >/dev/null && echo "seed accts: ok"

echo "== start admin-api"
PGPASSWORD="$APP_PW" DATABASE_URL="postgres://og_app@127.0.0.1:$DB_PORT/opengroupware" \
  PORT=$API_PORT nohup "$BIN/admin-api" >/tmp/ac-api.log 2>&1 < /dev/null &
sleep 3
echo "-- api health: $(curl -s $API/health)"
echo "-- set acme policy reject=12 add_header=7 greylist=3"
curl -s -o /dev/null -w "%{http_code}" -H "content-type: application/json" -H "x-actor: ac-smoke" \
  -X PUT -d '{"reject":12.0,"add_header":7.0,"greylist":3.0}' $API/api/v1/tenants/$T1/policy; echo

echo "== start admin-console"
ADMIN_API_URL="$API" ADMIN_CONSOLE_ACTOR="admin-console" PORT=$UI_PORT \
  nohup "$BIN/admin-console" >/tmp/ac-ui.log 2>&1 < /dev/null &
sleep 2
echo "-- ui health: $(curl -s $UI/health)"

LIST=$(curl -s $UI/)
DETAIL=$(curl -s $UI/tenants/$T1)

echo "== ASSERT tenants list"
PASS=1
echo "$LIST" | grep -q "opengroupware admin" || { echo "MISS shell"; PASS=0; }
echo "$LIST" | grep -q ">acme<" || { echo "MISS acme row"; PASS=0; }
echo "$LIST" | grep -q ">globex<" || { echo "MISS globex row"; PASS=0; }
echo "== ASSERT tenant detail"
echo "$DETAIL" | grep -q "acme.example" || { echo "MISS domain"; PASS=0; }
echo "$DETAIL" | grep -q "alice@acme.example" || { echo "MISS account alice"; PASS=0; }
echo "$DETAIL" | grep -q "bob@acme.example" || { echo "MISS account bob"; PASS=0; }
echo "$DETAIL" | grep -q "Abuse policy" || { echo "MISS policy section"; PASS=0; }
echo "$DETAIL" | grep -q "12.0" || { echo "MISS policy reject 12.0"; PASS=0; }
if echo "$DETAIL" | grep -q "old.acme.example"; then echo "-- note: suspended domain shown (expected, badged)"; fi

echo "== detail HTML (head)"; echo "$DETAIL" | head -c 0
echo "== ASSERT pass=$PASS"
[ "$PASS" = "1" ] && echo "SMOKE_PASS" || echo "SMOKE_FAIL"

cleanup
echo "ALL_DONE"
