#!/bin/bash
# Live smoke for abuse-api: record decisions, quarantine workflow, RLS.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
export PATH="$HOME/.cargo/bin:$PATH"
C=abuse-pg
# Throwaway dev creds via env, never embedded in a connection URL (the
# repo secret-scan hook rejects user:pass@host literals). sqlx + psql both
# fall back to PGPASSWORD when the URL omits the password.
SU_PW=$(openssl rand -hex 8); APP_PW=$(openssl rand -hex 8)
docker rm -f $C >/dev/null 2>&1
docker run -d --name $C -e POSTGRES_PASSWORD="$SU_PW" -e POSTGRES_USER=postgres \
  -e POSTGRES_DB=abuse -p 15435:5432 postgres:16 >/dev/null
sleep 6
export PGPASSWORD="$SU_PW"
docker exec -e PGPASSWORD="$SU_PW" $C psql -U postgres -d abuse \
  -c "CREATE ROLE og_app LOGIN PASSWORD '$APP_PW';" >/dev/null

cargo build -q -p abuse-api 2>&1 | tail -1
# migrate as superuser (password from PGPASSWORD; URL carries no secret)
PGPASSWORD="$SU_PW" DATABASE_URL="postgres://postgres@127.0.0.1:15435/abuse" \
  MIGRATE_ONLY=1 OPENGROUPWARE_ALLOW_UNSAFE_DB=1 ./target/debug/abuse-api >/tmp/abuse-mig.log 2>&1
grep -q "migrations applied" /tmp/abuse-mig.log && echo "migrate: ok"
docker exec -e PGPASSWORD="$SU_PW" $C psql -U postgres -d abuse -c \
  "GRANT SELECT,INSERT,UPDATE ON abuse_decision, quarantine_item, audit_log TO og_app;
   GRANT USAGE,SELECT ON ALL SEQUENCES IN SCHEMA public TO og_app;" >/dev/null

pkill -x abuse-api 2>/dev/null; sleep 1
PGPASSWORD="$APP_PW" DATABASE_URL="postgres://og_app@127.0.0.1:15435/abuse" PORT=18081 \
  nohup ./target/debug/abuse-api >/tmp/abuse-api.log 2>&1 < /dev/null &
sleep 3
B=http://127.0.0.1:18081
TA=11111111-1111-1111-1111-111111111111
TB=22222222-2222-2222-2222-222222222222
H=(-H "content-type: application/json" -H "x-actor: abuse-smoke")

echo "== health"; curl -s $B/health; echo
echo "== record quarantine decision (tenant A)"
curl -s -w " [%{http_code}]" "${H[@]}" -d '{"message_ref":"msg-1","recipient":"a@acme.example","sender":"spammer@bad.test","score":9.4,"action":"quarantine","verdict":"spam","subject":"cheap pills","symbols":[{"name":"BAYES_SPAM","score":5.1}]}' $B/api/v1/tenants/$TA/abuse/decisions; echo
echo "== record accept decision (tenant A)"
curl -s -w " [%{http_code}]" "${H[@]}" -d '{"message_ref":"msg-2","recipient":"a@acme.example","score":-1.0,"action":"accept","verdict":"ham"}' $B/api/v1/tenants/$TA/abuse/decisions; echo
echo "== list quarantine tenant A (expect 1 held)"
curl -s $B/api/v1/tenants/$TA/quarantine | head -c 400; echo
echo "== RLS: tenant B sees empty quarantine (expect [])"
curl -s $B/api/v1/tenants/$TB/quarantine; echo
echo "== invalid action (expect 422)"
curl -s -w " [%{http_code}]" "${H[@]}" -d '{"message_ref":"m3","recipient":"x","score":0,"action":"bogus"}' $B/api/v1/tenants/$TA/abuse/decisions; echo
ITEM=$(curl -s $B/api/v1/tenants/$TA/quarantine | python3 -c "import json,sys;print(json.load(sys.stdin)[0]['id'])")
echo "== release item $ITEM (expect status released)"
curl -s -w " [%{http_code}]" "${H[@]}" -X POST $B/api/v1/tenants/$TA/quarantine/$ITEM/release | head -c 300; echo
echo "== list quarantine tenant A after release (expect [])"
curl -s $B/api/v1/tenants/$TA/quarantine; echo
echo "== audit trail"
docker exec -e PGPASSWORD="$SU_PW" $C psql -U postgres -d abuse -tAc \
  "SET app.is_platform_admin=true; SELECT actor||' '||action FROM audit_log ORDER BY id"

pkill -x abuse-api 2>/dev/null
docker rm -f $C >/dev/null 2>&1
echo done
