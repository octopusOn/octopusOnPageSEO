// Synthetic backend for `pnpm dev` in a plain browser (no Tauri runtime): lets the UI be
// developed and reviewed without the Rust toolchain. Never bundled into the desktop app's
// IPC path; it is only installed when the Tauri internals are missing.

import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

import type { AstExpr, AstLiteral, AstPredicate, Cell, Column, ColumnKind, QueryAst } from "../api/types";
import { tokenize } from "../lib/queryBuilder";

const SITE = "acme.com";
const DAYS = 41;
const START = Date.UTC(2026, 8, 1);

function day(index: number): string {
  return new Date(START + index * 86_400_000).toISOString().slice(0, 10);
}

/** Deterministic pseudo-random numbers so screenshots are stable. */
function noise(seed: number): number {
  const x = Math.sin(seed * 12.9898) * 43758.5453;
  return x - Math.floor(x);
}

const CATEGORIES: [string | null, number][] = [
  ["search", 6900],
  [null, 1360],
  ["ai-training", 985],
  ["seo", 485],
];

const BOTS = [
  ["googlebot-desktop", "Googlebot Desktop", "Google", "search", "ip-list+dns"],
  ["googlebot-smartphone", "Googlebot Smartphone", "Google", "search", "ip-list+dns"],
  ["bingbot", "Bingbot", "Microsoft", "search", "ip-list+dns"],
  ["gptbot", "GPTBot", "OpenAI", "ai-training", "ip-list"],
  ["claudebot", "ClaudeBot", "Anthropic", "ai-training", "ip-list"],
  ["perplexitybot", "PerplexityBot", "Perplexity", "ai-search", "ip-list"],
  ["yandexbot", "YandexBot", "Yandex", "search", "dns"],
  ["ahrefsbot", "AhrefsBot", "Ahrefs", "seo", "ip-list+dns"],
  ["bytespider", "Bytespider", "ByteDance", "ai-training", "none"],
] as const;

const TEMPLATES = [
  ["top-urls", "Most requested URLs", "The 500 URLs with the most hits.", "group:url metric:hits,bytes,last_seen sort:hits:desc limit:500"],
  ["status-codes", "Status codes", "Hits and distinct URLs per HTTP status code.", "group:status metric:hits,unique_urls sort:hits:desc"],
  ["bots", "Bot overview", "Hits, distinct URLs and last visit per bot and verification verdict.", "bot:* group:bot,verification metric:hits,unique_urls,last_seen sort:hits:desc"],
  ["googlebot-daily", "Googlebot crawl volume", "Daily hits and distinct URLs of verified Googlebot crawlers.", "bot:googlebot* verification:verified group:day metric:hits,unique_urls sort:day:asc"],
  ["client-errors", "Pages returning 4xx", "Broken or missing URLs, most frequent first.", "status:4xx group:url,status metric:hits,last_seen sort:hits:desc limit:500"],
  ["ai-bots", "AI crawler activity", "Daily hits per AI crawler.", "bot_category:ai-* group:bot,day metric:hits sort:day:asc,hits:desc"],
  ["spoofed-bots", "Spoofed bots", "IPs claiming to be a verifiable bot but failing verification.", "verification:spoofed group:bot,ip metric:hits,unique_urls,last_seen sort:hits:desc limit:500"],
  ["url-summary", "URL crawl summary", "Per URL: hits, last status and crawl, days since the last request, size, speed and hits per major bot.", "group:url,content_type metric:hits,last_status,last_seen,days_since_last,bytes,avg_bytes,avg_ms,bot_hits,googlebot_hits,bingbot_hits sort:hits:desc limit:1000"],
  ["response-codes", "Response codes per URL", "Last status, whether responses were inconsistent and hits per status class for every URL.", "group:url metric:last_status,last_seen,hits,inconsistent,hits_1xx,hits_2xx,hits_3xx,hits_4xx,hits_5xx sort:hits:desc limit:1000"],
  ["ai-referrals", "Visits from AI assistants", "Human visits referred by ChatGPT, Perplexity, Gemini, Copilot, Claude and others per day.", "referer_type:ai -bot:* group:day,referer_host metric:hits,unique_urls sort:day:asc,hits:desc"],
  ["bot-bandwidth", "Bandwidth and CO2 by bot", "Bytes, average response size, estimated emissions and speed per bot.", "bot:* group:bot metric:hits,bytes,avg_bytes,co2_mg,avg_ms sort:bytes:desc"],
].map(([id, title, description, query]) => ({ id, title, description, query }));

