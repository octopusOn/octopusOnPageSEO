//! Two snapshots side by side: every number before and after, and what moved in between —
//! keywords won and lost, pages, referring domains, tracked keywords, audit findings, AI answers
//! and competitors. Free: it only reads what the snapshots keep.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::{
    AuditPoint, DomainPoint, KeywordPoint, LinkAuditPoint, LinkPoint, LocalPoint, Metrics,
    SnapshotData,
};

/// Rows of each moved list.
const LISTED: usize = 50;

/// Pages listed each way.
const PAGES_LISTED: usize = 20;

/// Which snapshot a comparison side is.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Side {
    /// Its id.
    pub id: i64,
    /// A name given to it.
    pub label: Option<String>,
    /// When it was taken, RFC 3339.
    pub taken_at: String,
}

/// One number before and after.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Kpi {
    /// `traffic`, `keywords`, `oa` …
    pub key: &'static str,
    /// `organic`, `links`, `rank`, `audit`, `ai`, `gsc`.
    pub group: &'static str,
    /// Before.
    pub before: Option<f64>,
    /// After.
    pub after: Option<f64>,
    /// After less before, when both are known.
    pub change: Option<f64>,
    /// The change in percent of before, when before is not 0.
    pub percent: Option<f64>,
    /// Higher is better (false for positions and findings).
    pub higher_better: bool,
}

impl Kpi {
    fn new(
        key: &'static str,
        group: &'static str,
        before: Option<f64>,
        after: Option<f64>,
        higher_better: bool,
    ) -> Self {
        let change = before.zip(after).map(|(before, after)| after - before);
        let percent = before
            .zip(change)
            .filter(|(before, _)| before.abs() > f64::EPSILON)
            .map(|(before, change)| change / before.abs() * 100.0);
        Self {
            key,
            group,
            before,
            after,
            change,
            percent,
            higher_better,
        }
    }

    /// Whether it moved the good way (`None` when it did not move or is unknown).
    pub fn good(&self) -> Option<bool> {
        let change = self.change.filter(|change| change.abs() > f64::EPSILON)?;
        Some((change > 0.0) == self.higher_better)
    }
}

/// A keyword's move.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct KeywordMove {
    /// The keyword.
    pub keyword: String,
    /// Before.
    pub before: Option<u32>,
    /// After.
    pub after: Option<u32>,
    /// Places gained (positive) or lost.
    pub change: Option<i64>,
    /// Traffic before.
    pub traffic_before: f64,
    /// Traffic after.
    pub traffic_after: f64,
    /// The address.
    pub url: Option<String>,
}

/// Ranked keywords between the two (among each snapshot's first 1,000).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Keywords {
    /// How many were won.
    pub gained: usize,
    /// How many were lost.
    pub lost: usize,
    /// How many moved up.
    pub improved: usize,
    /// How many moved down.
    pub declined: usize,
    /// Keywords that entered the first ten.
    pub entered_top10: usize,
    /// Keywords that left the first ten.
    pub left_top10: usize,
    /// The largest, by traffic.
    pub new_keywords: Vec<KeywordMove>,
    /// Keywords lost, by traffic.
    pub lost_keywords: Vec<KeywordMove>,
    /// Keywords that moved up, by traffic won.
    pub improved_keywords: Vec<KeywordMove>,
    /// Keywords that moved down, by traffic lost.
    pub declined_keywords: Vec<KeywordMove>,
}

/// A page's traffic before and after.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PageMove {
    /// The address.
    pub url: String,
    /// Before.
    pub before: f64,
    /// After.
    pub after: f64,
    /// The change.
    pub change: f64,
}

/// Pages that moved most.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pages {
    /// Pages that gained most.
    pub up: Vec<PageMove>,
    /// Pages that lost most.
    pub down: Vec<PageMove>,
}

/// Referring domains won and lost (among each snapshot's first 1,000).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Domains {
    /// How many were won.
    pub gained: usize,
    /// How many were lost.
    pub lost: usize,
    /// The strongest, by authority.
    pub gained_domains: Vec<DomainPoint>,
    /// Domains lost, by authority.
    pub lost_domains: Vec<DomainPoint>,
}

