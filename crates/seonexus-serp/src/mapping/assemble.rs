//! Extracted records → organic results and features of one page.

use std::collections::HashSet;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use url::Url;

use super::{Extracted, PostStep, Record};
use crate::challenge::{Challenge, ChallengeKind};
use crate::text::{first_count, non_empty};
use crate::{Feature, FeatureItem, Organic, Sitelink, registrable_domain};

/// How many matched results were usable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    /// Nodes the result selector matched.
    pub matched: usize,
    /// Results with a valid URL and a title.
    pub usable: usize,
}

impl Coverage {
    /// Share of matched results that were usable; 1 when nothing matched.
    #[allow(clippy::cast_precision_loss)] // counts of a result page
    pub fn ratio(&self) -> f64 {
        if self.matched == 0 {
            1.0
        } else {
            self.usable as f64 / self.matched as f64
        }
    }
}

/// One result page, read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedPage {
    /// Result page, from 1.
    pub page: u16,
    /// Organic results; positions count from 1 within the page.
    pub organic: Vec<Organic>,
    /// Feature blocks.
    pub features: Vec<Feature>,
    /// Result count the engine reported.
    pub total_results: Option<u64>,
    /// Error message the response carried.
    pub error: Option<String>,
    /// Consent, CAPTCHA or block page detected instead of results.
    pub challenge: Option<Challenge>,
    /// How well the result selectors fit the page.
    pub coverage: Coverage,
}

pub(super) struct Cleanup<'a> {
    pub base: &'a Url,
    pub post: &'a [PostStep],
    pub skip_urls: &'a [String],
}

impl Cleanup<'_> {
    /// Absolute, unwrapped URL without text fragments; `None` unless http(s).
    fn url(&self, raw: &str) -> Option<Url> {
        let mut url = self.base.join(raw.trim()).ok()?;
        if self.hides(&url) {
            return None;
        }
        for step in self.post {
            if let Some(target) = match step {
                PostStep::GoogleRedirect => google_target(&url),
                PostStep::BingRedirect => bing_target(&url),
            } {
                url = target;
            }
        }
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return None;
        }
        if url
            .fragment()
            .is_some_and(|fragment| fragment.starts_with(":~:"))
        {
            url.set_fragment(None);
        }
        Some(url)
    }

    /// Whether `url` is an engine link that hides its target: Google's encrypted
    /// `/goto?url=…` result links (2026) carry no address that can be read.
    fn hides(&self, url: &Url) -> bool {
        self.post.contains(&PostStep::GoogleRedirect)
            && url.path() == "/goto"
            && url
                .host_str()
                .is_some_and(|host| host.starts_with("www.google.") || host.starts_with("google."))
    }

    /// The site of a displayed URL (`https://www.karaca.com › kahve-makineleri`): its home
    /// address, for results whose link is hidden.
    fn site_of(displayed: &str) -> Option<Url> {
        let origin = displayed.split('›').next()?.trim();
        let origin = origin.split_whitespace().next()?;
        let mut url = if origin.contains("://") {
            Url::parse(origin).ok()?
        } else {
            Url::parse(&format!("https://{origin}")).ok()?
        };
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return None;
        }
        url.set_path("/");
        url.set_query(None);
        url.set_fragment(None);
        Some(url)
    }

    fn skipped(&self, url: &Url) -> bool {
        self.skip_urls
            .iter()
            .any(|prefix| url.as_str().starts_with(prefix.as_str()))
    }
}

fn google_target(url: &Url) -> Option<Url> {
    let host = url.host_str()?;
    if url.path() != "/url" || !(host.starts_with("www.google.") || host.starts_with("google.")) {
        return None;
    }
    url.query_pairs()
        .find(|(key, _)| key == "q" || key == "url")
        .and_then(|(_, value)| Url::parse(&value).ok())
}

fn bing_target(url: &Url) -> Option<Url> {
    if url.host_str()? != "www.bing.com" || url.path() != "/ck/a" {
        return None;
    }
    let encoded = url.query_pairs().find(|(key, _)| key == "u")?.1;
    let payload = encoded.strip_prefix("a1")?;
    let bytes = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    Url::parse(std::str::from_utf8(&bytes).ok()?).ok()
}

/// An organic result before positions are assigned.
struct Draft {
    /// Rank the provider gave it, if any.
    ranked: Option<u32>,
    /// Document order of its node (HTML).
    order: Option<usize>,
    /// Absolute position the provider gave it, if any.
    absolute: Option<u16>,
    organic: Organic,
}

