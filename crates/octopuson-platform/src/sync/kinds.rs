//! What travels between copies: every kind of record, the table it lives in and how its copies
//! are told apart.
//!
//! Kinds are listed parents first, so a first sync sends projects before what belongs to them.

/// The record a record belongs to.
pub(crate) struct Parent {
    /// The column holding the parent's id.
    pub column: &'static str,
    /// The parent's kind.
    pub kind: &'static str,
    /// The parent's table.
    pub table: &'static str,
    /// The field holding the parent's uid in a record's data.
    pub field: &'static str,
}

const PROJECT: Parent = Parent {
    column: "project_id",
    kind: "project",
    table: "projects",
    field: "project",
};

const LIST: Parent = Parent {
    column: "list_id",
    kind: "keyword_list",
    table: "keyword_lists",
    field: "list",
};

const BRIEF: Parent = Parent {
    column: "brief_id",
    kind: "content_brief",
    table: "content_briefs",
    field: "brief",
};

const REPORT: Parent = Parent {
    column: "report_id",
    kind: "report",
    table: "reports",
    field: "report",
};

/// How a record's key is found.
pub(crate) enum Key {
    /// Its `uid` column.
    Uid,
    /// Its parent's uid: one record per parent (a project's settings).
    Parent,
    /// One fixed key: the only row (the report branding).
    Fixed(&'static str),
    /// A column of its own (a cache entry's key).
    Column(&'static str),
}

/// How a missing `uid` is made.
pub(crate) enum Fill {
    /// The record has no `uid` column.
    None,
    /// A random one: copies made apart are different records.
    Random,
    /// The MD5 of this expression over the record `t` and its parent `p`: copies made apart of
    /// the same thing (a keyword tracked on two computers) are one record.
    From(&'static str),
}

/// The rows a run carries along.
pub(crate) struct Children {
    /// Their table.
    pub table: &'static str,
    /// The records they refer to: (column, kind, table), travelling as uids.
    pub refs: &'static [(&'static str, &'static str, &'static str)],
    /// Their other columns, in the order they travel.
    pub columns: &'static [&'static str],
    /// Columns holding JSON.
    pub json: &'static [&'static str],
}

/// A table of a project's Search Console and Analytics numbers.
pub(crate) struct GscTable {
    /// The field holding its rows in the data.
    pub field: &'static str,
    /// The table.
    pub table: &'static str,
    /// Its columns besides `project_id`, in the order they travel.
    pub columns: &'static [&'static str],
}

pub(crate) const GSC_TABLES: &[GscTable] = &[
    GscTable {
        field: "search",
        table: "gsc_rows",
        columns: &["kind", "period", "key", "clicks", "impressions", "position"],
    },
    GscTable {
        field: "query_pages",
        table: "gsc_query_pages",
        columns: &["query", "page", "clicks", "impressions", "position"],
    },
    GscTable {
        field: "traffic",
        table: "ga4_rows",
        columns: &[
            "kind",
            "period",
            "key",
            "sessions",
            "engaged_sessions",
            "key_events",
            "revenue",
        ],
    },
];

/// What a record is made of.
pub(crate) enum Shape {
    /// One row.
    Row,
    /// A finished run with its rows (rank checks, AI answers), which never change afterwards.
    Run(&'static Children),
    /// A project's Search Console and Analytics numbers, replaced whole by each Google sync.
    Gsc,
    /// A kept provider answer: the newer one wins, and expiry is not a change.
    Cache,
}

/// How a deletion that arrives is carried out: as the program itself deletes, so what goes
/// with the record goes on every copy.
pub(crate) enum Removal {
    /// The row (and a run's rows).
    Row,
    /// A project with everything it holds; its audits' crawls go too.
    Project,
    /// A tracked place with its checks.
    Tracker,
    /// A tracked keyword with its checks.
    Keyword,
    /// An AI question with its answers.
    Prompt,
    /// A keyword list with its keywords.
    List,
    /// A report with its deliveries.
    Report,
    /// A content brief with the pages and drafts scored against it.
    Brief,
    /// An audit; its crawl goes too.
    Audit,
    /// A Search Console snapshot: the project's numbers.
    Gsc,
    /// Never travels (cache entries expire on each copy by themselves).
    Local,
}

/// One kind of record.
pub(crate) struct Kind {
    /// Its name on the wire.
    pub name: &'static str,
    /// Its table.
    pub table: &'static str,
    /// How its key is found.
    pub key: Key,
    /// How a missing uid is made.
    pub fill: Fill,
    /// The record it belongs to.
    pub parent: Option<&'static Parent>,
    /// Whether it may have no parent (cache entries of no project).
    pub optional_parent: bool,
    /// Columns that travel (besides keys and the parent).
    pub columns: &'static [&'static str],
    /// Columns holding JSON.
    pub json: &'static [&'static str],
    /// Columns taken from the parent's row when the record is made here.
    pub inherit: &'static [&'static str],
    /// Only rows passing this condition over `t` travel (not runs still running).
    pub filter: Option<&'static str>,
    /// Times written by the program itself (last run, last sync): the later one stays,
    /// whichever copy's other fields win, so nothing due runs twice.
    pub later: &'static [&'static str],
    /// What it is made of.
    pub shape: Shape,
    /// How its deletion is carried out.
    pub removal: Removal,
}