/// A tracked keyword's move.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrackedMove {
    /// The keyword.
    pub keyword: String,
    /// Before.
    pub before: Option<u32>,
    /// After.
    pub after: Option<u32>,
    /// The change.
    pub change: Option<i64>,
}

/// Rank tracking between the two.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tracked {
    /// How many moved up.
    pub improved: usize,
    /// How many moved down.
    pub declined: usize,
    /// The largest moves.
    pub moves: Vec<TrackedMove>,
}

/// A check's findings before and after.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IssueMove {
    /// Its id.
    pub id: String,
    /// Its title.
    pub title: String,
    /// Before.
    pub before: f64,
    /// After.
    pub after: f64,
}

/// The site audit between the two.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Audit {
    /// Found before, gone now.
    pub fixed: Vec<IssueMove>,
    /// Not found before, found now.
    pub new_issues: Vec<IssueMove>,
    /// Checks with fewer findings.
    pub better: Vec<IssueMove>,
    /// Checks with more findings.
    pub worse: Vec<IssueMove>,
}

/// One question to one assistant whose answer changed for the brand.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CellMove {
    /// The question.
    pub prompt: String,
    /// The assistant.
    pub engine: String,
    /// Before.
    pub before: Option<f64>,
    /// After.
    pub after: Option<f64>,
}

/// A brand's share before and after.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BrandMove {
    /// Its key.
    pub key: String,
    /// Whether it is the project's own.
    pub own: bool,
    /// Before.
    pub before: f64,
    /// After.
    pub after: f64,
}

/// AI visibility between the two.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ai {
    /// Answers naming the brand now and not before.
    pub gained: Vec<CellMove>,
    /// Answers that named it before and do not now.
    pub lost: Vec<CellMove>,
    /// Brands.
    pub brands: Vec<BrandMove>,
}

/// A map keyword's place before and after.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LocalMove {
    /// The keyword.
    pub keyword: String,
    /// Average map rank before.
    pub arp_before: Option<f64>,
    /// Average map rank after.
    pub arp_after: Option<f64>,
    /// Share of local voice before.
    pub solv_before: f64,
    /// Share of local voice after.
    pub solv_after: f64,
}

/// Local SEO between the two: the keywords in both grids, the largest moves first.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Local {
    /// Keywords whose share of local voice rose.
    pub improved: usize,
    /// Keywords whose share of local voice fell.
    pub declined: usize,
    /// Every keyword in both.
    pub keywords: Vec<LocalMove>,
}

/// The toxic-link audit between the two.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LinkAudit {
    /// High-risk sites found now and not before.
    pub new_risky: Vec<String>,
    /// High-risk sites before that are gone or disavowed now.
    pub cleared: Vec<String>,
}

/// A site's numbers in both snapshots: the project's own first, then the competitors kept in both.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SiteRow {
    /// The domain.
    pub domain: String,
    /// Whether it is the project's own.
    pub own: bool,
    /// Numbers before and after.
    pub kpis: Vec<Kpi>,
}

/// Two snapshots compared.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Comparison {
    /// The earlier snapshot.
    pub from: Side,
    /// The later snapshot.
    pub to: Side,
    /// Days between them.
    pub days: i64,
    /// Numbers before and after.
    pub kpis: Vec<Kpi>,
    /// Keywords.
    pub keywords: Option<Keywords>,
    /// Pages.
    pub pages: Option<Pages>,
    /// Referring domains.
    pub domains: Option<Domains>,
    /// Rank tracking.
    pub tracked: Option<Tracked>,
    /// The site audit.
    pub audit: Option<Audit>,
    /// AI visibility.
    pub ai: Option<Ai>,
    /// Local SEO.
    #[serde(default)]
    pub local: Option<Local>,
    /// The toxic-link audit.
    #[serde(default)]
    pub link_audit: Option<LinkAudit>,
    /// The project's and the competitors' numbers.
    pub sites: Vec<SiteRow>,
}

fn organic(data: &SnapshotData) -> Option<&Metrics> {
    data.overview.as_ref()
}

fn links(data: &SnapshotData) -> Option<&LinkPoint> {
    data.links.as_ref()
}