pub(super) fn assemble(
    extracted: Extracted,
    challenge: Option<Challenge>,
    cleanup: &Cleanup<'_>,
    page: u16,
) -> ParsedPage {
    let matched = extracted.results.len();
    let mut usable = 0;
    let mut seen = HashSet::new();
    let mut drafts: Vec<Draft> = Vec::new();
    for record in extracted.results {
        let organic = match organic(&record, cleanup, page) {
            Read::Result(organic) => *organic,
            // A link back to the engine was read correctly; it just is not a result.
            Read::Skipped => {
                usable += 1;
                continue;
            }
            Read::Invalid => continue,
        };
        // A result matched twice (nested containers) is usable, just not new.
        usable += 1;
        if !seen.insert(organic.identity()) {
            continue;
        }
        let ranked = record
            .values
            .get("position")
            .and_then(|value| value.trim().parse::<u32>().ok());
        let absolute = record
            .values
            .get("absolute_position")
            .and_then(|value| value.trim().parse::<u16>().ok());
        drafts.push(Draft {
            ranked,
            order: record.order,
            absolute,
            organic,
        });
    }
    // Results a provider ranked keep that order; the others follow in document order.
    drafts.sort_by_key(|draft| draft.ranked.unwrap_or(u32::MAX));

    let mut features: Vec<(Option<usize>, Feature)> = extracted
        .features
        .into_iter()
        .map(|record| {
            let items = record
                .items
                .iter()
                .filter_map(|item| feature_item(item, cleanup))
                .collect();
            (
                record.order,
                Feature {
                    kind: record.kind,
                    absolute_position: None,
                    page,
                    items,
                },
            )
        })
        .collect();

    // Absolute positions: rank every block that has a document position.
    let mut blocks: Vec<(usize, bool, usize)> = drafts
        .iter()
        .enumerate()
        .filter_map(|(index, draft)| draft.order.map(|order| (order, true, index)))
        .chain(
            features
                .iter()
                .enumerate()
                .filter_map(|(index, (order, _))| order.map(|order| (order, false, index))),
        )
        .collect();
    blocks.sort_unstable();
    for (rank, (_, is_result, index)) in blocks.into_iter().enumerate() {
        let rank = u16::try_from(rank + 1).ok();
        if is_result {
            drafts[index].organic.absolute_position = rank;
        } else {
            features[index].1.absolute_position = rank;
        }
    }
    features.sort_by_key(|(order, _)| order.unwrap_or(usize::MAX));

    let organic: Vec<Organic> = drafts
        .into_iter()
        .enumerate()
        .map(|(index, draft)| {
            let mut organic = draft.organic;
            organic.position = u16::try_from(index + 1).unwrap_or(u16::MAX);
            if draft.absolute.is_some() {
                organic.absolute_position = draft.absolute;
            }
            organic
        })
        .collect();
    // A consent banner over results that can be read does not stop reading them (Bing shows
    // its cookie banner on the result page itself); only a consent page without results does.
    let challenge = challenge
        .filter(|challenge| challenge.kind != ChallengeKind::Consent || organic.is_empty());
    ParsedPage {
        page,
        organic,
        features: features.into_iter().map(|(_, feature)| feature).collect(),
        total_results: extracted.total.as_deref().and_then(first_count),
        error: extracted.error,
        challenge,
        coverage: Coverage { matched, usable },
    }
}

enum Read {
    Result(Box<Organic>),
    Skipped,
    Invalid,
}

fn organic(record: &Record, cleanup: &Cleanup<'_>, page: u16) -> Read {
    let raw = record.values.get("url");
    let hidden = raw.is_some_and(|raw| {
        cleanup
            .base
            .join(raw.trim())
            .is_ok_and(|url| cleanup.hides(&url))
    });
    let url = if hidden {
        // The engine hid the page address: keep the result with its site.
        record
            .values
            .get("displayed_url")
            .and_then(|displayed| Cleanup::site_of(displayed))
    } else {
        raw.and_then(|url| cleanup.url(url))
    };
    let Some(url) = url else {
        return Read::Invalid;
    };
    if cleanup.skipped(&url) {
        return Read::Skipped;
    }
    let Some(title) = record
        .values
        .get("title")
        .and_then(|title| non_empty(title))
    else {
        return Read::Invalid;
    };
    let Some(host) = url.host_str() else {
        return Read::Invalid;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let titles = record
        .lists
        .get("sitelink_titles")
        .cloned()
        .unwrap_or_default();
    let urls = record
        .lists
        .get("sitelink_urls")
        .cloned()
        .unwrap_or_default();
    let sitelinks = titles
        .into_iter()
        .zip(urls)
        .filter_map(|(title, url)| {
            Some(Sitelink {
                title: non_empty(&title)?,
                url: cleanup.url(&url)?.to_string(),
            })
        })
        .collect();
    Read::Result(Box::new(Organic {
        position: 0,
        absolute_position: None,
        page,
        domain: registrable_domain(&host),
        host,
        url: url.to_string(),
        title,
        snippet: record
            .values
            .get("snippet")
            .and_then(|text| non_empty(text)),
        displayed_url: record
            .values
            .get("displayed_url")
            .and_then(|text| non_empty(text)),
        date: record.values.get("date").and_then(|text| non_empty(text)),
        sitelinks,
        site_only: hidden,
    }))
}

fn feature_item(record: &Record, cleanup: &Cleanup<'_>) -> Option<FeatureItem> {
    let text = record.values.get("text").and_then(|text| non_empty(text));
    // Feature links may point at the engine itself (related searches do).
    let url = record.values.get("url").and_then(|url| cleanup.url(url));
    if text.is_none() && url.is_none() {
        return None;
    }
    let domain = url
        .as_ref()
        .and_then(Url::host_str)
        .map(|host| registrable_domain(&host.to_ascii_lowercase()));
    Some(FeatureItem {
        text,
        url: url.map(|url| url.to_string()),
        domain,
    })
}
