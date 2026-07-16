# 04 — Domain Model ERD

This is a product-oriented domain model for a greenfield mailgroupware platform.

It is not a clone of Zimbra's database schema. It is the model the new product should own.

## Core tenant/account/mailbox model

```mermaid
erDiagram
  TENANT ||--o{ DOMAIN : owns
  TENANT ||--o{ ADMIN_ROLE : defines
  TENANT ||--o{ POLICY_PROFILE : defines
  DOMAIN ||--o{ ACCOUNT : owns
  DOMAIN ||--o{ ALIAS : defines
  DOMAIN ||--o{ DISTRIBUTION_LIST : defines
  DOMAIN ||--o{ DNS_RECORD_REQUIREMENT : requires
  ACCOUNT ||--|| MAILBOX : has
  ACCOUNT }o--o{ ADMIN_ROLE : assigned
  ACCOUNT }o--o{ POLICY_PROFILE : uses
  ACCOUNT ||--o{ ALIAS : receives_as
  DISTRIBUTION_LIST }o--o{ ACCOUNT : includes

  TENANT {
    uuid id PK
    string name
    string plan
    string status
    datetime created_at
  }

  DOMAIN {
    uuid id PK
    uuid tenant_id FK
    string name
    string status
    bool catch_all_enabled
    datetime verified_at
  }

  ACCOUNT {
    uuid id PK
    uuid domain_id FK
    string primary_email
    string display_name
    string status
    int quota_mb
    datetime created_at
  }

  MAILBOX {
    uuid id PK
    uuid account_id FK
    string backend
    string route
    int used_bytes
    datetime created_at
  }

  ALIAS {
    uuid id PK
    uuid domain_id FK
    uuid account_id FK
    string address
    string target
    string status
  }

  DISTRIBUTION_LIST {
    uuid id PK
    uuid domain_id FK
    string address
    string visibility
    string posting_policy
  }

  ADMIN_ROLE {
    uuid id PK
    uuid tenant_id FK
    string name
    json permissions
  }

  POLICY_PROFILE {
    uuid id PK
    uuid tenant_id FK
    string name
    json mail_policy
    json abuse_policy
    json retention_policy
  }

  DNS_RECORD_REQUIREMENT {
    uuid id PK
    uuid domain_id FK
    string record_type
    string name
    string value
    string status
  }
```

## Mailbox content model

```mermaid
erDiagram
  MAILBOX ||--o{ FOLDER : contains
  MAILBOX ||--o{ MESSAGE : owns
  FOLDER ||--o{ MESSAGE_PLACEMENT : contains
  MESSAGE ||--o{ MESSAGE_PLACEMENT : appears_in
  MESSAGE }o--o{ TAG : tagged_with
  MESSAGE }o--|| BLOB : references
  THREAD ||--o{ MESSAGE : groups
  MESSAGE ||--o{ ATTACHMENT : has
  BLOB }o--|| STORAGE_VOLUME : stored_on

  FOLDER {
    uuid id PK
    uuid mailbox_id FK
    string name
    string type
    uuid parent_folder_id
    string sync_token
  }

  MESSAGE {
    uuid id PK
    uuid mailbox_id FK
    uuid thread_id FK
    string message_id
    string subject
    string from_addr
    string to_addrs
    datetime sent_at
    datetime received_at
    int size_bytes
    string flags
  }

  MESSAGE_PLACEMENT {
    uuid id PK
    uuid message_id FK
    uuid folder_id FK
    datetime placed_at
  }

  THREAD {
    uuid id PK
    uuid mailbox_id FK
    string normalized_subject
    datetime last_message_at
  }

  TAG {
    uuid id PK
    uuid mailbox_id FK
    string name
    string color
  }

  BLOB {
    uuid id PK
    string storage_key
    string sha256
    int size_bytes
    string compression
    string encryption_key_ref
  }

  ATTACHMENT {
    uuid id PK
    uuid message_id FK
    uuid blob_id FK
    string filename
    string mime_type
    int size_bytes
  }

  STORAGE_VOLUME {
    uuid id PK
    string backend
    string location
    string status
  }

  ATTACHMENT_DEDUP {
    uuid id PK
    string sha256
    uuid blob_id FK
    int usage_count
    string compression
  }
```

