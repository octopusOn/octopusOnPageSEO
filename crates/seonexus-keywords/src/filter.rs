//! Which keywords to keep.

use regex::{Regex, RegexBuilder};
use seonexus_text::{Language, fold, lower};
use serde::{Deserialize, Serialize};

/// Why a filter cannot be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FilterError {
    /// A regular expression does not compile.
    #[error("invalid regular expression `{pattern}`: {message}")]
    Regex {
        /// The expression.
        pattern: String,
        /// The compiler's message.
        message: String,
    },
}

/// What a filter asks for, as stored and edited.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FilterSpec {
    /// Keep keywords containing any of these words or phrases (none: keep all).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    /// Drop keywords containing any of these words or phrases (brands, other products).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
    /// Keep keywords matching this expression.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regex: Option<String>,
    /// Drop keywords matching this expression.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_regex: Option<String>,
    /// Lowest monthly volume kept; keywords without a volume pass.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_volume: Option<u64>,
    /// Highest monthly volume kept.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_volume: Option<u64>,
    /// Most words in a keyword.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_words: Option<usize>,
}

/// A compiled filter. Words and phrases match whole words, ignoring case and Turkish
/// diacritics (`kahve` matches `Kahve` and `kahveci` does not).
#[derive(Debug, Clone)]
pub struct Filter {
    include: Vec<Vec<String>>,
    exclude: Vec<Vec<String>>,
    regex: Option<Regex>,
    exclude_regex: Option<Regex>,
    min_volume: Option<u64>,
    max_volume: Option<u64>,
    max_words: Option<usize>,
}

fn phrase_words(phrase: &str) -> Vec<String> {
    fold(&lower(phrase, Language::Turkish))
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

fn contains(words: &[String], phrase: &[String]) -> bool {
    !phrase.is_empty() && words.windows(phrase.len()).any(|window| window == phrase)
}

fn compile(pattern: Option<&String>) -> Result<Option<Regex>, FilterError> {
    pattern
        .filter(|pattern| !pattern.trim().is_empty())
        .map(|pattern| {
            RegexBuilder::new(pattern)
                .case_insensitive(true)
                .size_limit(1 << 20)
                .build()
                .map_err(|error| FilterError::Regex {
                    pattern: pattern.clone(),
                    message: error.to_string(),
                })
        })
        .transpose()
}

impl Filter {
    /// Compiles a filter.
    ///
    /// # Errors
    /// [`FilterError::Regex`] for an invalid expression.
    pub fn new(spec: &FilterSpec) -> Result<Self, FilterError> {
        let phrases = |list: &[String]| {
            list.iter()
                .map(|phrase| phrase_words(phrase))
                .filter(|words| !words.is_empty())
                .collect()
        };
        Ok(Self {
            include: phrases(&spec.include),
            exclude: phrases(&spec.exclude),
            regex: compile(spec.regex.as_ref())?,
            exclude_regex: compile(spec.exclude_regex.as_ref())?,
            min_volume: spec.min_volume,
            max_volume: spec.max_volume,
            max_words: spec.max_words,
        })
    }

    /// Whether a keyword passes.
    pub fn keeps(&self, keyword: &str, volume: Option<u64>) -> bool {
        let words = phrase_words(keyword);
        if !self.include.is_empty() && !self.include.iter().any(|phrase| contains(&words, phrase)) {
            return false;
        }
        if self.exclude.iter().any(|phrase| contains(&words, phrase)) {
            return false;
        }
        if self
            .regex
            .as_ref()
            .is_some_and(|regex| !regex.is_match(keyword))
        {
            return false;
        }
        if self
            .exclude_regex
            .as_ref()
            .is_some_and(|regex| regex.is_match(keyword))
        {
            return false;
        }
        if let Some(volume) = volume
            && (self.min_volume.is_some_and(|min| volume < min)
                || self.max_volume.is_some_and(|max| volume > max))
        {
            return false;
        }
        self.max_words.is_none_or(|max| words.len() <= max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(spec: &FilterSpec) -> Filter {
        Filter::new(spec).unwrap()
    }

    #[test]
    fn matches_whole_words_ignoring_case_and_diacritics() {
        let only_coffee = filter(&FilterSpec {
            include: vec!["Kahve".to_owned(), "türk çayı".to_owned()],
            exclude: vec!["nescafe".to_owned()],
            ..FilterSpec::default()
        });
        assert!(only_coffee.keeps("TÜRK KAHVE fincanı", None));
        assert!(only_coffee.keeps("turk cayi demleme", None));
        assert!(!only_coffee.keeps("kahveci", None));
        assert!(!only_coffee.keeps("nescafe kahve", None));
    }

    #[test]
    fn applies_volumes_word_counts_and_expressions() {
        let filter = filter(&FilterSpec {
            regex: Some("^kahve".to_owned()),
            exclude_regex: Some(r"\d{4}".to_owned()),
            min_volume: Some(100),
            max_volume: Some(10_000),
            max_words: Some(3),
            ..FilterSpec::default()
        });
        assert!(filter.keeps("kahve makinesi", Some(5_000)));
        assert!(filter.keeps("kahve makinesi", None));
        assert!(!filter.keeps("kahve makinesi", Some(50)));
        assert!(!filter.keeps("kahve makinesi", Some(50_000)));
        assert!(!filter.keeps("filtre kahve", Some(500)));
        assert!(!filter.keeps("kahve makinesi 2026", Some(500)));
        assert!(!filter.keeps("kahve makinesi en ucuz fiyat", Some(500)));
        assert!(matches!(
            Filter::new(&FilterSpec {
                regex: Some("(".to_owned()),
                ..FilterSpec::default()
            }),
            Err(FilterError::Regex { .. })
        ));
    }
}
