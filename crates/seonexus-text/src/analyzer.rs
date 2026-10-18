//! Terms of texts and what sets of pages have in common.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::case::{fold, tokens};
use crate::{Language, is_stopword};

/// Longest n-gram counted.
const MAX_N: usize = 3;

/// A term: one to three words, identified by their folded stems.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Term {
    /// Folded stems joined by spaces (`turk kahve`), what terms are compared by.
    pub key: String,
    /// Number of words.
    pub n: u8,
}

/// Turns text into comparable words.
pub struct Analyzer {
    language: Language,
    stemmer: Option<rust_stemmers::Stemmer>,
}

impl std::fmt::Debug for Analyzer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Analyzer")
            .field("language", &self.language)
            .finish_non_exhaustive()
    }
}

/// A word of a text: its form as written and its folded stem.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Word {
    surface: String,
    stem: String,
    stop: bool,
}

impl Analyzer {
    /// An analyzer for a language.
    pub fn new(language: Language) -> Self {
        Self {
            language,
            // Snowball's Turkish stemmer is inconsistent across inflections (`kahvesi` →
            // `kahves`, `kahvesinin` → `kahve`); Turkish uses [`turkish_key`] instead.
            stemmer: (language != Language::Turkish)
                .then(|| language.stemmer())
                .flatten(),
        }
    }

    /// The language.
    pub fn language(&self) -> Language {
        self.language
    }

    fn words(&self, text: &str) -> Vec<Word> {
        tokens(text, self.language)
            .into_iter()
            .filter(|word| word.chars().count() > 1 || word.chars().all(|ch| ch.is_ascii_digit()))
            .map(|surface| {
                let stop = is_stopword(&surface, self.language);
                let stem = if stop {
                    fold(&surface)
                } else if self.language == Language::Turkish {
                    turkish_key(&fold(&surface))
                } else if let Some(stemmer) = &self.stemmer {
                    fold(&stemmer.stem(&surface))
                } else {
                    fold(&surface)
                };
                Word {
                    surface,
                    stem,
                    stop,
                }
            })
            .collect()
    }

