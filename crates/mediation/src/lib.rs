//! Marketplace-mediated messaging.
//!
//! Scoped aliases, disintermediation heuristics, and redaction that preserves a
//! sealed original. Deliberately free of mail/database dependencies so the
//! rules can be tuned against a corpus before any plumbing exists.
//!
//! Design: `hauliage/docs/design-mediated-messaging.md`.
//!
//! ## Two invariants worth not breaking
//!
//! **Symmetric redaction.** Sender and recipient see the same rendered body.
//! Per-recipient views would give an arbitrator two versions of one
//! conversation, and would leave the sender believing details were delivered
//! when they were not (§4a).
//!
//! **Observe-only by default.** [`Mode::Observe`] scores and logs but redacts
//! nothing. Freight text is full of container numbers, trailer registrations
//! and weighbridge tickets that resemble phone and account numbers; enforcement
//! before the false-positive rate is measured breaks real jobs (§4a.5).

pub mod alias;
pub mod symbols;

use serde::{Deserialize, Serialize};
use symbols::{Category, Stage, Symbol, RULES};

/// Whether findings are enforced or merely recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Score and log only. No redaction, no holds. **Launch in this mode.**
    Observe,
    /// Apply redactions and holds.
    Enforce,
}

/// What the pipeline should do with a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Deliver,
    /// Deliver, but with matched spans masked in the canonical body.
    Redact,
    /// Do not deliver; queue for human review.
    Hold,
    /// Refuse and tell the sender why.
    Reject,
}

/// Score bands. Tune against a corpus; these are starting points.
pub const REDACT_THRESHOLD: f64 = 5.0;
pub const HOLD_THRESHOLD: f64 = 9.0;
pub const REJECT_THRESHOLD: f64 = 15.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verdict {
    pub action: Action,
    pub score: f64,
    pub symbols: Vec<Symbol>,
    /// Body as every participant will see it. Equals the original in
    /// [`Mode::Observe`], or when nothing fired.
    pub rendered_body: String,
    /// Always the untouched input. Archived sealed; never rendered in-thread.
    pub sealed_original: String,
    /// Plain-language reasons, shown to the sender.
    pub reasons: Vec<String>,
    pub mode: Mode,
}

impl Verdict {
    #[must_use]
    pub fn was_modified(&self) -> bool {
        self.rendered_body != self.sealed_original
    }

    #[must_use]
    pub fn has_category(&self, category: Category) -> bool {
        self.symbols
            .iter()
            .any(|s| symbols::category_of(&s.name) == Some(category))
    }
}

/// Message being assessed.
#[derive(Debug, Clone)]
pub struct Message<'a> {
    pub body: &'a str,
    pub stage: Stage,
    pub mode: Mode,
}

const REDACTION_MARK: &str = "[removed]";

/// Scan, score, and produce a verdict.
///
/// Contact details pre-award are redacted, not blocked:
///
/// ```
/// use mediation::{scan, Message, Mode, Action};
/// use mediation::symbols::Stage;
///
/// let v = scan(&Message {
///     body: "Interested in this load. Call me on +263 77 123 4567.",
///     stage: Stage::PreAward,
///     mode: Mode::Enforce,
/// });
/// assert_eq!(v.action, Action::Redact);
/// assert!(v.was_modified());
/// assert!(v.sealed_original.contains("+263 77 123 4567"));
/// ```
///
/// Attempts to settle off-platform are held for review instead:
///
/// ```
/// use mediation::{scan, Message, Mode, Action};
/// use mediation::symbols::Stage;
///
/// let v = scan(&Message {
///     body: "For the next one we can deal directly.",
///     stage: Stage::PostAward,
///     mode: Mode::Enforce,
/// });
/// assert_eq!(v.action, Action::Hold);
/// ```
///
/// Stacked attempts escalate to outright rejection:
///
/// ```
/// use mediation::{scan, Message, Mode, Action};
/// use mediation::symbols::Stage;
///
/// let v = scan(&Message {
///     body: "We can deal directly and avoid the commission.",
///     stage: Stage::PostAward,
///     mode: Mode::Enforce,
/// });
/// assert_eq!(v.action, Action::Reject);
/// ```
#[must_use]
pub fn scan(msg: &Message<'_>) -> Verdict {
    let mut symbols: Vec<Symbol> = RULES
        .iter()
        .flat_map(|rule| rule.scan(msg.body, msg.stage))
        .collect();

    symbols.sort_by_key(|s| s.span.0);

    let score: f64 = symbols.iter().map(|s| s.weight).sum();

    let has = |c: Category| {
        symbols
            .iter()
            .any(|s| symbols::category_of(&s.name) == Some(c))
    };

    // Severe fraud signals bypass score bands entirely: a payment-redirection
    // attempt is held even when it is the only thing in the message.
    let severe_fraud = symbols
        .iter()
        .filter(|s| symbols::category_of(&s.name) == Some(Category::FraudSafety))
        .any(|s| s.weight >= 10.0);

    // Action is driven by the most serious *category* present, with score as a
    // secondary escalator. Score alone gets this wrong: two contact details in
    // one message would out-score a single bank-redirection attempt, yet the
    // right responses are opposite — redact the first, hold the second.
    let action = if msg.mode == Mode::Observe {
        Action::Deliver
    } else if severe_fraud {
        Action::Hold
    } else if has(Category::OffPlatformSettlement) {
        if score >= REJECT_THRESHOLD {
            Action::Reject
        } else {
            Action::Hold
        }
    } else if has(Category::ContactExchange) {
        // Never hold for contact details alone — redaction enforces blindness
        // without stalling the conversation (design §4a.2).
        Action::Redact
    } else if score >= HOLD_THRESHOLD {
        Action::Hold
    } else if score >= REDACT_THRESHOLD {
        Action::Redact
    } else {
        Action::Deliver
    };

    let rendered_body = if action == Action::Redact {
        redact(msg.body, &symbols)
    } else {
        msg.body.to_string()
    };

    Verdict {
        action,
        score,
        reasons: reasons_for(&symbols, msg.stage),
        symbols,
        rendered_body,
        sealed_original: msg.body.to_string(),
        mode: msg.mode,
    }
}