/// The numbers compared, by group.
#[allow(clippy::too_many_lines)] // one row per number
fn kpis(from: &SnapshotData, to: &SnapshotData) -> Vec<Kpi> {
    type Pick = fn(&SnapshotData) -> Option<f64>;
    let rows: [(&str, &str, Pick, bool); 25] = [
        (
            "traffic",
            "organic",
            |d| organic(d).map(|m| m.traffic),
            true,
        ),
        (
            "traffic_value",
            "organic",
            |d| organic(d).map(|m| m.traffic_value_usd),
            true,
        ),
        (
            "keywords",
            "organic",
            |d| organic(d).map(|m| m.keywords),
            true,
        ),
        ("top3", "organic", |d| organic(d).map(Metrics::top3), true),
        ("top10", "organic", |d| organic(d).map(Metrics::top10), true),
        ("oa", "links", |d| links(d).and_then(|l| l.rank), true),
        (
            "backlinks",
            "links",
            |d| links(d).map(|l| l.backlinks),
            true,
        ),
        (
            "referring_domains",
            "links",
            |d| links(d).map(|l| l.referring_domains),
            true,
        ),
        (
            "visibility",
            "rank",
            |d| d.rank.as_ref().and_then(|r| r.visibility),
            true,
        ),
        (
            "average_position",
            "rank",
            |d| d.rank.as_ref().and_then(|r| r.average_position),
            false,
        ),
        (
            "tracked_top10",
            "rank",
            |d| d.rank.as_ref().map(|r| r.top10),
            true,
        ),
        (
            "health",
            "audit",
            |d| d.audit.as_ref().and_then(|a| a.health),
            true,
        ),
        (
            "geo",
            "audit",
            |d| d.audit.as_ref().and_then(|a| a.geo),
            true,
        ),
        (
            "issues",
            "audit",
            |d| d.audit.as_ref().map(|a| a.issues),
            false,
        ),
        (
            "mention_rate",
            "ai",
            |d| d.ai.as_ref().map(|a| a.mention_rate),
            true,
        ),
        (
            "citation_rate",
            "ai",
            |d| d.ai.as_ref().map(|a| a.citation_rate),
            true,
        ),
        (
            "share_of_voice",
            "ai",
            |d| d.ai.as_ref().map(|a| a.share_of_voice),
            true,
        ),
        ("clicks", "gsc", |d| d.gsc.as_ref().map(|g| g.clicks), true),
        (
            "impressions",
            "gsc",
            |d| d.gsc.as_ref().map(|g| g.impressions),
            true,
        ),
        ("ctr", "gsc", |d| d.gsc.as_ref().map(|g| g.ctr), true),
        (
            "gsc_position",
            "gsc",
            |d| d.gsc.as_ref().and_then(|g| g.position),
            false,
        ),
        (
            "local_solv",
            "local",
            |d| d.local.as_ref().map(|l| l.solv),
            true,
        ),
        (
            "local_arp",
            "local",
            |d| d.local.as_ref().and_then(|l| l.arp),
            false,
        ),
        (
            "local_rating",
            "local",
            |d| d.local.as_ref().and_then(|l| l.rating),
            true,
        ),
        (
            "risky_links",
            "links",
            |d| d.link_audit.as_ref().map(|a| a.high - a.disavowed),
            false,
        ),
    ];
    let mut out: Vec<Kpi> = rows
        .iter()
        .map(|(key, group, pick, higher)| Kpi::new(key, group, pick(from), pick(to), *higher))
        .filter(|kpi| kpi.before.is_some() || kpi.after.is_some())
        .collect();
    let sessions = |d: &SnapshotData| d.gsc.as_ref().and_then(|g| g.sessions);
    if sessions(from).is_some() || sessions(to).is_some() {
        out.push(Kpi::new(
            "sessions",
            "gsc",
            sessions(from),
            sessions(to),
            true,
        ));
    }
    out
}

/// Places gained: a better (smaller) position is positive.
fn places(before: Option<u32>, after: Option<u32>) -> Option<i64> {
    Some(i64::from(before?) - i64::from(after?))
}