    /// Folded stems of a phrase's content words, in order (`Türk Kahvesi` → `turk kahve`).
    pub fn key(&self, phrase: &str) -> String {
        self.words(phrase)
            .into_iter()
            .filter(|word| !word.stop)
            .map(|word| word.stem)
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Stems of every word of a phrase, in order: content words stemmed, stop words folded
    /// (`Türk kahvesi nasıl yapılır` → `turk kahve nasil yap`). Keyword grouping keeps the
    /// stop words because they carry intent (`nasıl`, `fiyat`).
    pub fn all_stems(&self, phrase: &str) -> Vec<String> {
        self.words(phrase)
            .into_iter()
            .map(|word| word.stem)
            .collect()
    }

    /// Counts the terms of a text.
    pub fn analyze(&self, text: &str) -> DocTerms {
        let words = self.words(text);
        let mut counts: HashMap<Term, (u32, HashMap<String, u32>)> = HashMap::new();
        for start in 0..words.len() {
            for n in 1..=MAX_N {
                let Some(window) = words.get(start..start + n) else {
                    break;
                };
                // N-grams may contain stop words inside ("kahve ve çay") but not start or end with
                // one, and never cross more than one.
                if window[0].stop
                    || window[n - 1].stop
                    || window.iter().filter(|word| word.stop).count() > 1
                {
                    continue;
                }
                let term = Term {
                    key: window
                        .iter()
                        .filter(|word| !word.stop)
                        .map(|word| word.stem.as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                    n: u8::try_from(n).unwrap_or(u8::MAX),
                };
                let surface = window
                    .iter()
                    .map(|word| word.surface.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                let entry = counts.entry(term).or_default();
                entry.0 += 1;
                *entry.1.entry(surface).or_default() += 1;
            }
        }
        DocTerms {
            words: u32::try_from(words.len()).unwrap_or(u32::MAX),
            content_words: u32::try_from(words.iter().filter(|word| !word.stop).count())
                .unwrap_or(u32::MAX),
            stems: words
                .iter()
                .filter(|word| !word.stop)
                .map(|word| word.stem.clone())
                .collect(),
            counts: counts
                .into_iter()
                .map(|(term, (count, surfaces))| {
                    let surface = surfaces
                        .into_iter()
                        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
                        .map(|(surface, _)| surface)
                        .unwrap_or_default();
                    (term, (count, surface))
                })
                .collect(),
        }
    }

    /// Whether a text contains a phrase, word by word, comparing folded stems and ignoring
    /// stop words (`Türk kahvesi nasıl yapılır` contains `turk kahve`).
    pub fn contains(&self, text: &str, phrase: &str) -> bool {
        let wanted: Vec<String> = self.key(phrase).split(' ').map(str::to_owned).collect();
        if wanted.iter().all(String::is_empty) {
            return false;
        }
        let stems = self.analyze(text).stems;
        stems
            .windows(wanted.len())
            .any(|window| window == wanted.as_slice())
    }

    /// Share of a phrase's content words that occur in a text, in any order.
    #[allow(clippy::cast_precision_loss)] // word counts
    pub fn coverage(&self, text: &str, phrase: &str) -> f64 {
        let key = self.key(phrase);
        let wanted: HashSet<&str> = key.split(' ').filter(|stem| !stem.is_empty()).collect();
        if wanted.is_empty() {
            return 0.0;
        }
        let stems: HashSet<String> = self.analyze(text).stems.into_iter().collect();
        wanted.iter().filter(|stem| stems.contains(**stem)).count() as f64 / wanted.len() as f64
    }
}

/// Inflectional endings of folded Turkish words, longest first. Endings starting with the
/// buffer consonants `s`, `y` and `n` are only removed after a vowel (`kahve-si`, but `ders-i`).
const TURKISH_ENDINGS: &[&str] = &[
    "lerinden", "larindan", "lerinde", "larinda", "lerine", "larina", "lerini", "larini", "sinden",
    "sindan", "sunden", "sundan", "lerin", "larin", "sinde", "sinda", "sunde", "sunda", "sinin",
    "sunun", "inden", "indan", "unden", "undan", "imiz", "umuz", "iniz", "unuz", "leri", "lari",
    "sine", "sina", "sini", "suna", "sunu", "inde", "inda", "unda", "unde", "inin", "unun", "nden",
    "ndan", "ler", "lar", "nin", "nun", "nde", "nda", "den", "dan", "ten", "tan", "yle", "yla",
    "ine", "ina", "ini", "una", "unu", "de", "da", "te", "ta", "ye", "ya", "yi", "yu", "si", "su",
    "in", "un", "im", "um", "e", "a", "i", "u",
];

/// A key all inflections of a Turkish word share: endings removed (at most three, keeping
/// at least two letters) and a final softened consonant hardened (`kitabı` → `kitap`,
/// `rengi` → `renk`). It is for matching, not a dictionary stem.
pub fn turkish_key(folded: &str) -> String {
    let mut word: Vec<char> = folded.chars().collect();
    let is_vowel = |ch: char| matches!(ch, 'a' | 'e' | 'i' | 'o' | 'u');
    for _ in 0..3 {
        let stripped = TURKISH_ENDINGS.iter().find_map(|ending| {
            let ending: Vec<char> = ending.chars().collect();
            let keep = word.len().checked_sub(ending.len())?;
            if keep < 2 || word[keep..] != ending[..] {
                return None;
            }
            if matches!(ending[0], 's' | 'y' | 'n') && !is_vowel(word[keep - 1]) {
                return None;
            }
            Some(keep)
        });
        match stripped {
            Some(keep) => word.truncate(keep),
            None => break,
        }
    }
    if let Some(last) = word.last_mut() {
        *last = match *last {
            'b' => 'p',
            'd' => 't',
            'g' => 'k',
            other => other,
        };
    }
    word.into_iter().collect()
}

/// Term counts of one text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocTerms {
    /// Words.
    pub words: u32,
    /// Words that are not stop words.
    pub content_words: u32,
    /// Folded stems of the content words, in order.
    pub stems: Vec<String>,
    /// Count and most frequent written form of each term.
    pub counts: HashMap<Term, (u32, String)>,
}

impl DocTerms {
    /// How often a term occurs.
    pub fn count(&self, key: &str) -> u32 {
        self.counts
            .iter()
            .find(|(term, _)| term.key == key)
            .map_or(0, |(_, (count, _))| *count)
    }

    /// Whether a term occurs.
    pub fn has(&self, key: &str) -> bool {
        self.counts.keys().any(|term| term.key == key)
    }
}

/// A term many pages use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsensusTerm {
    /// The term.
    pub term: Term,
    /// Most frequent written form.
    pub surface: String,
    /// Pages using it.
    pub pages: u32,
    /// Share of the pages using it.
    pub share: f64,
    /// Average occurrences on the pages that use it.
    pub average: f64,
}

/// Terms used by at least `min_share` of the documents (and at least two of them), most
/// widely used first. Single words shorter than three letters and numbers alone are left out;
/// a term contained in a longer consensus term used by as many pages is dropped in its favour.
#[allow(clippy::cast_precision_loss)] // page and term counts
pub fn consensus(documents: &[&DocTerms], min_share: f64, limit: usize) -> Vec<ConsensusTerm> {
    if documents.is_empty() {
        return Vec::new();
    }
    let mut stats: BTreeMap<&Term, (u32, u32, BTreeMap<&str, u32>)> = BTreeMap::new();
    for document in documents {
        for (term, (count, surface)) in &document.counts {
            let entry = stats.entry(term).or_default();
            entry.0 += 1;
            entry.1 += count;
            *entry.2.entry(surface.as_str()).or_default() += count;
        }
    }
    let total = documents.len() as f64;
    let mut terms: Vec<ConsensusTerm> = stats
        .into_iter()
        .filter(|(term, (pages, _, _))| {
            *pages >= 2
                && f64::from(*pages) / total >= min_share
                && !(term.n == 1
                    && (term.key.chars().count() < 3
                        || term.key.chars().all(|ch| ch.is_ascii_digit())))
        })
        .map(|(term, (pages, occurrences, surfaces))| ConsensusTerm {
            term: term.clone(),
            surface: surfaces
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(surface, _)| surface.to_owned())
                .unwrap_or_default(),
            pages,
            share: f64::from(pages) / total,
            average: f64::from(occurrences) / f64::from(pages),
        })
        .collect();
    let longer: Vec<(String, u32)> = terms
        .iter()
        .filter(|term| term.term.n > 1)
        .map(|term| (format!(" {} ", term.term.key), term.pages))
        .collect();
    terms.retain(|term| {
        let needle = format!(" {} ", term.term.key);
        !longer.iter().any(|(phrase, pages)| {
            *phrase != needle && phrase.contains(&needle) && *pages >= term.pages
        })
    });
    terms.sort_by(|a, b| {
        b.pages
            .cmp(&a.pages)
            .then_with(|| b.term.n.cmp(&a.term.n))
            .then_with(|| b.average.total_cmp(&a.average))
            .then_with(|| a.term.key.cmp(&b.term.key))
    });
    terms.truncate(limit);
    terms
}

