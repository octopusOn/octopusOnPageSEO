use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// OctopusOn SERP Checker — search results from Google, Bing and Yandex, where a site ranks,
/// and how hard a keyword is.
#[derive(Debug, Parser)]
#[command(name = "octopuson-serp", version, about, long_about = None)]
pub(crate) struct Cli {
    /// Data directory (defaults to the platform application data directory).
    #[arg(
        long,
        global = true,
        env = "OCTOPUSSEO_SERP_DATA_DIR",
        value_name = "DIR"
    )]
    pub(crate) data_dir: Option<PathBuf>,

    /// Do not print progress.
    #[arg(short, long, global = true)]
    pub(crate) quiet: bool,

    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Search keywords and store the result pages.
    Search(SearchArgs),
    /// Show what a search would cost on each provider of the chain.
    Estimate(QueryArgs),
    /// Analyse a keyword: search, crawl the top results, score pages, sites and difficulty.
    Analyze(AnalyzeArgs),
    /// List stored analyses, newest first.
    Analyses {
        /// Only this keyword.
        #[arg(long)]
        keyword: Option<String>,
        /// Most analyses listed.
        #[arg(long, default_value_t = 50)]
        limit: usize,
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Print a stored analysis.
    Analysis {
        /// Analysis id.
        id: i64,
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Score a stored analysis again with the current models (no new requests).
    Rescore {
        /// Analysis id.
        id: i64,
    },
    /// List stored snapshots, newest first.
    Snapshots {
        /// Only this keyword.
        #[arg(long)]
        keyword: Option<String>,
        /// Only this engine.
        #[arg(long, value_enum)]
        engine: Option<EngineArg>,
        /// Most snapshots listed.
        #[arg(long, default_value_t = 50)]
        limit: usize,
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Print a stored snapshot.
    Show {
        /// Snapshot id.
        id: i64,
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Delete a stored snapshot and its raw responses.
    Delete {
        /// Snapshot id.
        id: i64,
    },
    /// Read a stored snapshot again from its raw responses with the current selectors (after
    /// a parser override or provider spec was fixed); rank checks that used it are corrected.
    Reparse {
        /// Snapshot ids.
        #[arg(required = true)]
        ids: Vec<i64>,
    },
    /// Write a rank report or an analysis to a .csv, .json or .xlsx file.
    #[command(subcommand)]
    Export(ExportCommand),
    /// Rank tracking projects.
    #[command(subcommand)]
    Project(ProjectCommand),
    /// Check the rankings of a project (or of every project that is due).
    Track {
        /// Project id; omit with --due.
        id: Option<i64>,
        /// Check every project whose schedule says a check is due.
        #[arg(long)]
        due: bool,
        /// Run even when the estimated cost is above the confirmation limit.
        #[arg(short, long)]
        yes: bool,
    },
    /// Manage SERP providers, their keys, chains and budgets.
    #[command(subcommand)]
    Providers(ProvidersCommand),
    /// Your OctopusOn SEO account: paid sources on credits instead of your own keys.
    #[command(subcommand)]
    Account(AccountCommand),
    /// Age, link authority and popularity of domains.
    Domain {
        /// Domains or hosts.
        #[arg(required = true)]
        domains: Vec<String>,
        /// Look up again instead of using cached facts.
        #[arg(long)]
        refresh: bool,
        /// Also ask DataForSEO for backlink summaries (paid).
        #[arg(long)]
        backlinks: bool,
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Optional datasets.
    #[command(subcommand)]
    Datasets(DatasetsCommand),
    /// Requests and spending per provider and month.
    Usage {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ScheduleArg {
    Daily,
    Weekly,
    Manual,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ProjectCommand {
    /// Create a project tracking a site on one engine and locale (add more with `track-on`).
    Create {
        /// Name.
        name: String,
        /// The site: domain, host or URL prefix.
        #[arg(long)]
        site: String,
        /// How `--site` is matched.
        #[arg(long, value_enum, default_value_t = MatchArg::Domain)]
        match_mode: MatchArg,
        /// Engine.
        #[arg(long, value_enum, default_value_t = EngineArg::Google)]
        engine: EngineArg,
        /// Country code.
        #[arg(long, default_value = "tr")]
        country: String,
        /// Language code.
        #[arg(long, default_value = "tr")]
        language: String,
        /// Device.
        #[arg(long, value_enum, default_value_t = DeviceArg::Desktop)]
        device: DeviceArg,
        /// Depth.
        #[arg(long, default_value_t = 20)]
        depth: u16,
        /// Schedule.
        #[arg(long, value_enum, default_value_t = ScheduleArg::Daily)]
        schedule: ScheduleArg,
        /// Providers to use instead of the engine's chain (repeatable).
        #[arg(long = "provider", value_name = "ID")]
        providers: Vec<String>,
    },
    /// List projects.
    List {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Print a project's rankings.
    Show {
        /// Project id.
        id: i64,
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Delete a project and its rank history.
    Delete {
        /// Project id.
        id: i64,
    },
    /// Add keywords.
    AddKeywords {
        /// Project id.
        id: i64,
        /// Keywords.
        keywords: Vec<String>,
        /// Read keywords from a file, one per line.
        #[arg(long)]
        file: Option<PathBuf>,
        /// Tags of the keywords (repeatable).
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// Remove a keyword and its history.
    RemoveKeyword {
        /// Project id.
        id: i64,
        /// Keyword id (see `project show`).
        keyword: i64,
    },
    /// Set the competitors whose share is reported.
    Competitors {
        /// Project id.
        id: i64,
        /// Domains.
        domains: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum DatasetsCommand {
    /// Show the Tranco list's licence and status, or download it with --accept-license.
    Tranco {
        /// Accept the licence notice and download the latest list.
        #[arg(long)]
        accept_license: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum EngineArg {
    Google,
    Bing,
    Yandex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum DeviceArg {
    Desktop,
    Mobile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum MatchArg {
    /// Any host of the domain.
    Domain,
    /// Exactly the host (ignoring `www.`).
    Host,
    /// URLs starting with the address.
    Prefix,
}

/// What to search.
#[derive(Debug, Args)]
pub(crate) struct QueryArgs {
    /// Keywords.
    #[arg(required = true, value_name = "KEYWORD")]
    pub(crate) keywords: Vec<String>,
    /// Search engine (default: google).
    #[arg(long, value_enum)]
    pub(crate) engine: Option<EngineArg>,
    /// Country code (default from settings, `tr`).
    #[arg(long)]
    pub(crate) country: Option<String>,
    /// Interface language code (default from settings, `tr`).
    #[arg(long)]
    pub(crate) language: Option<String>,
    /// Device.
    #[arg(long, value_enum)]
    pub(crate) device: Option<DeviceArg>,
    /// Deepest position wanted (10, 20, ... 100).
    #[arg(long)]
    pub(crate) depth: Option<u16>,
    /// Location name for providers that take one (`Istanbul, Turkey`).
    #[arg(long)]
    pub(crate) location: Option<String>,
    /// Google canonical location name (`Istanbul,Istanbul,Turkey`).
    #[arg(long)]
    pub(crate) google_location: Option<String>,
    /// Yandex region id (`11508` for Istanbul).
    #[arg(long)]
    pub(crate) lr: Option<u32>,
    /// Providers to try in order, instead of the engine's chain (repeatable).
    #[arg(long = "provider", value_name = "ID")]
    pub(crate) providers: Vec<String>,
}

/// A search.
#[derive(Debug, Args)]
pub(crate) struct SearchArgs {
    #[command(flatten)]
    pub(crate) query: QueryArgs,
    /// Report where this domain, host or URL prefix ranks.
    #[arg(long, value_name = "SITE")]
    pub(crate) site: Option<String>,
    /// How `--site` is matched.
    #[arg(long, value_enum, default_value_t = MatchArg::Domain)]
    pub(crate) match_mode: MatchArg,
    /// Run even when the estimated cost is above the confirmation limit.
    #[arg(short, long)]
    pub(crate) yes: bool,
    /// Print as JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

/// A keyword analysis.
#[derive(Debug, Args)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct AnalyzeArgs {
    #[command(flatten)]
    pub(crate) query: QueryArgs,
    /// Your site (domain, host or URL prefix) for opportunity and gaps.
    #[arg(long, value_name = "SITE")]
    pub(crate) site: Option<String>,
    /// How `--site` is matched.
    #[arg(long, value_enum, default_value_t = MatchArg::Domain)]
    pub(crate) match_mode: MatchArg,
    /// Your page to compare when it does not rank yet.
    #[arg(long, value_name = "URL")]
    pub(crate) page: Option<String>,
    /// Results to crawl (1–20).
    #[arg(long, default_value_t = 10)]
    pub(crate) pages: u16,
    /// Run JavaScript in a local browser before reading pages.
    #[arg(long)]
    pub(crate) javascript: bool,
    /// Ask Wordstat or DataForSEO for monthly searches.
    #[arg(long)]
    pub(crate) volume: bool,
    /// Ask DataForSEO for backlink summaries (paid).
    #[arg(long)]
    pub(crate) backlinks: bool,
    /// Run even when the estimated cost is above the confirmation limit.
    #[arg(short, long)]
    pub(crate) yes: bool,
    /// Print as JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Subcommand)]
pub(crate) enum AccountCommand {
    /// Connect this computer: the account site opens in your browser and comes straight back
    /// when you are signed in there. Every OctopusOn SEO program on the computer is connected.
    Login {
        /// Sign in with a code instead, approved on any device (for machines without a
        /// browser, such as over SSH).
        #[arg(long)]
        code: bool,
    },
    /// Disconnect this computer from the account.
    Logout,
    /// Show the account and its balance.
    Status {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ProvidersCommand {
    /// Choose how a built-in source is paid: `auto` (your key if set, else credits), `own` or
    /// `credit` (OctopusOn SEO credits; sign in with `octopuson-serp account login`).
    Mode {
        /// Provider id (serper, serpapi, dataforseo, yandex-cloud).
        provider: String,
        /// auto, own or credit.
        mode: String,
    },
    /// List providers and whether they are set up.
    List {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Save a secret; the value is read from standard input.
    SetKey {
        /// Provider id.
        provider: String,
        /// Secret name (see `providers list`).
        name: String,
    },
    /// Remove a secret.
    RemoveKey {
        /// Provider id.
        provider: String,
        /// Secret name.
        name: String,
    },
    /// Add or replace custom providers from a JSON file.
    Add {
        /// Spec file (`{"providers": [...]}`, an array or one spec).
        file: PathBuf,
    },
    /// Remove a custom provider and its secrets.
    Remove {
        /// Provider id.
        id: String,
    },
    /// Write custom provider specs as JSON (no secrets).
    Export {
        /// Only these providers.
        ids: Vec<String>,
        /// Write to this file instead of standard output.
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
    /// Read a saved response with a spec, without sending a request.
    Test {
        /// Spec file.
        spec: PathBuf,
        /// Response body file.
        #[arg(long)]
        response: PathBuf,
        /// Engine the response is from.
        #[arg(long, value_enum, default_value_t = EngineArg::Google)]
        engine: EngineArg,
        /// URL the response came from (for relative links).
        #[arg(long)]
        url: Option<String>,
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Show or set the providers an engine tries, in order.
    Chain {
        /// Engine.
        #[arg(value_enum)]
        engine: EngineArg,
        /// New order; omit to show the current one.
        ids: Vec<String>,
    },
    /// Switch a provider on.
    Enable {
        /// Provider id.
        id: String,
    },
    /// Switch a provider off.
    Disable {
        /// Provider id.
        id: String,
    },
    /// Set a provider's monthly budget in US dollars (`none` removes it).
    Budget {
        /// Provider id.
        id: String,
        /// Budget, or `none`.
        usd: String,
    },
}

/// What to export.
#[derive(Debug, Subcommand)]
pub(crate) enum ExportCommand {
    /// The latest positions of a project's keywords on every tracker.
    Rank {
        /// Project id.
        project: i64,
        /// Output file; its extension (.csv, .json, .xlsx) picks the format.
        output: PathBuf,
    },
    /// The ranking pages of an analysis with their scores and site facts.
    Analysis {
        /// Analysis id.
        id: i64,
        /// Output file; its extension (.csv, .json, .xlsx) picks the format.
        output: PathBuf,
    },
}
