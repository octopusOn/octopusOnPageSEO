//! Reading ease: Ateşman for Turkish, Flesch for English.

use serde::{Deserialize, Serialize};

use crate::Language;

/// A reading-ease score; higher is easier (0–100 for ordinary text).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Readability {
    /// The score.
    pub score: f64,
    /// Words counted.
    pub words: u32,
    /// Sentences counted.
    pub sentences: u32,
    /// Syllables counted.
    pub syllables: u32,
}

/// Reading ease of a text: Ateşman (1997) for Turkish, Flesch (1948) for English; `None` for
/// other languages or texts under 30 words.
#[allow(clippy::cast_precision_loss)] // counts of one page
pub fn readability(text: &str, language: Language) -> Option<Readability> {
    let words: Vec<&str> = text
        .split(|ch: char| !ch.is_alphanumeric() && ch != '\'' && ch != '’')
        .filter(|word| word.chars().any(char::is_alphabetic))
        .collect();
    if words.len() < 30 || !matches!(language, Language::Turkish | Language::English) {
        return None;
    }
    let sentences = count_sentences(text).max(1);
    let syllables: u32 = words
        .iter()
        .map(|word| match language {
            Language::Turkish => turkish_syllables(word),
            _ => english_syllables(word),
        })
        .sum();
    let word_count = u32::try_from(words.len()).unwrap_or(u32::MAX);
    let per_word = f64::from(syllables) / f64::from(word_count);
    let per_sentence = f64::from(word_count) / f64::from(sentences);
    let score = match language {
        Language::Turkish => 198.825 - 40.175 * per_word - 2.610 * per_sentence,
        _ => 206.835 - 1.015 * per_sentence - 84.6 * per_word,
    };
    Some(Readability {
        score: score.clamp(0.0, 100.0),
        words: word_count,
        sentences,
        syllables,
    })
}

fn count_sentences(text: &str) -> u32 {
    let mut sentences = 0;
    let mut in_sentence = false;
    for ch in text.chars() {
        if matches!(ch, '.' | '!' | '?' | '…') {
            if in_sentence {
                sentences += 1;
            }
            in_sentence = false;
        } else if ch.is_alphanumeric() {
            in_sentence = true;
        }
    }
    sentences + u32::from(in_sentence)
}

/// Turkish syllables are its vowels.
fn turkish_syllables(word: &str) -> u32 {
    let count = word
        .chars()
        .filter(|ch| "aeıioöuüâîûAEIİOÖUÜÂÎÛ".contains(*ch))
        .count();
    u32::try_from(count.max(1)).unwrap_or(u32::MAX)
}

/// Vowel groups, less a silent final `e`.
fn english_syllables(word: &str) -> u32 {
    let lower = word.to_ascii_lowercase();
    let mut groups = 0u32;
    let mut previous_vowel = false;
    for ch in lower.chars() {
        let vowel = "aeiouy".contains(ch);
        if vowel && !previous_vowel {
            groups += 1;
        }
        previous_vowel = vowel;
    }
    let silent_e = lower.ends_with('e')
        && !lower.ends_with("le")
        && !lower.ends_with("ee")
        && !lower.ends_with("ye");
    if silent_e && groups > 1 {
        groups -= 1;
    }
    groups.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_simple_text_as_easy() {
        let turkish = "Kahve güzel. Su sıcak. Çay da var. ".repeat(6);
        let score = readability(&turkish, Language::Turkish).unwrap();
        assert_eq!(score.sentences, 18);
        assert!(score.score > 80.0, "{score:?}");
        let english = "The cat sat. The dog ran. We had tea. ".repeat(6);
        assert!(readability(&english, Language::English).unwrap().score > 90.0);
        assert!(readability("kısa metin", Language::Turkish).is_none());
        assert!(readability(&english, Language::German).is_none());
    }

    #[test]
    fn counts_syllables() {
        assert_eq!(turkish_syllables("kahvesi"), 3);
        assert_eq!(english_syllables("coffee"), 2);
        assert_eq!(english_syllables("table"), 2);
    }
}