const KINDS: Record<string, ColumnKind> = {
  day: "date",
  hour: "timestamp",
  status: "integer",
  hits: "integer",
  bytes: "integer",
  unique_urls: "integer",
  unique_ips: "integer",
  unique_statuses: "integer",
  avg_ms: "float",
  max_ms: "float",
  p95_ms: "float",
  first_seen: "timestamp",
  last_seen: "timestamp",
  avg_bytes: "float",
  co2_mg: "float",
  inconsistent: "boolean",
  last_status: "integer",
  days_since_last: "integer",
};

function valueFor(column: string, row: number): Cell {
  switch (column) {
    case "url":
    case "path":
      return row % 5 === 0 ? `/blog/post-${row}` : `/products/item-${row * 7}`;
    case "directory":
      return ["/blog/", "/products/", "/category/", "/images/", "/"][row % 5]!;
    case "status":
      return [200, 404, 301, 304, 503][row % 5]!;
    case "status_class":
      return ["2xx", "3xx", "4xx", "5xx"][row % 4]!;
    case "bot":
      return BOTS[row % BOTS.length]![0];
    case "bot_category":
      return ["search", "ai-training", "seo", null][row % 4]!;
    case "verification":
      return ["verified", "spoofed", "unverified"][row % 3]!;
    case "ip":
      return `66.249.66.${(row % 250) + 1}`;
    case "ua":
      return "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";
    case "day":
      return day(row % DAYS);
    case "first_seen":
    case "last_seen":
    case "ts":
    case "hour":
      return new Date(START + (row % DAYS) * 86_400_000 + 3_600_000 * (row % 24)).toISOString();
    case "status_codes":
      return row % 3 === 0 ? "200,304,404" : "200";
    case "last_status":
      return [200, 404, 301, 200, 503][row % 5]!;
    case "inconsistent":
      return row % 3 === 0;
    case "days_since_last":
      return row % 17;
    case "content_type":
      return ["html", "html", "image", "js", "css"][row % 5]!;
    case "referer_type":
      return ["direct", "internal", "search", "ai", "social"][row % 5]!;
    case "referer_host":
      return ["chatgpt.com", "perplexity.ai", "gemini.google.com", "copilot.microsoft.com", "claude.ai"][row % 5]!;
    default:
      return Math.round(5000 * noise(row + column.length) + 10);
  }
}

function runQuery(text: string, offset: number, limit: number) {
  const group = /group:([\w,]+)/.exec(text)?.[1]?.split(",") ?? [];
  const metrics = /metric:([\w,]+)/.exec(text)?.[1]?.split(",") ?? (group.length ? ["hits"] : []);
  let columns: Column[];
  let rows: Cell[][];
  if (group.join() === "day,bot_category") {
    columns = [
      { name: "day", kind: "date" },
      { name: "bot_category", kind: "text" },
      { name: "hits", kind: "integer" },
    ];
    rows = [];
    for (let d = 0; d < DAYS; d += 1) {
      for (const [category, base] of CATEGORIES) {
        rows.push([day(d), category, Math.round(base * (0.85 + 0.3 * noise(d * 7 + base)))]);
      }
    }
  } else if (group.join() === "day,bot") {
    columns = [
      { name: "day", kind: "date" },
      { name: "bot", kind: "text" },
      { name: "hits", kind: "integer" },
    ];
    rows = [];
    for (let d = 0; d < DAYS; d += 1) {
      rows.push([day(d), "googlebot-desktop", Math.round(2900 * (0.9 + 0.2 * noise(d)))]);
      rows.push([day(d), "googlebot-smartphone", Math.round(2400 * (0.9 + 0.25 * noise(d + 99)))]);
    }
  } else if (group.length === 0 && metrics.length === 0) {
    const listing = ["ts", "ip", "method", "url", "status", "bytes", "response_ms", "ua", "bot", "verification", "referer", "host"];
    columns = listing.map((name) => ({
      name,
      kind: name === "ts" ? "timestamp" : name === "status" || name === "bytes" ? "integer" : name === "response_ms" ? "float" : "text",
    }));
    rows = Array.from({ length: 2000 }, (_, row) =>
      columns.map((c) => (c.name === "method" ? "GET" : c.name === "referer" || c.name === "host" ? null : valueFor(c.name, row))),
    );
  } else {
    columns = [...group, ...metrics].map((name) => ({ name, kind: KINDS[name] ?? "text" }));
    rows = Array.from({ length: group.length ? 1200 : 1 }, (_, row) => columns.map((c) => valueFor(c.name, row)));
  }
  const page = rows.slice(offset, offset + limit);
  return {
    outcome: { site: SITE, query: text, source: group.includes("ip") ? "raw hits" : "daily URL rollup", columns, rows: page.length, elapsed_ms: 14 },
    rows: page,
    hasMore: offset + limit < rows.length,
  };
}

