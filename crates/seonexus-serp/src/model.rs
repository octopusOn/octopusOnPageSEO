//! Queries and the result pages they return.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::SerpError;

/// Results per search engine page; Google ignores `num=100` since September 2025.
pub const RESULTS_PER_PAGE: u16 = 10;

/// Deepest result position a query may ask for.
pub const MAX_DEPTH: u16 = 100;

/// A web search engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    /// Google Search.
    Google,
    /// Microsoft Bing.
    Bing,
    /// Yandex Search.
    Yandex,
}

impl Engine {
    /// Every engine, in display order.
    pub const ALL: [Self; 3] = [Self::Google, Self::Bing, Self::Yandex];

    /// Lowercase identifier (`google`, `bing`, `yandex`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::Bing => "bing",
            Self::Yandex => "yandex",
        }
    }
}

impl fmt::Display for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Engine {
    type Err = SerpError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "google" => Ok(Self::Google),
            "bing" => Ok(Self::Bing),
            "yandex" => Ok(Self::Yandex),
            other => Err(SerpError::InvalidQuery(format!(
                "unknown search engine `{other}`"
            ))),
        }
    }
}

/// Which kind of device the results are for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Device {
    /// Desktop results.
    #[default]
    Desktop,
    /// Smartphone results.
    Mobile,
}

impl Device {
    /// Lowercase identifier (`desktop`, `mobile`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Mobile => "mobile",
        }
    }
}

impl FromStr for Device {
    type Err = SerpError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "desktop" => Ok(Self::Desktop),
            "mobile" | "smartphone" => Ok(Self::Mobile),
            other => Err(SerpError::InvalidQuery(format!("unknown device `{other}`"))),
        }
    }
}

/// A place results are localised to, in the forms the engines and providers understand.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Location {
    /// Human-readable name, e.g. `Istanbul, Turkey`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Google Ads canonical name, e.g. `Istanbul,Istanbul,Turkey` (encoded into `uule`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google_canonical: Option<String>,
    /// Yandex region id (`lr`), e.g. `11508` for Istanbul.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yandex_lr: Option<u32>,
    /// Location code of an API provider, passed through as is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_code: Option<String>,
}

impl Location {
    /// Whether no form of the location is set.
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.google_canonical.is_none()
            && self.yandex_lr.is_none()
            && self.provider_code.is_none()
    }
}

/// One search: a keyword on an engine, localised and limited to a depth.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SerpQuery {
    /// Search engine.
    pub engine: Engine,
    /// The keyword as typed.
    pub keyword: String,
    /// ISO 3166-1 alpha-2 country code, lowercase (`tr`).
    pub country: String,
    /// ISO 639-1 interface language, lowercase (`tr`).
    pub language: String,
    /// Optional city or region.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    /// Device the results are for.
    #[serde(default)]
    pub device: Device,
    /// Deepest organic position wanted (10, 20, ... up to [`MAX_DEPTH`]).
    pub depth: u16,
}

impl SerpQuery {
    /// A query for the first page of Turkish results.
    pub fn new(engine: Engine, keyword: impl Into<String>) -> Self {
        Self {
            engine,
            keyword: keyword.into(),
            country: "tr".to_owned(),
            language: "tr".to_owned(),
            location: None,
            device: Device::Desktop,
            depth: RESULTS_PER_PAGE,
        }
    }

    /// Sets country and language.
    #[must_use]
    pub fn localised(mut self, country: &str, language: &str) -> Self {
        country
            .trim()
            .to_ascii_lowercase()
            .clone_into(&mut self.country);
        language
            .trim()
            .to_ascii_lowercase()
            .clone_into(&mut self.language);
        self
    }

    /// Sets the depth.
    #[must_use]
    pub fn with_depth(mut self, depth: u16) -> Self {
        self.depth = depth;
        self
    }

    /// Result pages needed to reach [`Self::depth`].
    pub fn pages(&self) -> u16 {
        self.depth.clamp(1, MAX_DEPTH).div_ceil(RESULTS_PER_PAGE)
    }