impl Kind {
    /// Whether its copies made apart are different records: then a deletion wins over an edit
    /// made meanwhile elsewhere.
    pub(crate) const fn random(&self) -> bool {
        matches!(self.fill, Fill::Random)
    }
}

const NOT_RUNNING: Option<&str> = Some("t.status <> 'running'");

const RUN_COLUMNS: &[&str] = &[
    "scheduled",
    "status",
    "total",
    "checked",
    "failed",
    "cost_usd",
    "error",
    "started_at",
    "finished_at",
];

const RANK_CHECKS: Children = Children {
    table: "rank_checks",
    refs: &[
        ("keyword_id", "tracked_keyword", "tracked_keywords"),
        ("tracker_id", "rank_tracker", "rank_trackers"),
    ],
    columns: &[
        "checked_at",
        "position",
        "url",
        "pages",
        "features",
        "ai_overview",
        "ai_cited",
        "snippet",
        "results",
        "error",
    ],
    json: &["pages", "features", "results"],
};

const AI_ANSWERS: Children = Children {
    table: "ai_answers",
    refs: &[("prompt_id", "ai_prompt", "ai_prompts")],
    columns: &[
        "engine",
        "via",
        "model",
        "checked_at",
        "text",
        "mentioned",
        "position",
        "cited",
        "sentiment",
        "brands",
        "citations",
        "error",
    ],
    json: &["brands", "citations"],
};

const LOCAL_POINTS: Children = Children {
    table: "local_points",
    refs: &[],
    columns: &[
        "keyword",
        "grid_row",
        "grid_col",
        "lat",
        "lng",
        "position",
        "top",
        "error",
        "checked_at",
    ],
    json: &["top"],
};

/// Cache classes that travel: reports people open again elsewhere. Live result pages are
/// cheap and fill up quickly, so they stay.
pub(crate) const CACHE_CLASSES: &str = "('domain', 'backlinks', 'keywords', 'ai')";

/// Largest kept answer that travels, in bytes of JSON.
pub(crate) const CACHE_MAX: usize = 2_000_000;