/// Mask matched spans, right to left so earlier offsets stay valid.
fn redact(body: &str, symbols: &[Symbol]) -> String {
    let mut out = body.to_string();
    let mut spans: Vec<(usize, usize)> = symbols.iter().map(|s| s.span).collect();
    spans.sort_unstable();
    spans.dedup();
    for (start, end) in spans.into_iter().rev() {
        if start <= end && end <= out.len() && out.is_char_boundary(start) && out.is_char_boundary(end) {
            out.replace_range(start..end, REDACTION_MARK);
        }
    }
    out
}

/// Sender-facing explanations. Factual, not scolding — the goal is that they
/// learn the rule once (§4a.4).
fn reasons_for(symbols: &[Symbol], stage: Stage) -> Vec<String> {
    let mut reasons = Vec::new();
    let has = |c: Category| {
        symbols
            .iter()
            .any(|s| symbols::category_of(&s.name) == Some(c))
    };

    if has(Category::ContactExchange) && stage == Stage::PreAward {
        reasons.push(
            "Contact details were not delivered. Bidding is blind until a job is awarded — \
             once your quote is accepted you can exchange details freely."
                .to_string(),
        );
    }
    if has(Category::OffPlatformSettlement) {
        reasons.push(
            "Payment arrangements must stay on the platform. Funds are held in safeguarded \
             custody and released on proof of delivery, which protects both sides."
                .to_string(),
        );
    }
    if has(Category::FraudSafety) {
        reasons.push(
            "This message was held for review because it matches a known fraud pattern. \
             Payment details are never changed by message."
                .to_string(),
        );
    }
    reasons
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observe(body: &str, stage: Stage) -> Verdict {
        scan(&Message { body, stage, mode: Mode::Observe })
    }
    fn enforce(body: &str, stage: Stage) -> Verdict {
        scan(&Message { body, stage, mode: Mode::Enforce })
    }
    fn fired(v: &Verdict, name: &str) -> bool {
        v.symbols.iter().any(|s| s.name == name)
    }

    // ---- true positives -------------------------------------------------

    #[test]
    fn phone_with_cue_fires_pre_award() {
        let v = enforce("Interested — call me on +263 77 123 4567", Stage::PreAward);
        assert!(fired(&v, "LL_PHONE_IN_BID"));
        assert_eq!(v.action, Action::Redact);
        assert!(v.rendered_body.contains("[removed]"));
        assert!(v.sealed_original.contains("+263 77 123 4567"));
    }

    #[test]
    fn whatsapp_link_fires() {
        let v = enforce("Message me https://wa.me/263771234567", Stage::PreAward);
        assert!(fired(&v, "LL_MESSAGING_HANDLE"));
    }

    #[test]
    fn off_platform_terms_fire_at_any_stage() {
        for stage in [Stage::PreAward, Stage::PostAward] {
            let v = enforce("We can deal directly and avoid the commission", stage);
            assert!(fired(&v, "LL_OFFPLATFORM_TERMS"), "stage {stage:?}");
        }
    }

    #[test]
    fn bank_details_with_cue_fire() {
        let v = enforce(
            "Please pay into account 0114 5566 7788 at our branch",
            Stage::PostAward,
        );
        assert!(fired(&v, "LL_BANK_DETAILS"));
    }

    #[test]
    fn mid_consignment_bank_change_is_always_held() {
        let v = enforce(
            "Our bank details have changed, please use the new account for this load.",
            Stage::PostAward,
        );
        assert!(fired(&v, "LL_BANK_CHANGE"));
        assert_eq!(v.action, Action::Hold, "payment redirection must never auto-deliver");
    }

    #[test]
    fn credential_request_is_held() {
        let v = enforce("Please send your password so I can check the load", Stage::PostAward);
        assert!(fired(&v, "LL_CREDENTIAL_REQUEST"));
        assert_eq!(v.action, Action::Hold);
    }

    // ---- false positives: real freight text that must NOT fire -----------
    // Every case below is text a dispatcher plausibly sends. A system that
    // redacts these is worse than no system.

    #[test]
    fn container_number_is_not_a_bank_account() {
        let v = observe("Container MSKU 663451 8 loading at Mashava", Stage::PostAward);
        assert!(!fired(&v, "LL_BANK_DETAILS"), "container ref must not fire: {:?}", v.symbols);
    }

    #[test]
    fn trailer_registration_is_not_a_phone_number() {
        let v = observe("Trailer ABC 1234 5678 arriving at the weighbridge", Stage::PreAward);
        assert!(!fired(&v, "LL_PHONE_IN_BID"), "trailer reg must not fire: {:?}", v.symbols);
    }

    #[test]
    fn tonnage_and_rates_do_not_fire() {
        let v = observe(
            "Bulk chrome, 30 tonnes, rate 1850 per load, 70% after loading 30% on POD",
            Stage::PreAward,
        );
        assert_eq!(v.score, 0.0, "ordinary load terms must score zero: {:?}", v.symbols);
    }

    #[test]
    fn weighbridge_ticket_does_not_fire() {
        let v = observe("Weighbridge ticket 00987654321 attached for the Shurugwi load", Stage::PostAward);
        assert!(!fired(&v, "LL_BANK_DETAILS"), "{:?}", v.symbols);
    }

    #[test]
    fn urgency_alone_is_not_enough() {
        let v = enforce("Please load urgently, the vessel stacks Friday", Stage::PostAward);
        assert_eq!(v.action, Action::Deliver);
    }

    // ---- stage sensitivity ----------------------------------------------

    #[test]
    fn contact_details_are_fine_after_award() {
        let v = enforce(
            "Driver is Tendai, call him on +263 77 123 4567 at the gate",
            Stage::PostAward,
        );
        assert!(!fired(&v, "LL_PHONE_IN_BID"), "post-award contact is operational necessity");
        assert_eq!(v.action, Action::Deliver);
        assert!(!v.was_modified());
    }

    // ---- mode ------------------------------------------------------------

    #[test]
    fn observe_mode_scores_but_never_modifies() {
        let body = "Call me on +263 77 123 4567 and we can deal directly";
        let v = observe(body, Stage::PreAward);
        assert!(v.score > 0.0, "should still score");
        assert!(!v.symbols.is_empty());
        assert_eq!(v.action, Action::Deliver);
        assert!(!v.was_modified(), "observe mode must never alter the body");
        assert_eq!(v.rendered_body, body);
    }

    // ---- redaction integrity --------------------------------------------

    #[test]
    fn multiple_redactions_keep_body_coherent() {
        let v = enforce(
            "Call +263 77 123 4567 or email me at bob@carrier.co.zw instead",
            Stage::PreAward,
        );
        assert!(!v.rendered_body.contains("+263 77 123 4567"));
        assert!(!v.rendered_body.contains("bob@carrier.co.zw"));
        assert!(v.rendered_body.contains("Call"));
        assert!(v.rendered_body.contains("instead"));
    }

    #[test]
    fn sealed_original_always_survives() {
        let body = "bank details have changed, pay into 0114 5566 7788 urgently";
        let v = enforce(body, Stage::PostAward);
        assert_eq!(v.sealed_original, body, "the original must always be preserved intact");
    }

    #[test]
    fn unicode_body_does_not_panic() {
        let v = enforce("Olá — ligue para +258 84 123 4567 por favor 🚚", Stage::PreAward);
        assert!(v.rendered_body.contains("Olá"));
    }
}