    /// Checks that the query can be sent.
    ///
    /// # Errors
    /// [`SerpError::InvalidQuery`] for an empty or overlong keyword, a malformed country or
    /// language code, or a depth outside 1..=[`MAX_DEPTH`].
    pub fn validate(&self) -> Result<(), SerpError> {
        let keyword = self.keyword.trim();
        if keyword.is_empty() {
            return Err(SerpError::InvalidQuery("the keyword is empty".to_owned()));
        }
        if keyword.chars().count() > 400 {
            return Err(SerpError::InvalidQuery(
                "the keyword is longer than 400 characters".to_owned(),
            ));
        }
        if !is_code(&self.country) {
            return Err(SerpError::InvalidQuery(format!(
                "`{}` is not a two-letter country code",
                self.country
            )));
        }
        if !is_code(&self.language) {
            return Err(SerpError::InvalidQuery(format!(
                "`{}` is not a two-letter language code",
                self.language
            )));
        }
        if self.depth == 0 || self.depth > MAX_DEPTH {
            return Err(SerpError::InvalidQuery(format!(
                "depth must be between 1 and {MAX_DEPTH}"
            )));
        }
        Ok(())
    }
}

/// The form a keyword is identified by: whitespace collapsed and lowercased, with Turkish and
/// Azerbaijani rules for those languages (`I` → `ı`, `İ` → `i`), so `İstanbul Otelleri` and
/// `istanbul  otelleri` are the same keyword.
pub fn keyword_key(keyword: &str, language: &str) -> String {
    let squashed = crate::text::squash(keyword);
    if matches!(language, "tr" | "az") {
        squashed
            .chars()
            .flat_map(|ch| match ch {
                'I' => vec!['ı'],
                'İ' => vec!['i'],
                other => other.to_lowercase().collect(),
            })
            .collect()
    } else {
        squashed.to_lowercase()
    }
}

fn is_code(code: &str) -> bool {
    code.len() == 2 && code.bytes().all(|byte| byte.is_ascii_lowercase())
}

/// A sub-link shown under an organic result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sitelink {
    /// Link text.
    pub title: String,
    /// Absolute URL.
    pub url: String,
}

/// One organic (unpaid) result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organic {
    /// Rank among organic results, from 1, counted across pages.
    pub position: u16,
    /// Rank among every block of the page (ads, features and results), from 1, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub absolute_position: Option<u16>,
    /// Result page the result was on, from 1.
    pub page: u16,
    /// Absolute URL of the result.
    pub url: String,
    /// Lowercase host of [`Self::url`].
    pub host: String,
    /// Registrable domain (eTLD+1) of [`Self::host`].
    pub domain: String,
    /// Result title.
    pub title: String,
    /// Snippet text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    /// URL as displayed (breadcrumbs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub displayed_url: Option<String>,
    /// Date shown with the snippet, as displayed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// Sitelinks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sitelinks: Vec<Sitelink>,
    /// Only the site is known: the engine hid the address of the page (Google's encrypted
    /// result links), so `url` is the site's home address taken from the displayed URL.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub site_only: bool,
}

impl Organic {
    /// What makes two results the same: the address, or the site and title when only the
    /// site is known (two pages of one site then share an address).
    pub fn identity(&self) -> String {
        if self.site_only {
            format!("{}\u{0}{}", self.url, self.title)
        } else {
            self.url.clone()
        }
    }
}

/// A non-organic block of a result page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FeatureKind {
    /// Paid results above the organic results.
    AdsTop,
    /// Paid results below the organic results.
    AdsBottom,
    /// Answer box quoting one page.
    FeaturedSnippet,
    /// "People also ask" questions.
    PeopleAlsoAsk,
    /// Map with local businesses.
    LocalPack,
    /// Video carousel.
    Videos,
    /// Image pack.
    Images,
    /// Generated answer (Google AI Overview, Yandex Neuro, Bing Copilot answer).
    AiOverview,
    /// Related searches.
    RelatedSearches,
    /// News carousel.
    TopStories,
    /// Entity panel.
    KnowledgePanel,
    /// Product listings.
    Shopping,
    /// Anything else a mapping names.
    Other,
}

