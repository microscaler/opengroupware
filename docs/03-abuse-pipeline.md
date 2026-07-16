# 03 — Abuse Pipeline: Spam, Phishing, Malware, and Quarantine

This is the most important subsystem after identity and mailbox storage.

A modern mailgroupware platform should treat abuse filtering as a first-class workflow, not as a hidden daemon that occasionally adds `X-Spam` headers.

## Goals

- Reject obvious abusive traffic before accepting responsibility for the message.
- Score ambiguous mail with explainable evidence.
- Quarantine suspicious mail in a product-owned workflow.
- Feed user/admin decisions back into training.
- Protect outbound reputation from compromised accounts.
- Give domain admins policy control without forcing them to edit low-level Rspamd/Postfix config.

## Abuse architecture

```mermaid
flowchart TB
  classDef ingress fill:#fff7d6,stroke:#f9a825,color:#111
  classDef engine fill:#e8f7e8,stroke:#2e7d32,color:#111
  classDef product fill:#e8f0fe,stroke:#1565c0,color:#111
  classDef data fill:#f3e5f5,stroke:#6a1b9a,color:#111
  classDef risk fill:#fdeaea,stroke:#b71c1c,color:#111

  SMTP[SMTP ingress/submission]:::ingress
  Rspamd[Rspamd scoring engine]:::engine
  ClamAV[ClamAV/clamd]:::engine
  Redis[(Redis/Valkey state)]:::data
  Reputation[DNSBL/URIBL/SURBL/reputation]:::risk
  Fuzzy[Fuzzy hashes/campaign detection]:::engine
  URL[URL and attachment analysis]:::engine
  PolicyAPI[Product policy API]:::product
  Quarantine[(Quarantine store)]:::data
  AbuseUI[Abuse console]:::product
  Mailbox[Mailbox store]:::engine
  Training[Training/feedback jobs]:::product
  Metrics[(Metrics/audit log)]:::data
  DMARCReceiver[DMARC report receiver]:::product
  ThreatIntel[Threat intelligence store]:::data
  ShadowCopy[Outbound shadow-copy service]:::product
  SecurityMailbox[(Security mailbox)]:::data
  QuarantineDigest[Quarantine digest service]:::product
  UserNotification[(User notification store)]:::data

  SMTP --> Rspamd
  Rspamd --> ClamAV
  Rspamd --> Redis
  Rspamd --> Reputation
  Rspamd --> Fuzzy
  Rspamd --> URL
  Rspamd --> PolicyAPI
  Rspamd --> Quarantine
  Rspamd --> Mailbox
  Rspamd --> Metrics
  Rspamd --> DMARCReceiver

  DMARCReceiver --> DMARCReport[DMARC report DB]:::data
  DMARCReport --> DMARCAutoRemediate[DMARC auto-remediation<br/>reject/quarantine on fail]:::engine

  Reputation --> ThreatIntel
  ThreatIntel --> Rspamd

  AbuseUI --> Quarantine
  AbuseUI --> PolicyAPI
  AbuseUI --> Training
  Training --> Rspamd
  Training --> Metrics

  QuarantineDigest --> Quarantine
  QuarantineDigest --> UserNotification
  UserNotification --> UserEmail[Daily/weekly digest email]

  SMTP -->|outbound| OutboundSubmit[SMTP submission]:::ingress
  OutboundSubmit --> OutboundScan[Outbound Rspamd scan]:::engine
  OutboundScan --> ShadowCopy
  ShadowCopy --> SecurityMailbox
  OutboundSubmit --> DKIM[DKIM signer]:::engine
  DKIM --> Queue[Outbound queue]:::engine
  Queue --> RemoteMX[Remote MX]:::risk
```

### Threat intelligence integration

Threat intel feeds are consumed and stored in `THREAT_INTEL_OBSERVATION`. The
abuse pipeline checks threat intel at three points:

1. **SMTP connection**: IP reputation check against blocklists
2. **Content scan**: URL domains and sender domains against threat feeds
3. **Policy evaluation**: VIP/supplier domains against lookalike detection

```mermaid
flowchart LR
  Feed[Threat intel feed<br/>commercial or open-source] --> Store[THREAT_INTEL table]
  Store --> Rspamd[Rspamd threat modules]
  Store --> Policy[Product policy API]

  Store --> DomainCheck[Domain threat check]
  Store --> IPCheck[IP reputation check]
  Store --> URLCheck[URL reputation check]
  Store --> FileCheck[File/hash reputation check]

  DomainCheck --> Rspamd
  IPCheck --> Rspamd
  URLCheck --> Rspamd
  FileCheck --> Rspamd
```

### DMARC reporting

DMARC aggregate and forensic reports are received, parsed, and stored:

```mermaid
flowchart LR
  ExternalOrg[External organization] --> DMARCReports[DMARC aggregate/forensic reports]
  DMARCReports --> Receiver[DMARC report receiver<br/>SMTP endpoint]
  Receiver --> Parser[Parser<br/>XML → relational tables]
  Parser --> DMARCAggregate[DMARC aggregate report records]
  Parser --> DMARCForensic[DMARC forensic report records]
  DMARCAggregate --> Analytics[Deliverability analytics]
  DMARCAggregate --> AutoRemediate[Auto-remediation]
  DMARCForensic --> ForensicCase[Forensic case investigation]
```

DMARC auto-remediation:
- SPF fail + DKIM fail → reject or quarantine (configurable)
- DMARC policy = reject → automatic reject
- DMARC policy = quarantine → automatic quarantine
- DMARC policy = none → score-only (no auto-action)

### Outbound shadow-copy

Shadow-copying sends a BCC of every outbound message to a security mailbox for
compliance and security audit:

```mermaid
flowchart LR
  UserAuth[Authenticated user submits] --> Outbound[Outbound Rspamd scan]
  Outbound --> ShadowPolicy{Shadow-copy policy?}
  ShadowPolicy -->|always| Shadow[Copy to security mailbox]
  ShadowPolicy -->|by_recipient| Shadow
  ShadowPolicy -->|by_domain| Shadow
  ShadowPolicy -->|by_keyword| Shadow
  ShadowPolicy -->|by_outbound_volume| Shadow
  ShadowPolicy -->|none| Deliver[Deliver normally]
  Shadow --> SecurityMailbox[(Security mailbox<br/>immutable, legal hold)]
```

Shadow-copy policies (configurable per tenant):
- `always` — shadow every outbound message
- `by_recipient` — shadow when recipient is external
- `by_domain` — shadow when recipient matches blocked TLD list
- `by_keyword` — shadow when body contains credential/payment keywords
- `by_outbound_volume` — shadow when user exceeds daily outbound threshold

### Quarantine digest

Users receive periodic digests of quarantined messages:

```mermaid
flowchart LR
  Quarantine[(Quarantine store)] --> DigestJob[Digest job scheduler]
  DigestJob --> Query[Query pending quarantine items per user]
  Query --> Render[Render digest email]
  Render --> Send[Send digest email]
  Send --> UserInbox[User's regular inbox]
  DigestJob --> UpdateMark[Mark digested items<br/>in QUARANTINE_SUBSCRIPTION_EVENT]
```

Digest frequency: `daily`, `weekly`, or `immediate` (real-time notification).
Digest includes: message preview, sender, subject, spam score, action buttons
(release, delete, mark as spam/ham).

## Inbound classification decision tree

