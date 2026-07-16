# ADR-0005: Data ownership boundary — product DB is control plane only

- **Status**: Accepted
- **Date**: 2026-07-16
- **Owner decision**: see docs/13-ownership-review.md (P1)

## Context

The doc-04 ERD modeled mailbox content in the product PostgreSQL: `MESSAGE`,
`FOLDER`, `MESSAGE_PLACEMENT`, `THREAD`, `BLOB`, `ATTACHMENT_DEDUP`,
`STORAGE_VOLUME`, `MESSAGE_DELIVERY`, `BOUNCE`, `DSN`. Stalwart (ADR-0004)
already owns all of this state natively — mail store, blob store with dedup,
MTA queue, sync state. Mirroring it creates two sources of truth, a permanent
synchronization pipeline, and an isolation hole (the mirrored tables carry no
`tenant_id`, so neither RLS nor sharding as designed can apply to them).

## Decision

The product database owns **control-plane state only**:

tenants, domains, accounts (directory metadata — no credentials, see
ADR-0006), policy profiles and inheritance, quotas, quarantine workflow state,
abuse decisions (one row per message, not per symbol), audit log, jobs,
migration state, DNS requirements, DMARC report aggregates.

Stalwart owns **all mail-plane state**: mailboxes, folders, messages, threads,
flags, blobs, attachment dedup, delivery/queue state, protocol sync state.

The product accesses mail-plane data exclusively through Stalwart APIs
(JMAP + management API). It never copies messages into Postgres. Where a
product feature needs a message reference (e.g. quarantine), it stores
Stalwart identifiers plus the minimal decision metadata, not content.

## Consequences

- ~40% of the doc-04 ERD is deleted; docs 04/07/09 to be revised.
- Citus sharding of `message` (doc 09) is moot; flat DB + RLS suffices.
- Quarantine holds messages in the abuse pipeline's own store (or Stalwart
  quarantine folder), referenced by ID from Postgres.
- Restore-first principle applies per plane: pgBackRest for control plane,
  Stalwart-native export/restic for mail plane.
