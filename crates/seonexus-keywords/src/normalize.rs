//! Keyword identities.
//!
//! People and tools spell one search many ways: `Türk Kahvesi`, `türk kahvesi `, `turk
//! kahvesi`, `türk-kahvesi`. [`normalize`] gives each keyword three forms: how it is shown,
//! the identity that merges such variants (case, whitespace, diacritics and dashes), and the
//! stems that grouping compares.

use seonexus_text::{Analyzer, Language, fold, lower};
use serde::{Deserialize, Serialize};

/// Longest keyword kept, in characters (volume sources reject longer ones).
pub const MAX_CHARS: usize = 120;

/// A keyword's forms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Normalized {
    /// As shown: trimmed, inner whitespace collapsed, lowercase in the keyword's language.
    pub text: String,
    /// Identity: variants with other case, diacritics, dashes or spacing share it.
    pub key: String,
    /// Stems of every word, stop words folded (`turk kahve nasil yap`).
    pub stems: Vec<String>,
}

/// Forms of a keyword; `None` for text that is not a keyword (empty, only punctuation, or
/// longer than [`MAX_CHARS`]).
pub fn normalize(raw: &str, language: Language) -> Option<Normalized> {
    let trimmed = raw
        .trim()
        .trim_matches(|ch: char| ch == '"' || ch == '\'' || ch == '“' || ch == '”');
    let text = lower(
        &trimmed.split_whitespace().collect::<Vec<_>>().join(" "),
        language,
    );
    if text.is_empty() || text.chars().count() > MAX_CHARS {
        return None;
    }
    let key = identity(&text);
    if key.is_empty() {
        return None;
    }
    let stems = Analyzer::new(language).all_stems(&text);
    Some(Normalized { text, key, stems })
}

/// Folded words joined by single spaces, punctuation other than inner apostrophes,
/// ampersands and dots dropped.
fn identity(lowered: &str) -> String {
    let folded = fold(lowered);
    folded
        .split(|ch: char| !(ch.is_alphanumeric() || ch == '\'' || ch == '&' || ch == '.'))
        .map(|word| word.trim_matches(|ch: char| ch == '\'' || ch == '.'))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(text: &str) -> String {
        normalize(text, Language::Turkish).unwrap().key
    }

    #[test]
    fn merges_spelling_variants() {
        let variants = [
            "Türk Kahvesi",
            " türk   kahvesi ",
            "turk kahvesi",
            "TÜRK KAHVESİ",
            "türk-kahvesi",
            "\"türk kahvesi\"",
        ];
        for variant in variants {
            assert_eq!(key(variant), "turk kahvesi", "{variant}");
        }
        assert_eq!(
            normalize("TÜRK KAHVESİ", Language::Turkish).unwrap().text,
            "türk kahvesi"
        );
        // Different searches stay apart.
        assert_ne!(key("kahve makinesi"), key("kahve makineleri"));
        assert_eq!(key("node.js nedir"), "node.js nedir");
        assert_eq!(key("h&m indirim"), "h&m indirim");
    }

    #[test]
    fn stems_keep_intent_words_and_drop_suffixes() {
        let stems = |text| normalize(text, Language::Turkish).unwrap().stems;
        let question = stems("Türk kahvesi nasıl yapılır");
        assert_eq!(question.len(), 4);
        assert_eq!(question[2], "nasil");
        // Inflections of one word share its stem.
        assert_eq!(stems("kahvesi")[0], stems("kahveler")[0]);
        assert_eq!(stems("kahvesinin")[0], stems("kahvesi")[0]);
        assert_eq!(stems("makineleri")[0], stems("makinesi")[0]);
    }

    #[test]
    fn rejects_non_keywords() {
        for text in ["", "   ", "--", &"a".repeat(MAX_CHARS + 1)] {
            assert!(normalize(text, Language::Turkish).is_none(), "{text}");
        }
    }
}
