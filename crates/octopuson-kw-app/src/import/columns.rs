//! What the columns of keyword exports mean (Ahrefs, Semrush, Keyword Planner, Search
//! Console, Wordstat; English, Turkish and Russian headers) and how their numbers are
//! written.

use serde::{Deserialize, Serialize};

/// Meaning of a column.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColumnRole {
    /// The keyword.
    Keyword,
    /// Average monthly searches (a number or a range like `1K – 10K`).
    Volume,
    /// Cost per click.
    Cpc,
    /// Advertiser competition, 0–1 or Low/Medium/High.
    Competition,
    /// Advertiser competition, 0–100.
    CompetitionIndex,
    /// Keyword difficulty, 0–100.
    Difficulty,
    /// Search Console clicks.
    Clicks,
    /// Search Console impressions.
    Impressions,
    /// Average position.
    Position,
    /// Searches in one month (`YYYY-MM`).
    Month(String),
}

const KEYWORD: &[&str] = &[
    "keyword",
    "keywords",
    "keyword phrase",
    "phrase",
    "query",
    "queries",
    "top queries",
    "search query",
    "search term",
    "search terms",
    "anahtar kelime",
    "anahtar kelimeler",
    "kelime",
    "sorgu",
    "sorgular",
    "en cok yapilan sorgular",
    "en populer sorgular",
    "ключевое слово",
    "ключевые слова",
    "фраза",
    "запрос",
];
const VOLUME: &[&str] = &[
    "volume",
    "search volume",
    "avg monthly searches",
    "average monthly searches",
    "monthly searches",
    "monthly volume",
    "global volume",
    "sv",
    "hacim",
    "arama hacmi",
    "aylik arama",
    "aylik aramalar",
    "ort aylik aramalar",
    "ortalama aylik arama",
    "ortalama aylik aramalar",
    "aylik ortalama arama",
    "число запросов",
    "показов в месяц",
    "частотность",
];
const CPC: &[&str] = &[
    "cpc",
    "cpc usd",
    "avg cpc",
    "average cpc",
    "tbm",
    "ortalama tbm",
    "tiklama basina maliyet",
];
const COMPETITION: &[&str] = &[
    "competition",
    "competitive density",
    "com",
    "rekabet",
    "конкуренция",
];
const COMPETITION_INDEX: &[&str] = &[
    "competition indexed value",
    "competition index",
    "rekabet endeksli deger",
    "rekabet dizine eklenmis deger",
];
const DIFFICULTY: &[&str] = &[
    "kd",
    "keyword difficulty",
    "difficulty",
    "seo difficulty",
    "zorluk",
    "anahtar kelime zorlugu",
];
const CLICKS: &[&str] = &["clicks", "tiklamalar", "tiklama", "клики"];
const IMPRESSIONS: &[&str] = &["impressions", "gosterimler", "gosterim", "показы"];
const POSITION: &[&str] = &[
    "position",
    "average position",
    "avg position",
    "konum",
    "ortalama konum",
    "pozisyon",
    "позиция",
];

const MONTHS: [[&str; 2]; 12] = [
    ["jan", "oca"],
    ["feb", "sub"],
    ["mar", "mar"],
    ["apr", "nis"],
    ["may", "may"],
    ["jun", "haz"],
    ["jul", "tem"],
    ["aug", "agu"],
    ["sep", "eyl"],
    ["oct", "eki"],
    ["nov", "kas"],
    ["dec", "ara"],
];

/// A header folded for matching: lower case, Turkish letters without marks, only letters,
/// digits and single spaces (`Avg. monthly searches` → `avg monthly searches`).
pub(crate) fn fold(header: &str) -> String {
    let mut folded = String::new();
    for character in header.chars().flat_map(char::to_lowercase) {
        let character = match character {
            'ı' | 'i' | 'î' | 'ì' | 'í' => 'i',
            'ş' => 's',
            'ğ' => 'g',
            'ü' | 'û' => 'u',
            'ö' => 'o',
            'ç' => 'c',
            'â' | 'à' | 'á' => 'a',
            '\u{307}' => continue,
            other => other,
        };
        if character.is_alphanumeric() {
            folded.push(character);
        } else if !folded.is_empty() && !folded.ends_with(' ') {
            folded.push(' ');
        }
    }
    folded.trim_end().to_owned()
}

/// `Searches: Sep 2025` / `Aramalar: Eyl 2025` → `2025-09`.
fn month(folded: &str) -> Option<String> {
    let rest = folded
        .strip_prefix("searches ")
        .or_else(|| folded.strip_prefix("aramalar "))?;
    let (name, year) = rest.split_once(' ')?;
    let number = MONTHS
        .iter()
        .position(|names| names.iter().any(|candidate| name.starts_with(candidate)))?;
    let year: u16 = year
        .parse()
        .ok()
        .filter(|year| (2000..=2100).contains(year))?;
    Some(format!("{year}-{:02}", number + 1))
}

