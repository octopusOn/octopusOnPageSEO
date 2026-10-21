//! Gap analysis: the keywords competitors rank for and a site does not (or ranks worse for),
//! and the sites linking to competitors but not to it.
//!
//! Keyword gaps come from DataForSEO Labs `domain_intersection`, two domains per task: one task
//! per competitor, merged here. Link gaps come from the Backlinks `domain_intersection`, which
//! takes up to 20 targets and the sites to leave out.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::keywords::keyword_data;
use crate::{DfsError, Market, count, items_of, number, target_domain};

/// Endpoint of the keywords two domains share or do not share.
pub const KEYWORD_INTERSECTION: &str = "dataforseo_labs/google/domain_intersection/live";
/// Endpoint of the sites linking to several targets.
pub const LINK_INTERSECTION: &str = "backlinks/domain_intersection/live";

/// Most competitors one gap compares.
pub const MAX_COMPETITORS: usize = 5;

/// Where a domain ranks for a keyword.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Ranking {
    /// The domain.
    pub domain: String,
    /// Position among organic results.
    pub position: Option<u32>,
    /// The ranking page.
    pub url: Option<String>,
    /// Estimated monthly traffic from the keyword.
    pub traffic: f64,
}

/// A keyword of a gap, with where the site and its competitors rank.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GapKeyword {
    /// The keyword.
    pub keyword: String,
    /// Monthly searches.
    pub volume: Option<u64>,
    /// Keyword difficulty, 0–100.
    pub difficulty: Option<u32>,
    /// Cost per click, US dollars.
    pub cpc: Option<f64>,
    /// Main search intent.
    pub intent: Option<String>,
    /// Where the site ranks; `None` for a keyword it lacks.
    pub own: Option<Ranking>,
    /// Where the competitors rank, best first.
    pub competitors: Vec<Ranking>,
    /// The site ranks, but below the best competitor.
    pub weak: bool,
}

/// A site linking to competitors but not to the target.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LinkGapRow {
    /// The linking site.
    pub domain: String,
    /// Its authority, 0–100.
    pub rank: Option<u32>,
    /// Backlinks it gives each competitor, by competitor.
    pub backlinks: BTreeMap<String, u64>,
}

/// A gap as pages get it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KeywordGap {
    /// The site.
    pub own: String,
    /// The competitors compared.
    pub competitors: Vec<String>,
    /// The keywords.
    pub keywords: Vec<GapKeyword>,
}

/// Domains as the intersections take them, without the site itself or duplicates, at most
/// [`MAX_COMPETITORS`].
///
/// # Errors
/// [`DfsError::Config`] without a competitor.
pub fn competitors(own: &str, candidates: &[String]) -> Result<Vec<String>, DfsError> {
    let own = target_domain(own);
    let mut clean: Vec<String> = Vec::new();
    for candidate in candidates {
        let domain = target_domain(candidate);
        if !domain.is_empty() && domain != own && !clean.contains(&domain) {
            clean.push(domain);
        }
    }
    clean.truncate(MAX_COMPETITORS);
    if clean.is_empty() {
        return Err(DfsError::Config(
            "competitors: choose at least one competitor".to_owned(),
        ));
    }
    Ok(clean)
}

/// The task of the keywords `first` ranks for: shared with `second` (`shared`) or that `second`
/// lacks, most searched first.
///
/// # Errors
/// [`DfsError::Config`] for an empty domain or an unknown country.
pub fn keyword_intersection_task(
    first: &str,
    second: &str,
    market: &Market,
    shared: bool,
    limit: u32,
) -> Result<Value, DfsError> {
    let (first, second) = (target_domain(first), target_domain(second));
    if first.is_empty() || second.is_empty() {
        return Err(DfsError::Config("the domain is empty".to_owned()));
    }
    let mut task = market.fields()?;
    task["target1"] = json!(first);
    task["target2"] = json!(second);
    task["intersections"] = json!(shared);
    task["item_types"] = json!(["organic"]);
    task["limit"] = json!(crate::site::rows(limit));
    task["order_by"] = json!(["keyword_data.keyword_info.search_volume,desc"]);
    Ok(task)
}

#[allow(clippy::cast_possible_truncation)]
fn ranking(domain: &str, element: Option<&Value>) -> Option<Ranking> {
    let element = element.filter(|element| element.is_object())?;
    Some(Ranking {
        domain: domain.to_owned(),
        position: element
            .get("rank_group")
            .and_then(count)
            .map(|n| n.min(u64::from(u32::MAX)) as u32),
        url: element
            .get("url")
            .and_then(Value::as_str)
            .map(str::to_owned),
        traffic: element.get("etv").and_then(number).unwrap_or_default(),
    })
}

