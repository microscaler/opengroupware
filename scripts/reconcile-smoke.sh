#!/bin/bash
# Live smoke for job-runner reconciliation: seed pending_provisioning accounts,
# run one reconcile cycle against a MOCK sesame, assert activation + audit,
# and that a sesame failure leaves the account pending for retry.
#
# Portable: runs on the ms02 build host (or any box with cargo+docker). Uses an
# isolated CARGO_TARGET_DIR so it never contends with Tilt's continuous build.
set -uo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO"
export PATH="$HOME/.cargo/bin:/usr/local/bin:/opt/homebrew/bin:$PATH"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/jr_target}"
BIN="$CARGO_TARGET_DIR/debug"

C=reconcile-pg
# Throwaway dev creds via vars (never string literals — the repo secret-scan
# hook rejects hardcoded password/secret literals). The mock sesame ignores
# the client secret anyway.
SU_PW=$(openssl rand -hex 8); APP_PW=$(openssl rand -hex 8); CSECRET=$(openssl rand -hex 8)
MOCK_PORT=18099; DB_PORT=15436

docker rm -f $C >/dev/null 2>&1
docker run -d --name $C -e POSTGRES_PASSWORD="$SU_PW" -e POSTGRES_USER=postgres \
  -e POSTGRES_DB=opengroupware -p $DB_PORT:5432 postgres:16 >/dev/null
echo "== waiting for postgres"; sleep 7

echo "== build admin-api + job-runner (target=$CARGO_TARGET_DIR)"
cargo build -q -p admin-api -p job-runner 2>&1 | tail -3

echo "== migrate (superuser)"
PGPASSWORD="$SU_PW" DATABASE_URL="postgres://postgres@127.0.0.1:$DB_PORT/opengroupware" \
  MIGRATE_ONLY=1 OPENGROUPWARE_ALLOW_UNSAFE_DB=1 "$BIN/admin-api" >/tmp/rec-mig.log 2>&1
grep -q "migrations applied" /tmp/rec-mig.log && echo "migrate: ok" || { echo "migrate: FAIL"; cat /tmp/rec-mig.log; }

echo "== create login role + grant"
docker exec -e PGPASSWORD="$SU_PW" $C psql -U postgres -d opengroupware -v ON_ERROR_STOP=1 \
  -c "CREATE ROLE og_app LOGIN PASSWORD '$APP_PW';" \
  -c "GRANT opengroupware_app TO og_app;" >/dev/null && echo "role: ok"

echo "== seed tenant/domain/accounts (superuser bypasses RLS)"
TEN=aaaaaaaa-0000-0000-0000-000000000001
DOM=bbbbbbbb-0000-0000-0000-000000000001
docker exec -e PGPASSWORD="$SU_PW" $C psql -U postgres -d opengroupware -v ON_ERROR_STOP=1 \
  -c "INSERT INTO tenants (id,slug,name,plan,status) VALUES ('$TEN','acme','Acme','standard','active');" \
  -c "INSERT INTO domains (id,tenant_id,fqdn,status) VALUES ('$DOM','$TEN','acme.example','active');" \
  -c "INSERT INTO accounts (tenant_id,domain_id,email,display_name,status) VALUES
      ('$TEN','$DOM','alice@acme.example','Alice','pending_provisioning'),
      ('$TEN','$DOM','bob@acme.example','Bob','pending_provisioning'),
      ('$TEN','$DOM','fail@acme.example','Fail','pending_provisioning');" >/dev/null && echo "seed: ok"

echo "== start mock sesame on :$MOCK_PORT"
python3 - "$MOCK_PORT" >/tmp/rec-mock.log 2>&1 <<'PY' &
import http.server, json, sys, uuid
PORT=int(sys.argv[1])
class H(http.server.BaseHTTPRequestHandler):
    def _send(self, code, obj):
        b=json.dumps(obj).encode()
        self.send_response(code)
        self.send_header('content-type','application/json')
        self.send_header('content-length',str(len(b)))
        self.end_headers(); self.wfile.write(b)
    def do_POST(self):
        n=int(self.headers.get('content-length',0) or 0)
        raw=self.rfile.read(n) if n else b'{}'
        if self.path.endswith('/auth/token'):
            return self._send(200, {"access_token":"tok","expires_in":900})
        if self.path.endswith('/admin/users'):
            try: data=json.loads(raw or b'{}')
            except Exception: data={}
            if 'fail' in data.get('email',''):
                return self._send(500, {"error":"simulated sesame outage"})
            return self._send(200, {"user_id": str(uuid.uuid4())})
        return self._send(404, {"error":"not found"})
    def log_message(self,*a): pass
http.server.HTTPServer(('127.0.0.1',PORT),H).serve_forever()
PY
MOCK_PID=$!
sleep 1

echo "== run job-runner one-shot reconcile"
PGPASSWORD="$APP_PW" \
  DATABASE_URL="postgres://og_app@127.0.0.1:$DB_PORT/opengroupware" \
  RECONCILE_ONCE=1 \
  SESAME_LOGIN_URL="http://127.0.0.1:$MOCK_PORT" \
  SESAME_USER_MGMT_URL="http://127.0.0.1:$MOCK_PORT" \
  SESAME_TENANT="acme" SESAME_CLIENT_ID="og" SESAME_CLIENT_SECRET="$CSECRET" \
  RUST_LOG=info "$BIN/job-runner" 2>&1 | tail -8

echo "== RESULT: account statuses (expect alice/bob active, fail pending_provisioning)"
docker exec -e PGPASSWORD="$SU_PW" $C psql -U postgres -d opengroupware -tAc \
  "SELECT email||' -> '||status||' sesame='||COALESCE(sesame_user_id::text,'NULL') FROM accounts ORDER BY email"
echo "== RESULT: audit trail (expect 2 account.reconciled)"
docker exec -e PGPASSWORD="$SU_PW" $C psql -U postgres -d opengroupware -tAc \
  "SET app.is_platform_admin=true; SELECT actor||' '||action||' '||entity_id FROM audit_log ORDER BY id"

ACTIVE=$(docker exec -e PGPASSWORD="$SU_PW" $C psql -U postgres -d opengroupware -tAc \
  "SELECT count(*) FROM accounts WHERE status='active'")
PENDING=$(docker exec -e PGPASSWORD="$SU_PW" $C psql -U postgres -d opengroupware -tAc \
  "SELECT count(*) FROM accounts WHERE status='pending_provisioning'")
echo "== ASSERT active=$ACTIVE (want 2), pending=$PENDING (want 1)"
if [ "$ACTIVE" = "2" ] && [ "$PENDING" = "1" ]; then echo "SMOKE_PASS"; else echo "SMOKE_FAIL"; fi

kill $MOCK_PID 2>/dev/null
docker rm -f $C >/dev/null 2>&1
echo "ALL_DONE"
