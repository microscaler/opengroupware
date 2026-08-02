//! Scoped relay aliases.
//!
//! Aliases are scoped to a thread rather than permanent per user, so that
//! masking survives a relationship going sour and addresses cannot be
//! harvested from one interaction and reused later.
//!
//! Tokens must be opaque: an alias that encodes the counterparty's identity or
//! an enumerable id defeats blind bidding.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AliasScope {
    /// Pre-award. Reveals nothing about the party behind it.
    Quote,
    /// Post-award, bound to a consignment.
    Consignment,
    /// General organisation contact.
    Organisation,
}

impl AliasScope {
    #[must_use]
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Quote => "q",
            Self::Consignment => "c",
            Self::Organisation => "org",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alias {
    pub scope: AliasScope,
    /// Opaque, non-guessable. Never derived from a party identifier.
    pub token: String,
    pub domain: String,
}

impl Alias {
    #[must_use]
    pub fn new(scope: AliasScope, token: impl Into<String>, domain: impl Into<String>) -> Self {
        Self { scope, token: token.into(), domain: domain.into() }
    }

    #[must_use]
    pub fn address(&self) -> String {
        format!("{}-{}@{}", self.scope.prefix(), self.token, self.domain)
    }

    /// Parse an inbound recipient back to a scope and token.
    #[must_use]
    pub fn parse(address: &str) -> Option<Self> {
        let (local, domain) = address.split_once('@')?;
        let (prefix, token) = local.split_once('-')?;
        if token.is_empty() {
            return None;
        }
        let scope = match prefix {
            "q" => AliasScope::Quote,
            "c" => AliasScope::Consignment,
            "org" => AliasScope::Organisation,
            _ => return None,
        };
        Some(Self::new(scope, token, domain))
    }

    /// Pre-award aliases must leak nothing about the party.
    #[must_use]
    pub const fn is_masked(&self) -> bool {
        matches!(self.scope, AliasScope::Quote)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let a = Alias::new(AliasScope::Quote, "7f3a9c21", "msg.loadlinker.com");
        let addr = a.address();
        assert_eq!(addr, "q-7f3a9c21@msg.loadlinker.com");
        assert_eq!(Alias::parse(&addr), Some(a));
    }

    #[test]
    fn scopes_parse() {
        for (addr, scope) in [
            ("q-abc@msg.loadlinker.com", AliasScope::Quote),
            ("c-def@msg.loadlinker.com", AliasScope::Consignment),
            ("org-ghi@msg.loadlinker.com", AliasScope::Organisation),
        ] {
            assert_eq!(Alias::parse(addr).map(|a| a.scope), Some(scope));
        }
    }

    #[test]
    fn rejects_malformed() {
        for bad in ["notanalias", "x-abc@d.com", "q-@d.com", "q-abc"] {
            assert!(Alias::parse(bad).is_none(), "should reject {bad}");
        }
    }

    #[test]
    fn quote_aliases_are_masked() {
        assert!(Alias::new(AliasScope::Quote, "t", "d").is_masked());
        assert!(!Alias::new(AliasScope::Consignment, "t", "d").is_masked());
    }
}