pub(crate) const KINDS: &[Kind] = &[
    Kind {
        name: "project",
        table: "projects",
        key: Key::Uid,
        fill: Fill::Random,
        parent: None,
        optional_parent: false,
        columns: &[
            "name",
            "domain",
            "country",
            "language",
            "competitors",
            "brands",
            "settings",
            "created_at",
            "updated_at",
        ],
        json: &["competitors", "brands", "settings"],
        inherit: &[],
        filter: None,
        later: &[],
        shape: Shape::Row,
        removal: Removal::Project,
    },
    Kind {
        name: "rank_settings",
        table: "rank_settings",
        key: Key::Parent,
        fill: Fill::None,
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &["schedule", "depth", "ai_overview", "last_run_at"],
        json: &[],
        inherit: &[],
        filter: None,
        later: &["last_run_at"],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "audit_settings",
        table: "audit_settings",
        key: Key::Parent,
        fill: Fill::None,
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &["schedule", "max_pages", "last_run_at", "check_outbound"],
        json: &[],
        inherit: &[],
        filter: None,
        later: &["last_run_at"],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "ai_settings",
        table: "ai_settings",
        key: Key::Parent,
        fill: Fill::None,
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &["schedule", "engines", "models", "rivals", "last_run_at"],
        json: &["engines", "models", "rivals"],
        inherit: &[],
        filter: None,
        later: &["last_run_at"],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "gsc_settings",
        table: "gsc_settings",
        key: Key::Parent,
        fill: Fill::None,
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &[
            "property",
            "ga4_property",
            "ga4_name",
            "days",
            "schedule",
            "last_sync_at",
            "compare",
        ],
        json: &[],
        inherit: &[],
        filter: None,
        later: &["last_sync_at"],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "rank_tracker",
        table: "rank_trackers",
        key: Key::Uid,
        fill: Fill::From(
            "p.uid || '|' || t.engine || '|' || t.country || '|' || t.language || '|' || t.device || '|' || coalesce(t.location, '')",
        ),
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &[
            "engine",
            "country",
            "language",
            "device",
            "location",
            "created_at",
        ],
        json: &[],
        inherit: &[],
        filter: None,
        later: &[],
        shape: Shape::Row,
        removal: Removal::Tracker,
    },
    Kind {
        name: "tracked_keyword",
        table: "tracked_keywords",
        key: Key::Uid,
        fill: Fill::From("p.uid || '|' || t.keyword"),
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &["keyword", "tags", "volume", "added_at"],
        json: &["tags"],
        inherit: &[],
        filter: None,
        later: &[],
        shape: Shape::Row,
        removal: Removal::Keyword,
    },
    Kind {
        name: "ai_prompt",
        table: "ai_prompts",
        key: Key::Uid,
        fill: Fill::From("p.uid || '|' || t.prompt"),
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &["prompt", "tags", "added_at"],
        json: &["tags"],
        inherit: &[],
        filter: None,
        later: &[],
        shape: Shape::Row,
        removal: Removal::Prompt,
    },
    Kind {
        name: "keyword_list",
        table: "keyword_lists",
        key: Key::Uid,
        fill: Fill::Random,
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &["name", "created_at"],
        json: &[],
        inherit: &[],
        filter: None,
        later: &[],
        shape: Shape::Row,
        removal: Removal::List,
    },
    Kind {
        name: "list_keyword",
        table: "list_keywords",
        key: Key::Uid,
        fill: Fill::From("p.uid || '|' || t.keyword"),
        parent: Some(&LIST),
        optional_parent: false,
        columns: &["keyword", "data", "added_at"],
        json: &["data"],
        inherit: &[],
        filter: None,
        later: &[],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "branding",
        table: "report_branding",
        key: Key::Fixed("branding"),
        fill: Fill::None,
        parent: None,
        optional_parent: false,
        columns: &[
            "company",
            "logo",
            "accent",
            "website",
            "footer",
            "hide_octopuson",
        ],
        json: &[],
        inherit: &[],
        filter: Some("t.id = 1"),
        later: &[],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "report",
        table: "reports",
        key: Key::Uid,
        fill: Fill::Random,
        parent: Some(&PROJECT),
        optional_parent: false,
        // `share_views` stays where links are opened: the web.
        columns: &[
            "name",
            "language",
            "days",
            "widgets",
            "schedule",
            "recipients",
            "design",
            "colors",
            "share_token",
            "last_sent_at",
            "created_at",
            "updated_at",
        ],
        json: &["widgets", "recipients", "colors"],
        inherit: &[],
        filter: None,
        later: &["last_sent_at"],
        shape: Shape::Row,
        removal: Removal::Report,
    },
    Kind {
        name: "report_delivery",
        table: "report_deliveries",
        key: Key::Uid,
        fill: Fill::Random,
        parent: Some(&REPORT),
        optional_parent: false,
        columns: &["scheduled", "status", "recipients", "error", "sent_at"],
        json: &[],
        inherit: &["project_id"],
        filter: None,
        later: &[],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "audit",
        table: "audits",
        key: Key::Uid,
        fill: Fill::Random,
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &[
            "crawl_id",
            "scheduled",
            "status",
            "start_url",
            "pages",
            "health",
            "geo",
            "issues",
            "metered_pages",
            "charged_micros",
            "error",
            "started_at",
            "finished_at",
        ],
        json: &[],
        inherit: &[],
        filter: NOT_RUNNING,
        later: &[],
        shape: Shape::Row,
        removal: Removal::Audit,
    },
    Kind {
        name: "rank_run",
        table: "rank_runs",
        key: Key::Uid,
        fill: Fill::Random,
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: RUN_COLUMNS,
        json: &[],
        inherit: &[],
        filter: NOT_RUNNING,
        later: &[],
        shape: Shape::Run(&RANK_CHECKS),
        removal: Removal::Row,
    },
    Kind {
        name: "ai_run",
        table: "ai_runs",
        key: Key::Uid,
        fill: Fill::Random,
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: RUN_COLUMNS,
        json: &[],
        inherit: &[],
        filter: NOT_RUNNING,
        later: &[],
        shape: Shape::Run(&AI_ANSWERS),
        removal: Removal::Row,
    },
    Kind {
        name: "gsc_sync",
        table: "gsc_syncs",
        key: Key::Uid,
        fill: Fill::Random,
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &[
            "scheduled",
            "status",
            "period_start",
            "period_end",
            "rows",
            "currency",
            "limited",
            "error",
            "started_at",
            "finished_at",
        ],
        json: &[],
        inherit: &[],
        filter: NOT_RUNNING,
        later: &[],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "snapshot_settings",
        table: "snapshot_settings",
        key: Key::Parent,
        fill: Fill::None,
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &["schedule", "competitors", "last_run_at"],
        json: &["competitors"],
        inherit: &[],
        filter: None,
        later: &["last_run_at"],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "snapshot",
        table: "snapshots",
        key: Key::Uid,
        fill: Fill::Random,
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &[
            "label",
            "kind",
            "status",
            "taken_at",
            "finished_at",
            "cost_usd",
            "error",
            "data",
        ],
        json: &["data"],
        inherit: &[],
        filter: NOT_RUNNING,
        later: &[],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "link_decision",
        table: "link_decisions",
        key: Key::Uid,
        fill: Fill::From("p.uid || '|' || t.domain"),
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &["domain", "decision", "note", "decided_at"],
        json: &[],
        inherit: &[],
        filter: None,
        later: &[],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "link_prospect",
        table: "link_prospects",
        key: Key::Uid,
        fill: Fill::From("p.uid || '|' || t.domain"),
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &[
            "domain",
            "url",
            "source",
            "status",
            "note",
            "added_at",
            "updated_at",
        ],
        json: &[],
        inherit: &[],
        filter: None,
        later: &[],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "local_settings",
        table: "local_settings",
        key: Key::Parent,
        fill: Fill::None,
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &[
            "business",
            "keywords",
            "grid",
            "radius_km",
            "schedule",
            "last_run_at",
        ],
        json: &["business", "keywords"],
        inherit: &[],
        filter: None,
        later: &["last_run_at"],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "local_run",
        table: "local_runs",
        key: Key::Uid,
        fill: Fill::Random,
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &[
            "scheduled",
            "status",
            "total",
            "checked",
            "failed",
            "cost_usd",
            "error",
            "started_at",
            "finished_at",
            "grid",
            "radius_km",
            "lat",
            "lng",
        ],
        json: &[],
        inherit: &[],
        filter: NOT_RUNNING,
        later: &[],
        shape: Shape::Run(&LOCAL_POINTS),
        removal: Removal::Row,
    },
    Kind {
        name: "local_review",
        table: "local_reviews",
        key: Key::Uid,
        fill: Fill::From("p.uid || '|' || t.review_id"),
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &[
            "review_id",
            "rating",
            "text",
            "author",
            "author_url",
            "url",
            "published_at",
            "owner_answer",
            "answered_at",
            "photos",
            "local_guide",
            "first_seen",
        ],
        json: &[],
        inherit: &[],
        filter: None,
        later: &[],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "local_profile",
        table: "local_profiles",
        key: Key::Uid,
        fill: Fill::From("p.uid || '|' || t.day"),
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &["day", "rating", "votes", "profile", "taken_at"],
        json: &["profile"],
        inherit: &[],
        filter: None,
        later: &["taken_at"],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "content_brief",
        table: "content_briefs",
        key: Key::Uid,
        fill: Fill::Random,
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &[
            "keyword",
            "country",
            "language",
            "mobile",
            "status",
            "brief",
            "source",
            "ai_text",
            "ai_model",
            "cost_usd",
            "error",
            "created_at",
            "updated_at",
        ],
        json: &["brief", "source"],
        inherit: &[],
        filter: NOT_RUNNING,
        later: &[],
        shape: Shape::Row,
        removal: Removal::Brief,
    },
    Kind {
        name: "content_score",
        table: "content_scores",
        key: Key::Uid,
        fill: Fill::From("p.uid || '|' || t.kind || '|' || coalesce(t.url, '')"),
        parent: Some(&BRIEF),
        optional_parent: false,
        columns: &[
            "kind",
            "url",
            "title",
            "description",
            "body",
            "score",
            "result",
            "suggestions",
            "scored_at",
        ],
        json: &["result"],
        inherit: &["project_id"],
        filter: None,
        later: &[],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "alert_settings",
        table: "alert_settings",
        key: Key::Parent,
        fill: Fill::None,
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &[
            "enabled",
            "rules",
            "email",
            "recipients",
            "lang",
            "last_digest_at",
            "links_last_run",
        ],
        json: &["rules", "recipients"],
        inherit: &[],
        filter: None,
        later: &["last_digest_at", "links_last_run"],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "alert",
        table: "alerts",
        key: Key::Uid,
        fill: Fill::From("p.uid || '|' || t.key"),
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &[
            "key",
            "kind",
            "severity",
            "page",
            "params",
            "detail",
            "created_at",
            "read_at",
        ],
        json: &["params", "detail"],
        inherit: &[],
        filter: None,
        later: &["read_at"],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "opportunity_state",
        table: "opportunity_states",
        key: Key::Uid,
        fill: Fill::From("p.uid || '|' || t.key"),
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &["key", "state", "until", "changed_at"],
        json: &[],
        inherit: &[],
        filter: None,
        later: &[],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "vital_check",
        table: "vital_checks",
        key: Key::Uid,
        fill: Fill::From("p.uid || '|' || t.target || '|' || t.device || '|' || t.day"),
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &[
            "target",
            "scope",
            "device",
            "day",
            "field",
            "lab",
            "history",
            "checked_at",
        ],
        json: &["field", "lab", "history"],
        inherit: &[],
        filter: None,
        later: &[],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "ad_copy",
        table: "ad_copies",
        key: Key::Uid,
        fill: Fill::From(
            "p.uid || '|' || t.domain || '|' || t.title || '|' || coalesce(t.description, '')",
        ),
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &[
            "domain",
            "title",
            "description",
            "breadcrumb",
            "url",
            "keywords",
            "first_seen",
            "last_seen",
        ],
        json: &["keywords"],
        inherit: &[],
        filter: None,
        later: &["last_seen"],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "metric_point",
        table: "metric_points",
        key: Key::Uid,
        fill: Fill::From(
            "p.uid || '|' || t.target || '|' || t.kind || '|' || t.market || '|' || t.day",
        ),
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &["target", "kind", "market", "day", "metrics", "taken_at"],
        json: &["metrics"],
        inherit: &[],
        filter: None,
        later: &[],
        shape: Shape::Row,
        removal: Removal::Row,
    },
    Kind {
        name: "gsc_data",
        table: "projects",
        key: Key::Parent,
        fill: Fill::None,
        parent: Some(&PROJECT),
        optional_parent: false,
        columns: &[],
        json: &[],
        inherit: &[],
        filter: None,
        later: &[],
        shape: Shape::Gsc,
        removal: Removal::Gsc,
    },
    Kind {
        name: "cache",
        table: "cache",
        key: Key::Column("key"),
        fill: Fill::None,
        parent: Some(&PROJECT),
        optional_parent: true,
        columns: &[
            "endpoint",
            "class",
            "body",
            "fetched_at",
            "expires_at",
            "cost_usd",
            "via",
        ],
        json: &["body"],
        inherit: &[],
        filter: None,
        later: &[],
        shape: Shape::Cache,
        removal: Removal::Local,
    },
];