fn in_top10(position: Option<u32>) -> bool {
    position.is_some_and(|position| position <= 10)
}

fn keywords(from: &[KeywordPoint], to: &[KeywordPoint]) -> Keywords {
    let before: HashMap<&str, &KeywordPoint> =
        from.iter().map(|k| (k.keyword.as_str(), k)).collect();
    let after: HashMap<&str, &KeywordPoint> = to.iter().map(|k| (k.keyword.as_str(), k)).collect();
    let mv = |old: Option<&KeywordPoint>, new: Option<&KeywordPoint>| {
        let keyword = new.or(old).map(|k| k.keyword.clone()).unwrap_or_default();
        KeywordMove {
            keyword,
            before: old.and_then(|k| k.position),
            after: new.and_then(|k| k.position),
            change: places(old.and_then(|k| k.position), new.and_then(|k| k.position)),
            traffic_before: old.map_or(0.0, |k| k.traffic),
            traffic_after: new.map_or(0.0, |k| k.traffic),
            url: new.or(old).and_then(|k| k.url.clone()),
        }
    };
    let mut new_keywords: Vec<KeywordMove> = to
        .iter()
        .filter(|k| !before.contains_key(k.keyword.as_str()))
        .map(|k| mv(None, Some(k)))
        .collect();
    let mut lost_keywords: Vec<KeywordMove> = from
        .iter()
        .filter(|k| !after.contains_key(k.keyword.as_str()))
        .map(|k| mv(Some(k), None))
        .collect();
    let both: Vec<KeywordMove> = to
        .iter()
        .filter_map(|k| {
            before
                .get(k.keyword.as_str())
                .map(|old| mv(Some(old), Some(k)))
        })
        .collect();
    let entered_top10 = to
        .iter()
        .filter(|k| {
            in_top10(k.position)
                && !before
                    .get(k.keyword.as_str())
                    .is_some_and(|old| in_top10(old.position))
        })
        .count();
    let left_top10 = from
        .iter()
        .filter(|k| {
            in_top10(k.position)
                && !after
                    .get(k.keyword.as_str())
                    .is_some_and(|new| in_top10(new.position))
        })
        .count();
    let mut improved_keywords: Vec<KeywordMove> = both
        .iter()
        .filter(|m| m.change.is_some_and(|c| c > 0))
        .cloned()
        .collect();
    let mut declined_keywords: Vec<KeywordMove> = both
        .iter()
        .filter(|m| m.change.is_some_and(|c| c < 0))
        .cloned()
        .collect();
    let gain = |m: &KeywordMove| m.traffic_after - m.traffic_before;
    new_keywords.sort_by(|a, b| b.traffic_after.total_cmp(&a.traffic_after));
    lost_keywords.sort_by(|a, b| b.traffic_before.total_cmp(&a.traffic_before));
    improved_keywords.sort_by(|a, b| gain(b).total_cmp(&gain(a)).then(b.change.cmp(&a.change)));
    declined_keywords.sort_by(|a, b| gain(a).total_cmp(&gain(b)).then(a.change.cmp(&b.change)));
    let counts = (
        new_keywords.len(),
        lost_keywords.len(),
        improved_keywords.len(),
        declined_keywords.len(),
    );
    for list in [
        &mut new_keywords,
        &mut lost_keywords,
        &mut improved_keywords,
        &mut declined_keywords,
    ] {
        list.truncate(LISTED);
    }
    Keywords {
        gained: counts.0,
        lost: counts.1,
        improved: counts.2,
        declined: counts.3,
        entered_top10,
        left_top10,
        new_keywords,
        lost_keywords,
        improved_keywords,
        declined_keywords,
    }
}