const overview = {
  id: 1,
  client: "Acme",
  name: SITE,
  format: "builtin:combined",
  created_at: "2026-10-12T20:04:06Z",
  imports: 2,
  hits: 400000,
  urls: 1025,
  user_agents: 8,
  first_request: "2026-09-01T00:00:05Z",
  last_request: "2026-10-11T23:59:51Z",
  history: [
    { id: 2, path: "/var/log/nginx/access.log.1.gz", size_bytes: 1801135, format: "builtin:combined", finished_at: "2026-10-12T20:04:07Z", elapsed_ms: 410, total_lines: 100000, parsed_lines: 100000, inserted_rows: 100000, duplicate_rows: 0, first_request: "2026-09-01T00:00:37Z", last_request: "2026-10-11T23:59:51Z" },
    { id: 1, path: "/var/log/nginx/access.log", size_bytes: 63504974, format: "builtin:combined", finished_at: "2026-10-12T20:04:06Z", elapsed_ms: 1210, total_lines: 300000, parsed_lines: 300000, inserted_rows: 300000, duplicate_rows: 0, first_request: "2026-09-01T00:00:05Z", last_request: "2026-10-11T23:59:47Z" },
  ],
};

const crawl = {
  site: SITE,
  period: null,
  total_hits: 400000,
  categories: [
    { name: "search", value: 283828, ratio: 0.71 },
    { name: "browser", value: 55915, ratio: 0.14 },
    { name: "ai-training", value: 40376, ratio: 0.101 },
    { name: "seo", value: 19881, ratio: 0.05 },
  ],
  bots: [
    { bot: "googlebot-desktop", verified: 119974, spoofed: 12148, unverified: 0 },
    { bot: "googlebot-smartphone", verified: 99590, spoofed: 388, unverified: 0 },
    { bot: "bingbot", verified: 39796, spoofed: 0, unverified: 0 },
    { bot: "gptbot", verified: 24320, spoofed: 0, unverified: 0 },
    { bot: "ahrefsbot", verified: 19881, spoofed: 0, unverified: 0 },
    { bot: "claudebot", verified: 0, spoofed: 16056, unverified: 0 },
    { bot: "yandexbot", verified: 11418, spoofed: 514, unverified: 0 },
  ],
  spoofed_hits: 29106,
  alerts: [
    { series: "googlebot", day: "2026-10-11", kind: "drop", value: 1452, baseline: 6604 },
    { series: "all", day: "2026-10-09", kind: "server-errors", value: 0.041, baseline: 0.003 },
    { series: "ai", day: "2026-10-04", kind: "spike", value: 4120, baseline: 610 },
  ],
  search: {
    hits: 270778,
    unique_urls: 1025,
    hits_per_day: 6604,
    unique_urls_per_day: 412,
    errors: 12275,
    avg_bytes: 18342,
    avg_ms: 182,
    status_classes: [
      { name: "2xx", value: 243723, ratio: 0.9 },
      { name: "3xx", value: 14780, ratio: 0.055 },
      { name: "4xx", value: 10998, ratio: 0.041 },
      { name: "5xx", value: 1277, ratio: 0.005 },
    ],
    non_success_ratio: 0.1,
    parameter_ratio: 0.097,
    asset_ratio: 0.079,
  },
};

