//! How brands appear in an answer: which are mentioned and in what order, which of their sites
//! are cited, and the tone of the sentences that mention them.
//!
//! Names are matched as whole words after folding case and accents (Turkish dotless i, ş, ğ
//! and the rest), so "Örnek Mağaza" matches "ornek magaza" and "ÖRNEK MAĞAZA'nın". The tone is
//! a lexicon score from -1 (negative) to 1 (positive) over the mentioning sentences, with
//! Turkish, English, German and Spanish words; words are matched by their start, so Turkish
//! suffixes still count.

use serde::{Deserialize, Serialize};

use crate::Citation;

/// A brand watched in answers: its site and the names it goes by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Brand {
    /// Its site's domain (`example.com`), also the key reports use.
    pub key: String,
    /// Names it is written as.
    pub names: Vec<String>,
    /// The project's own brand.
    pub own: bool,
}

impl Brand {
    /// The brand of a site: the given names, plus the domain and its first label, so a site
    /// without names is still found (folding reads `rakip-magaza` as `rakip magaza`).
    pub fn of_site(domain: &str, names: &[String], own: bool) -> Self {
        let domain = crate::domain_of(domain);
        let label = domain.split('.').next().unwrap_or_default().to_owned();
        let mut all: Vec<String> = names
            .iter()
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
            .collect();
        for extra in [domain.clone(), label] {
            if extra.chars().count() >= 3 && !all.iter().any(|name| fold(name) == fold(&extra)) {
                all.push(extra);
            }
        }
        Self {
            key: domain,
            names: all,
            own,
        }
    }
}

/// How one brand appears in an answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrandHit {
    /// The brand's key (its domain).
    pub key: String,
    /// The project's own brand.
    pub own: bool,
    /// Its place among the brands mentioned, from 1; `None` when only cited.
    pub position: Option<u32>,
    /// Its site is among the sources.
    pub cited: bool,
    /// Tone of the sentences mentioning it, -1 to 1; `None` when not mentioned.
    pub sentiment: Option<f64>,
}

/// How the project's brand and its rivals appear in an answer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    /// The own brand is mentioned.
    pub mentioned: bool,
    /// Its place among the brands mentioned, from 1.
    pub position: Option<u32>,
    /// Its site is cited.
    pub cited: bool,
    /// Tone of its mentions, -1 to 1.
    pub sentiment: Option<f64>,
    /// Every brand mentioned or cited, in order of first mention.
    pub brands: Vec<BrandHit>,
}

/// Lowercase ASCII-ish text: accents and Turkish letters folded, anything else that is not a
/// letter or digit a space, spaces collapsed.
pub fn fold(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    for c in text.chars() {
        let mapped: &str = match c {
            'ı' | 'İ' | 'I' | 'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Î' => "i",
            'ş' | 'Ş' => "s",
            'ğ' | 'Ğ' => "g",
            'ç' | 'Ç' => "c",
            'ö' | 'Ö' | 'ó' | 'ò' | 'ô' | 'õ' | 'Ó' | 'Ô' => "o",
            'ü' | 'Ü' | 'ú' | 'ù' | 'û' | 'Ú' | 'Û' => "u",
            'ä' | 'Ä' | 'á' | 'à' | 'â' | 'ã' | 'Á' | 'À' | 'Â' => "a",
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' => "e",
            'ñ' | 'Ñ' => "n",
            'ß' => "ss",
            _ => "",
        };
        if !mapped.is_empty() {
            folded.push_str(mapped);
        } else if c.is_alphanumeric() {
            folded.extend(c.to_lowercase());
        } else if !folded.ends_with(' ') {
            folded.push(' ');
        }
    }
    folded.trim().to_owned()
}

/// Where a folded name first stands as whole words in folded text.
fn find_words(text: &str, name: &str) -> Option<usize> {
    if name.is_empty() {
        return None;
    }
    let padded = format!(" {text} ");
    padded.find(&format!(" {name} "))
}

const POSITIVE: &[&str] = &[
    // Turkish
    "en iyi",
    "guvenilir",
    "kaliteli",
    "oner",
    "tavsiye",
    "populer",
    "uygun fiyat",
    "hizli",
    "memnun",
    "basarili",
    "lider",
    "harika",
    "mukemmel",
    "begen",
    "one cikan",
    "guclu",
    "avantaj",
    "iyi",
    // English
    "best",
    "reliable",
    "trusted",
    "quality",
    "recommend",
    "popular",
    "affordable",
    "fast",
    "excellent",
    "great",
    "leading",
    "top",
    "good",
    "strong",
    "favorite",
    "favourite",
    // German
    "beste",
    "zuverlassig",
    "empfehl",
    "empfohlen",
    "beliebt",
    "gunstig",
    "schnell",
    "ausgezeichnet",
    "hervorragend",
    "fuhrend",
    "gut",
    "stark",
    // Spanish
    "mejor",
    "fiable",
    "confiable",
    "recomend",
    "economic",
    "rapid",
    "excelente",
    "buen",
    "destaca",
];