/// One intersection answer: per keyword, its numbers and the two domains' rankings.
fn intersection(
    first: &str,
    second: &str,
    result: &[Value],
) -> Vec<(GapKeyword, Option<Ranking>, Option<Ranking>)> {
    items_of(result)
        .iter()
        .filter_map(|item| {
            let data = keyword_data(item.get("keyword_data")?)?;
            let keyword = GapKeyword {
                keyword: data.keyword,
                volume: data.volume,
                difficulty: data.difficulty,
                cpc: data.cpc,
                intent: data.intent,
                ..GapKeyword::default()
            };
            Some((
                keyword,
                ranking(first, item.get("first_domain_serp_element")),
                ranking(second, item.get("second_domain_serp_element")),
            ))
        })
        .collect()
}

fn sort_rankings(rankings: &mut [Ranking]) {
    rankings.sort_by_key(|ranking| ranking.position.unwrap_or(u32::MAX));
}

/// The keywords the competitors rank for and `own` does not, merged from one answer per
/// competitor (asked with the competitor first and `intersections: false`): those more
/// competitors share first, then the most searched.
pub fn missing(own: &str, answers: &[(String, Vec<Value>)]) -> KeywordGap {
    let mut merged: BTreeMap<String, GapKeyword> = BTreeMap::new();
    for (competitor, result) in answers {
        for (keyword, theirs, _) in intersection(competitor, own, result) {
            let entry = merged
                .entry(keyword.keyword.clone())
                .or_insert_with(|| keyword.clone());
            if let Some(theirs) = theirs {
                entry.competitors.push(theirs);
            }
        }
    }
    finish(own, answers, merged)
}

/// The keywords `own` shares with the competitors, merged from one answer per competitor
/// (asked with `own` first and `intersections: true`); `weak` where a competitor ranks higher.
pub fn shared(own: &str, answers: &[(String, Vec<Value>)]) -> KeywordGap {
    let mut merged: BTreeMap<String, GapKeyword> = BTreeMap::new();
    for (competitor, result) in answers {
        for (keyword, mine, theirs) in intersection(own, competitor, result) {
            let entry = merged
                .entry(keyword.keyword.clone())
                .or_insert_with(|| keyword.clone());
            if entry.own.is_none() {
                entry.own = mine;
            }
            if let Some(theirs) = theirs {
                entry.competitors.push(theirs);
            }
        }
    }
    for entry in merged.values_mut() {
        let mine = entry
            .own
            .as_ref()
            .and_then(|ranking| ranking.position)
            .unwrap_or(u32::MAX);
        entry.weak = entry
            .competitors
            .iter()
            .filter_map(|ranking| ranking.position)
            .any(|theirs| theirs < mine);
    }
    finish(own, answers, merged)
}

fn finish(
    own: &str,
    answers: &[(String, Vec<Value>)],
    merged: BTreeMap<String, GapKeyword>,
) -> KeywordGap {
    let mut keywords: Vec<GapKeyword> = merged.into_values().collect();
    for keyword in &mut keywords {
        sort_rankings(&mut keyword.competitors);
    }
    keywords.sort_by(|left, right| {
        right
            .competitors
            .len()
            .cmp(&left.competitors.len())
            .then(right.volume.cmp(&left.volume))
            .then(left.keyword.cmp(&right.keyword))
    });
    KeywordGap {
        own: target_domain(own),
        competitors: answers
            .iter()
            .map(|(competitor, _)| competitor.clone())
            .collect(),
        keywords,
    }
}

/// The task of the sites linking to every one of `competitors` but not to `own`, strongest
/// first.
///
/// # Errors
/// [`DfsError::Config`] without competitors.
pub fn link_gap_task(own: &str, competitors: &[String], limit: u32) -> Result<Value, DfsError> {
    if competitors.is_empty() {
        return Err(DfsError::Config(
            "competitors: choose at least one competitor".to_owned(),
        ));
    }
    let targets: serde_json::Map<String, Value> = competitors
        .iter()
        .enumerate()
        .map(|(index, competitor)| ((index + 1).to_string(), json!(target_domain(competitor))))
        .collect();
    Ok(json!({
        "targets": targets,
        "exclude_targets": [target_domain(own)],
        "intersection_mode": "all",
        "limit": crate::site::rows(limit),
        "order_by": ["1.rank,desc"],
        "rank_scale": "one_hundred",
        "include_subdomains": true,
    }))
}

/// Reads a link gap answer; `competitors` in the order they were asked.
#[allow(clippy::cast_possible_truncation)]
pub fn link_gap_from_result(competitors: &[String], result: &[Value]) -> Vec<LinkGapRow> {
    items_of(result)
        .iter()
        .filter_map(|item| {
            let sides = item.get("domain_intersection")?.as_object()?;
            let first = sides.get("1")?;
            let mut backlinks = BTreeMap::new();
            for (index, competitor) in competitors.iter().enumerate() {
                if let Some(side) = sides.get(&(index + 1).to_string()) {
                    backlinks.insert(
                        competitor.clone(),
                        side.get("backlinks").and_then(count).unwrap_or_default(),
                    );
                }
            }
            Some(LinkGapRow {
                domain: first.get("target")?.as_str()?.to_owned(),
                rank: first
                    .get("rank")
                    .and_then(count)
                    .map(|n| n.min(u64::from(u32::MAX)) as u32),
                backlinks,
            })
        })
        .collect()
}
