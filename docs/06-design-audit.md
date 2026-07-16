# 06 — Design Audit: Gap Analysis

Produced from analysis of docs/01–05. Each finding references source documents.

## Severity key

- **P0**: Blocks production-readiness or introduces data-loss risk.
- **P1**: Missing capability that a credible Zimbra competitor must ship.
- **P2**: Real operational blind spot but no architectural blocker.
- **P3**: Missing but low impact; can be deferred without harm.

---

## 1. Multi-tenancy isolation

| # | Gap | Severity | Source docs | Notes |
|---|-----|----------|-------------|-------|
| 1.1 | No tenant data isolation strategy documented | **P0** | 02, 04 | Tenants share PostgreSQL, Redis, blob storage. No mention of row-level security, schema-per-tenant, or network isolation. In a SaaS or shared hosting model this is a hard blocker. |
| 1.2 | No per-tenant resource quotas beyond mailbox size | **P1** | 04 | Accounts have `quota_mb`. No CPU, IOPS, API rate, or connection limits per tenant. A noisy tenant can starve others. |
| 1.3 | No multi-tenant admin delegation model | **P1** | 04, 05 | `ADMIN_ROLE` is per-tenant but has no hierarchy or scope. Can you delegate domain-admin or mailbox-admin without giving full tenant-admin? Zimbra and Exchange both support this. |
| 1.4 | No tenant data segregation in search / blob | **P0** | 02, 04 | Search index and blob storage are flat per mailbox. No tenant-scoped index partitioning or S3 prefixes with IAM policies. |

**Recommendation:** Document the isolation model early. Default to PostgreSQL RLS + S3 prefix-based storage isolation. Add resource quota fields to `POLICY_PROFILE`. Define a delegation hierarchy in `ADMIN_ROLE`.

---

## 2. Security and identity