fn pages(from: &[super::PagePoint], to: &[super::PagePoint]) -> Pages {
    let before: HashMap<&str, f64> = from.iter().map(|p| (p.url.as_str(), p.traffic)).collect();
    let after: HashMap<&str, f64> = to.iter().map(|p| (p.url.as_str(), p.traffic)).collect();
    let urls: HashSet<&str> = before.keys().chain(after.keys()).copied().collect();
    let mut moves: Vec<PageMove> = urls
        .into_iter()
        .map(|url| {
            let (old, new) = (
                before.get(url).copied().unwrap_or(0.0),
                after.get(url).copied().unwrap_or(0.0),
            );
            PageMove {
                url: url.to_owned(),
                before: old,
                after: new,
                change: new - old,
            }
        })
        .filter(|m| m.change.abs() > f64::EPSILON)
        .collect();
    moves.sort_by(|a, b| {
        b.change
            .total_cmp(&a.change)
            .then_with(|| a.url.cmp(&b.url))
    });
    let up: Vec<PageMove> = moves
        .iter()
        .filter(|m| m.change > 0.0)
        .take(PAGES_LISTED)
        .cloned()
        .collect();
    let down: Vec<PageMove> = moves
        .iter()
        .rev()
        .filter(|m| m.change < 0.0)
        .take(PAGES_LISTED)
        .cloned()
        .collect();
    Pages { up, down }
}

fn domains(from: &[DomainPoint], to: &[DomainPoint]) -> Domains {
    let before: HashSet<&str> = from.iter().map(|d| d.domain.as_str()).collect();
    let after: HashSet<&str> = to.iter().map(|d| d.domain.as_str()).collect();
    let strongest = |list: Vec<DomainPoint>| {
        let mut list = list;
        list.sort_by(|a, b| {
            b.rank
                .unwrap_or(0.0)
                .total_cmp(&a.rank.unwrap_or(0.0))
                .then_with(|| a.domain.cmp(&b.domain))
        });
        list
    };
    let gained = strongest(
        to.iter()
            .filter(|d| !before.contains(d.domain.as_str()))
            .cloned()
            .collect(),
    );
    let lost = strongest(
        from.iter()
            .filter(|d| !after.contains(d.domain.as_str()))
            .cloned()
            .collect(),
    );
    Domains {
        gained: gained.len(),
        lost: lost.len(),
        gained_domains: gained.into_iter().take(LISTED).collect(),
        lost_domains: lost.into_iter().take(LISTED).collect(),
    }
}

fn tracked(from: &super::RankPoint, to: &super::RankPoint) -> Tracked {
    let before: HashMap<&str, Option<u32>> = from
        .keywords
        .iter()
        .map(|k| (k.keyword.as_str(), k.position))
        .collect();
    let mut moves: Vec<TrackedMove> = to
        .keywords
        .iter()
        .filter_map(|k| {
            let old = *before.get(k.keyword.as_str())?;
            Some(TrackedMove {
                keyword: k.keyword.clone(),
                before: old,
                after: k.position,
                change: places(old, k.position),
            })
        })
        .filter(|m| m.before != m.after)
        .collect();
    moves.sort_by(|a, b| {
        b.change
            .unwrap_or(0)
            .abs()
            .cmp(&a.change.unwrap_or(0).abs())
            .then_with(|| a.keyword.cmp(&b.keyword))
    });
    let improved = moves
        .iter()
        .filter(|m| m.change.is_some_and(|c| c > 0) || (m.before.is_none() && m.after.is_some()))
        .count();
    let declined = moves
        .iter()
        .filter(|m| m.change.is_some_and(|c| c < 0) || (m.before.is_some() && m.after.is_none()))
        .count();
    moves.truncate(LISTED);
    Tracked {
        improved,
        declined,
        moves,
    }
}

