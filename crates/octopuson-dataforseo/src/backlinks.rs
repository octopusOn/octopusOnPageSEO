//! Backlinks from the DataForSEO Backlinks API: a site's link profile, its backlinks, referring
//! domains, anchors and most linked pages, links won and lost over time, and the authority of
//! many sites at once.
//!
//! Ranks are asked on the 0–100 scale; the platform shows them as the Octopus Authority (OA).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{DfsError, count, items_of, prices, target_domain};

/// Endpoint of a link profile.
pub const SUMMARY: &str = "backlinks/summary/live";
/// Endpoint of backlinks.
pub const BACKLINKS: &str = "backlinks/backlinks/live";
/// Endpoint of referring domains.
pub const REFERRING_DOMAINS: &str = "backlinks/referring_domains/live";
/// Endpoint of anchors.
pub const ANCHORS: &str = "backlinks/anchors/live";
/// Endpoint of a site's pages by links.
pub const DOMAIN_PAGES: &str = "backlinks/domain_pages/live";
/// Endpoint of links won and lost over time.
pub const NEW_LOST: &str = "backlinks/timeseries_new_lost_summary/live";
/// Endpoint of the ranks of many targets.
pub const BULK_RANKS: &str = "backlinks/bulk_ranks/live";

/// Most targets one bulk rank request takes.
pub const MAX_BULK: usize = 1_000;

/// A count by kind (top-level domain, country, platform …), most first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Share {
    /// The kind.
    pub name: String,
    /// How many.
    pub count: u64,
}

/// A site's link profile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// The site or page.
    pub target: String,
    /// Authority, 0–100.
    pub rank: Option<u32>,
    /// Live backlinks.
    pub backlinks: u64,
    /// Domains linking.
    pub referring_domains: u64,
    /// Referring domains with nofollow links only.
    pub referring_domains_nofollow: u64,
    /// Registrable domains linking.
    pub referring_main_domains: u64,
    /// IP addresses linking.
    pub referring_ips: u64,
    /// Subnets linking.
    pub referring_subnets: u64,
    /// Pages linking.
    pub referring_pages: u64,
    /// Backlinks pointing to pages that are gone.
    pub broken_backlinks: u64,
    /// Pages of the target that are gone but linked.
    pub broken_pages: u64,
    /// Spam score of the backlinks, 0–100.
    pub spam_score: Option<u32>,
    /// When the target was first seen.
    pub first_seen: Option<String>,
    /// Links by attribute (`nofollow`, `ugc`, `sponsored` …).
    pub attributes: Vec<Share>,
    /// Links by type (`anchor`, `image`, `redirect` …).
    pub link_types: Vec<Share>,
    /// Linking sites by top-level domain.
    pub top_level_domains: Vec<Share>,
    /// Linking sites by country.
    pub countries: Vec<Share>,
    /// Linking sites by platform (`cms`, `blogs`, `news` …).
    pub platforms: Vec<Share>,
}

/// One backlink.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // the Backlinks API's own flags of a link
pub struct BacklinkRow {
    /// The linking page.
    pub url_from: String,
    /// The linking site.
    pub domain_from: Option<String>,
    /// Title of the linking page.
    pub title: Option<String>,
    /// The linked page of the target.
    pub url_to: Option<String>,
    /// Its status code (404 for a broken link).
    pub url_to_status: Option<u32>,
    /// Anchor text (or image alt text).
    pub anchor: Option<String>,
    /// Whether it passes authority.
    pub dofollow: bool,
    /// `anchor`, `image`, `redirect`, `canonical` …
    pub kind: Option<String>,
    /// Authority of the linking page, 0–100.
    pub page_rank: Option<u32>,
    /// Authority of the linking site, 0–100.
    pub domain_rank: Option<u32>,
    /// Spam score of the link, 0–100.
    pub spam_score: Option<u32>,
    /// First seen.
    pub first_seen: Option<String>,
    /// Last seen.
    pub last_seen: Option<String>,
    /// Found since the last update.
    pub is_new: bool,
    /// Lost since the last update.
    pub is_lost: bool,
    /// Points to a page that is gone.
    pub is_broken: bool,
    /// Country of the linking site.
    pub country: Option<String>,
}

/// A domain linking to the target.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReferringDomainRow {
    /// The domain.
    pub domain: String,
    /// Its authority, 0–100.
    pub rank: Option<u32>,
    /// Backlinks from it.
    pub backlinks: u64,
    /// Whether all its links are nofollow.
    pub nofollow_only: bool,
    /// Spam score, 0–100.
    pub spam_score: Option<u32>,
    /// First seen.
    pub first_seen: Option<String>,
}