/// Kinds that came after sync first shipped (0.1.1). A copy that synced before it knew them
/// skipped their records, so it asks for them again once it knows them.
const LATER_KINDS: &[&str] = &[
    "snapshot_settings",
    "snapshot",
    "metric_point",
    "ad_copy",
    "link_decision",
    "link_prospect",
    "local_settings",
    "local_run",
    "local_review",
    "local_profile",
    "content_brief",
    "content_score",
    "vital_check",
    "opportunity_state",
    "alert_settings",
    "alert",
];

/// The version of what a kind carries: raised when a kind gains columns (a report's design and
/// colours), so copies that took its records without them ask for them again.
fn kind_version(name: &str) -> u32 {
    match name {
        "report" | "audit_settings" | "gsc_settings" => 2,
        _ => 1,
    }
}

/// Every kind this program syncs, with the version of what it carries.
pub fn sync_kind_versions() -> Vec<(&'static str, u32)> {
    KINDS
        .iter()
        .map(|kind| (kind.name, kind_version(kind.name)))
        .collect()
}

/// What a copy that synced before it recorded its kinds knew: every kind of the first version
/// except those that came later.
pub fn first_sync_kinds() -> Vec<(&'static str, u32)> {
    KINDS
        .iter()
        .filter(|kind| !LATER_KINDS.contains(&kind.name))
        .map(|kind| (kind.name, 1))
        .collect()
}

/// The kind named `name`.
pub(crate) fn kind_named(name: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|kind| kind.name == name)
}

/// The table of the kind named `name` that has a `uid` column.
pub(crate) fn uid_table(kind: &str) -> Option<&'static str> {
    kind_named(kind)
        .filter(|kind| !matches!(kind.fill, Fill::None))
        .map(|kind| kind.table)
}