Notes on blob storage: identical attachments across messages share a single `BLOB` record.
`ATTACHMENT` references it via `blob_id`. `ATTACHMENT_DEDUP` tracks sharing counts and
prevents orphan deletes. When usage_count reaches zero the blob is eligible for GC.

## Groupware model

```mermaid
erDiagram
  MAILBOX ||--o{ CALENDAR : owns
  CALENDAR ||--o{ CALENDAR_EVENT : contains
  CALENDAR_EVENT ||--o{ EVENT_ATTENDEE : invites
  MAILBOX ||--o{ ADDRESS_BOOK : owns
  ADDRESS_BOOK ||--o{ CONTACT : contains
  MAILBOX ||--o{ TASK_LIST : owns
  TASK_LIST ||--o{ TASK : contains
  MAILBOX ||--o{ SHARE_GRANT : grants
  SHARE_GRANT }o--|| ACCOUNT : grantee

  CALENDAR {
    uuid id PK
    uuid mailbox_id FK
    string name
    string color
    string visibility
  }

  CALENDAR_EVENT {
    uuid id PK
    uuid calendar_id FK
    string uid
    string title
    datetime starts_at
    datetime ends_at
    string recurrence_rule
    string location
  }

  EVENT_ATTENDEE {
    uuid id PK
    uuid event_id FK
    string email
    string role
    string participation_status
  }

  ADDRESS_BOOK {
    uuid id PK
    uuid mailbox_id FK
    string name
  }

  CONTACT {
    uuid id PK
    uuid address_book_id FK
    string full_name
    string email
    string phone
    json vcard
  }

  TASK_LIST {
    uuid id PK
    uuid mailbox_id FK
    string name
  }

  TASK {
    uuid id PK
    uuid task_list_id FK
    string title
    string status
    datetime due_at
  }

  SHARE_GRANT {
    uuid id PK
    uuid mailbox_id FK
    uuid grantee_account_id FK
    string resource_type
    uuid resource_id
    string permission
  }
  ACCOUNT ||--o{ ACCOUNT_DELEGATE : delegates
  ACCOUNT_DELEGATE }o--|| ACCOUNT : delegate_account
  RESOURCE ||--o{ RESOURCE_CALENDAR : contains
  RESOURCE_CALENDAR ||--o{ RESOURCE_BOOKING : has
  RESOURCE }o--o{ ACCOUNT_DELEGATE : delegated_to

  RESOURCE {
    uuid id PK
    uuid domain_id FK
    string name
    string type
    json config
    bool enabled
  }

  RESOURCE_CALENDAR {
    uuid id PK
    uuid resource_id FK
    string name
    string visibility
  }

  RESOURCE_CALENDAR_SLOT {
    uuid id PK
    uuid resource_calendar_id FK
    string start_slot
    string end_slot
    string day_of_week
    bool is_working
    string timezone
  }

  RESOURCE_BOOKING {
    uuid id PK
    uuid resource_calendar_id FK
    string event_uid
    datetime starts_at
    datetime ends_at
    string organizer_email
    string status
    int attendees_count
  }

  ACCOUNT_DELEGATE {
    uuid id PK
    uuid source_account_id FK
    uuid delegate_account_id FK
    string type
    string level
    bool auto_implicit
    bool send_notifications
  }
```

Supported delegation levels:
- `full_access` — read, create, modify, delete on behalf of owner
- `write` — read and create only
- `bcc` — silently copy all incoming mail to delegate
- `auto_reply` — delegate can send auto-replies as owner

Resource types: `room`, `equipment`, `space`.

## Abuse/quarantine model

