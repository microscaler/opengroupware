//! LoadLinker symbol set — marketplace-specific detections layered on top of
//! Rspamd's standard abuse scoring.
//!
//! Two design rules, both learned the hard way in freight:
//!
//! 1. **Context, not patterns.** Freight correspondence is dense with numbers
//!    that look like phone numbers and account numbers — container numbers,
//!    trailer registrations, weighbridge tickets, permit references. Matching
//!    on shape alone produces a system that redacts a container number and
//!    breaks the job. Every symbol here requires a *contextual cue* near the
//!    match before it fires.
//!
//! 2. **Award stage changes everything.** Exchanging a phone number is a
//!    violation before award and an operational necessity after it. The same
//!    text scores differently depending on `Stage`.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Where the conversation sits in the consignment lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Bidding is blind. Identity and contact details must not pass.
    PreAward,
    /// Parties are contracted and need to actually execute the job.
    PostAward,
}

/// A single detection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub weight: f64,
    /// Byte range in the scanned text, for redaction.
    pub span: (usize, usize),
    /// What matched, retained for the sealed original and for reviewers.
    pub matched: String,
}

/// Rule categories, used to decide redaction and messaging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Contact detail exchange — only an issue pre-award.
    ContactExchange,
    /// Attempts to settle outside the platform.
    OffPlatformSettlement,
    /// Fraud and safety signals — serious at any stage.
    FraudSafety,
}

pub struct Rule {
    pub name: &'static str,
    pub category: Category,
    pub pre_award_weight: f64,
    pub post_award_weight: f64,
    pattern: &'static Lazy<Regex>,
    /// Words that must appear within `cue_window` bytes for the rule to fire.
    /// Empty means the pattern is specific enough to stand alone.
    cues: &'static [&'static str],
    cue_window: usize,
}

// ---------------------------------------------------------------------------
// Patterns
// ---------------------------------------------------------------------------

/// International and local formats used across ZW / ZA / MZ / BW.
static PHONE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?x)
        (?:\+|00)\s?(?:263|27|258|267|260)\s?\d[\d\s\-]{6,12}
        |
        \b0(?:7|8)\d[\d\s\-]{6,10}
    ").expect("phone regex")
});

static EMAIL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").expect("email regex")
});

static MESSAGING_HANDLE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(?:wa\.me/\S+|chat\.whatsapp\.com/\S+|t\.me/\S+|whats\s?app|telegram|signal)\b")
        .expect("handle regex")
});

/// Long digit runs that look like bank accounts, plus IBAN shapes.
static BANK_ACCOUNT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?x)
        \b[A-Z]{2}\d{2}[A-Z0-9]{10,28}\b
        |
        \b\d[\d\s\-]{7,20}\d\b
    ").expect("bank regex")
});

static MOBILE_MONEY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(?:ecocash|onemoney|telecash|m-?pesa|mukuru|innbucks)\b")
        .expect("mobile money regex")
});

static OFFPLATFORM_PHRASE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?ix)(?:
          off\s?(?:the\s)?platform
        | outside\s+(?:the\s+)?(?:platform|system|app)
        | avoid(?:ing)?\s+(?:the\s+)?(?:fee|fees|commission)
        | without\s+(?:the\s+)?(?:platform|loadlinker)
        | cash\s+on\s+(?:delivery|collection)
        | pay\s+(?:the\s+)?driver\s+(?:direct|directly|cash)
        | deal\s+direct(?:ly)?
        | cut\s+out\s+the\s+middle\s?man
    )").expect("offplatform regex")
});

static BANK_CHANGE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?ix)(?:
          (?:new|updated|changed|different)\s+(?:bank|account|banking)\s+(?:details|account|number)
        | (?:bank|account)\s+details\s+have\s+changed
        | use\s+(?:this|these|the\s+new)\s+(?:bank|account)\s+details
        | do\s+not\s+use\s+(?:the\s+)?(?:old|previous)\s+account
    )").expect("bank change regex")
});

static URGENCY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(?:urgent(?:ly)?|immediately|right\s+now|asap|before\s+(?:close|cut\s?off))\b")
        .expect("urgency regex")
});

static CREDENTIALS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:your\s+)?(?:password|otp|one[\s-]?time\s+pin|login\s+(?:details|code))")
        .expect("credentials regex")
});

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

