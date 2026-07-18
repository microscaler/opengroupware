//! Request/response types for the abuse/quarantine API.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AbuseDecision {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub message_ref: String,
    pub direction: String,
    pub recipient: String,
    pub sender: String,
    pub score: f32,
    pub action: String,
    pub verdict: String,
    pub symbols: serde_json::Value,
    pub scanned_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct QuarantineItem {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub decision_id: Uuid,
    pub message_ref: String,
    pub recipient: String,
    pub sender: String,
    pub subject: String,
    pub reason: String,
    pub status: String,
    pub reported_as: Option<String>,
    pub held_at: chrono::DateTime<chrono::Utc>,
    pub resolved_at: Option<chrono::DateTime<chrono::Utc>>,
    pub resolved_by: Option<String>,
}

/// Payload from the mail pipeline recording a scan verdict. When `action` is
/// `quarantine`, a quarantine_item is created in the same transaction.
#[derive(Debug, Deserialize)]
pub struct RecordDecision {
    pub message_ref: String,
    #[serde(default = "default_direction")]
    pub direction: String,
    pub recipient: String,
    #[serde(default)]
    pub sender: String,
    pub score: f32,
    pub action: String,
    #[serde(default = "default_verdict")]
    pub verdict: String,
    #[serde(default)]
    pub symbols: serde_json::Value,
    #[serde(default)]
    pub subject: String,
}

fn default_direction() -> String {
    "inbound".to_string()
}
fn default_verdict() -> String {
    "ham".to_string()
}

/// User/admin feedback on a quarantined message → training signal.
#[derive(Debug, Deserialize)]
pub struct ReportFeedback {
    /// "spam" or "ham".
    pub verdict: String,
}

pub const VALID_ACTIONS: [&str; 5] = ["accept", "junk", "quarantine", "reject", "discard"];
pub const VALID_VERDICTS: [&str; 4] = ["ham", "spam", "phishing", "malware"];