```mermaid
erDiagram
  TENANT ||--o{ ABUSE_POLICY : defines
  DOMAIN ||--o{ ABUSE_POLICY : overrides
  ACCOUNT ||--o{ USER_ABUSE_PREFERENCE : customizes
  MESSAGE ||--o{ ABUSE_SCAN : scanned_by
  ABUSE_SCAN ||--o{ ABUSE_SYMBOL : includes
  ABUSE_SCAN ||--o{ AUTH_RESULT : includes
  ABUSE_SCAN ||--o{ URL_OBSERVATION : includes
  ABUSE_SCAN ||--o{ ATTACHMENT_OBSERVATION : includes
  ABUSE_SCAN ||--|| ABUSE_DECISION : produces
  ABUSE_DECISION ||--o| QUARANTINE_ITEM : may_create
  QUARANTINE_ITEM ||--o{ QUARANTINE_ACTION : has
  ACCOUNT ||--o{ QUARANTINE_ACTION : performed_by
  ACCOUNT ||--o{ QUARANTINE_SUBSCRIPTION : has
  TENANT ||--o{ ABUSE_POLICY : defines

  TENANT_RESOURCE_QUOTA {
    uuid id PK
    uuid tenant_id FK
    int max_accounts
    int max_domains
    int max_messages
    int max_storage_bytes
    int smtp_connections_per_minute
    int api_requests_per_minute
    int search_queries_per_minute
  }

  ABUSE_POLICY {
    uuid id PK
    string scope_type
    uuid scope_id
    float reject_threshold
    float quarantine_threshold
    float junk_threshold
    json vip_impersonation_rules
    json attachment_rules
  }

  USER_ABUSE_PREFERENCE {
    uuid id PK
    uuid account_id FK
    bool user_quarantine_enabled
    bool external_banner_enabled
    json allowlist
    json blocklist
  }

  ABUSE_SCAN {
    uuid id PK
    uuid message_id FK
    string engine
    string version
    float score
    datetime scanned_at
  }

  ABUSE_SYMBOL {
    uuid id PK
    uuid scan_id FK
    string symbol
    float weight
    string category
    string evidence
  }

  AUTH_RESULT {
    uuid id PK
    uuid scan_id FK
    string mechanism
    string domain
    string result
  }

  URL_OBSERVATION {
    uuid id PK
    uuid scan_id FK
    string url_hash
    string domain
    string risk
    json evidence
  }

  ATTACHMENT_OBSERVATION {
    uuid id PK
    uuid scan_id FK
    string filename
    string sha256
    string mime_type
    string verdict
  }

  ABUSE_DECISION {
    uuid id PK
    uuid scan_id FK
    string action
    string reason
    uuid policy_id FK
  }

  QUARANTINE_ITEM {
    uuid id PK
    uuid message_id FK
    uuid decision_id FK
    string status
    datetime expires_at
  }

  QUARANTINE_ACTION {
    uuid id PK
    uuid quarantine_item_id FK
    uuid actor_account_id FK
    string action
    string reason
    datetime created_at
  }

  QUARANTINE_SUBSCRIPTION {
    uuid id PK
    uuid account_id FK
    string frequency
    string format
    bool enabled
    datetime created_at
    datetime last_sent_at
  }

  QUARANTINE_SUBSCRIPTION_EVENT {
    uuid id PK
    uuid subscription_id FK
    string event_type
    json payload
    datetime created_at
  }
```

## Backup, migration, and audit model

```mermaid
erDiagram
  TENANT ||--o{ JOB : owns
  JOB ||--o{ JOB_EVENT : emits
  JOB ||--o| BACKUP_SNAPSHOT : creates
  JOB ||--o| MIGRATION_BATCH : runs
  BACKUP_SNAPSHOT ||--o{ RESTORE_POINT : contains
  MIGRATION_BATCH ||--o{ MIGRATION_ITEM : contains
  ACCOUNT ||--o{ AUDIT_EVENT : actor
  TENANT ||--o{ AUDIT_EVENT : records

  JOB {
    uuid id PK
    uuid tenant_id FK
    string type
    string status
    int progress_percent
    datetime created_at
    datetime finished_at
  }

  JOB_EVENT {
    uuid id PK
    uuid job_id FK
    string level
    string message
    json details
    datetime created_at
  }

  BACKUP_SNAPSHOT {
    uuid id PK
    uuid job_id FK
    string scope_type
    uuid scope_id
    string storage_key
    string status
    datetime created_at
  }

  RESTORE_POINT {
    uuid id PK
    uuid backup_snapshot_id FK
    string object_type
    uuid object_id
    datetime point_in_time
  }

  MIGRATION_BATCH {
    uuid id PK
    uuid job_id FK
    string source_type
    json connection_config
    string status
  }

  MIGRATION_ITEM {
    uuid id PK
    uuid migration_batch_id FK
    string source_identifier
    string target_identifier
    string status
    string error
  }

  AUDIT_EVENT {
    uuid id PK
    uuid tenant_id FK
    uuid actor_account_id FK
    string action
    string object_type
    uuid object_id
    json before_after
    datetime created_at
  }
```

