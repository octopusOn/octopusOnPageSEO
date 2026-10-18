use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// OctopusOn SEO Keyword Research — discover keywords, group them into clusters one page can
/// target, and map the clusters to your site.
#[derive(Debug, Parser)]
#[command(name = "octopuson-keywords", version, about, long_about = None)]
pub(crate) struct Cli {
    /// Data directory (defaults to the platform application data directory).
    #[arg(
        long,
        global = true,
        env = "OCTOPUSSEO_KW_DATA_DIR",
        value_name = "DIR"
    )]
    pub(crate) data_dir: Option<PathBuf>,

    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Create, list, show or delete projects.
    #[command(subcommand)]
    Project(ProjectCommand),
    /// Add keywords to a project (from the arguments, or one per line from standard input).
    Add {
        /// Project id.
        project: i64,
        /// Keywords; read from standard input when none are given.
        keywords: Vec<String>,
        /// Source recorded for them.
        #[arg(long, default_value = "manual")]
        source: String,
    },
    /// Import keywords from a file (Ahrefs, Semrush, Keyword Planner, Search Console,
    /// Wordstat exports; CSV, TSV, TXT, XLSX or a Search Console ZIP).
    Import {
        /// Project id.
        project: i64,
        /// The file.
        file: PathBuf,
        /// Only show how the file would be read.
        #[arg(long)]
        preview: bool,
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// List a project's keywords.
    Keywords(KeywordsArgs),
    /// Leave keywords out of clustering and exports.
    Exclude {
        /// Project id.
        project: i64,
        /// Keyword ids.
        #[arg(required = true)]
        ids: Vec<i64>,
    },
    /// Take excluded keywords back.
    Include {
        /// Project id.
        project: i64,
        /// Keyword ids.
        #[arg(required = true)]
        ids: Vec<i64>,
    },
    /// Find new keywords for seeds and competitors through the paid sources.
    Discover(DiscoverArgs),
    /// Ask volumes, difficulty or intent for a project's keywords (DataForSEO).
    Enrich(EnrichArgs),
    /// Fetch result pages of a project's keywords for clustering and mapping.
    Serp(SerpArgs),
    /// Map the newest clustering to the site again (no new requests).
    Map {
        /// Project id.
        project: i64,
    },
    /// Switch the free Google or Yandex suggestion endpoints on or off.
    Suggest(SuggestArgs),
    /// The attended browser mode for result pages: its state, settings, chains and profile.
    #[command(subcommand)]
    Browser(BrowserCommand),
    /// Group a project's keywords into clusters.
    Cluster(ClusterArgs),
    /// List the clusters of a project's newest (or a given) clustering.
    Clusters {
        /// Project id.
        project: i64,
        /// Clustering id (the newest when absent).
        #[arg(long)]
        clustering: Option<i64>,
        /// Only clusters with a keyword containing this text.
        #[arg(long)]
        search: Option<String>,
        /// Most clusters listed.
        #[arg(long, default_value_t = 50)]
        limit: u64,
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Write keywords or clusters to a .csv, .json or .xlsx file.
    #[command(subcommand)]
    Export(ExportCommand),
    /// Keys, switches and budgets of the paid sources.
    #[command(subcommand)]
    Providers(ProvidersCommand),
    /// Your Google connection: Search Console and Google Ads with your own OAuth client.
    #[command(subcommand)]
    Google(GoogleCommand),
    /// Your OctopusOn SEO account: paid sources on credits instead of your own keys.
    #[command(subcommand)]
    Account(AccountCommand),
    /// Requests and spending per provider and month.
    Usage {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub(crate) enum ProjectCommand {
    /// Create a project.
    Create(ProjectArgs),
    /// List projects.
    List {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Show a project.
    Show {
        /// Project id.
        id: i64,
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Delete a project with its keywords and clusterings.
    Delete {
        /// Project id.
        id: i64,
        /// Do not ask.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Args)]
pub(crate) struct ProjectArgs {
    /// Name.
    pub(crate) name: String,
    /// Country (ISO 3166-1 alpha-2).
    #[arg(long)]
    pub(crate) country: Option<String>,
    /// Language (ISO 639-1).
    #[arg(long)]
    pub(crate) language: Option<String>,
    /// Engines, primary first (repeat the option).
    #[arg(long = "engine", value_enum)]
    pub(crate) engines: Vec<EngineArg>,
    /// Your site.
    #[arg(long)]
    pub(crate) site: Option<String>,
    /// Competitor sites (repeat the option).
    #[arg(long = "competitor")]
    pub(crate) competitors: Vec<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum EngineArg {
    Google,
    Yandex,
    Bing,
}

impl EngineArg {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::Yandex => "yandex",
            Self::Bing => "bing",
        }
    }
}

#[derive(Debug, Args)]
pub(crate) struct KeywordsArgs {
    /// Project id.
    pub(crate) project: i64,
    /// Only keywords containing this text (case and diacritics ignored).
    #[arg(long)]
    pub(crate) search: Option<String>,
    /// Only keywords with this tag.
    #[arg(long)]
    pub(crate) tag: Option<String>,
    /// Only keywords found by this source.
    #[arg(long)]
    pub(crate) source: Option<String>,
    /// Only this intent.
    #[arg(long)]
    pub(crate) intent: Option<String>,
    /// At least these monthly searches.
    #[arg(long)]
    pub(crate) min_volume: Option<u64>,
    /// At most these monthly searches.
    #[arg(long)]
    pub(crate) max_volume: Option<u64>,
    /// Only members of this cluster of the newest clustering (as numbered in `clusters`).
    #[arg(long)]
    pub(crate) cluster: Option<u32>,
    /// List excluded keywords instead.
    #[arg(long)]
    pub(crate) excluded: bool,
    /// Order.
    #[arg(long, value_enum, default_value_t = SortArg::Volume)]
    pub(crate) sort: SortArg,
    /// Smallest first.
    #[arg(long)]
    pub(crate) ascending: bool,
    /// Most keywords listed.
    #[arg(long, default_value_t = 50)]
    pub(crate) limit: u64,
    /// Keywords skipped.
    #[arg(long, default_value_t = 0)]
    pub(crate) offset: u64,
    /// Print as JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum SortArg {
    Keyword,
    Volume,
    Cpc,
    Competition,
    Difficulty,
    Intent,
    Cluster,
    Added,
}

#[derive(Debug, Args)]
pub(crate) struct ClusterArgs {
    /// Project id.
    pub(crate) project: i64,
    /// Method (default from the settings: hybrid).
    #[arg(long, value_enum)]
    pub(crate) method: Option<MethodArg>,
    /// Least wording similarity, 0–1.
    #[arg(long)]
    pub(crate) threshold: Option<f64>,
    /// Least shared results in the top ten, 1–9.
    #[arg(long)]
    pub(crate) serp_threshold: Option<u16>,
    /// Compare result pages with the pillar only (soft) or with every member (hard).
    #[arg(long, value_enum)]
    pub(crate) mode: Option<ModeArg>,
    /// Compare result pages by page or by site.
    #[arg(long, value_enum)]
    pub(crate) level: Option<LevelArg>,
    /// Engine whose result pages are compared (default: the project's primary engine).
    #[arg(long, value_enum)]
    pub(crate) engine: Option<EngineArg>,
    /// Device whose result pages are compared.
    #[arg(long, default_value = "desktop")]
    pub(crate) device: String,
    /// Print as JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum MethodArg {
    Lexical,
    Serp,
    Hybrid,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ModeArg {
    Soft,
    Hard,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum LevelArg {
    Page,
    Site,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ExportCommand {
    /// Keywords with their numbers.
    Keywords {
        /// Project id.
        project: i64,
        /// Output file (.csv, .json or .xlsx).
        file: PathBuf,
        /// Excluded keywords instead.
        #[arg(long)]
        excluded: bool,
    },
    /// One row per keyword with its cluster's totals and mapping.
    Clusters {
        /// Project id.
        project: i64,
        /// Output file (.csv, .json or .xlsx).
        file: PathBuf,
        /// Clustering id (the newest when absent).
        #[arg(long)]
        clustering: Option<i64>,
    },
}

#[derive(Debug, Args)]
pub(crate) struct DiscoverArgs {
    /// Project id.
    pub(crate) project: i64,
    /// Seed keywords (repeat the option).
    #[arg(long = "seed")]
    pub(crate) seeds: Vec<String>,
    /// A file with one seed per line.
    #[arg(long)]
    pub(crate) seeds_file: Option<PathBuf>,
    /// Sources to ask (comma separated).
    #[arg(long = "source", value_enum, value_delimiter = ',', required = true)]
    pub(crate) sources: Vec<SourceArg>,
    /// Expansions of each seed for autocomplete (comma separated).
    #[arg(long = "expand", value_enum, value_delimiter = ',')]
    pub(crate) expansions: Vec<ExpandArg>,
    /// Competitor domains for `competitors` (the project's competitors when absent).
    #[arg(long = "competitor")]
    pub(crate) competitors: Vec<String>,
    /// Results per Wordstat or Labs request.
    #[arg(long, default_value_t = 500)]
    pub(crate) limit: u32,
    /// Keep only keywords containing one of these words or phrases.
    #[arg(long)]
    pub(crate) include: Vec<String>,
    /// Drop keywords containing one of these words or phrases.
    #[arg(long)]
    pub(crate) exclude: Vec<String>,
    /// Keep only keywords with at least these monthly searches (when known).
    #[arg(long)]
    pub(crate) min_volume: Option<u64>,
    /// Keep only keywords of at most this many words.
    #[arg(long)]
    pub(crate) max_words: Option<usize>,
    /// Only show the requests and cost.
    #[arg(long)]
    pub(crate) estimate: bool,
    /// Do not ask before costs above the limit in the settings.
    #[arg(long)]
    pub(crate) yes: bool,
    /// Print as JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum SourceArg {
    /// Wordstat top requests and associations (Yandex).
    Wordstat,
    /// Google Ads keyword ideas (DataForSEO).
    AdsIdeas,
    /// DataForSEO Labs keyword ideas.
    LabsIdeas,
    /// DataForSEO Labs related searches.
    Related,
    /// DataForSEO Labs suggestions.
    Suggestions,
    /// Keywords competitors rank for (DataForSEO Labs).
    Competitors,
    /// Google autocomplete through DataForSEO.
    Autocomplete,
    /// Google autocomplete through SerpApi.
    SerpapiAutocomplete,
    /// Google Ads Keyword Planner ideas with your own account.
    GoogleAds,
    /// The free Google suggestion endpoint (switch it on first: `octopuson-keywords suggest`).
    GoogleSuggest,
    /// The free Yandex suggestion endpoint (switch it on first: `octopuson-keywords suggest`).
    YandexSuggest,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ExpandArg {
    /// The seed with each letter a–z.
    Alphabet,
    /// The seed with ç ğ ı ö ş ü.
    TurkishLetters,
    /// Question words.
    Questions,
    /// Buying words.
    Commercial,
}

#[derive(Debug, Args)]
#[allow(clippy::struct_excessive_bools)] // command-line flags
pub(crate) struct EnrichArgs {
    /// Project id.
    pub(crate) project: i64,
    /// Numbers to ask (comma separated; volume when absent).
    #[arg(long = "ask", value_enum, value_delimiter = ',')]
    pub(crate) asks: Vec<AskArg>,
    /// Take volumes from your own Google Ads account instead of DataForSEO.
    #[arg(long)]
    pub(crate) google_ads: bool,
    /// Ask again for keywords that have the number already.
    #[arg(long)]
    pub(crate) all: bool,
    /// Only show the requests and cost.
    #[arg(long)]
    pub(crate) estimate: bool,
    /// Do not ask before costs above the limit in the settings.
    #[arg(long)]
    pub(crate) yes: bool,
    /// Print as JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum AskArg {
    /// Google Ads volume, cost per click, competition and monthly searches.
    Volume,
    /// Keyword difficulty.
    Difficulty,
    /// Search intent.
    Intent,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ProvidersCommand {
    /// List the paid sources with their keys, switches, budgets and spending.
    List {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Save a key (read from standard input, never from the command line).
    SetKey {
        /// Provider (wordstat, dataforseo, serpapi).
        provider: String,
        /// Key name (`wordstat`: `api_key`, `folder_id`; `dataforseo`: `credentials`;
        /// `serpapi`: `api_key`).
        name: String,
    },
    /// Remove a saved key.
    RemoveKey {
        /// Provider.
        provider: String,
        /// Key name.
        name: String,
    },
    /// Set a monthly budget in US dollars, or remove it with `none`.
    Budget {
        /// Provider.
        provider: String,
        /// US dollars per month, or `none`.
        usd: String,
    },
    /// Switch a provider on.
    Enable {
        /// Provider.
        provider: String,
    },
    /// Switch a provider off.
    Disable {
        /// Provider.
        provider: String,
    },
    /// Choose how a paid source is paid: `auto` (your key if set, else credits), `own` or
    /// `credit` (OctopusOn SEO credits; sign in with `octopuson-keywords account login`).
    Mode {
        /// Provider (wordstat, dataforseo, serpapi, serper, yandex-cloud).
        provider: String,
        /// auto, own or credit.
        mode: String,
    },
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
pub(crate) enum GoogleCommand {
    /// Save your OAuth client ("Desktop app" type); the secret is read from standard input.
    Client {
        /// Client id (`….apps.googleusercontent.com`).
        client_id: String,
    },
    /// Sign in in your browser.
    Login {
        /// Also connect Google Ads (needs Basic access for your Google Cloud project).
        #[arg(long)]
        ads: bool,
        /// Connect Google Ads only.
        #[arg(long)]
        ads_only: bool,
    },
    /// Sign out and revoke the access.
    Logout,
    /// Show the connection.
    Status {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// List your Search Console properties.
    Sites {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Import a property's queries and pages into a project.
    Import {
        /// Project id.
        project: i64,
        /// Property (`sc-domain:example.com` or `https://www.example.com/`).
        site: String,
        /// Days, ending three days ago.
        #[arg(long, default_value_t = 90)]
        days: u32,
        /// Include searches from every country.
        #[arg(long)]
        all_countries: bool,
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Set the Google Ads account keyword planning runs under.
    AdsAccount {
        /// Customer id (`123-456-7890`).
        customer_id: String,
        /// Manager account id, when the customer is reached through one.
        #[arg(long)]
        manager: Option<String>,
    },
}

#[derive(Debug, Args)]
#[allow(clippy::struct_excessive_bools)] // command-line flags
pub(crate) struct SerpArgs {
    /// Project id.
    pub(crate) project: i64,
    /// Engine (default: the project's primary engine).
    #[arg(long, value_enum)]
    pub(crate) engine: Option<EngineArg>,
    /// Device.
    #[arg(long, default_value = "desktop")]
    pub(crate) device: String,
    /// Only the most searched keywords.
    #[arg(long)]
    pub(crate) top: Option<u32>,
    /// Only the most searched members of each cluster of the newest clustering.
    #[arg(long)]
    pub(crate) per_cluster: Option<u32>,
    /// Fetch again even when a fresh page is stored.
    #[arg(long)]
    pub(crate) refresh: bool,
    /// Only show the keywords and cost.
    #[arg(long)]
    pub(crate) estimate: bool,
    /// Do not ask before costs above the limit in the settings.
    #[arg(long)]
    pub(crate) yes: bool,
    /// Print as JSON.
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct SuggestArgs {
    /// Google suggestions.
    #[arg(long, value_enum)]
    pub(crate) google: Option<Switch>,
    /// Yandex suggestions.
    #[arg(long, value_enum)]
    pub(crate) yandex: Option<Switch>,
    /// I have read the terms warning and accept the risk.
    #[arg(long)]
    pub(crate) accept_terms: bool,
    /// Pause between requests, milliseconds (at least 2000).
    #[arg(long)]
    pub(crate) pause_ms: Option<u64>,
    /// Requests per engine and day (at most 2000).
    #[arg(long)]
    pub(crate) daily_limit: Option<u32>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum BrowserCommand {
    /// The executable, the chains that list the browser and today's pages per engine.
    Status {
        /// Print as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Change the window's settings.
    Set {
        /// Chrome or Edge executable (`auto` to find it).
        #[arg(long)]
        executable: Option<String>,
        /// Seconds between two result pages (at least 10).
        #[arg(long)]
        delay_secs: Option<u64>,
        /// Most result pages per engine and day.
        #[arg(long)]
        daily_cap: Option<u32>,
        /// One proxy for the window, `http://`, `https://` or `socks5://` (`none` to remove).
        #[arg(long)]
        proxy: Option<String>,
    },
    /// Add the browser to the end of an engine's result-page chain.
    Add {
        /// Engine.
        #[arg(value_enum)]
        engine: EngineArg,
    },
    /// Take the browser out of an engine's result-page chain.
    Remove {
        /// Engine.
        #[arg(value_enum)]
        engine: EngineArg,
    },
    /// Delete the window's profile (cookies, consent choices).
    ResetProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Switch {
    On,
    Off,
}
