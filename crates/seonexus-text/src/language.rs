//! Languages the analysis knows, and guessing a text's language.

use serde::{Deserialize, Serialize};

use crate::stopwords::is_stopword;

/// A language with stop words and a stemmer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Turkish.
    Turkish,
    /// English.
    English,
    /// German.
    German,
    /// Russian.
    Russian,
    /// Anything else: words are lowercased and counted, nothing is removed or stemmed.
    Other,
}

impl Language {
    /// Languages with stop words, in guessing order.
    pub const KNOWN: [Self; 4] = [Self::Turkish, Self::English, Self::German, Self::Russian];

    /// The language of an ISO 639-1 code (`tr`, `en`, ...); [`Language::Other`] otherwise.
    pub fn from_code(code: &str) -> Self {
        match code.trim().to_ascii_lowercase().as_str() {
            "tr" | "az" => Self::Turkish,
            "en" => Self::English,
            "de" => Self::German,
            "ru" | "uk" | "be" | "kk" => Self::Russian,
            _ => Self::Other,
        }
    }

    /// ISO 639-1 code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Turkish => "tr",
            Self::English => "en",
            Self::German => "de",
            Self::Russian => "ru",
            Self::Other => "und",
        }
    }

    /// Whether `I`/`İ` lowercase to `ı`/`i`.
    pub(crate) const fn dotted_i(self) -> bool {
        matches!(self, Self::Turkish)
    }

    /// Whether a suffix after an apostrophe belongs to the word before it (`Ankara'da`).
    pub(crate) const fn drops_apostrophe_suffix(self) -> bool {
        matches!(self, Self::Turkish)
    }

    pub(crate) fn stemmer(self) -> Option<rust_stemmers::Stemmer> {
        let algorithm = match self {
            Self::Turkish => rust_stemmers::Algorithm::Turkish,
            Self::English => rust_stemmers::Algorithm::English,
            Self::German => rust_stemmers::Algorithm::German,
            Self::Russian => rust_stemmers::Algorithm::Russian,
            Self::Other => return None,
        };
        Some(rust_stemmers::Stemmer::create(algorithm))
    }
}

/// The likeliest language of a text: the known language whose stop words make up the largest
/// share of its words (at least 5 %), Russian for mostly Cyrillic text, else `fallback`.
pub fn guess_language(text: &str, fallback: Language) -> Language {
    let letters = text.chars().filter(|ch| ch.is_alphabetic()).count();
    if letters == 0 {
        return fallback;
    }
    let cyrillic = text
        .chars()
        .filter(|ch| ('\u{400}'..='\u{4ff}').contains(ch))
        .count();
    if cyrillic * 2 > letters {
        return Language::Russian;
    }
    let mut best = (fallback, 0usize);
    let words: Vec<String> = text
        .split(|ch: char| !ch.is_alphabetic())
        .filter(|word| !word.is_empty())
        .take(2_000)
        .map(str::to_lowercase)
        .collect();
    if words.is_empty() {
        return fallback;
    }
    let turkish_letters = text
        .chars()
        .filter(|ch| matches!(ch, 'ı' | 'ğ' | 'ş' | 'İ' | 'Ğ' | 'Ş'))
        .count();
    for language in Language::KNOWN {
        let mut hits = words
            .iter()
            .filter(|word| is_stopword(word, language))
            .count();
        if language == Language::Turkish {
            // Letters only Turkish uses are strong evidence.
            hits += turkish_letters / 2;
        }
        if hits > best.1 {
            best = (language, hits);
        }
    }
    if best.1 * 20 < words.len() {
        return fallback;
    }
    best.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_codes() {
        assert_eq!(Language::from_code("TR"), Language::Turkish);
        assert_eq!(Language::from_code("ja"), Language::Other);
        assert_eq!(Language::Turkish.code(), "tr");
    }

    #[test]
    fn guesses_languages() {
        let turkish = "Bu yazıda türk kahvesinin nasıl yapıldığını ve neden bu kadar sevildiğini anlatıyoruz.";
        assert_eq!(guess_language(turkish, Language::Other), Language::Turkish);
        let english =
            "This guide explains how to brew the coffee and why it is so popular with the people.";
        assert_eq!(guess_language(english, Language::Other), Language::English);
        assert_eq!(
            guess_language("Как приготовить кофе по-турецки", Language::Other),
            Language::Russian
        );
        let german = "Der Kaffee wird mit dem Wasser und dem Zucker in einer Kanne gekocht.";
        assert_eq!(guess_language(german, Language::Other), Language::German);
        assert_eq!(
            guess_language("12345 !!!", Language::English),
            Language::English
        );
    }
}