## Policy inheritance model

```mermaid
flowchart TB
  Global[Global platform defaults]
  Tenant[Tenant policy]
  Domain[Domain policy]
  COS[Class-of-service/group policy]
  Account[Account policy]
  Effective[Effective runtime policy]

  Global --> Tenant --> Domain --> COS --> Account --> Effective

  Effective --> MailLimits[Mailbox quota, attachment size, send limits]
  Effective --> Abuse[Spam/phishing thresholds]
  Effective --> Security[Auth, MFA, session, app passwords]
  Effective --> Retention[Retention, backup, purge]
  Effective --> Sharing[Calendar/contact/delegation rules]
```

## DNS model

The product should generate and track DNS requirements per domain.

```mermaid
erDiagram
  DOMAIN ||--o{ DNS_RECORD_REQUIREMENT : requires
  DNS_RECORD_REQUIREMENT ||--o{ DNS_CHECK_RESULT : checked_by

  DNS_RECORD_REQUIREMENT {
    uuid id PK
    uuid domain_id FK
    string type
    string host
    string expected_value
    bool required
    string purpose
  }

  DNS_CHECK_RESULT {
    uuid id PK
    uuid dns_record_requirement_id FK
    string observed_value
    string status
    datetime checked_at
  }
```

Required records should include MX, SPF, DKIM, DMARC, autodiscovery where applicable, MTA-STS/TLS-RPT later, and optional BIMI later.

## Message delivery and bounce model

Messages have a full delivery lifecycle tracked in the DB. Bounce handling,
DSNs, and delivery receipts are first-class data.

```mermaid
erDiagram
  MESSAGE ||--o{ MESSAGE_DELIVERY : delivers_to
  MESSAGE_DELIVERY ||--o{ MESSAGE_DELIVERY_STATUS : updates
  MESSAGE ||--o{ MESSAGE_BOUNCE : generates
  MESSAGE ||--o{ MESSAGE_DSN : generates
  MESSAGE ||--o{ MESSAGE_READ_RECEIPT : triggers
  ACCOUNT ||--o{ ACCOUNT_SESSION : maintains
  ACCOUNT ||--o{ APP_PASSWORD : owns
  TENANT ||--o{ TENANT_RESOURCE_QUOTA : enforces

  MESSAGE_DELIVERY {
    uuid id PK
    uuid message_id FK
    string recipient
    string smtp_host
    int smtp_port
    datetime queued_at
    datetime delivered_at
    string delivery_method
  }

  MESSAGE_DELIVERY_STATUS {
    uuid id PK
    uuid delivery_id FK
    string status
    string smtp_response
    json diagnostic_code
    datetime updated_at
  }

  MESSAGE_BOUNCE {
    uuid id PK
    uuid message_id FK
    string bounce_type
    string bounce_category
    string smtp_code
    string diagnostic_code
    string original_recipient
    json original_message_headers
    datetime bounced_at
  }

  MESSAGE_DSN {
    uuid id PK
    uuid message_id FK
    string original_message_id
    string reporting_mta
    string action
    string final_recipient
    string status_code
    string diagnostic_code
    datetime received_at
  }

  MESSAGE_READ_RECEIPT {
    uuid id PK
    uuid message_id FK
    uuid account_id FK
    string user_agent
    json ip_address
    datetime created_at
  }

  ACCOUNT_SESSION {
    uuid id PK
    uuid account_id FK
    string device_id
    string user_agent
    string ip_address
    datetime last_active
    datetime expires_at
    bool is_mobile
    json client_metadata
  }

  APP_PASSWORD {
    uuid id PK
    uuid account_id FK
    string name
    string password_hash
    datetime last_used_at
    bool enabled
    datetime created_at
  }

  TENANT_RESOURCE_QUOTA {
    uuid id PK
    uuid tenant_id FK
    int max_accounts
    int max_domains
    int max_messages
    int max_storage_bytes
    int smtp_connections_per_minute
    int api_requests_per_minute
    int search_queries_per_minute
    bool overage_allowed
    string overage_action
  }
```

