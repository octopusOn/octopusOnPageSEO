// English texts; the shape every other language must match (see `Dictionary`).
// Placeholders such as {version} are filled by `format()`.

export const en = {
  meta: {
    title: "octopusSEO Log Analyser — see how Google and AI bots crawl your site",
    description:
      "Free desktop app and CLI for SEO log file analysis: verified Googlebot and AI crawler activity, crawl budget, errors, robots.txt compliance and orphan pages. Windows, macOS and Linux.",
  },
  nav: {
    features: "Features",
    download: "Download",
    docs: "Docs",
    changelog: "Changelog",
    github: "GitHub",
    menu: "Menu",
    language: "Language",
  },
  hero: {
    badge: "New: robots.txt compliance and AI referral reports",
    title: "Your server logs know",
    highlight: "what Google and AI bots really do",
    subtitle:
      "octopusSEO Log Analyser turns raw access logs into crawl insights: which pages Googlebot and AI crawlers fetch, where they hit errors, which bots are fake and where your crawl budget goes.",
    primary: "Download for {os}",
    primaryGeneric: "Download free",
    secondary: "Explore features",
    note: "Free · Windows, macOS & Linux · Your logs stay on your computer",
  },
  mockup: {
    title: "acme.com — last 30 days",
    hits: "Verified search hits",
    urls: "URLs crawled",
    errors: "Crawl errors",
    spoofed: "Fake bot hits",
    chart: "Daily crawl by bot",
    table: "Bot",
    verified: "verified",
    fake: "spoofed",
  },
  formats: {
    title: "Reads the logs you already have — format detected automatically",
  },
  features: {
    eyebrow: "Everything in one place",
    title: "From raw log lines to SEO decisions",
    subtitle: "Built by SEOs for agencies that analyse dozens of sites every month.",
    items: {
      verification: {
        title: "Verified bots, not user agents",
        text: "110+ crawlers identified and checked against the operators' official IP lists and reverse DNS. Fake Googlebots are flagged, not counted.",
      },
      ai: {
        title: "AI visibility",
        text: "See what GPTBot, ClaudeBot, PerplexityBot and ChatGPT-User fetch — and which visits ChatGPT, Perplexity or Gemini send you.",
      },
      robots: {
        title: "robots.txt compliance",
        text: "Which bots request disallowed URLs, which ones your robots.txt blocks entirely — evaluated exactly like Google does.",
      },
      budget: {
        title: "Crawl budget",
        text: "Crawl frequency per URL and directory, days since the last crawl, parameter URLs, redirects and static files eating your budget.",
      },
      orphans: {
        title: "Orphan & uncrawled pages",
        text: "Match crawler exports and XML sitemaps against logs; extra columns are joined like a VLOOKUP.",
      },
      reports: {
        title: "39 reports + query language",
        text: "Ready-made reports and a simple form for your own filters. Save, share and export to Excel, CSV or JSON.",
      },
      geo: {
        title: "Countries & networks",
        text: "Country and network (ASN) of every client IP reveal scraper clouds and where fake bots come from. Looked up locally.",
      },
      alerts: {
        title: "Crawl alerts",
        text: "Sudden Googlebot drops, AI crawl spikes and server-error bursts are highlighted on the overview.",
      },
    },
  },
  ai: {
    eyebrow: "AI search is here",
    title: "Know when AI assistants read — and recommend — your pages",
    text: "Training crawlers, AI search indexers and real-time assistant fetches are separated, so you see which content feeds models, which gets cited, and the human visits AI assistants send back.",
    bullets: [
      "Separate categories for AI training, AI search and AI assistants",
      "Errors served to AI bots — every failed fetch is a lost citation",
      "Visits referred by ChatGPT, Perplexity, Gemini, Copilot and Claude",
      "Bandwidth and estimated CO₂ per crawler",
    ],
  },
  terminal: {
    eyebrow: "Automate it",
    title: "A command line for every report",
    text: "The snx-log CLI shares the data with the desktop app. Script monthly reports, schedule imports on your servers and export straight to Excel.",
    points: ["~750,000 lines per second", "Runs 100% on your computer"],
  },
  steps: {
    title: "Three steps to your first insight",
    items: [
      { title: "Drop your logs", text: "Plain, gzip, zstd, bzip2 or zip — files or whole folders. Re-imports are safe." },
      { title: "We verify the bots", text: "Every bot hit becomes verified, spoofed or unverified using official data." },
      { title: "Act on the reports", text: "Fix errors bots hit, stop crawl waste, unblock what matters and track AI visibility." },
    ],
  },
  stats: {
    bots: "crawlers recognised",
    formats: "log formats",
    reports: "ready-made reports",
    speed: "lines per second",
  },
  faq: {
    title: "Frequently asked questions",
    items: [
      {
        q: "Is octopusSEO Log Analyser really free?",
        a: "Yes. The desktop app and the CLI are free and open source, with no limit on log lines or sites.",
      },
      {
        q: "Do my logs leave my computer?",
        a: "No. Everything is stored and analysed locally. The only downloads are public crawler IP lists and, if you enable it, the IP geolocation database.",
      },
      {
        q: "Which log formats are supported?",
        a: "Nginx and Apache (including custom formats), IIS, CloudFront, AWS load balancers, HAProxy, Traefik, Caddy, Cloudflare, Google Cloud, JSON and CSV logs. The format is detected automatically.",
      },
      {
        q: "How large can my logs be?",
        a: "Hundreds of millions of lines. Logs are imported at roughly 750,000 lines per second and stored compressed at about 8% of their original size.",
      },
      {
        q: "Which languages does the app speak?",
        a: "The desktop app is available in English and Turkish; reports and exports work the same in both.",
      },
    ],
  },
  cta: {
    title: "See your site through Googlebot's eyes",
    text: "Download the app, drop in a log file and get your first report in minutes.",
    button: "Download free",
  },
  footer: {
    tagline: "SEO software that shows how search engines and AI really see your site.",
    product: "Product",
    resources: "Resources",
    company: "Company",
    logAnalyser: "Log Analyser",
    spider: "SEO Spider",
    comingSoon: "coming soon",
    contact: "Contact",
    issues: "Report an issue",
    rights: "© {year} octopusSEO. Free and open source software.",
    geo: "Country and network data: IP Geolocation by DB-IP",
  },
  featuresPage: {
    title: "Features",
    subtitle: "Everything the Log Analyser does, in detail.",
    sections: {
      bots: {
        title: "Bot identification and verification",
        text: "User agents are trivial to fake. Every bot hit is checked, so reports count real crawlers only.",
        bullets: [
          "110+ crawlers in categories: search, AI training, AI search, AI assistants, ads, SEO tools, social, monitoring",
          "Official IP lists of Google, Bing, OpenAI, Anthropic, Perplexity, Apple and more, plus forward-confirmed reverse DNS",
          "Verdict per hit: verified, spoofed or unverified — re-check any time",
          "Your own bots: custom user-agent patterns with IP ranges, IP list URLs or reverse DNS",
        ],
      },
      ai: {
        title: "AI crawlers and AI search",
        text: "Understand how AI platforms use your content — and what they send back.",
        bullets: [
          "Daily activity of every AI crawler and the URLs it fetched",
          "Errors served to AI bots and assistants",
          "Human visits referred by AI assistants, with landing pages",
          "Bandwidth and estimated emissions per bot",
        ],
      },
      robots: {
        title: "robots.txt compliance",
        text: "Check every logged bot request against your live robots.txt or any version of it.",
        bullets: [
          "Google-style evaluation: most specific user-agent group, longest match, Allow wins ties",
          "Disallowed requests per bot, from verified and from fake IPs",
          "Bots blocked site-wide, such as AI training crawlers",
          "Export every violation with the deciding rule",
        ],
      },
      crawl: {
        title: "Crawl budget and technical health",
        text: "Find where crawlers waste time and what they cannot reach.",
        bullets: [
          "URL summary: last status, last crawl, days since last crawl, size, speed and hits per bot",
          "Response codes per URL with 1xx–5xx columns and inconsistent responses",
          "Redirects, 4xx/5xx, slow and large URLs, parameter URLs, static files",
          "Alerts for crawl drops, spikes and server-error bursts",
        ],
      },
      urls: {
        title: "Uncrawled and orphan pages",
        text: "Combine your crawl data with what bots actually did.",
        bullets: [
          "Crawler CSV exports, plain lists and XML sitemaps (with sitemap indexes)",
          "Listed but never crawled, crawled but not listed (orphans)",
          "All other columns of your export joined to the log data",
          "Period comparison per URL, directory or bot — before and after a migration",
        ],
      },
      data: {
        title: "Your data, your way",
        text: "Fast, private and scriptable.",
        bullets: [
          "Formats detected automatically; custom Nginx and Apache formats supported",
          "Import settings per site: bots only, include/exclude URLs, strip parameters",
          "Query language with saved queries; Excel, CSV and JSON exports",
          "Share a whole site as one archive; command-line tool for automation",
        ],
      },
    },
  },
  download: {
    title: "Download octopusSEO Log Analyser",
    subtitle: "Free for Windows, macOS and Linux. The CLI is included for automation.",
    recommended: "Recommended for your system",
    version: "Version {version}",
    released: "Released {date}",
    totalDownloads: "{count} downloads",
    desktop: "Desktop app",
    cli: "Command line (snx-log)",
    file: "File",
    size: "Size",
    checksum: "SHA-256",
    getIt: "Download",
    unavailableTitle: "Downloads are coming soon",
    unavailableText:
      "The first public release is being prepared. Follow the project or build it from source in the meantime.",
    allReleases: "All releases",
    sourceTitle: "Build from source",
    sourceText: "Requires Rust (stable) and, for the desktop app, Node.js 22+ and pnpm.",
    requirementsTitle: "System requirements",
    requirements: [
      "Windows 10/11, macOS 11 or newer (Apple Silicon and Intel), or a current Linux distribution",
      "4 GB RAM; disk space of about 10% of your raw log size",
      "Internet only for crawler IP list and geolocation updates",
    ],
    platforms: {
      macos: "macOS",
      windows: "Windows",
      linux: "Linux",
    },
    arch: { universal: "Universal", arm64: "ARM64", x64: "x64", arm32: "ARM 32-bit", appleSilicon: "Apple Silicon", intel: "Intel" },
    kinds: {
      dmg: "Disk image",
      exe: "Installer",
      msi: "MSI package",
      appimage: "AppImage",
      deb: "Debian / Ubuntu package",
      rpm: "Fedora / RHEL package",
      exeOffline: "Installer with offline WebView2",
      cli: "CLI archive",
    },
  },
  docs: {
    title: "Documentation",
    subtitle: "Get started in minutes, then go deeper with the query language.",
    quickstart: "Quick start",
    quickstartText: "Create a client and a site, import logs and open a report. The desktop app does the same with a few clicks.",
    query: "Query language",
    queryText: "Every report is a query: conditions, then optional grouping, metrics, sorting and a limit.",
    examples: "Examples",
    exampleItems: [
      { query: "bot:googlebot* verification:verified status:4xx date:last30d group:url", text: "Errors verified Googlebot hit in the last 30 days" },
      { query: "bot_category:ai-* group:bot,day metric:hits", text: "Daily activity per AI crawler" },
      { query: "referer_type:ai -bot:* group:url,referer_host", text: "Pages visited from AI assistants" },
      { query: "verification:spoofed group:bot,as_org,country", text: "Where fake bots come from" },
      { query: "group:url metric:last_status,days_since_last,googlebot_hits", text: "Crawl recency per URL" },
    ],
    fields: "Fields",
    metrics: "Metrics",
    more: "Full reference on GitHub",
    guide: "User guide (Turkish)",
  },
  changelog: {
    title: "Changelog",
    subtitle: "New features and fixes in every release.",
    empty: "No releases have been published yet.",
    viewOnGithub: "View on GitHub",
  },
  notFound: {
    title: "Page not found",
    text: "This page does not exist — maybe a crawler made it up.",
    home: "Back to the home page",
  },
  common: {
    copy: "Copy",
    copied: "Copied",
    learnMore: "Learn more",
  },
};

export type Dictionary = typeof en;
