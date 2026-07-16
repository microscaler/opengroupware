// Abuse provider interface — spam/phishing/malware scoring.

use crate::{DateTime, Page, ProviderContext, ProviderError, QuarantineItem, Uuid};
use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringResult {
    pub message_id: Uuid,
    pub tenant_id: Uuid,
    pub spam_score: f64,
    pub spam_threshold: f64,
    pub is_spam: bool,
    pub virus: bool,
    pub phishing_score: f64,
    pub dmarc_result: Option<DmarcResult>,
    pub action: ScoringAction,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DmarcResult {
    Pass,
    Fail,
    Softfail,
    None,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScoringAction {
    Accept,
    Greylist,
    Quarantine,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatIntelObservation {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub indicator_type: ThreatIndicatorType,
    pub indicator_value: String,
    pub severity: ThreatSeverity,
    pub source: String,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThreatIndicatorType {
    IpAddress,
    Domain,
    Url,
    FileHash,
    EmailAddress,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThreatSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[async_trait::async_trait]
pub trait AbuseProvider: Send + Sync {
    async fn score_message(
        &self,
        ctx: &ProviderContext,
        raw_message: &[u8],
    ) -> Result<ScoringResult, ProviderError>;
    async fn check_threat_intel(
        &self,
        ctx: &ProviderContext,
        indicator_type: ThreatIndicatorType,
        indicator_value: &str,
    ) -> Result<Option<ThreatIntelObservation>, ProviderError>;
    async fn train_bayes(
        &self,
        ctx: &ProviderContext,
        message_id: Uuid,
        is_spam: bool,
    ) -> Result<(), ProviderError>;
    async fn quarantine_message(
        &self,
        ctx: &ProviderContext,
        message_id: Uuid,
        reason: &str,
        scoring_result: &ScoringResult,
    ) -> Result<Uuid, ProviderError>;
    async fn release_quarantine(
        &self,
        ctx: &ProviderContext,
        quarantine_id: Uuid,
        account_id: Uuid,
    ) -> Result<(), ProviderError>;
    async fn delete_quarantine(
        &self,
        ctx: &ProviderContext,
        quarantine_id: Uuid,
    ) -> Result<(), ProviderError>;
    async fn list_quarantine(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        cursor: Option<String>,
        limit: u32,
    ) -> Result<Page<QuarantineItem>, ProviderError>;
}