Supported session fields: concurrent session limit enforced at login time.
Idle timeout (configurable per policy) triggers session expiry.
Concurrent session revocation is admin-controlled.

App passwords are one-time-use hashed tokens for IMAP/DAV clients when MFA is enabled.

---

## Outbound security and shadow-copy model

Enterprise security requires outbound message audit via shadow-copying (BCC to
security mailbox) and S/MIME encryption tracking.

```mermaid
erDiagram
  MESSAGE ||--o{ OUTBOUND_SHADOW_COPY : creates
  OUTBOUND_SHADOW_COPY ||--|| SECURITY_MAILBOX : stores_in
  SECURITY_MAILBOX }o--|| MAILBOX : contains
  MESSAGE ||--o{ SMIME_ENCRYPTION : protected_by

  OUTBOUND_SHADOW_COPY {
    uuid id PK
    uuid message_id FK
    uuid security_mailbox_id FK
    string reason
    string policy_id FK
    datetime shadowed_at
  }

  SECURITY_MAILBOX {
    uuid id PK
    uuid mailbox_id FK
    string retention_policy
    string access_level
    bool immutable
    bool legal_hold_applied
  }

  SMIME_ENCRYPTION {
    uuid id PK
    uuid message_id FK
    string method
    string certificate_subject
    string certificate_issuer
    string key_size
    datetime encrypted_at
  }
```

Shadow-copy policy fields:
- `always` — shadow every outbound message
- `by_recipient` — shadow when recipient is external
- `by_domain` — shadow when recipient matches blocked TLD list
- `by_keyword` — shadow when body contains credential/payment keywords
- `by_outbound_volume` — shadow when user exceeds daily outbound threshold

---

## DMARC reporting and threat intelligence model

DMARC aggregate/forensic reports and threat intelligence observations are stored
for compliance and adaptive filtering.

```mermaid
erDiagram
  DOMAIN ||--o{ DMARC_REPORT : receives
  DMARC_REPORT ||--o{ DMARC_REPORT_RECORD : contains
  DOMAIN ||--o{ THREAT_INTEL_OBSERVATION : tracks
  THREAT_INTEL_OBSERVATION ||--o{ THREAT_INTEL_SOURCE : sourced_from

  DMARC_REPORT {
    uuid id PK
    uuid domain_id FK
    string report_id
    string format
    string source_org
    string source_ip
    string date_begin
    string date_end
    string original_contact
    string modified_contact
    string report_type
    string attachment_encoding
    string attachment_filename
    datetime received_at
    json raw_attachment_metadata
  }

  DMARC_REPORT_RECORD {
    uuid id PK
    uuid dmarc_report_id FK
    string source_ip
    int message_count
    string alignment_dkim
    string alignment_spf
    float dkim_aligned
    float spf_aligned
    string dkim_mfa
    string spf_mfa
    string first_seen
    string last_seen
  }

  THREAT_INTEL_OBSERVATION {
    uuid id PK
    uuid domain_id FK
    string indicator_type
    string indicator_value
    string indicator_category
    float confidence_score
    string source
    datetime observed_at
    bool is_blocklisted
    json threat_details
  }

  THREAT_INTEL_SOURCE {
    uuid id PK
    string name
    string type
    string feed_url
    bool active
    float trust_score
    json credentials
    datetime last_update
  }
```

Threat intel supports indicators: domain, IP, URL hash, email hash, file hash,
display name, attachment filename. Categories: phishing, malware, spam,
business email compromise (BEC), credential harvesting.