const NEGATIVE: &[&str] = &[
    // Turkish
    "sikayet",
    "kotu",
    "pahali",
    "sorun",
    "yavas",
    "guvenilmez",
    "dolandir",
    "gecikme",
    "zayif",
    "olumsuz",
    "hatali",
    "yetersiz",
    // English
    "complain",
    "bad",
    "poor",
    "expensive",
    "problem",
    "issue",
    "slow",
    "unreliable",
    "scam",
    "delay",
    "weak",
    "negative",
    "worst",
    "avoid",
    // German
    "beschwerde",
    "schlecht",
    "teuer",
    "langsam",
    "unzuverlassig",
    "betrug",
    "verzogerung",
    "schwach",
    // Spanish
    "queja",
    "malo",
    "mala",
    "caro",
    "problema",
    "lento",
    "estafa",
    "retraso",
    "debil",
    "peor",
    "evitar",
];

/// Lexicon entries in a folded sentence: phrases as whole words, single words by their start
/// (four letters or more) or whole (shorter).
fn lexicon_hits(sentence: &str, lexicon: &[&str]) -> u32 {
    let words: Vec<&str> = sentence.split(' ').collect();
    let mut hits = 0;
    for entry in lexicon {
        let found = if entry.contains(' ') {
            find_words(sentence, entry).is_some()
        } else if entry.len() >= 4 {
            words.iter().any(|word| word.starts_with(entry))
        } else {
            words.contains(entry)
        };
        if found {
            hits += 1;
        }
    }
    hits
}

/// Sentences of a text: split after `.`, `!` or `?` followed by a space, and at line ends (so
/// the dot of a domain does not split).
fn sentences(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\n' {
            found.push(std::mem::take(&mut current));
            continue;
        }
        current.push(c);
        if matches!(c, '.' | '!' | '?') && chars.peek().is_none_or(|next| next.is_whitespace()) {
            found.push(std::mem::take(&mut current));
        }
    }
    found.push(current);
    found
        .into_iter()
        .map(|sentence| fold(&sentence))
        .filter(|sentence| !sentence.is_empty())
        .collect()
}

/// Whether `domain` is `site` or one of its subdomains.
fn belongs(domain: &str, site: &str) -> bool {
    domain == site
        || domain
            .strip_suffix(site)
            .is_some_and(|rest| rest.ends_with('.'))
}

/// Reads an answer for `brands` (the own brand among them).
#[allow(clippy::cast_precision_loss)] // word counts
pub fn analyze(text: &str, citations: &[Citation], brands: &[Brand]) -> Analysis {
    let folded = fold(text);
    let sentences = sentences(text);
    let mut hits: Vec<(Option<usize>, BrandHit)> = Vec::new();
    for brand in brands {
        let names: Vec<String> = brand
            .names
            .iter()
            .map(|name| fold(name))
            .filter(|name| !name.is_empty())
            .collect();
        let first = names
            .iter()
            .filter_map(|name| find_words(&folded, name))
            .min();
        let cited = citations
            .iter()
            .any(|citation| belongs(&citation.domain, &brand.key));
        if first.is_none() && !cited {
            continue;
        }
        let sentiment = first.map(|_| {
            let (mut positive, mut negative) = (0_u32, 0_u32);
            for sentence in &sentences {
                if names
                    .iter()
                    .any(|name| find_words(sentence, name).is_some())
                {
                    positive += lexicon_hits(sentence, POSITIVE);
                    negative += lexicon_hits(sentence, NEGATIVE);
                }
            }
            let total = positive + negative;
            if total == 0 {
                0.0
            } else {
                ((f64::from(positive) - f64::from(negative)) / f64::from(total) * 100.0).round()
                    / 100.0
            }
        });
        hits.push((
            first,
            BrandHit {
                key: brand.key.clone(),
                own: brand.own,
                position: None,
                cited,
                sentiment,
            },
        ));
    }
    hits.sort_by_key(|(first, _)| first.unwrap_or(usize::MAX));
    let mut place = 0_u32;
    for (first, hit) in &mut hits {
        if first.is_some() {
            place += 1;
            hit.position = Some(place);
        }
    }
    let brands: Vec<BrandHit> = hits.into_iter().map(|(_, hit)| hit).collect();
    let own = brands.iter().find(|hit| hit.own);
    Analysis {
        mentioned: own.is_some_and(|hit| hit.position.is_some()),
        position: own.and_then(|hit| hit.position),
        cited: own.is_some_and(|hit| hit.cited),
        sentiment: own.and_then(|hit| hit.sentiment),
        brands,
    }
}
