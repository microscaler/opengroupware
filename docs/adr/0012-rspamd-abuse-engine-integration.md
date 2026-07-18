# ADR-0012: Rspamd abuse-engine integration

- **Status**: Accepted (dev). Milter hop deferred to the mailbox-backend slice.
- **Context**: config-compiler renders per-tenant Rspamd `settings` from
  control-plane desired state (ADR-0003), and abuse-api owns the quarantine
  workflow (ADR-0005). Neither was connected to a running Rspamd. This ADR
  records how they connect.

## Decisions

### 1. config-compiler is co-located with Rspamd (init container + sidecar)

k3s has no ReadWriteMany storage, so the rendered `settings.conf` cannot be
shared from a standalone config-compiler pod to a separate Rspamd pod. Instead
config-compiler runs inside the Rspamd pod:

- an **init container** (`RENDER_ONCE=1`) writes `settings.conf` into a shared
  `emptyDir` at `/etc/rspamd/local.d/` before Rspamd starts, so Rspamd boots
  with the current per-tenant thresholds;
- a **sidecar** re-renders on its interval as desired state changes.

The standalone `config-compiler` Deployment is removed; its behaviour is
unchanged, only its placement.

### 2. `rcpt` is a domain-anchored regex, not a bare domain

Rspamd's settings `rcpt` selector matches the *full* recipient address; a bare
`"acme.example"` does **not** match `user@acme.example`. config-compiler emits a
per-domain regex `"/@acme\.example$/"` (dots escaped). Verified against a live
Rspamd: a message to `x@globex.example` reports `required_score = 20.0` (the
tenant's policy, not the default 15) and the log shows
`apply static settings tenant_globex … rcpt matched; settings_id: tenant_globex`.

### 3. The generated file is validated as real Rspamd config

`config-compile-smoke.sh` runs `rspamadm configtest` on config-compiler's actual
output. This caught a real defect: `local.d/settings.conf` is *already* wrapped
in an implicit `settings { }` section by Rspamd, so emitting our own wrapper
produced illegal `settings { settings { … } }` nesting. config-compiler now
emits the per-tenant blocks unwrapped.

### 4. Verdict ingestion reuses abuse-api's decision endpoint

Rspamd's scan verdict (score, action, symbols) maps onto
`POST /tenants/{id}/abuse/decisions`; a `reject`/high-score verdict becomes a
`quarantine` decision, which opens a `quarantine_item`. `rspamd-abuse-smoke.sh`
proves the whole seam: Rspamd scores a message with the tenant's settings, and
the verdict lands in the quarantine workflow.

## Deferred (mailbox-backend slice)

- **Stalwart → Rspamd milter.** Real inbound mail is scored by wiring Stalwart's
  milter/scan hook to Rspamd, and the component that posts the verdict to
  abuse-api lives at that MTA integration point. Until then the producer is
  exercised by the smoke, not by live mail.
- **Live reload.** Rspamd reads `local.d` on boot; picking up later re-renders
  without a restart needs a reload signal (`shareProcessNamespace` + SIGHUP, or
  the controller reload command).
- **Redis-backed modules** (ratelimit, greylist state, bayes, fuzzy) are not yet
  configured. Static rules and the settings module — what per-tenant policy
  relies on — work without Redis.
