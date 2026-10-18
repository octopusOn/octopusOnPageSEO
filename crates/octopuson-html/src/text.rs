//! Visible text statistics and fingerprints for duplicate detection.

use xxhash_rust::xxh3::xxh3_64;

/// Words that make up one shingle of the near-duplicate fingerprint.
const SHINGLE: usize = 3;

/// Word count, exact-duplicate hash and near-duplicate fingerprint of a page's visible text.
pub(crate) fn text_stats(text: &str) -> (u32, u64, u64) {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect();
    let count = u32::try_from(words.len()).unwrap_or(u32::MAX);
    let normalized = words.join(" ");
    let exact = if words.is_empty() {
        0
    } else {
        xxh3_64(normalized.as_bytes())
    };
    (count, exact, simhash(&words))
}

/// 64-bit SimHash of word shingles: pages whose fingerprints differ in few bits (see
/// [`hamming_distance`]) share most of their text.
pub fn simhash<S: AsRef<str>>(words: &[S]) -> u64 {
    let mut weights = [0i32; 64];
    let mut add = |hash: u64| {
        for (bit, weight) in weights.iter_mut().enumerate() {
            *weight += if hash >> bit & 1 == 1 { 1 } else { -1 };
        }
    };
    if words.len() < SHINGLE {
        for word in words {
            add(xxh3_64(word.as_ref().as_bytes()));
        }
    } else {
        for window in words.windows(SHINGLE) {
            let shingle = window
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<_>>()
                .join(" ");
            add(xxh3_64(shingle.as_bytes()));
        }
    }
    if words.is_empty() {
        return 0;
    }
    weights
        .iter()
        .enumerate()
        .filter(|(_, weight)| **weight > 0)
        .fold(0u64, |hash, (bit, _)| hash | 1 << bit)
}

/// Number of differing bits between two SimHash fingerprints (0 = same text, ≤ 3 is a near
/// duplicate for pages of a few hundred words).
pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_words_and_ignores_punctuation() {
        let (count, exact, _) = text_stats("Hello,  world — 2026 !");
        assert_eq!(count, 3);
        assert_eq!(exact, text_stats("hello world 2026").1);
        assert_eq!(text_stats("   ").0, 0);
    }

    #[test]
    fn near_duplicates_have_close_fingerprints() {
        let base = "the quick brown fox jumps over the lazy dog near the river bank on a sunny afternoon while birds sing in the tall green trees and children play";
        let changed = base.replace("sunny", "cloudy");
        let other = "completely different text about server logs crawl budget and search engine bots verifying their addresses with reverse dns lookups every single day of the week";
        let (_, _, a) = text_stats(base);
        let (_, _, b) = text_stats(&changed);
        let (_, _, c) = text_stats(other);
        assert!(hamming_distance(a, b) < hamming_distance(a, c));
        assert!(hamming_distance(a, b) <= 12);
        assert_eq!(hamming_distance(a, a), 0);
    }
}