| # | Gap | Severity | Source docs | Notes |
|---|-----|----------|-------------|-------|
| 2.1 | No mention of OAuth 2.0 / PKCE for web app auth | **P1** | 02, 04 | Web UI currently relies on OIDC (Keycloak). Modern web apps also need OAuth2 for third-party integrations and browser-session security (PKCE, sameSite, CSRF). |
| 2.2 | No email 2FA / MFA policy model | **P1** | 04, 03 | `USER_ABUSE_PREFERENCE` has no MFA flag. OIDC provider may enforce it, but per-account MFA toggle and grace periods are standard in Zimbra. |
| 2.3 | No app password / service account model | **P1** | 04 | Legacy IMAP/DAV clients need app passwords for accounts with MFA enabled. No mention in the domain model or roadmap. |
| 2.4 | No API auth model beyond OIDC | **P2** | 02, 05 | Admin API has no mention of API keys, service tokens, or scope-based OAuth. `AdminAPI --> ProductDB` implies API calls but no auth strategy. |
| 2.5 | No TLS certificate management | **P2** | 02 | Edge mentions "TLS/load balancer" but not auto-provisioning (Let's Encrypt / ACME). Manual cert rotation is a support burden. |
| 2.6 | No session management model | **P2** | 04, 03 | `USER_ABUSE_PREFERENCE` has no session fields. No mention of idle timeout, concurrent sessions, device management, or session revocation. |

**Recommendation:** Add `ACCOUNT` fields for `mfa_enabled`, `mfa_method`, `app_passwords` (hashed tokens). Define API auth strategy for `AdminAPI`. Document TLS lifecycle.

---

## 3. High availability and disaster recovery

| # | Gap | Severity | Source docs | Notes |
|---|-----|----------|-------------|-------|
| 3.1 | No HA strategy for PostgreSQL | **P0** | 02 | Single-node MVP shows PostgreSQL on the same node. No replication, WAL archiving, failover, or backup recovery RPO/RTO targets. |
| 3.2 | No HA for Redis/Valkey | **P1** | 02 | Rspamd state and rate-limit data is lost on Redis crash. No Sentinel or clustering mentioned. |
| 3.3 | No mailbox backend HA | **P0** | 02, 05 | Dovecot/Stalwart are single-process. No shared storage (NFS/Gluster), no mailbox replication, no failover. |
| 3.4 | No cross-region DR plan | **P1** | 02 | Backup store is mentioned but no replication across regions, no DR runbooks, no RTO/RPO definitions. |
| 3.5 | No SMTP queue persistence across restarts | **P1** | 02 | Outbound queue is ephemeral in the diagram. Postfix handles this, but the design doesn't call it out — important for the "restore-first" principle. |
| 3.6 | No rolling update / staged rollout plan | **P1** | 02, 05 | "Should have" includes "staged upgrades" but no mechanism described. How do you update the config compiler without downtime? |

**Recommendation:** Document RPO/RTO targets. Add PostgreSQL streaming replication to the "small cluster" shape. Define mailbox backend HA options (shared storage vs. replication). Document upgrade procedure.

---

## 4. Compliance and legal

| # | Gap | Severity | Source docs | Notes |
|---|-----|----------|-------------|-------|
| 4.1 | No retention/deletion compliance model | **P1** | 04 | `POLICY_PROFILE` has `retention_policy` JSON but no structured model. GDPR requires configurable retention, right-to-erasure workflows, and data export. No mention of PII catalog. |
| 4.2 | No email archiving / legal hold | **P2** | 05, 04 | Deferred in roadmap. Real enterprise buyers require WORM (write-once-read-many) immutable archive. Zimbra Network Edition ships this. |
| 4.3 | No eDiscovery search | **P2** | 05 | Deferred. Compliance buyers need cross-mailbox keyword search, hold lists, and export formats (PST, MBOX). |
| 4.4 | No data residency / geo-control | **P1** | 02, 04 | Multi-region deployment diagram exists but no per-tenant data residency controls. EU customers need data staying in-region. |
| 4.5 | No GDPR data processing addendum model | **P3** | 04 | Tenant model has `plan` and `status` but no DPA signing, subprocessor list, or data flow documentation. |

**Recommendation:** Add structured retention policy (min/max retention per scope, delete method). Design immutable archive table early even if unused. Add `TENANT.data_residency_region` field.

---

## 5. Performance and scalability

| # | Gap | Severity | Source docs | Notes |
|---|-----|----------|-------------|-------|
| 5.1 | No mail throughput targets | **P1** | 02, 05 | No mention of messages/sec, concurrent connections, or mailbox count per node. Can't size infrastructure without targets. |
| 5.2 | No search performance model | **P1** | 02, 04 | Search index is mentioned but no latency targets, no index partitioning, no full-text search strategy beyond "backend-native vs. OpenSearch/Tantivy". |
| 5.3 | No connection pooling strategy | **P2** | 02 | Admin API and config compiler connect to PostgreSQL. No PgBouncer/Pgpool mentioned. Connection limits will be hit under load. |
| 5.4 | No message size limits model | **P2** | 04, 03 | `Effective` policy mentions "attachment size" but no global SMTP message size limit, no chunked upload for large attachments. |
| 5.5 | No async job backpressure | **P2** | 05 | `JobRunner` handles migration, backup, training. No queue depth limits, prioritization, or cancellation. A stuck backup can block everything. |
| 5.6 | No read/write split for PostgreSQL | **P2** | 02 | Admin API reads heavily from ProductDB. No read replicas mentioned for scaling. |

**Recommendation:** Define per-node throughput targets (e.g., 10K msg/min per SMTP node). Choose PgBouncer for connection pooling. Add read replicas in the "small cluster" shape. Document async job priorities.

---

## 6. Protocol coverage gaps

| # | Gap | Severity | Source docs | Notes |
|---|-----|----------|-------------|-------|
| 6.1 | No POP3 mention | **P2** | 02, 04 | Most competitors still support POP3 for archival clients. Stalwart supports it but the design doesn't call it out. |
| 6.2 | No SMTP UTF-8 / EAI support | **P2** | 02 | RFC 6531 (SMTPUTF8) is essential for non-ASCII email addresses. Not mentioned in protocol list. |
| 6.3 | No S/MIME or PGP support | **P2** | 04, 05 | Document-level encryption is a standard Zimbra feature. No mention in protocol coverage, domain model, or roadmap. |
| 6.4 | No IMAP IDLE / push notification model | **P2** | 02 | Modern webmail needs real-time push (Server-Sent Events or WebSocket). Only JMAP push is implicitly covered. |
| 6.5 | No IMAP MOVE / UIDPLUS / QRESYNC | **P3** | 02 | Modern IMAP extensions for efficient client sync. Dovecot supports them, but the design should call out compliance levels. |
| 6.6 | No JMAP Push / Session state | **P2** | 02 | JMAP has its own push model. If using JMAP as primary API, the push strategy needs design, not just "JMAP API" label. |

**Recommendation:** Add SMTPUTF8, IMAP IDLE/push, and S/MIME to protocol list. Decide on webmail push technology (JMAP push vs. SSE vs. WebSocket).

---

## 7. Data model gaps

| # | Gap | Severity | Source docs | Notes |
|---|-----|----------|-------------|-------|
| 7.1 | No `RESOURCE` entity | **P1** | 04 | Meeting rooms and equipment booking are standard groupware. "Resource calendars" is a "should have" but the entity doesn't exist. |
| 7.2 | No `FREEBUSY` / scheduling assistant | **P1** | 04 | No model for free-busy queries, scheduling assistants, or meeting proposal workflows beyond basic CalDAV events. |
| 7.3 | No `MESSAGE_VERSION` / edit history | **P1** | 04 | If messages can be updated (e.g., meeting responses, collaborative editing), version tracking is needed. |
| 7.4 | No `MESSAGE_DELIVERY_STATUS` / DSN | **P2** | 04 | Disposition Notification Messages (delivery receipts) have no model. Required for BCC, delivery status notifications, and bounce tracking. |
| 7.5 | `BLOB` has no deduplication model | **P1** | 04 | Identical attachments across messages are common. SHA256 hash is stored but no dedup index. Without it, blob storage bloats. |
| 7.6 | No `ACCOUNT_DELEGATE` model | **P1** | 04 | Delegation (BCC, shared mailbox, full access) is listed as "should have" but the entity is missing. `SHARE_GRANT` covers calendar/contacts sharing only. |
| 7.7 | `MESSAGE` has no `thread_parent_id` | **P3** | 04 | `thread_id` is a foreign key, which implies a tree. But `MESSAGE_PLACEMENT` is separate from `THREAD`, which is correct — however no parent-message relationship for threaded replies. |
| 7.8 | No `DOMAIN_TAXONOMY` / custom fields | **P2** | 04 | Zimbra allows custom message properties. Not needed for MVP but worth a `MESSAGE_PROPS` JSONB column in `MESSAGE`. |

**Recommendation:** Add `RESOURCE` (type: room/equipment) to the groupware model. Add `ACCOUNT_DELEGATE`. Add attachment dedup to `BLOB` section. Consider `MESSAGE_PROPS` JSONB.

---

## 8. Operational gaps

| # | Gap | Severity | Source docs | Notes |
|---|-----|----------|-------------|-------|
| 8.1 | No healthcheck endpoints documented | **P2** | 02 | No health/liveness/readiness endpoints mentioned for any service. Kubernetes and load balancers require these. |
| 8.2 | No log aggregation format / structured logging | **P2** | 02 | "Observability" mentions Prometheus/Grafana/Loki but no log format (JSON), correlation IDs, or trace propagation (OpenTelemetry headers). |
| 8.3 | No feature flag / toggle system | **P3** | 05 | No mechanism for gradual rollout, canary, or emergency feature disable. Important for the config compiler and new features. |
| 8.4 | No configuration schema validation | **P2** | 02, 05 | Config compiler "renders configs" but no schema definition or validation against service config formats. Drift detection needs a schema to diff against. |
| 8.5 | No secret management strategy | **P2** | 02 | Passwords, API keys, DKIM private keys — no mention of secrets vault (HashiCorp Vault, SSM Parameter Store, Kubernetes Secrets). |
| 8.6 | No service mesh / inter-service auth | **P2** | 02 | Internal services talk to each other (AdminAPI → ConfigCompiler → Services) with no mTLS or service identity. |

**Recommendation:** Add structured JSON logging with correlation IDs. Define config schema format (JSON Schema or similar). Add mTLS for internal communication. Document secrets management approach.

---

## 9. Migration gaps

| # | Gap | Severity | Source docs | Notes |
|---|-----|----------|-------------|-------|
| 9.1 | No Exchange/Office 365 migration path | **P1** | 05, 04 | "Migration/import/export" mentions imapsync and Zimbra TGZ. Exchange and O365 are the dominant migration targets. Without EWS/Graph API migration, you lose enterprise conversions. |
| 9.2 | No contact/calendar migration | **P2** | 05 | Migration mentions IMAP only. Calendar and contacts don't migrate with imapsync. Need vCard/iCAL import. |
| 9.3 | No migration progress tracking per-user | **P1** | 04 | `MIGRATION_ITEM` has source/target/status/error but no byte count, message count, or progress percentage per item. Long migrations need progress reports. |
| 9.4 | No migration resume / checkpoint | **P1** | 05 | If a migration of 100K messages fails at message 95K, it must resume from checkpoint. No mention of restartable migration. |

**Recommendation:** Add Exchange/IMAP credential fields to `MIGRATION_BATCH`. Add `MIGRATION_ITEM` fields for `messages_migrated`, `bytes_migrated`, `resume_cursor`. Design migration to be idempotent.

---

## 10. Abuse pipeline gaps

| # | Gap | Severity | Source docs | Notes |
|---|-----|----------|-------------|-------|
| 10.1 | No inbound DMARC reporting | **P2** | 03 | DMARC aggregate and forensic reports are mentioned as "do not defer" but no consumer/service is designed. Rspamd can generate but the platform needs a receiver. |
| 10.2 | No outbound BCC / shadow copy for security | **P2** | 03 | Enterprise security requires outbound message shadow-copying for audit. Not mentioned in the outbound flow. |
| 10.3 | No bulk sender / reputation management | **P2** | 03 | Marketing/Bulk senders need dedicated IPs, warmup schedules, and bounce management. Not in the design. |
| 10.4 | No DMARC failure auto-remediation | **P1** | 03 | When DMARC fails for inbound, the policy should auto-apply (reject/quarantine). Mentioned in policy table but not in the pipeline diagram. |
| 10.5 | No quarantine notification/subscription | **P1** | 03 | Users need daily/weekly digest of quarantined messages, not just manual console access. No subscription model. |
| 10.6 | No threat intelligence feed integration | **P2** | 03 | "Threat intel" is deferred but the domain model has no `THREAT_INTEL` or `BLOCKED_DOMAIN` tables. Even basic blocklists need storage. |

**Recommendation:** Add `QUARANTINE_SUBSCRIPTION` model. Document DMARC report receiver service. Add `THREAT_INTEL_OBSERVATION` to abuse model. Design daily quarantine digest.

---

## 11. Roadmap and sequencing risks

| # | Gap | Severity | Source docs | Notes |
|---|-----|----------|-------------|-------|
| 11.1 | Config compiler depends on mailbox backend before it's chosen | **P1** | 05 | Config compiler (`a3`) starts before mailbox backend spike (`b2`). The compiler needs to know what services it's configuring. This is a circular dependency. |
| 11.2 | Webmail MVP (`d1`) depends on mailbox (`b2`) and IMAP/JMAP (`b3`) — but `d3` (admin console) only depends on config compiler (`a3`) | **P2** | 05 | Admin console could be built in parallel with mail core. The Gantt doesn't explicitly parallelize it. |
| 11.3 | No spike for backup/restore technology | **P2** | 05 | Backup is "Should have" but no spike before the MVP slice. Restic/Borg are file-level only — not adequate for mailbox-level restore without a strategy. |
| 11.4 | No parallelization between Track A and Track B | **P1** | 02, 05 | Both tracks are evaluated but only one path is taken. The decision gate and timeline aren't specified. Spikes should run in parallel with a fixed deadline. |
| 11.5 | No staging/test environment strategy | **P2** | 05 | No mention of how the config compiler is tested, how migrations are validated, or how a staging cluster mirrors production. |

**Recommendation:** Restructure Gantt so mailbox backend spike runs in parallel with foundation. Add config-compiler spike for each track (Track A compiler vs. Track B compiler). Fix circular dependency: admin API should not wait for config compiler to start.

---

## Summary table

| Category | P0 | P1 | P2 | P3 | Total |
|----------|----|----|----|----|-------|
| Multi-tenancy | 2 | 2 | 0 | 0 | 4 |
| Security | 0 | 3 | 3 | 0 | 6 |
| HA / DR | 2 | 3 | 0 | 0 | 5 |
| Compliance | 0 | 2 | 2 | 1 | 5 |
| Performance | 0 | 2 | 4 | 0 | 6 |
| Protocol | 0 | 0 | 5 | 1 | 6 |
| Data model | 0 | 4 | 2 | 1 | 7 |
| Operational | 0 | 0 | 5 | 0 | 5 |
| Migration | 0 | 3 | 1 | 0 | 4 |
| Abuse | 0 | 2 | 4 | 0 | 6 |
| Roadmap | 0 | 2 | 2 | 0 | 4 |
| **Grand total** | **4** | **25** | **28** | **3** | **60** |

---

## Critical path recommendations

Three items must be resolved before implementation starts:

1. **Multi-tenancy isolation model** (P0 x2): Without RLS, schema-per-tenant, or network segmentation documented, the platform is not multi-tenant safe. Pick a model.

2. **Mailbox backend HA** (P0 x2): The "restore-first" principle and production-readiness claims fail without PostgreSQL replication, mailbox backend failover, and RPO/RTO targets.

3. **Config compiler / mailbox backend circular dependency** (P1): The roadmap has config compiler starting before the mailbox backend is chosen. Either run the config compiler spike in parallel with the mailbox spike, or restructure the Gantt.