/// Consensus terms a page does not use.
pub fn gaps<'a>(page: &DocTerms, consensus: &'a [ConsensusTerm]) -> Vec<&'a ConsensusTerm> {
    consensus
        .iter()
        .filter(|term| !page.has(&term.term.key))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turkish() -> Analyzer {
        Analyzer::new(Language::Turkish)
    }

    #[test]
    fn stems_and_folds_turkish_phrases() {
        let analyzer = turkish();
        assert_eq!(
            analyzer.key("Türk Kahvesi"),
            analyzer.key("turk kahvesinin")
        );
        assert!(analyzer.contains(
            "Bu tarif ile türk kahvesi nasıl yapılır öğrenin",
            "Türk kahvesi"
        ));
        assert!(!analyzer.contains("kahve türk", "türk kahvesi"));
        assert!((analyzer.coverage("kahve türk", "türk kahvesi") - 1.0).abs() < 1e-9);
        assert!(!analyzer.contains("anything", "ve"));
    }

    #[test]
    fn keys_turkish_inflections_alike() {
        let key = |word: &str| turkish_key(&fold(&crate::lower(word, Language::Turkish)));
        for (forms, expected) in [
            (
                &[
                    "kahve",
                    "kahvesi",
                    "kahvesinin",
                    "kahveyi",
                    "kahveye",
                    "kahvenin",
                    "kahveler",
                    "kahvelerinden",
                ][..],
                "kahv",
            ),
            (&["otel", "otelleri", "otelde", "otelin"][..], "otel"),
            (&["fiyat", "fiyatı", "fiyatları"][..], "fiyat"),
            (&["araba", "arabası", "arabaları", "arabasında"][..], "arap"),
            (&["kitap", "kitabı", "kitapları"][..], "kitap"),
            (&["ders", "dersi", "dersler"][..], "ders"),
            (&["ajans", "ajansı", "ajanslar"][..], "ajans"),
            (&["ev", "evi", "evde"][..], "ev"),
            (&["renk", "rengi"][..], "renk"),
        ] {
            for form in forms {
                assert_eq!(key(form), expected, "{form}");
            }
        }
        assert_eq!(key("su"), "su");
    }

    #[test]
    fn counts_terms_and_ngrams() {
        let analyzer = turkish();
        let doc = analyzer.analyze("Türk kahvesi ve çay. Türk kahvesi lezzetlidir.");
        let kahve = analyzer.key("türk kahvesi");
        assert_eq!(doc.count(&kahve), 2);
        assert!(doc.counts.keys().any(|term| term.n == 3)); // "kahvesi ve çay"
        assert!(!doc.counts.keys().any(|term| term.key.is_empty()));
        let (_, surface) = doc
            .counts
            .iter()
            .find(|(term, _)| term.key == kahve)
            .unwrap()
            .1
            .clone();
        assert_eq!(surface, "türk kahvesi");
        assert_eq!(doc.words, 7);
    }

    #[test]
    fn finds_consensus_and_gaps() {
        let analyzer = turkish();
        let pages = [
            analyzer.analyze(
                "Türk kahvesi nasıl yapılır? Cezve, kahve ve su gerekir. Köpük önemlidir.",
            ),
            analyzer.analyze("Türk kahvesi tarifi: cezve ile pişirin, köpüklü servis edin."),
            analyzer.analyze("Türk kahvesi için cezve seçimi ve köpük püf noktaları."),
            analyzer.analyze("Espresso makinesi önerileri."),
        ];
        let refs: Vec<&DocTerms> = pages.iter().collect();
        let terms = consensus(&refs, 0.5, 20);
        let keys: Vec<&str> = terms.iter().map(|term| term.term.key.as_str()).collect();
        assert!(keys.contains(&analyzer.key("türk kahvesi").as_str()));
        assert!(keys.contains(&analyzer.key("cezve").as_str()));
        // "türk" alone is used by exactly the pages using "türk kahvesi".
        assert!(!keys.contains(&analyzer.key("türk").as_str()));
        let espresso = gaps(&pages[3], &terms);
        assert!(
            espresso
                .iter()
                .any(|term| term.term.key == analyzer.key("cezve"))
        );
        assert_eq!(consensus(&[], 0.5, 10).len(), 0);
    }
}