/// The role of a header, if it has a known one.
pub(crate) fn role(header: &str) -> Option<ColumnRole> {
    let folded = fold(header);
    if let Some(month) = month(&folded) {
        return Some(ColumnRole::Month(month));
    }
    let lists: [(&[&str], ColumnRole); 9] = [
        (KEYWORD, ColumnRole::Keyword),
        (VOLUME, ColumnRole::Volume),
        (CPC, ColumnRole::Cpc),
        (COMPETITION_INDEX, ColumnRole::CompetitionIndex),
        (COMPETITION, ColumnRole::Competition),
        (DIFFICULTY, ColumnRole::Difficulty),
        (CLICKS, ColumnRole::Clicks),
        (IMPRESSIONS, ColumnRole::Impressions),
        (POSITION, ColumnRole::Position),
    ];
    lists
        .into_iter()
        .find(|(names, _)| names.contains(&folded.as_str()))
        .map(|(_, role)| role)
        // `CPC (USD)`, `CPC ($)`, `KD %`: a known name followed by a unit.
        .or_else(|| {
            let first = folded.split(' ').next().unwrap_or_default();
            match first {
                "cpc" => Some(ColumnRole::Cpc),
                "kd" => Some(ColumnRole::Difficulty),
                _ => None,
            }
        })
}

/// Text without spaces, currency and percent signs or comparison marks.
fn bare(text: &str) -> String {
    text.chars()
        .filter(|character| {
            !character.is_whitespace()
                && !matches!(
                    character,
                    '$' | '€' | '£' | '₺' | '₽' | '%' | '<' | '>' | '~' | '≈'
                )
        })
        .collect::<String>()
        .trim_end_matches(|character: char| character.is_ascii_alphabetic())
        .to_owned()
}

/// A decimal number written with a decimal point or comma, with or without thousands
/// separators (`1,234.5`, `1.234,5`, `0,45`, `12.5`).
pub(crate) fn decimal(text: &str) -> Option<f64> {
    let bare = bare(text);
    if bare.is_empty() {
        return None;
    }
    let normalized = match (bare.rfind(','), bare.rfind('.')) {
        (Some(comma), Some(point)) if comma > point => bare.replace('.', "").replace(',', "."),
        (Some(_), None) if bare.matches(',').count() == 1 => bare.replace(',', "."),
        (Some(_), _) => bare.replace(',', ""),
        (None, Some(_)) if bare.matches('.').count() > 1 => bare.replace('.', ""),
        _ => bare,
    };
    normalized
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

/// A count with thousands separators or a short suffix (`12,100`, `1.300`, `1K`, `2,5 B`,
/// `1M`).
pub(crate) fn count(text: &str) -> Option<u64> {
    let compact: String = text
        .chars()
        .filter(|character| {
            !character.is_whitespace() && !matches!(character, '<' | '>' | '~' | '≈')
        })
        .collect::<String>()
        .to_lowercase();
    let (number, multiplier) = if let Some(number) = compact.strip_suffix("mn") {
        (number, 1_000_000.0)
    } else if let Some(number) = compact.strip_suffix('m') {
        (number, 1_000_000.0)
    } else if let Some(number) = compact
        .strip_suffix('k')
        .or_else(|| compact.strip_suffix('b'))
    {
        (number, 1_000.0)
    } else {
        let digits: String = compact
            .chars()
            .filter(|c| !matches!(c, ',' | '.'))
            .collect();
        return digits.parse().ok();
    };
    let value = decimal(number)? * multiplier;
    // Counts are far below 2^53, so the conversion is exact.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    (value >= 0.0).then(|| value.round() as u64)
}

/// A volume or a range of volumes (`1K – 10K`, `100-1000`): `(volume, low, high)`.
pub(crate) fn volume(text: &str) -> Option<(Option<u64>, Option<u64>, Option<u64>)> {
    let text = text.trim();
    for separator in ['–', '—', '-'] {
        if let Some((low, high)) = text.split_once(separator)
            && !low.trim().is_empty()
        {
            let (low, high) = (count(low)?, count(high)?);
            return Some((None, Some(low.min(high)), Some(low.max(high))));
        }
    }
    count(text).map(|volume| (Some(volume), None, None))
}

/// Advertiser competition in 0–1: a share, a percentage, or Low/Medium/High.
pub(crate) fn competition(text: &str) -> Option<f64> {
    match fold(text).as_str() {
        "low" | "dusuk" | "низкая" => return Some(0.25),
        "medium" | "orta" | "средняя" => return Some(0.5),
        "high" | "yuksek" | "высокая" => return Some(0.75),
        _ => {}
    }
    let value = decimal(text)?;
    if text.contains('%') || value > 1.0 {
        return (0.0..=100.0).contains(&value).then_some(value / 100.0);
    }
    (value >= 0.0).then_some(value)
}
