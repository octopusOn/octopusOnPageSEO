//! Discovery queries derived from a seed: what to type into an autocomplete box to see what
//! people search around it.

use seonexus_text::Language;
use serde::{Deserialize, Serialize};

/// A way to widen a seed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Expansion {
    /// The seed followed by each letter a–z (`kahve a`, `kahve b`, …).
    Alphabet,
    /// The seed followed by the letters of the Turkish alphabet outside a–z (`ç ğ ı ö ş ü`).
    TurkishLetters,
    /// Question words (`kahve nasıl`, `kahve nedir`, `how kahve`).
    Questions,
    /// Buying words (`kahve fiyat`, `en iyi kahve`).
    Commercial,
}

impl Expansion {
    /// Every expansion, in a stable order.
    pub const ALL: [Self; 4] = [
        Self::Alphabet,
        Self::TurkishLetters,
        Self::Questions,
        Self::Commercial,
    ];
}

const TURKISH_QUESTIONS: &[&str] = &[
    "{} nedir",
    "{} nasıl",
    "{} ne işe yarar",
    "{} neden",
    "{} ne zaman",
    "{} nerede",
    "{} hangi",
    "{} kaç",
    "{} mi",
    "{} zararları",
    "{} faydaları",
];
const ENGLISH_QUESTIONS: &[&str] = &[
    "what is {}",
    "how to {}",
    "why {}",
    "when {}",
    "where {}",
    "which {}",
    "{} vs",
    "is {}",
];
const TURKISH_COMMERCIAL: &[&str] = &[
    "{} fiyat",
    "{} fiyatları",
    "en iyi {}",
    "ucuz {}",
    "{} yorum",
    "{} indirim",
    "{} satın al",
    "{} önerisi",
    "{} markaları",
    "{} karşılaştırma",
];
const ENGLISH_COMMERCIAL: &[&str] = &[
    "best {}",
    "{} price",
    "cheap {}",
    "{} review",
    "buy {}",
    "{} deals",
    "{} alternatives",
    "{} vs",
];

/// Queries that widen `seed`, without duplicates and without the seed itself.
pub fn expand(seed: &str, language: Language, expansions: &[Expansion]) -> Vec<String> {
    let seed = seed.split_whitespace().collect::<Vec<_>>().join(" ");
    if seed.is_empty() {
        return Vec::new();
    }
    let mut queries: Vec<String> = Vec::new();
    let mut push = |query: String| {
        if query != seed && !queries.contains(&query) {
            queries.push(query);
        }
    };
    let turkish = language == Language::Turkish;
    for expansion in expansions {
        match expansion {
            Expansion::Alphabet => {
                for letter in 'a'..='z' {
                    push(format!("{seed} {letter}"));
                }
            }
            Expansion::TurkishLetters => {
                for letter in ['ç', 'ğ', 'ı', 'ö', 'ş', 'ü'] {
                    push(format!("{seed} {letter}"));
                }
            }
            Expansion::Questions => {
                for pattern in if turkish {
                    TURKISH_QUESTIONS
                } else {
                    ENGLISH_QUESTIONS
                } {
                    push(pattern.replace("{}", &seed));
                }
            }
            Expansion::Commercial => {
                for pattern in if turkish {
                    TURKISH_COMMERCIAL
                } else {
                    ENGLISH_COMMERCIAL
                } {
                    push(pattern.replace("{}", &seed));
                }
            }
        }
    }
    queries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widens_a_seed_without_repeats() {
        let queries = expand(
            " türk  kahvesi ",
            Language::Turkish,
            &[
                Expansion::Alphabet,
                Expansion::TurkishLetters,
                Expansion::Questions,
            ],
        );
        assert_eq!(queries[0], "türk kahvesi a");
        assert_eq!(queries.len(), 26 + 6 + TURKISH_QUESTIONS.len());
        assert!(queries.contains(&"türk kahvesi ş".to_owned()));
        assert!(queries.contains(&"türk kahvesi nasıl".to_owned()));
        let english = expand("coffee", Language::English, &[Expansion::Commercial]);
        assert!(english.contains(&"best coffee".to_owned()));
        // `{} vs` appears in both English lists; it is asked once.
        let both = expand(
            "coffee",
            Language::English,
            &[Expansion::Questions, Expansion::Commercial],
        );
        assert_eq!(both.iter().filter(|query| *query == "coffee vs").count(), 1);
        assert_eq!(expand("  ", Language::Turkish, &Expansion::ALL).len(), 0);
    }
}
