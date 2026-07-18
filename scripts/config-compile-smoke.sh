#!/bin/bash
# Live smoke for config-compiler: seed tenants/domains, run one compile cycle,
# assert the rendered + schema-validated Rspamd settings UCL and that only
# ACTIVE tenants+domains are included, plus the audit row.
#
# Portable: runs on the ms02 build host. Isolated CARGO_TARGET_DIR so it never
# contends with Tilt's continuous build.
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO"
export PATH="$HOME/.cargo/bin:/usr/local/bin:/opt/homebrew/bin:$PATH"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/jr_target}"
BIN="$CARGO_TARGET_DIR/debug"

C=configc-pg
SU_PW=$(openssl rand -hex 8); APP_PW=$(openssl rand -hex 8)
DB_PORT=15437
OUT=/tmp/rspamd-settings.conf
rm -f "$OUT"

docker rm -f $C >/dev/null 2>&1
docker run -d --name $C -e POSTGRES_PASSWORD="$SU_PW" -e POSTGRES_USER=postgres \
  -e POSTGRES_DB=opengroupware -p $DB_PORT:5432 postgres:16 >/dev/null
echo "== waiting for postgres"; sleep 7

echo "== build admin-api + config-compiler (target=$CARGO_TARGET_DIR)"
cargo build -q -p admin-api -p config-compiler 2>&1 | tail -3

echo "== migrate (superuser)"
PGPASSWORD="$SU_PW" DATABASE_URL="postgres://postgres@127.0.0.1:$DB_PORT/opengroupware" \
  MIGRATE_ONLY=1 OPENGROUPWARE_ALLOW_UNSAFE_DB=1 "$BIN/admin-api" >/tmp/cc-mig.log 2>&1
grep -q "migrations applied" /tmp/cc-mig.log && echo "migrate: ok" || { echo "migrate: FAIL"; cat /tmp/cc-mig.log; }

docker exec -e PGPASSWORD="$SU_PW" $C psql -U postgres -d opengroupware -v ON_ERROR_STOP=1 \
  -c "CREATE ROLE og_app LOGIN PASSWORD '$APP_PW';" \
  -c "GRANT opengroupware_app TO og_app;" >/dev/null && echo "role: ok"

echo "== seed: 2 active tenants+domains, 1 suspended tenant, 1 suspended domain"
T1=aaaaaaaa-0000-0000-0000-000000000001
T2=aaaaaaaa-0000-0000-0000-000000000002
docker exec -e PGPASSWORD="$SU_PW" $C psql -U postgres -d opengroupware -v ON_ERROR_STOP=1 \
  -c "INSERT INTO tenants (id,slug,name,plan,status) VALUES
       ('$T1','acme','Acme','standard','active'),
       ('$T2','globex','Globex','standard','active'),
       ('aaaaaaaa-0000-0000-0000-000000000003','ghost','Ghost','standard','suspended');" \
  -c "INSERT INTO domains (tenant_id,fqdn,status) VALUES
       ('$T1','acme.example','active'),
       ('$T1','old.acme.example','suspended'),
       ('$T2','globex.example','active'),
       ('aaaaaaaa-0000-0000-0000-000000000003','ghost.example','active');" >/dev/null 2>/tmp/cc-seed.log \
  && echo "seed: ok" || { echo "seed: FAIL"; cat /tmp/cc-seed.log; }

echo "== run config-compiler one-shot compile"
PGPASSWORD="$APP_PW" \
  DATABASE_URL="postgres://og_app@127.0.0.1:$DB_PORT/opengroupware" \
  RENDER_ONCE=1 RSPAMD_SETTINGS_PATH="$OUT" RUST_LOG=info "$BIN/config-compiler" 2>&1 | tail -5

echo "== RESULT: rendered UCL ($OUT)"
cat "$OUT" 2>/dev/null || echo "(no output file!)"

echo "== RESULT: audit"
docker exec -e PGPASSWORD="$SU_PW" $C psql -U postgres -d opengroupware -tAc \
  "SET app.is_platform_admin=true; SELECT actor||' '||action||' '||payload::text FROM audit_log ORDER BY id"

PASS=1
grep -q "tenant_acme {" "$OUT" 2>/dev/null || { echo "MISS tenant_acme"; PASS=0; }
grep -q "tenant_globex {" "$OUT" 2>/dev/null || { echo "MISS tenant_globex"; PASS=0; }
grep -q '"acme.example"' "$OUT" 2>/dev/null || { echo "MISS acme.example"; PASS=0; }
if grep -q "ghost" "$OUT" 2>/dev/null; then echo "LEAK suspended tenant ghost"; PASS=0; fi
if grep -q "old.acme.example" "$OUT" 2>/dev/null; then echo "LEAK suspended domain"; PASS=0; fi
AUD=$(docker exec -e PGPASSWORD="$SU_PW" -e PGOPTIONS="-c app.is_platform_admin=true" $C \
  psql -U postgres -d opengroupware -tAc \
  "SELECT count(*) FROM audit_log WHERE action='config.compiled'")
[ "$AUD" = "1" ] || { echo "audit count=$AUD (want 1)"; PASS=0; }
echo "== ASSERT pass=$PASS"
[ "$PASS" = "1" ] && echo "SMOKE_PASS" || echo "SMOKE_FAIL"

docker rm -f $C >/dev/null 2>&1
echo "ALL_DONE"
