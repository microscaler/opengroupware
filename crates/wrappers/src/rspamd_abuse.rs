// Rspamd abuse provider — concrete implementation of AbuseProvider.

use types::providers::{
    DmarcResult, ScoringAction, ScoringResult, ThreatIndicatorType, ThreatIntelObservation,
};
use types::{Page, ProviderContext, ProviderError, QuarantineItem, Uuid};

/// Configuration for connecting to Rspamd.
#[derive(Debug, Clone)]
pub struct RspamdConfig {
    pub host: String,
    pub port: u16,
    pub password: String,
    pub bayes_path: String,
}

/// Concrete implementation of AbuseProvider backed by Rspamd.
pub struct RspamdAbuseProvider {
    config: RspamdConfig,
}

impl RspamdAbuseProvider {
    #[must_use]
    pub const fn new(host: String, port: u16, password: String, bayes_path: String) -> Self {
        Self {
            config: RspamdConfig {
                host,
                port,
                password,
                bayes_path,
            },
        }
    }
}

#[async_trait::async_trait]
impl types::providers::AbuseProvider for RspamdAbuseProvider {
    async fn score_message(
        &self,
        ctx: &ProviderContext,
        raw_message: &[u8],
    ) -> Result<ScoringResult, ProviderError> {
        // TODO: POST /rspamd/scan with raw message
        // Maps to: RSPAMD_SCORE
        let _ = (ctx, raw_message);
        Ok(ScoringResult {
            message_id: Uuid::new_v4(),
            tenant_id: ctx.tenant_id,
            spam_score: 0.0,
            spam_threshold: 5.0,
            is_spam: false,
            virus: false,
            phishing_score: 0.0,
            dmarc_result: Some(DmarcResult::Pass),
            action: ScoringAction::Accept,
        })
    }

    async fn check_threat_intel(
        &self,
        ctx: &ProviderContext,
        indicator_type: ThreatIndicatorType,
        indicator_value: &str,
    ) -> Result<Option<ThreatIntelObservation>, ProviderError> {
        // TODO: Check against Rspamd RCPT/URL/FP feeds
        // Maps to: RSPAMD_THREAT_INTEL
        let _ = (ctx, indicator_type, indicator_value);
        Ok(None)
    }

    async fn train_bayes(
        &self,
        ctx: &ProviderContext,
        message_id: Uuid,
        is_spam: bool,
    ) -> Result<(), ProviderError> {
        // TODO: POST /rspamd/train_{ham,spam} with message bytes
        // Maps to: RSPAMD_TRAIN_BAYES
        let _ = (ctx, message_id, is_spam);
        Ok(())
    }

    async fn quarantine_message(
        &self,
        ctx: &ProviderContext,
        message_id: Uuid,
        reason: &str,
        scoring_result: &ScoringResult,
    ) -> Result<Uuid, ProviderError> {
        // TODO: Move message to Rspamd quarantine bucket
        // Maps to: RSPAMD_QUARANTINE
        let _ = (ctx, message_id, reason, scoring_result);
        Ok(Uuid::new_v4())
    }

    async fn release_quarantine(
        &self,
        ctx: &ProviderContext,
        quarantine_id: Uuid,
        account_id: Uuid,
    ) -> Result<(), ProviderError> {
        // TODO: Release from Rspamd quarantine to user mailbox
        // Maps to: RSPAMD_RELEASE_QUARANTINE
        let _ = (ctx, quarantine_id, account_id);
        Ok(())
    }

    async fn delete_quarantine(
        &self,
        ctx: &ProviderContext,
        quarantine_id: Uuid,
    ) -> Result<(), ProviderError> {
        // TODO: Delete from Rspamd quarantine
        // Maps to: RSPAMD_DELETE_QUARANTINE
        let _ = (ctx, quarantine_id);
        Ok(())
    }

    async fn list_quarantine(
        &self,
        ctx: &ProviderContext,
        account_id: Uuid,
        cursor: Option<String>,
        limit: u32,
    ) -> Result<Page<QuarantineItem>, ProviderError> {
        // TODO: GET /rspamd/quarantine?account=:id&cursor=:c&limit=:n
        // Maps to: RSPAMD_LIST_QUARANTINE
        let _ = (ctx, account_id, cursor, limit);
        Ok(Page {
            items: vec![],
            next_cursor: None,
            has_more: false,
        })
    }
}
