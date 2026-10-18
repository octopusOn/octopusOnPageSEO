//! Pages an engine shows instead of results: consent dialogs, CAPTCHAs and blocks.
//!
//! Detection only tells the caller to stop and hand the page to the person using the
//! program; nothing here answers a challenge.

use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

use crate::SerpError;

/// What kind of interruption a page is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChallengeKind {
    /// A cookie or terms consent page.
    Consent,
    /// A CAPTCHA or "unusual traffic" check.
    Captcha,
    /// Access refused outright.
    Blocked,
}

/// A detected interruption.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Challenge {
    /// Kind of interruption.
    pub kind: ChallengeKind,
    /// Which rule matched, for the log.
    pub reason: String,
}

/// One way to recognise an interruption. Every condition that is set must hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChallengeRule {
    /// Kind reported when the rule matches.
    pub kind: ChallengeKind,
    /// Substring of the final page URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_contains: Option<String>,
    /// CSS selector that matches an element of the page (HTML pages only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    /// Substring of the response body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_contains: Option<String>,
}

/// Rules ready to run.
#[derive(Debug, Clone, Default)]
pub struct ChallengeRules {
    rules: Vec<(ChallengeRule, Option<Selector>)>,
}

impl ChallengeRules {
    /// Compiles rules.
    ///
    /// # Errors
    /// [`SerpError::InvalidSpec`] for a rule without conditions or with an invalid selector.
    pub fn compile(rules: &[ChallengeRule]) -> Result<Self, SerpError> {
        let mut compiled = Vec::with_capacity(rules.len());
        for rule in rules {
            if rule.url_contains.is_none()
                && rule.selector.is_none()
                && rule.text_contains.is_none()
            {
                return Err(SerpError::InvalidSpec(
                    "a challenge rule needs url_contains, selector or text_contains".to_owned(),
                ));
            }
            let selector = rule
                .selector
                .as_deref()
                .map(|selector| {
                    Selector::parse(selector).map_err(|error| {
                        SerpError::InvalidSpec(format!(
                            "invalid challenge selector `{selector}`: {error}"
                        ))
                    })
                })
                .transpose()?;
            compiled.push((rule.clone(), selector));
        }
        Ok(Self { rules: compiled })
    }

    /// Whether there are no rules.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// The first rule that matches a page, if any.
    ///
    /// `document` is the parsed body when the response is HTML.
    pub fn detect(&self, url: &str, body: &str, document: Option<&Html>) -> Option<Challenge> {
        self.rules.iter().find_map(|(rule, selector)| {
            let url_ok = rule
                .url_contains
                .as_deref()
                .is_none_or(|needle| url.contains(needle));
            let text_ok = rule
                .text_contains
                .as_deref()
                .is_none_or(|needle| body.contains(needle));
            let selector_ok = selector.as_ref().is_none_or(|selector| {
                document.is_some_and(|document| document.select(selector).next().is_some())
            });
            (url_ok && text_ok && selector_ok).then(|| Challenge {
                kind: rule.kind,
                reason: describe(rule),
            })
        })
    }
}

fn describe(rule: &ChallengeRule) -> String {
    let mut parts = Vec::new();
    if let Some(url) = &rule.url_contains {
        parts.push(format!("URL contains `{url}`"));
    }
    if let Some(selector) = &rule.selector {
        parts.push(format!("page has `{selector}`"));
    }
    if let Some(text) = &rule.text_contains {
        parts.push(format!("text contains `{text}`"));
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(kind: ChallengeKind) -> ChallengeRule {
        ChallengeRule {
            kind,
            url_contains: None,
            selector: None,
            text_contains: None,
        }
    }

    #[test]
    fn matches_every_condition_of_a_rule() {
        let rules = ChallengeRules::compile(&[
            ChallengeRule {
                url_contains: Some("/sorry/".to_owned()),
                ..rule(ChallengeKind::Captcha)
            },
            ChallengeRule {
                selector: Some("form#consent".to_owned()),
                text_contains: Some("cookies".to_owned()),
                ..rule(ChallengeKind::Consent)
            },
        ])
        .unwrap();
        let sorry = rules.detect("https://www.google.com/sorry/index", "", None);
        assert_eq!(
            sorry.map(|challenge| challenge.kind),
            Some(ChallengeKind::Captcha)
        );

        let html = "<form id=consent>We use cookies</form>";
        let document = Html::parse_document(html);
        let consent = rules.detect("https://example.com/", html, Some(&document));
        assert_eq!(
            consent.map(|challenge| challenge.kind),
            Some(ChallengeKind::Consent)
        );

        let other = Html::parse_document("<form id=consent>Hello</form>");
        assert!(
            rules
                .detect("https://example.com/", "Hello", Some(&other))
                .is_none()
        );
    }

    #[test]
    fn rejects_empty_and_invalid_rules() {
        assert!(ChallengeRules::compile(&[rule(ChallengeKind::Blocked)]).is_err());
        assert!(
            ChallengeRules::compile(&[ChallengeRule {
                selector: Some("div[".to_owned()),
                ..rule(ChallengeKind::Blocked)
            }])
            .is_err()
        );
    }
}