```mermaid
flowchart TB
  Start[Inbound SMTP connection]
  Conn[Connection checks<br/>IP reputation, rDNS, HELO, TLS, rate limits]
  Auth[Sender auth<br/>SPF, DKIM, DMARC, ARC]
  Content[Content scan<br/>headers, MIME, text, HTML, attachments]
  Malware[Malware scan<br/>ClamAV/clamd]
  URL[URL analysis<br/>reputation, redirects, lookalikes, punycode]
  Impersonation[Impersonation checks<br/>display name, VIPs, internal domains, suppliers]
  Score[Aggregate symbols into score]
  Policy[Apply tenant/domain/user policy]

  Reject[Reject during SMTP]
  Quarantine[Quarantine]
  Tag[Deliver to Junk / add warning banner]
  Deliver[Deliver normally]

  Start --> Conn
  Conn -->|obvious bot/bad sender| Reject
  Conn --> Auth
  Auth --> Content
  Content --> Malware
  Malware -->|malware high confidence| Reject
  Malware --> URL
  URL --> Impersonation
  Impersonation --> Score
  Score --> Policy
  Policy -->|score >= reject threshold| Reject
  Policy -->|score >= quarantine threshold| Quarantine
  Policy -->|score >= junk threshold| Tag
  Policy -->|score below threshold| Deliver
```

## Scoring symbol model

The product should not expose raw Rspamd internals to normal admins, but it should retain symbol evidence for explainability.

```mermaid
erDiagram
  MESSAGE ||--o{ ABUSE_SCAN : scanned_by
  ABUSE_SCAN ||--o{ ABUSE_SYMBOL : contains
  ABUSE_SCAN ||--o{ URL_OBSERVATION : includes
  ABUSE_SCAN ||--o{ ATTACHMENT_OBSERVATION : includes
  ABUSE_SCAN ||--o{ AUTH_RESULT : includes
  ABUSE_SCAN ||--o{ ACTION_DECISION : produces
  ACTION_DECISION ||--o| QUARANTINE_ITEM : may_create

  MESSAGE {
    uuid id
    string message_id
    string tenant_id
    string domain_id
    string sender
    string rcpt
    datetime received_at
  }

  ABUSE_SCAN {
    uuid id
    uuid message_id
    string scan_engine
    float score
    string disposition
    datetime scanned_at
  }

  ABUSE_SYMBOL {
    uuid id
    uuid abuse_scan_id
    string symbol
    float weight
    string category
    string description
  }

  URL_OBSERVATION {
    uuid id
    uuid abuse_scan_id
    string url
    string normalized_domain
    bool punycode
    bool redirect_chain_seen
    string reputation
  }

  ATTACHMENT_OBSERVATION {
    uuid id
    uuid abuse_scan_id
    string filename
    string mime_type
    string hash_sha256
    string malware_verdict
  }

  AUTH_RESULT {
    uuid id
    uuid abuse_scan_id
    string mechanism
    string result
    string domain
  }

  ACTION_DECISION {
    uuid id
    uuid abuse_scan_id
    string action
    string policy_id
    string reason
  }

  QUARANTINE_ITEM {
    uuid id
    uuid message_id
    string status
    datetime expires_at
    string release_policy
  }
```

## Policy layers

Policies should stack from broad to narrow.

```mermaid
flowchart TB
  Global[Global platform policy]
  Tenant[Tenant policy]
  Domain[Domain policy]
  Group[Group/COS policy]
  User[User preference]
  Emergency[Emergency override]
  Effective[Effective scan policy]

  Global --> Tenant --> Domain --> Group --> User --> Effective
  Emergency --> Effective
```

Suggested policy fields:

| Policy field | Example |
|---|---|
| Reject threshold | `score >= 15` |
| Quarantine threshold | `score >= 8` |
| Junk threshold | `score >= 5` |
| Malware action | reject, quarantine, hold-for-admin |
| DMARC fail action | reject, quarantine, score-only |
| External sender banner | enabled/disabled by domain/group |
| VIP impersonation list | CEO/CFO/founders/security aliases |
| Allowed sender domains | customer/supplier allowlist with auth requirements |
| High-risk TLD score | configurable symbol/weight |
| URL shortener policy | score, rewrite, quarantine, allow |
| Attachment policy | block executable/script/archive types |
| Outbound rate limits | per-user/per-domain/per-IP/device |

