#!/bin/bash
# Live smoke for the abuse engine: Rspamd consumes config-compiler-format
# per-tenant settings, applies the right thresholds per recipient domain, and a
# scan verdict is ingested by abuse-api into the quarantine workflow.
#
# Proves the seam config-compiler -> Rspamd -> abuse-api. The Stalwart -> Rspamd
# milter hop (real mail flow) lands in the mailbox-backend slice.
#
# Portable: runs on the ms02 build host. Isolated CARGO_TARGET_DIR.
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO"
export PATH="$HOME/.cargo/bin:/usr/local/bin:/opt/homebrew/bin:$PATH"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/jr_target}"
BIN="$CARGO_TARGET_DIR/debug"

R=rspamd-ab; C=abuse-ab-pg
SU_PW=$(openssl rand -hex 8); APP_PW=$(openssl rand -hex 8)
DB_PORT=15440; API_PORT=18083; API=http://127.0.0.1:$API_PORT
TA=11111111-1111-1111-1111-111111111111
SDIR=/tmp/rspamd-ab/local.d
GTUBE='XJS*C4JDBQADN1.NSBN3*2IDNEN*GTUBE-STANDARD-ANTI-UBE-TEST-EMAIL*C.34X'
PASS=1

cleanup() { pkill -x abuse-api 2>/dev/null; docker rm -f $R $C >/dev/null 2>&1; }
cleanup

# --- Rspamd with per-tenant settings in config-compiler's local.d format ----
rm -rf /tmp/rspamd-ab; mkdir -p "$SDIR"
cat > "$SDIR/settings.conf" <<'EOF'
tenant_acme {
  priority = 10;
  rcpt = ["/@acme\.example$/"];
  apply { actions { reject = 15; add_header = 6; greylist = 4; } }
}
tenant_globex {
  priority = 10;
  rcpt = ["/@globex\.example$/"];
  apply { actions { reject = 20; add_header = 8; greylist = 5; } }
}
EOF
docker run -d --name $R -v "$SDIR":/etc/rspamd/local.d:ro rspamd/rspamd:latest >/dev/null
echo "== waiting for rspamd"; sleep 6

echo "== per-tenant proof: normal msg to globex must see required_score=20 (its policy)"
REQ=$(printf 'From: s@bad.test\nTo: x@globex.example\nSubject: hi\n\nhello there\n' \
  | docker exec -i $R rspamc -j --rcpt x@globex.example 2>/dev/null \
  | python3 -c 'import json,sys; print(json.load(sys.stdin).get("required_score"))' 2>/dev/null)
echo "globex required_score=$REQ"
[ "$REQ" = "20.0" ] || { echo "per-tenant threshold NOT applied (want 20.0)"; PASS=0; }
docker logs $R 2>&1 | grep -q "settings_id: tenant_globex" && echo "settings_id: tenant_globex matched" \
  || { echo "rspamd did not record tenant_globex match"; PASS=0; }

echo "== scan GTUBE to acme -> reject verdict to ingest"
SCAN=$(printf 'From: s@bad.test\nTo: alice@acme.example\nSubject: spam\n\n%s\n' "$GTUBE" \
  | docker exec -i $R rspamc -j --rcpt alice@acme.example 2>/dev/null)
RACTION=$(echo "$SCAN" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("action"))' 2>/dev/null)
RSCORE=$(echo "$SCAN" | python3 -c 'import json,sys; print(round(json.load(sys.stdin).get("score",0),1))' 2>/dev/null)
echo "rspamd verdict: action=$RACTION score=$RSCORE"
[ "$RACTION" = "reject" ] || { echo "GTUBE did not reject"; PASS=0; }

# --- abuse-api stack -------------------------------------------------------
docker run -d --name $C -e POSTGRES_PASSWORD="$SU_PW" -e POSTGRES_USER=postgres \
  -e POSTGRES_DB=abuse -p $DB_PORT:5432 postgres:16 >/dev/null
echo "== waiting for postgres"; sleep 7
cargo build -q -p abuse-api 2>&1 | tail -2
PGPASSWORD="$SU_PW" DATABASE_URL="postgres://postgres@127.0.0.1:$DB_PORT/abuse" \
  MIGRATE_ONLY=1 OPENGROUPWARE_ALLOW_UNSAFE_DB=1 "$BIN/abuse-api" >/tmp/rab-mig.log 2>&1
grep -q "migrations applied" /tmp/rab-mig.log && echo "migrate: ok" || { echo "migrate: FAIL"; cat /tmp/rab-mig.log; PASS=0; }
docker exec -e PGPASSWORD="$SU_PW" $C psql -U postgres -d abuse -v ON_ERROR_STOP=1 \
  -c "CREATE ROLE og_app LOGIN PASSWORD '$APP_PW';" >/dev/null
docker exec -e PGPASSWORD="$SU_PW" $C psql -U postgres -d abuse -c \
  "GRANT SELECT,INSERT,UPDATE ON abuse_decision, quarantine_item, audit_log TO og_app;
   GRANT USAGE,SELECT ON ALL SEQUENCES IN SCHEMA public TO og_app;" >/dev/null
PGPASSWORD="$APP_PW" DATABASE_URL="postgres://og_app@127.0.0.1:$DB_PORT/abuse" PORT=$API_PORT \
  nohup "$BIN/abuse-api" >/tmp/rab-api.log 2>&1 < /dev/null &
sleep 3
echo "-- abuse-api health: $(curl -s $API/health)"

echo "== ingest the rspamd reject verdict as a quarantine decision"
curl -s -o /dev/null -w "decision [%{http_code}]\n" -H "content-type: application/json" -H "x-actor: rspamd-bridge" \
  -d "{\"message_ref\":\"gtube-1\",\"recipient\":\"alice@acme.example\",\"sender\":\"spammer@bad.test\",\"score\":$RSCORE,\"action\":\"quarantine\",\"verdict\":\"spam\",\"subject\":\"spam\",\"symbols\":[{\"name\":\"GTUBE\",\"score\":$RSCORE}]}" \
  $API/api/v1/tenants/$TA/abuse/decisions

QCOUNT=$(curl -s $API/api/v1/tenants/$TA/quarantine | python3 -c 'import json,sys; print(len(json.load(sys.stdin)))' 2>/dev/null)
echo "quarantine items for tenant A: $QCOUNT (want 1)"
[ "$QCOUNT" = "1" ] || PASS=0

echo "== ASSERT pass=$PASS"
[ "$PASS" = "1" ] && echo "SMOKE_PASS" || echo "SMOKE_FAIL"
cleanup
echo "ALL_DONE"