impl FeatureKind {
    /// Every kind.
    pub const ALL: [Self; 13] = [
        Self::AdsTop,
        Self::AdsBottom,
        Self::FeaturedSnippet,
        Self::PeopleAlsoAsk,
        Self::LocalPack,
        Self::Videos,
        Self::Images,
        Self::AiOverview,
        Self::RelatedSearches,
        Self::TopStories,
        Self::KnowledgePanel,
        Self::Shopping,
        Self::Other,
    ];

    /// Kebab-case identifier, as serialized.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AdsTop => "ads-top",
            Self::AdsBottom => "ads-bottom",
            Self::FeaturedSnippet => "featured-snippet",
            Self::PeopleAlsoAsk => "people-also-ask",
            Self::LocalPack => "local-pack",
            Self::Videos => "videos",
            Self::Images => "images",
            Self::AiOverview => "ai-overview",
            Self::RelatedSearches => "related-searches",
            Self::TopStories => "top-stories",
            Self::KnowledgePanel => "knowledge-panel",
            Self::Shopping => "shopping",
            Self::Other => "other",
        }
    }

    /// Whether the block takes clicks away from the organic results above the fold.
    pub const fn competes_for_clicks(self) -> bool {
        matches!(
            self,
            Self::AdsTop
                | Self::FeaturedSnippet
                | Self::LocalPack
                | Self::AiOverview
                | Self::Shopping
                | Self::TopStories
        )
    }
}

impl FromStr for FeatureKind {
    type Err = SerpError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let wanted = value.trim().to_ascii_lowercase().replace('_', "-");
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == wanted)
            .or(match wanted.as_str() {
                "ads" => Some(Self::AdsTop),
                "paa" | "related-questions" => Some(Self::PeopleAlsoAsk),
                "answer-box" | "snippet" => Some(Self::FeaturedSnippet),
                "ai-overviews" | "ai" => Some(Self::AiOverview),
                "related" => Some(Self::RelatedSearches),
                "news" => Some(Self::TopStories),
                "local" | "maps" => Some(Self::LocalPack),
                "video" => Some(Self::Videos),
                "image" => Some(Self::Images),
                _ => None,
            })
            .ok_or_else(|| SerpError::InvalidSpec(format!("unknown feature kind `{value}`")))
    }
}

/// One entry of a feature: a question, an ad, a related search, a cited page.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureItem {
    /// Visible text (question, related search, ad headline).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Linked URL, absolute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Registrable domain of [`Self::url`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
}

/// A non-organic block and what it shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feature {
    /// What the block is.
    pub kind: FeatureKind,
    /// Rank among every block of the page, from 1, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub absolute_position: Option<u16>,
    /// Result page, from 1.
    pub page: u16,
    /// Entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<FeatureItem>,
}

/// Everything a query returned, from one provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SerpSnapshot {
    /// The query.
    pub query: SerpQuery,
    /// Provider id (`browser`, `serper`, a custom provider's id).
    pub provider: String,
    /// Revision of the provider protocol or custom spec.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_rev: Option<String>,
    /// Parser profile that read HTML results (`google-html@1`), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parser_rev: Option<String>,
    /// When the first page was fetched, RFC 3339.
    pub fetched_at: String,
    /// Result count the engine reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_results: Option<u64>,
    /// Organic results by position.
    pub organic: Vec<Organic>,
    /// Non-organic blocks in page order.
    #[serde(default)]
    pub features: Vec<Feature>,
    /// Problems that did not stop the search (a page that failed, low parser coverage).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl SerpSnapshot {
    /// The blocks of one kind.
    pub fn features_of(&self, kind: FeatureKind) -> impl Iterator<Item = &Feature> {
        self.features
            .iter()
            .filter(move |feature| feature.kind == kind)
    }

    /// Whether the page shows a block of `kind`.
    pub fn has_feature(&self, kind: FeatureKind) -> bool {
        self.features_of(kind).next().is_some()
    }

    /// Number of paid results above the organic results of the first page.
    pub fn top_ad_count(&self) -> usize {
        self.features_of(FeatureKind::AdsTop)
            .filter(|feature| feature.page == 1)
            .map(|feature| feature.items.len().max(1))
            .sum()
    }

    /// The organic results up to `depth`.
    pub fn top(&self, depth: u16) -> impl Iterator<Item = &Organic> {
        self.organic
            .iter()
            .filter(move |result| result.position <= depth)
    }
}