## Quarantine workflow

```mermaid
stateDiagram-v2
  [*] --> Created
  Created --> AwaitingReview: suspicious message held
  AwaitingReview --> Released: admin/user releases
  AwaitingReview --> Deleted: admin/user deletes
  AwaitingReview --> AutoExpired: retention timeout
  AwaitingReview --> Escalated: malware or impersonation evidence
  Released --> TrainHam
  Deleted --> TrainSpam
  Escalated --> SecurityIncident
  TrainHam --> [*]
  TrainSpam --> [*]
  AutoExpired --> [*]
  SecurityIncident --> [*]
```

## User feedback loop

```mermaid
sequenceDiagram
  autonumber
  participant User
  participant Web as Webmail UX
  participant API as Abuse API
  participant Train as Training job
  participant Rspamd
  participant Audit as Audit log

  User->>Web: Clicks Junk or Not Junk
  Web->>API: Submit feedback with message id and action
  API->>Audit: Record feedback event
  API->>Train: Enqueue training job
  Train->>Rspamd: Learn spam/ham or update local corpus
  Rspamd-->>Train: Training result
  Train->>Audit: Store outcome
```

## Outbound abuse workflow

```mermaid
stateDiagram-v2
  [*] --> Normal
  Normal --> Suspicious: unusual rate/geo/content/recipient pattern
  Suspicious --> Throttled: soft limit exceeded
  Suspicious --> Held: high-risk outbound scan
  Throttled --> Normal: rate normalizes
  Held --> Released: admin approves
  Held --> Disabled: confirmed compromise
  Disabled --> Remediated: password reset + sessions revoked + MFA enforced
  Remediated --> Normal
```

## Phishing-specific checks

```mermaid
mindmap
  root((Phishing detection))
    Authentication
      SPF fail
      DKIM fail
      DMARC reject/quarantine
      ARC validation
    Identity deception
      Display-name spoofing
      Internal-domain lookalike
      Supplier-domain lookalike
      Reply-to mismatch
      From/envelope mismatch
      Unicode homoglyphs
    URL risk
      Punycode domains
      New domains
      URL shorteners
      Redirect chains
      Lookalike domains
      Mixed-script labels
      Suspicious TLDs
    Content risk
      Credential-harvest wording
      Invoice/payment change
      MFA reset lure
      Urgency/threat language
      QR-code phishing
    Attachment risk
      Executables
      Macros
      Encrypted archives
      HTML attachments
      LNK/ISO/script files
    Behavioral risk
      New sender to many users
      First-contact with payment terms
      Outbound burst from account
      Impossible travel submission
```

## Minimum viable abuse implementation

|| Milestone | Required behavior |
||---|---|
|| MVP-1 | Rspamd integrated with SMTP ingress; ClamAV scanning; basic SPF/DKIM/DMARC; Redis state. |
|| MVP-2 | Quarantine store and abuse UI; user/admin release/delete workflow. |
|| MVP-3 | User Junk/Not Junk feedback loop; Bayes/neural training jobs. |
|| MVP-4 | Outbound scanning, rate limits, account throttling, compromised-account workflow. |
|| MVP-5 | VIP/supplier impersonation rules, lookalike domain detection, URL evidence UI. |
|| MVP-6 | DMARC report receiver and auto-remediation; shadow-copy for security audit. |
|| Later | URL rewriting/sandboxing, detonation, commercial threat intel, legal hold integration. |

## Do not defer these

- Quarantine retention model.
- Evidence storage for scan symbols.
- User/admin training workflow.
- Outbound rate limiting.
- DMARC reporting posture + auto-remediation.
- Domain-specific policy overrides.
- Safe release path that preserves auditability.
- Quarantine digest (daily/weekly user notifications).
- Threat intelligence storage (blocklist lookups).