/// An anchor text and how much it links.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AnchorRow {
    /// The text (empty for images without alt text).
    pub anchor: String,
    /// Backlinks with it.
    pub backlinks: u64,
    /// Domains using it.
    pub referring_domains: u64,
    /// Domains using it with nofollow links.
    pub referring_domains_nofollow: u64,
    /// First seen.
    pub first_seen: Option<String>,
}

/// A page of the target and the links it gets.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LinkedPageRow {
    /// The page.
    pub url: String,
    /// Its title.
    pub title: Option<String>,
    /// Its status code.
    pub status: Option<u32>,
    /// Its authority, 0–100.
    pub rank: Option<u32>,
    /// Backlinks to it.
    pub backlinks: u64,
    /// Domains linking to it.
    pub referring_domains: u64,
    /// Broken backlinks to it.
    pub broken_backlinks: u64,
}

/// Links won and lost in one period.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NewLostPoint {
    /// End of the period, `yyyy-mm-dd`.
    pub date: String,
    /// Backlinks won.
    pub new_backlinks: u64,
    /// Backlinks lost.
    pub lost_backlinks: u64,
    /// Referring domains won.
    pub new_referring_domains: u64,
    /// Referring domains lost.
    pub lost_referring_domains: u64,
}

/// A target and its authority.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankRow {
    /// The target.
    pub target: String,
    /// Authority, 0–100.
    pub rank: Option<u32>,
}

/// Which backlinks a list holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkStatus {
    /// Live backlinks.
    #[default]
    Live,
    /// Live backlinks found since the last update.
    New,
    /// Backlinks lost.
    Lost,
    /// Live backlinks pointing to pages that are gone.
    Broken,
}

/// How a backlink list is asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BacklinkQuery {
    /// Which backlinks.
    pub status: LinkStatus,
    /// Only links passing authority.
    pub dofollow_only: bool,
    /// The best link of each linking site only.
    pub one_per_domain: bool,
}

/// A target as the Backlinks API takes it: an address stays a page, anything else becomes a
/// bare domain.
///
/// # Errors
/// [`DfsError::Config`] for an empty target.
pub fn target(input: &str) -> Result<String, DfsError> {
    let input = input.trim();
    let page = input
        .split_once("://")
        .filter(|(scheme, _)| matches!(*scheme, "http" | "https"))
        .and_then(|(_, rest)| rest.split_once('/'))
        .is_some_and(|(_, path)| !path.trim_end_matches('/').is_empty());
    let target = if page {
        input.to_owned()
    } else {
        target_domain(input)
    };
    if target.is_empty() {
        return Err(DfsError::Config("the target is empty".to_owned()));
    }
    Ok(target)
}

fn base(target_input: &str) -> Result<Value, DfsError> {
    Ok(json!({
        "target": target(target_input)?,
        "include_subdomains": true,
        "rank_scale": "one_hundred",
    }))
}

/// The task of a link profile.
///
/// # Errors
/// [`DfsError::Config`] for an empty target.
pub fn summary_task(target: &str) -> Result<Value, DfsError> {
    let mut task = base(target)?;
    task["internal_list_limit"] = json!(10);
    task["backlinks_status_type"] = json!("live");
    Ok(task)
}

/// The task of a list of backlinks, strongest linking pages first.
///
/// # Errors
/// [`DfsError::Config`] for an empty target.
pub fn backlinks_task(target: &str, query: BacklinkQuery, limit: u32) -> Result<Value, DfsError> {
    let mut task = base(target)?;
    task["limit"] = json!(crate::site::rows(limit));
    task["mode"] = json!(if query.one_per_domain {
        "one_per_domain"
    } else {
        "as_is"
    });
    task["order_by"] = json!(["page_from_rank,desc"]);
    task["backlinks_status_type"] = json!(match query.status {
        LinkStatus::Lost => "lost",
        _ => "live",
    });
    let mut filters: Vec<Value> = Vec::new();
    match query.status {
        LinkStatus::New => filters.push(json!(["is_new", "=", true])),
        LinkStatus::Broken => filters.push(json!(["is_broken", "=", true])),
        LinkStatus::Live | LinkStatus::Lost => {}
    }
    if query.dofollow_only {
        filters.push(json!(["dofollow", "=", true]));
    }
    if !filters.is_empty() {
        let mut joined: Vec<Value> = Vec::new();
        for (index, filter) in filters.into_iter().enumerate() {
            if index > 0 {
                joined.push(json!("and"));
            }
            joined.push(filter);
        }
        task["filters"] = if joined.len() == 1 {
            joined.remove(0)
        } else {
            Value::Array(joined)
        };
    }
    Ok(task)
}