fn audit(from: &AuditPoint, to: &AuditPoint) -> Audit {
    let before: HashMap<&str, &super::IssuePoint> =
        from.checks.iter().map(|c| (c.id.as_str(), c)).collect();
    let after: HashMap<&str, &super::IssuePoint> =
        to.checks.iter().map(|c| (c.id.as_str(), c)).collect();
    let ids: HashSet<&str> = before.keys().chain(after.keys()).copied().collect();
    let mut result = Audit {
        fixed: Vec::new(),
        new_issues: Vec::new(),
        better: Vec::new(),
        worse: Vec::new(),
    };
    let mut ids: Vec<&str> = ids.into_iter().collect();
    ids.sort_unstable();
    for id in ids {
        let (old, new) = (before.get(id), after.get(id));
        let count = |c: Option<&&super::IssuePoint>| c.map_or(0.0, |c| c.count);
        let mv = IssueMove {
            id: id.to_owned(),
            title: new.or(old).map(|c| c.title.clone()).unwrap_or_default(),
            before: count(old),
            after: count(new),
        };
        match (mv.before > 0.0, mv.after > 0.0) {
            (true, false) => result.fixed.push(mv),
            (false, true) => result.new_issues.push(mv),
            (true, true) if mv.after < mv.before => result.better.push(mv),
            (true, true) if mv.after > mv.before => result.worse.push(mv),
            _ => {}
        }
    }
    for list in [&mut result.fixed, &mut result.new_issues] {
        list.sort_by(|a, b| b.before.max(b.after).total_cmp(&a.before.max(a.after)));
    }
    result
        .better
        .sort_by(|a, b| (b.before - b.after).total_cmp(&(a.before - a.after)));
    result
        .worse
        .sort_by(|a, b| (b.after - b.before).total_cmp(&(a.after - a.before)));
    result
}

fn ai(from: &super::AiPoint, to: &super::AiPoint) -> Ai {
    let key = |prompt: &str, engine: &str| format!("{engine}\u{1f}{prompt}");
    let before: HashMap<String, &super::CellPoint> = from
        .cells
        .iter()
        .map(|c| (key(&c.prompt, &c.engine), c))
        .collect();
    let mut gained = Vec::new();
    let mut lost = Vec::new();
    for cell in &to.cells {
        let Some(old) = before.get(&key(&cell.prompt, &cell.engine)) else {
            continue;
        };
        let mv = CellMove {
            prompt: cell.prompt.clone(),
            engine: cell.engine.clone(),
            before: old.position,
            after: cell.position,
        };
        match (old.mentioned, cell.mentioned) {
            (false, true) => gained.push(mv),
            (true, false) => lost.push(mv),
            _ => {}
        }
    }
    let shares: HashMap<&str, f64> = from
        .brands
        .iter()
        .map(|b| (b.key.as_str(), b.share))
        .collect();
    let mut brands: Vec<BrandMove> = to
        .brands
        .iter()
        .map(|b| BrandMove {
            key: b.key.clone(),
            own: b.own,
            before: shares.get(b.key.as_str()).copied().unwrap_or(0.0),
            after: b.share,
        })
        .collect();
    brands.sort_by(|a, b| b.own.cmp(&a.own).then(b.after.total_cmp(&a.after)));
    gained.truncate(LISTED);
    lost.truncate(LISTED);
    Ai {
        gained,
        lost,
        brands,
    }
}

fn local(from: &LocalPoint, to: &LocalPoint) -> Local {
    let before: HashMap<&str, &super::LocalKeywordPoint> = from
        .keywords
        .iter()
        .map(|k| (k.keyword.as_str(), k))
        .collect();
    let mut keywords: Vec<LocalMove> = to
        .keywords
        .iter()
        .filter_map(|k| {
            let old = before.get(k.keyword.as_str())?;
            Some(LocalMove {
                keyword: k.keyword.clone(),
                arp_before: old.arp,
                arp_after: k.arp,
                solv_before: old.solv,
                solv_after: k.solv,
            })
        })
        .collect();
    let moved = |m: &LocalMove| m.solv_after - m.solv_before;
    keywords.sort_by(|a, b| {
        moved(b)
            .abs()
            .total_cmp(&moved(a).abs())
            .then_with(|| a.keyword.cmp(&b.keyword))
    });
    Local {
        improved: keywords.iter().filter(|m| moved(m) > 0.0).count(),
        declined: keywords.iter().filter(|m| moved(m) < 0.0).count(),
        keywords,
    }
}

fn link_audit(from: &LinkAuditPoint, to: &LinkAuditPoint) -> LinkAudit {
    let before: HashSet<&str> = from.risky.iter().map(String::as_str).collect();
    let after: HashSet<&str> = to.risky.iter().map(String::as_str).collect();
    let mut new_risky: Vec<String> = to
        .risky
        .iter()
        .filter(|domain| !before.contains(domain.as_str()))
        .cloned()
        .collect();
    let mut cleared: Vec<String> = from
        .risky
        .iter()
        .filter(|domain| !after.contains(domain.as_str()))
        .cloned()
        .collect();
    new_risky.truncate(LISTED);
    cleared.truncate(LISTED);
    LinkAudit { new_risky, cleared }
}