function compareTable(presences: string[]) {
  const columns: Column[] = [
    { name: "url", kind: "text" },
    { name: "presence", kind: "text" },
    { name: "hits", kind: "integer" },
    { name: "last_seen", kind: "timestamp" },
    { name: "status_codes", kind: "text" },
  ];
  const rows = Array.from({ length: 1045 }, (_, row) => {
    const presence = presences[row < 783 ? 0 : row < 803 ? 1 : 2]!;
    return [valueFor("url", row), presence, presence === "not-in-logs" ? 0 : 300 - (row % 300), presence === "not-in-logs" ? null : valueFor("last_seen", row), presence === "not-in-logs" ? null : valueFor("status_codes", row)];
  });
  return { columns, rows };
}

const customBots: {
  id: string;
  name: string;
  operator: string;
  category: string;
  patterns: string[];
  ip_ranges: string[];
  ip_list_url: string | null;
  dns_suffixes: string[];
}[] = [];

let geoRelease: { country: string; asn: string; fetched_at: string } | null = null;

const DEFAULT_SETTINGS = {
  host: null as string | null,
  traffic: "all" as "all" | "bots",
  bot_categories: [] as string[],
  include_urls: [] as string[],
  exclude_urls: [] as string[],
  strip_parameters: [] as string[],
  strip_tracking_parameters: false,
};
const siteSettings = new Map<string, typeof DEFAULT_SETTINGS>();

const FIELDS: [string, "text" | "integer" | "date" | "boolean"][] = [
  ["date", "date"], ["ip", "text"], ["method", "text"], ["url", "text"], ["path", "text"],
  ["directory", "text"], ["params", "boolean"], ["status", "integer"], ["bytes", "integer"],
  ["response_ms", "integer"], ["ua", "text"], ["referer", "text"], ["host", "text"],
  ["protocol", "text"], ["bot", "text"], ["bot_category", "text"], ["verification", "text"],
  ["content_type", "text"], ["referer_host", "text"], ["referer_type", "text"],
  ["country", "text"], ["asn", "integer"], ["as_org", "text"],
];

