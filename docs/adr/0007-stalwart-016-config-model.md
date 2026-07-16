# ADR-0007: Stalwart 0.16 configuration model

- **Status**: Accepted
- **Date**: 2026-07-16
- **Context**: the original manifests carried a fictional image reference and
  an inline-TOML config that no shipped Stalwart accepts. Verified live
  against `docker.io/stalwartlabs/stalwart` **v0.16.13**.

## Findings (verified against the running binary)

1. **Image**: `ghcr.io/stalwart-mail/stalwart-mail` does not exist. The
   official image is `docker.io/stalwartlabs/stalwart`. Pin `v0.16.13`.

2. **The `--config` file is JSON, and it defines ONLY the bootstrap data
   store** — not listeners, directories, or auth. Its top-level object *is*
   a DataStore spec:
   ```json
   {
     "@type": "PostgreSql",
     "host": "…", "port": 5432, "database": "stalwart",
     "authUsername": "…",
     "authSecret": { "@type": "Value", "value": "…" }
   }
   ```
   - store `@type` ∈ `RocksDb | Sqlite | FoundationDb | PostgreSql | MySql`
   - fields are **camelCase** (`authUsername`, `authSecret`, `poolMaxConnections`, `useTls`)
   - `authSecret` is a tagged enum: `None | Value | EnvironmentVariable | File`

3. **Everything else lives in the data store**, configured through the
   admin WebUI or the management API and persisted there. Bootstrap prints a
   temporary admin account on first run. There is no inline TOML for
   listeners/directories in 0.16.

## Decision

- **Image**: `docker.io/stalwartlabs/stalwart:v0.16.13` everywhere.
- **Bootstrap store**: a small JSON config, delivered as a *Secret* (so the
  DB credential stays encrypted at rest via sops), pointing Stalwart at its
  own PostgreSQL database (`stalwart`, separate from the opengroupware
  control-plane DB — ADR-0005: Stalwart owns mail-plane state).
- **The sesame SQL directory (ADR-0006 v2 / PRD F3) is applied post-boot via
  the management API**, not inline. The exact rp_directory query shape is
  already validated (`scripts/rp-directory-smoke.sh`); it is carried in a
  ConfigMap and applied by a one-shot Job (`stalwart-configure`) that calls
  the management API after the server is ready. This replaces the invalid
  inline `[directory.sesame]` block.

## Consequences

- GitOps for Stalwart settings is API-driven, not file-driven. The
  declarative source of truth is the ConfigMap consumed by the configure
  Job; the store holds the applied state.
- First-boot admin bootstrap must be captured or reset via the API in the
  configure Job (fallback-admin credential from a secret).
- The directory-application step is the remaining live-cluster task
  (needs a running Stalwart to POST settings to); config *format* and the
  rp_directory query are both proven.