/// The task of referring domains, strongest first.
///
/// # Errors
/// [`DfsError::Config`] for an empty target.
pub fn referring_domains_task(target: &str, limit: u32) -> Result<Value, DfsError> {
    let mut task = base(target)?;
    task["limit"] = json!(crate::site::rows(limit));
    task["order_by"] = json!(["rank,desc"]);
    task["internal_list_limit"] = json!(1);
    Ok(task)
}

/// The task of anchors, used by the most domains first.
///
/// # Errors
/// [`DfsError::Config`] for an empty target.
pub fn anchors_task(target: &str, limit: u32) -> Result<Value, DfsError> {
    let mut task = base(target)?;
    task["limit"] = json!(crate::site::rows(limit));
    task["order_by"] = json!(["referring_domains,desc"]);
    Ok(task)
}

/// The task of the target's pages, linked from the most domains first.
///
/// # Errors
/// [`DfsError::Config`] for an empty target.
pub fn pages_task(target: &str, limit: u32) -> Result<Value, DfsError> {
    let mut task = json!({ "target": crate::target_domain(target) });
    if task["target"].as_str().is_none_or(str::is_empty) {
        return Err(DfsError::Config("the target is empty".to_owned()));
    }
    task["limit"] = json!(crate::site::rows(limit));
    task["order_by"] = json!(["page_summary.referring_domains,desc"]);
    Ok(task)
}

/// The task of links won and lost per month over DataForSEO's own default range. A
/// `date_from` is left out: DataForSEO refused every one we sent (40501, Invalid Field), even
/// within the year its documentation allows.
///
/// # Errors
/// [`DfsError::Config`] for an empty target.
pub fn new_lost_task(target: &str) -> Result<Value, DfsError> {
    let domain = crate::target_domain(target);
    if domain.is_empty() {
        return Err(DfsError::Config("the target is empty".to_owned()));
    }
    Ok(json!({
        "target": domain,
        "group_range": "month",
        "include_subdomains": true,
    }))
}

/// The task of the authority of `targets` (at most [`MAX_BULK`], duplicates dropped).
///
/// # Errors
/// [`DfsError::Config`] without targets.
pub fn bulk_ranks_task(targets: &[String]) -> Result<Value, DfsError> {
    let mut clean: Vec<String> = Vec::new();
    for entry in targets {
        if let Ok(entry) = target(entry)
            && !clean.contains(&entry)
        {
            clean.push(entry);
        }
    }
    clean.truncate(MAX_BULK);
    if clean.is_empty() {
        return Err(DfsError::Config(
            "targets: enter at least one domain".to_owned(),
        ));
    }
    Ok(json!({ "targets": clean, "rank_scale": "one_hundred" }))
}

#[allow(clippy::cast_possible_truncation)]
fn small(value: &Value, name: &str) -> Option<u32> {
    value
        .get(name)
        .and_then(count)
        .map(|n| n.min(u64::from(u32::MAX)) as u32)
}

fn whole(value: &Value, name: &str) -> u64 {
    value.get(name).and_then(count).unwrap_or_default()
}

fn text(value: &Value, name: &str) -> Option<String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn flag(value: &Value, name: &str) -> bool {
    value.get(name).and_then(Value::as_bool).unwrap_or(false)
}

/// A `{"name": count}` object as shares, most first.
fn shares(value: Option<&Value>) -> Vec<Share> {
    let mut shares: Vec<Share> = value
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(name, count_value)| {
                    Some(Share {
                        name: name.clone(),
                        count: count(count_value)?,
                    })
                })
                .filter(|share| share.count > 0)
                .collect()
        })
        .unwrap_or_default();
    shares.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then(left.name.cmp(&right.name))
    });
    shares
}

/// Reads a link profile answer.
pub fn summary_from_result(result: &[Value]) -> Summary {
    let first = result.first().unwrap_or(&Value::Null);
    Summary {
        target: text(first, "target").unwrap_or_default(),
        rank: small(first, "rank"),
        backlinks: whole(first, "backlinks"),
        referring_domains: whole(first, "referring_domains"),
        referring_domains_nofollow: whole(first, "referring_domains_nofollow"),
        referring_main_domains: whole(first, "referring_main_domains"),
        referring_ips: whole(first, "referring_ips"),
        referring_subnets: whole(first, "referring_subnets"),
        referring_pages: whole(first, "referring_pages"),
        broken_backlinks: whole(first, "broken_backlinks"),
        broken_pages: whole(first, "broken_pages"),
        spam_score: small(first, "backlinks_spam_score"),
        first_seen: text(first, "first_seen"),
        attributes: shares(first.get("referring_links_attributes")),
        link_types: shares(first.get("referring_links_types")),
        top_level_domains: shares(first.get("referring_links_tld")),
        countries: shares(first.get("referring_links_countries")),
        platforms: shares(first.get("referring_links_platform_types")),
    }
}