/// A site's main numbers.
fn site_kpis(
    before: (Option<&Metrics>, Option<&LinkPoint>),
    after: (Option<&Metrics>, Option<&LinkPoint>),
) -> Vec<Kpi> {
    vec![
        Kpi::new(
            "traffic",
            "organic",
            before.0.map(|m| m.traffic),
            after.0.map(|m| m.traffic),
            true,
        ),
        Kpi::new(
            "keywords",
            "organic",
            before.0.map(|m| m.keywords),
            after.0.map(|m| m.keywords),
            true,
        ),
        Kpi::new(
            "top10",
            "organic",
            before.0.map(Metrics::top10),
            after.0.map(Metrics::top10),
            true,
        ),
        Kpi::new(
            "oa",
            "links",
            before.1.and_then(|l| l.rank),
            after.1.and_then(|l| l.rank),
            true,
        ),
        Kpi::new(
            "referring_domains",
            "links",
            before.1.map(|l| l.referring_domains),
            after.1.map(|l| l.referring_domains),
            true,
        ),
    ]
}

fn sites(from: &SnapshotData, to: &SnapshotData) -> Vec<SiteRow> {
    let mut rows = Vec::new();
    if (from.overview.is_some() || from.links.is_some())
        && (to.overview.is_some() || to.links.is_some())
    {
        rows.push(SiteRow {
            domain: to.domain.clone(),
            own: true,
            kpis: site_kpis(
                (from.overview.as_ref(), from.links.as_ref()),
                (to.overview.as_ref(), to.links.as_ref()),
            ),
        });
    }
    for competitor in &to.competitors {
        let Some(old) = from
            .competitors
            .iter()
            .find(|c| c.domain == competitor.domain)
        else {
            continue;
        };
        rows.push(SiteRow {
            domain: competitor.domain.clone(),
            own: false,
            kpis: site_kpis(
                (old.overview.as_ref(), old.links.as_ref()),
                (competitor.overview.as_ref(), competitor.links.as_ref()),
            ),
        });
    }
    rows
}

/// Days between two RFC 3339 times.
fn days_between(from: &str, to: &str) -> i64 {
    let seconds = |text: &str| octopuson_serp_score::rank::seconds_of(text);
    seconds(from)
        .zip(seconds(to))
        .map_or(0, |(from, to)| (to - from).div_euclid(86_400))
}

/// Compares snapshot `from` (before) with `to` (after).
pub fn compare(from: (&Side, &SnapshotData), to: (&Side, &SnapshotData)) -> Comparison {
    let (from_side, before) = from;
    let (to_side, after) = to;
    Comparison {
        from: from_side.clone(),
        to: to_side.clone(),
        days: days_between(&from_side.taken_at, &to_side.taken_at),
        kpis: kpis(before, after),
        keywords: before
            .keywords
            .as_ref()
            .zip(after.keywords.as_ref())
            .map(|(a, b)| keywords(a, b)),
        pages: before
            .pages
            .as_ref()
            .zip(after.pages.as_ref())
            .map(|(a, b)| pages(a, b)),
        domains: before
            .domains
            .as_ref()
            .zip(after.domains.as_ref())
            .map(|(a, b)| domains(a, b)),
        tracked: before
            .rank
            .as_ref()
            .zip(after.rank.as_ref())
            .map(|(a, b)| tracked(a, b)),
        audit: before
            .audit
            .as_ref()
            .zip(after.audit.as_ref())
            .map(|(a, b)| audit(a, b)),
        ai: before
            .ai
            .as_ref()
            .zip(after.ai.as_ref())
            .map(|(a, b)| ai(a, b)),
        local: before
            .local
            .as_ref()
            .zip(after.local.as_ref())
            .map(|(a, b)| local(a, b)),
        link_audit: before
            .link_audit
            .as_ref()
            .zip(after.link_audit.as_ref())
            .map(|(a, b)| link_audit(a, b)),
        sites: sites(before, after),
    }
}