pub static RULES: &[Rule] = &[
    Rule {
        name: "LL_PHONE_IN_BID",
        category: Category::ContactExchange,
        pre_award_weight: 6.0,
        post_award_weight: 0.0,
        pattern: &PHONE,
        // Without a cue this fires on trailer numbers and weighbridge tickets.
        cues: &["call", "phone", "whatsapp", "reach me", "contact", "ring", "cell", "mobile", "number is", "dial"],
        cue_window: 60,
    },
    Rule {
        name: "LL_ALT_EMAIL",
        category: Category::ContactExchange,
        pre_award_weight: 6.0,
        post_award_weight: 0.0,
        pattern: &EMAIL,
        cues: &[],
        cue_window: 0,
    },
    Rule {
        name: "LL_MESSAGING_HANDLE",
        category: Category::ContactExchange,
        pre_award_weight: 7.0,
        post_award_weight: 0.0,
        pattern: &MESSAGING_HANDLE,
        cues: &[],
        cue_window: 0,
    },
    Rule {
        name: "LL_BANK_DETAILS",
        category: Category::OffPlatformSettlement,
        pre_award_weight: 9.0,
        post_award_weight: 7.0,
        pattern: &BANK_ACCOUNT,
        // Critical: a bare digit run is a container or consignment reference far
        // more often than it is an account number.
        cues: &["account", "acc no", "bank", "iban", "swift", "branch", "pay into", "eft", "transfer to", "deposit"],
        cue_window: 80,
    },
    Rule {
        name: "LL_MOBILE_MONEY",
        category: Category::OffPlatformSettlement,
        pre_award_weight: 8.0,
        post_award_weight: 6.0,
        pattern: &MOBILE_MONEY,
        cues: &[],
        cue_window: 0,
    },
    Rule {
        name: "LL_OFFPLATFORM_TERMS",
        category: Category::OffPlatformSettlement,
        pre_award_weight: 9.0,
        post_award_weight: 8.0,
        pattern: &OFFPLATFORM_PHRASE,
        cues: &[],
        cue_window: 0,
    },
    Rule {
        name: "LL_BANK_CHANGE",
        category: Category::FraudSafety,
        // Mid-consignment payment redirection is the classic BEC pattern and
        // the single most costly fraud in freight. Never auto-deliver.
        pre_award_weight: 10.0,
        post_award_weight: 10.0,
        pattern: &BANK_CHANGE,
        cues: &[],
        cue_window: 0,
    },
    Rule {
        name: "LL_URGENCY_PRESSURE",
        category: Category::FraudSafety,
        // Weak alone; meaningful stacked with LL_BANK_CHANGE.
        pre_award_weight: 2.0,
        post_award_weight: 2.0,
        pattern: &URGENCY,
        cues: &["bank", "account", "payment", "pay", "transfer"],
        cue_window: 120,
    },
    Rule {
        name: "LL_CREDENTIAL_REQUEST",
        category: Category::FraudSafety,
        pre_award_weight: 10.0,
        post_award_weight: 10.0,
        pattern: &CREDENTIALS,
        cues: &["send", "share", "give", "provide", "confirm", "what is"],
        cue_window: 60,
    },
];

impl Rule {
    pub const fn weight_for(&self, stage: Stage) -> f64 {
        match stage {
            Stage::PreAward => self.pre_award_weight,
            Stage::PostAward => self.post_award_weight,
        }
    }

    /// True when one of this rule's cue words sits near `span`.
    fn cue_present(&self, haystack_lower: &str, span: (usize, usize)) -> bool {
        if self.cues.is_empty() {
            return true;
        }
        let start = span.0.saturating_sub(self.cue_window);
        let end = usize::min(span.1 + self.cue_window, haystack_lower.len());
        // Snap to char boundaries — cue windows are byte offsets.
        let start = floor_boundary(haystack_lower, start);
        let end = ceil_boundary(haystack_lower, end);
        let window = &haystack_lower[start..end];
        self.cues.iter().any(|cue| window.contains(cue))
    }

    pub fn scan(&self, text: &str, stage: Stage) -> Vec<Symbol> {
        let weight = self.weight_for(stage);
        if weight <= 0.0 {
            return Vec::new();
        }
        let lower = text.to_lowercase();
        self.pattern
            .find_iter(text)
            .filter(|m| self.cue_present(&lower, (m.start(), m.end())))
            .map(|m| Symbol {
                name: self.name.to_string(),
                weight,
                span: (m.start(), m.end()),
                matched: m.as_str().to_string(),
            })
            .collect()
    }
}

fn floor_boundary(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil_boundary(s: &str, mut i: usize) -> usize {
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Category of a symbol by name, for redaction decisions.
#[must_use]
pub fn category_of(symbol_name: &str) -> Option<Category> {
    RULES
        .iter()
        .find(|r| r.name == symbol_name)
        .map(|r| r.category)
}