/// Reads a backlink list answer.
pub fn backlinks_from_result(result: &[Value]) -> Vec<BacklinkRow> {
    items_of(result)
        .iter()
        .filter_map(|item| {
            Some(BacklinkRow {
                url_from: text(item, "url_from")?,
                domain_from: text(item, "domain_from"),
                title: text(item, "page_from_title"),
                url_to: text(item, "url_to"),
                url_to_status: small(item, "url_to_status_code"),
                anchor: text(item, "anchor").or_else(|| text(item, "alt")),
                dofollow: flag(item, "dofollow"),
                kind: text(item, "item_type"),
                page_rank: small(item, "page_from_rank"),
                domain_rank: small(item, "domain_from_rank"),
                spam_score: small(item, "backlink_spam_score"),
                first_seen: text(item, "first_seen"),
                last_seen: text(item, "last_seen"),
                is_new: flag(item, "is_new"),
                is_lost: flag(item, "is_lost"),
                is_broken: flag(item, "is_broken"),
                country: text(item, "domain_from_country"),
            })
        })
        .collect()
}

/// Reads a referring domain answer.
pub fn referring_domains_from_result(result: &[Value]) -> Vec<ReferringDomainRow> {
    items_of(result)
        .iter()
        .filter_map(|item| {
            let backlinks = whole(item, "backlinks");
            Some(ReferringDomainRow {
                domain: text(item, "domain")?,
                rank: small(item, "rank"),
                backlinks,
                nofollow_only: whole(item, "referring_pages") > 0
                    && whole(item, "referring_pages_nofollow") >= whole(item, "referring_pages"),
                spam_score: small(item, "backlinks_spam_score"),
                first_seen: text(item, "first_seen"),
            })
        })
        .collect()
}

/// Reads an anchor answer.
pub fn anchors_from_result(result: &[Value]) -> Vec<AnchorRow> {
    items_of(result)
        .iter()
        .map(|item| AnchorRow {
            anchor: text(item, "anchor").unwrap_or_default(),
            backlinks: whole(item, "backlinks"),
            referring_domains: whole(item, "referring_domains"),
            referring_domains_nofollow: whole(item, "referring_domains_nofollow"),
            first_seen: text(item, "first_seen"),
        })
        .collect()
}

/// Reads a linked pages answer.
pub fn pages_from_result(result: &[Value]) -> Vec<LinkedPageRow> {
    items_of(result)
        .iter()
        .filter_map(|item| {
            let summary = item.get("page_summary").unwrap_or(&Value::Null);
            Some(LinkedPageRow {
                url: text(item, "page")?,
                title: item
                    .pointer("/meta/title")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                status: small(item, "status_code"),
                rank: small(summary, "rank"),
                backlinks: whole(summary, "backlinks"),
                referring_domains: whole(summary, "referring_domains"),
                broken_backlinks: whole(summary, "broken_backlinks"),
            })
        })
        .collect()
}

/// Reads a won-and-lost answer, oldest first.
pub fn new_lost_from_result(result: &[Value]) -> Vec<NewLostPoint> {
    let mut points: Vec<NewLostPoint> = items_of(result)
        .iter()
        .filter_map(|item| {
            Some(NewLostPoint {
                date: text(item, "date")?.chars().take(10).collect(),
                new_backlinks: whole(item, "new_backlinks"),
                lost_backlinks: whole(item, "lost_backlinks"),
                new_referring_domains: whole(item, "new_referring_domains"),
                lost_referring_domains: whole(item, "lost_referring_domains"),
            })
        })
        .collect();
    points.sort_by(|left, right| left.date.cmp(&right.date));
    points
}

/// Reads a bulk rank answer.
pub fn bulk_ranks_from_result(result: &[Value]) -> Vec<RankRow> {
    items_of(result)
        .iter()
        .filter_map(|item| {
            Some(RankRow {
                target: text(item, "target")?,
                rank: small(item, "rank"),
            })
        })
        .collect()
}

/// Estimated cost of a Backlinks request returning up to `rows` rows.
pub fn estimate(rows: u32) -> f64 {
    prices::backlinks(rows)
}