const unquote = (value: string) =>
  value.startsWith('"') && value.endsWith('"') ? value.slice(1, -1).replace(/\\(["\\])/g, "$1") : value;

/** Approximates the Rust parser for flat queries, enough to demo the filter form. */
function mockParse(text: string): QueryAst {
  const query: QueryAst = { filter: null, group: [], metrics: [], sort: [], limit: null };
  const conditions: AstExpr[] = [];
  let or = false;
  const kind = (field: string) => FIELDS.find(([name]) => name === field)?.[1];
  const literal = (field: string, value: string): AstLiteral =>
    kind(field) === "integer" ? { integer: Number(value) } : kind(field) === "date" ? { date: value } : { text: value };
  for (let token of tokenize(text.replace(/^\(|\)(?=\s|$)/g, ""))) {
    if (token === "OR") {
      or = true;
      continue;
    }
    const negated = token.startsWith("-");
    if (negated) token = token.slice(1);
    const colon = token.indexOf(":");
    if (colon < 0) throw { kind: "syntax", message: `unexpected ${token}` };
    const field = token.slice(0, colon);
    const value = token.slice(colon + 1);
    if (field === "group") query.group = value.split(",");
    else if (field === "metric") query.metrics = value.split(",");
    else if (field === "limit") query.limit = Number(value);
    else if (field === "sort")
      query.sort = value.split(",").map((key) => {
        const [column = "", direction] = key.split(":");
        return { column, direction: direction === "asc" ? "asc" : "desc" };
      });
    else {
      if (!kind(field)) throw { kind: "syntax", message: `unknown field ${field}` };
      let predicate: AstPredicate;
      const raw = unquote(value);
      if (value.startsWith("~")) predicate = { regex: unquote(value.slice(1)) };
      else if (value.startsWith(">=")) predicate = { compare: ["ge", literal(field, value.slice(2))] };
      else if (value.startsWith("<=")) predicate = { compare: ["le", literal(field, value.slice(2))] };
      else if (/^\dxx$/.test(value)) predicate = { status_class: Number(value[0]) };
      else if (/^last\d+d$/.test(value)) predicate = { last_days: Number(value.slice(4, -1)) };
      else if (value.includes("..") && kind(field) !== "text") {
        const [low = "", high = ""] = value.split("..");
        predicate = { between: [literal(field, low), literal(field, high)] };
      } else if (kind(field) === "boolean") predicate = { one_of: [{ boolean: value === "true" }] };
      else if (raw.includes("*")) predicate = { glob: raw };
      else predicate = { one_of: value.split(",").map((v) => literal(field, unquote(v))) };
      const condition: AstExpr = { condition: { field, predicate } };
      conditions.push(negated ? { not: condition } : condition);
    }
  }
  if (conditions.length === 1) query.filter = conditions[0]!;
  else if (conditions.length > 1) query.filter = or ? { or: conditions } : { and: conditions };
  return query;
}

interface MockSaved {
  name: string;
  description: string;
  query: string;
  created_at: string;
  updated_at: string;
}

const savedQueries: MockSaved[] = [
  {
    name: "Googlebot 404 (haftalık)",
    description: "Pazartesi kontrolü",
    query: "date:last7d bot:googlebot* verification:verified status:404 group:url metric:hits,last_seen sort:hits:desc",
    created_at: "2026-10-12T20:00:00Z",
    updated_at: "2026-10-12T20:00:00Z",
  },
];

export function installMockBackend(): void {
  mockWindows("main");
  mockIPC(async (command, payload) => {
    const args = (payload ?? {}) as Record<string, unknown>;
    await new Promise((resolve) => setTimeout(resolve, 120));
    switch (command) {
      case "app_info":
        return { version: "0.1.0 (demo)", dataDir: "~/Library/Application Support/com.octopusSEO.octopusSEO-Log-Analyser", memoryLimitMb: 1024 };
      case "list_clients":
        return [{ id: 1, name: "Acme", created_at: "2026-10-12T20:04:06Z" }, { id: 2, name: "Globex", created_at: "2026-10-12T20:05:00Z" }];
      case "list_sites":
        return [
          { id: 1, client: "Acme", name: SITE, format: "builtin:combined", created_at: "2026-10-12T20:04:06Z" },
          { id: 2, client: "Acme", name: "shop.acme.com", format: "auto", created_at: "2026-10-12T20:04:06Z" },
          { id: 3, client: "Globex", name: "globex.io", format: "builtin:traefik-clf", created_at: "2026-10-12T20:04:06Z" },
        ];
      case "site_overview":
        return args.name === SITE ? structuredClone(overview) : { ...overview, name: String(args.name), hits: 0, imports: 0, history: [] };
      case "list_formats":
        return [
          { id: "traefik-json", name: "Traefik (JSON)" },
          { id: "json", name: "JSON (Caddy, Cloudflare, Google Cloud, Nginx JSON, …)" },
          { id: "aws-elb", name: "AWS Elastic Load Balancing (ALB / Classic)" },
          { id: "haproxy", name: "HAProxy (HTTP log)" },
          { id: "iis", name: "Microsoft IIS (W3C default fields)" },
          { id: "cloudfront", name: "Amazon CloudFront (standard logs)" },
          { id: "traefik-clf", name: "Traefik CLF" },
          { id: "combined", name: "Nginx / Apache combined" },
          { id: "common", name: "Apache common" },
        ];
      case "site_settings":
        return siteSettings.get(String(args.site)) ?? DEFAULT_SETTINGS;
      case "set_site_settings": {
        const settings = args.settings as typeof DEFAULT_SETTINGS;
        const bad = [...settings.include_urls, ...settings.exclude_urls].find((pattern) => {
          try {
            new RegExp(pattern);
            return false;
          } catch {
            return true;
          }
        });
        if (bad !== undefined) throw { kind: "invalid-input", message: `invalid URL pattern \`${bad}\`` };
        const host = settings.host?.trim().toLowerCase().replace(/^https?:\/\//, "").replace(/^www\./, "").replace(/\/.*$/, "") || null;
        const saved = { ...settings, host };
        siteSettings.set(String(args.site), saved);
        return saved;
      }
      case "delete_import": {
        const index = overview.history.findIndex((entry) => entry.id === args.importId);
        if (index < 0) throw { kind: "not-found", message: `import ${String(args.importId)} not found` };
        const [removed] = overview.history.splice(index, 1);
        overview.hits -= removed?.inserted_rows ?? 0;
        overview.imports -= 1;
        return null;
      }
      case "crawl_overview":
        return { ...crawl, period: args.period ?? null };
      case "list_templates":
        return TEMPLATES;
      case "list_saved_queries":
        return [...savedQueries].sort((a, b) => a.name.localeCompare(b.name));
      case "save_query": {
        const name = String(args.name).trim();
        const index = savedQueries.findIndex((q) => q.name === name);
        if (index >= 0 && !args.overwrite) throw { kind: "duplicate", message: `a saved query named \`${name}\` already exists` };
        const now = new Date().toISOString();
        const entry = { name, description: String(args.description ?? "").trim(), query: String(args.query).trim().replace(/\s+/g, " "), created_at: now, updated_at: now };
        if (index >= 0) savedQueries[index] = entry;
        else savedQueries.push(entry);
        return entry;
      }
      case "remove_saved_query":
        savedQueries.splice(savedQueries.findIndex((q) => q.name === args.name), 1);
        return null;
      case "query_schema":
        return {
          fields: FIELDS.map(([name, kind]) => ({ name, kind })),
          dimensions: ["url", "path", "directory", "status", "status_class", "method", "ip", "ua", "referer", "host", "protocol", "day", "hour", "month", "bot", "bot_category", "verification", "content_type", "referer_host", "referer_type", "country", "asn", "as_org"],
          metrics: [
            "hits", "bytes", "avg_ms", "max_ms", "p95_ms", "unique_urls", "unique_ips", "first_seen", "last_seen", "status_codes", "unique_statuses",
            "avg_bytes", "errors", "hits_1xx", "hits_2xx", "hits_3xx", "hits_4xx", "hits_5xx", "last_status", "days_since_last", "co2_mg", "inconsistent",
            "bot_hits", "search_hits", "ai_hits", "googlebot_hits", "googlebot_smartphone_hits", "googlebot_desktop_hits", "bingbot_hits", "yandex_hits",
          ],
        };
      case "suggest_values": {
        const pools: Record<string, [string, number][]> = {
          bot: [["googlebot-desktop", 119974], ["googlebot-smartphone", 99590], ["bingbot", 39796], ["gptbot", 24320], ["ahrefsbot", 19881], ["claudebot", 16056], ["yandexbot", 11932]],
          ua: [
            ["Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)", 132122],
            ["Mozilla/5.0 (Linux; Android 6.0.1; Nexus 5X Build/MMB29P) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.7390.122 Mobile Safari/537.36 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)", 99978],
            ["Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36", 55915],
            ["Mozilla/5.0 (compatible; bingbot/2.0; +http://www.bing.com/bingbot.htm)", 39796],
            ["Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; GPTBot/1.2; +https://openai.com/gptbot)", 24320],
            ["Mozilla/5.0 (compatible; AhrefsBot/7.0; +http://ahrefs.com/robot/)", 19881],
          ],
          status: [["200", 360062], ["404", 16182], ["304", 11075], ["301", 10786], ["503", 1895]],
          bot_category: [["search", 283828], ["ai-training", 40376], ["seo", 19881]],
          content_type: [["html", 301220], ["image", 52010], ["js", 21877], ["css", 14120], ["font", 4410]],
          referer_type: [["direct", 290114], ["internal", 61022], ["search", 30177], ["ai", 2211], ["social", 1640]],
          country: [["US", 210551], ["TR", 98214], ["DE", 31002], ["NL", 18877], ["IE", 9210]],
          as_org: [["Google LLC", 220114], ["Turk Telekom", 51990], ["Microsoft Corporation", 39801], ["Hetzner Online GmbH", 9875], ["Amazon.com, Inc.", 7712]],
          referer_host: [["google.com", 28410], ["chatgpt.com", 1544], ["perplexity.ai", 402], ["bing.com", 1240], ["gemini.google.com", 265]],
          verification: [["verified", 315379], ["spoofed", 29106], ["unverified", 254]],
          url: Array.from({ length: 30 }, (_, i) => [i % 2 ? `/products/item-${i * 7}` : `/blog/post-${i}`, 400 - i * 9] as [string, number]),
          method: [["GET", 398112], ["HEAD", 1500], ["POST", 388]],
        };
        // Like the backend: case-insensitive containment where `*` matches any text.
        const escaped = String(args.search ?? "").trim().replace(/[.+?^${}()|[\]\\]/g, "\\$&");
        const needle = new RegExp(escaped.replace(/\*/g, ".*"), "i");
        return (pools[String(args.field)] ?? [])
          .filter(([value]) => needle.test(value))
          .slice(0, Number(args.limit ?? 50))
          .map(([value, hits]) => ({ value, hits }));
      }
      case "parse_query":
        return mockParse(String(args.text ?? ""));
      case "check_query":
      case "run_query": {
        const spec = args.spec as { text?: string };
        return runQuery(spec.text ?? "", Number(args.offset ?? 0), Number(args.limit ?? 500));
      }
      case "list_bots":
        return [
          ...customBots.map((bot) => ({
            id: bot.id,
            name: bot.name || bot.id,
            operator: bot.operator,
            category: bot.category,
            verification: bot.ip_ranges.length || bot.ip_list_url ? (bot.dns_suffixes.length ? "ip-list+dns" : "ip-list") : bot.dns_suffixes.length ? "dns" : "none",
            custom: true,
          })),
          ...BOTS.map(([id, name, operator, category, verification]) => ({ id, name, operator, category, verification, custom: false })),
        ];
      case "export_site":
        return { site: String(args.site), path: String(args.path), files: 14, bytes: 182_331_904 };
      case "import_site":
        throw { kind: "error", message: "Site archives can be opened in the desktop app only." };
      case "robots_compliance": {
        const bots = [
          { bot: "gptbot", name: "GPTBot", category: "ai-training", group: "gptbot", blocked: true, user_triggered: false, hits: 24320, disallowed_hits: 24320, verified_disallowed_hits: 24320, disallowed_urls: 812 },
          { bot: "claudebot", name: "ClaudeBot", category: "ai-training", group: "*", blocked: false, user_triggered: false, hits: 16056, disallowed_hits: 1204, verified_disallowed_hits: 0, disallowed_urls: 77 },
          { bot: "chatgpt-user", name: "ChatGPT-User", category: "ai-assistant", group: "*", blocked: false, user_triggered: true, hits: 2210, disallowed_hits: 35, verified_disallowed_hits: 35, disallowed_urls: 9 },
          { bot: "googlebot-smartphone", name: "Googlebot Smartphone", category: "search", group: "googlebot", blocked: false, user_triggered: false, hits: 99978, disallowed_hits: 12, verified_disallowed_hits: 0, disallowed_urls: 3 },
          { bot: "bingbot", name: "Bingbot", category: "search", group: "*", blocked: false, user_triggered: false, hits: 39796, disallowed_hits: 0, verified_disallowed_hits: 0, disallowed_urls: 0 },
        ];
        const columns: Column[] = [
          { name: "url", kind: "text" },
          { name: "bot", kind: "text" },
          { name: "verification", kind: "text" },
          { name: "hits", kind: "integer" },
          { name: "last_seen", kind: "timestamp" },
          { name: "group", kind: "text" },
          { name: "rule", kind: "text" },
        ];
        const rows = Array.from({ length: 120 }, (_, row) => {
          const b = bots[row % 4]!;
          return [row % 3 === 0 ? `/admin/page-${row}` : valueFor("url", row), b.bot, row % 4 === 1 ? "spoofed" : "verified", 400 - row * 3, valueFor("last_seen", row), b.group, b.blocked ? "Disallow: /" : row % 3 === 0 ? "Disallow: /admin/" : "Disallow: /*?sessionid="];
        });
        return {
          report: { site: SITE, robots: String(args.robots ?? `https://${SITE}/robots.txt`), filter: String(args.filter ?? ""), sitemaps: [`https://${SITE}/sitemap.xml`], bots, rows: rows.length },
          columns,
          rows,
        };
      }
      case "geo_status":
        return { release: geoRelease, attribution: "IP Geolocation by DB-IP (https://db-ip.com)" };
      case "update_geo_data":
        geoRelease = { country: "2026-10", asn: "2026-10", fetched_at: new Date().toISOString() };
        return { release: geoRelease, country_ranges: 710834, asn_ranges: 477362 };
      case "list_custom_bots":
        return customBots;
      case "save_custom_bots": {
        const bots = (args.bots as typeof customBots).map((bot) => ({
          ...bot,
          id: bot.id.trim().toLowerCase(),
          name: bot.name.trim() || bot.id.trim().toLowerCase(),
          patterns: bot.patterns.map((p) => p.toLowerCase()),
        }));
        const bad = bots.find((bot) => !/^[a-z0-9-]+$/.test(bot.id));
        if (bad) throw { kind: "invalid-input", message: `invalid custom bots: invalid bot id \`${bad.id}\`: use lowercase letters, digits and -` };
        if (bots.some((bot) => BOTS.some(([id]) => id === bot.id))) throw { kind: "invalid-input", message: "invalid custom bots: built-in bot id" };
        customBots.splice(0, customBots.length, ...bots);
        return customBots;
      }
      case "export_custom_bots":
        return customBots.length;
      case "import_custom_bots":
        return customBots;
      case "bot_range_status":
        return ["google-common", "bing", "openai-gptbot", "anthropic", "perplexity-bot", "apple", "duckduckgo-bot"].map((id, index) => ({
          id,
          operator: id.split("-")[0],
          url: `https://example.com/${id}.json`,
          prefixes: 20 + index * 37,
          fetched_at: "2026-10-12T20:24:00Z",
          age_days: index === 3 ? 34 : 1,
          origin: index % 2 ? "bundled" : "downloaded",
        }));
      case "verify_site_bots":
        return { site: SITE, dns_used: true, dns_unavailable: false, reclassified_user_agents: 0, checked: 254, changed: 12, rebuilt_bots: ["googlebot-smartphone"], verdicts: { verified: 1512, spoofed: 520 } };
      case "verify_ip":
        return { ip: args.ip, bot: { id: args.bot, name: String(args.bot), operator: "Google", category: "search", verification: "ip-list+dns" }, verdict: String(args.ip).startsWith("66.249") ? "verified" : "spoofed", method: "ip-list" };
      case "compare_urls":
        return {
          report: { site: SITE, filter: "bot_category:search verification:verified", sources: [{ source: "sitemap.xml", urls: 803 }], skipped: 0, summary: { listed: 803, logged: 1025, both: 783, not_in_logs: 20, not_in_list: 242 }, rows: 1045 },
          ...compareTable(["both", "not-in-logs", "not-in-list"]),
        };
      case "compare_periods":
        return {
          report: { site: SITE, group: String(args.group), summary: { before_hits: 80110, after_hits: 80444, new: 3, lost: 1, up: 4, down: 2 }, rows: 6 },
          columns: [
            { name: String(args.group), kind: "text" },
            { name: "hits_before", kind: "integer" },
            { name: "hits_after", kind: "integer" },
            { name: "change", kind: "integer" },
            { name: "change_pct", kind: "float" },
            { name: "trend", kind: "text" },
          ],
          rows: [
            ["/blog/", 23503, 23638, 135, 0.57, "up"],
            ["/images/", 3942, 4076, 134, 3.4, "up"],
            ["/new-landing/", 0, 812, 812, null, "new"],
            ["/category/", 7831, 7742, -89, -1.14, "down"],
            ["/old/", 412, 0, -412, -100, "lost"],
            ["/products/", 39214, 39213, -1, 0, "down"],
          ],
        };
      case "import_logs":
        await new Promise((resolve) => setTimeout(resolve, 1500));
        return { site: SITE, format_id: "combined", files: [], inserted_rows: 0, duplicate_rows: 0, total_lines: 0, dns_verification: true, elapsed_ms: 1500, lines_per_second: 0 };
      case "cancel_import":
        return true;
      default:
        throw { kind: "error", message: `demo backend: ${command} is not available` };
    }
  });
}
