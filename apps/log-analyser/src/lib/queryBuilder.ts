// Builds query-language text from the quick filters of the UI.

export type DatePreset = "all" | "last7d" | "last30d" | "last90d" | "custom";

export interface DateFilter {
  preset: DatePreset;
  from?: string;
  to?: string;
}

export type TrafficPreset =
  | "all"
  | "search-verified"
  | "googlebot-verified"
  | "googlebot-smartphone"
  | "bingbot-verified"
  | "ai"
  | "seo"
  | "bots"
  | "humans"
  | "spoofed";

export const TRAFFIC_PRESETS: Record<TrafficPreset, string> = {
  all: "",
  "search-verified": "bot_category:search verification:verified",
  "googlebot-verified": "bot:googlebot* verification:verified",
  "googlebot-smartphone": "bot:googlebot-smartphone verification:verified",
  "bingbot-verified": "bot:bingbot verification:verified",
  ai: "bot_category:ai-*",
  seo: "bot_category:seo",
  bots: "bot:*",
  humans: "-bot:*",
  spoofed: "verification:spoofed",
};

/** `date:` value of a date filter, or null for all data. */
export function dateValue(filter: DateFilter): string | null {
  switch (filter.preset) {
    case "all":
      return null;
    case "custom": {
      const from = filter.from?.trim();
      const to = filter.to?.trim();
      if (from && to) return from === to ? from : `${from}..${to}`;
      if (from) return `${from}..9999-12-31`;
      if (to) return `0001-01-01..${to}`;
      return null;
    }
    default:
      return filter.preset;
  }
}

/** Joins non-empty query fragments with spaces. */
export function joinQuery(...parts: (string | null | undefined)[]): string {
  return parts
    .map((part) => part?.trim() ?? "")
    .filter((part) => part.length > 0)
    .join(" ");
}

/** Quick filters followed by the user's query text. */
export function composeQuery(date: DateFilter, traffic: TrafficPreset, text: string): string {
  const day = dateValue(date);
  return joinQuery(day ? `date:${day}` : null, TRAFFIC_PRESETS[traffic], text);
}

/** Splits query text at spaces outside double quotes. */
export function tokenize(text: string): string[] {
  const tokens: string[] = [];
  let current = "";
  let quoted = false;
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index]!;
    if (char === "\\" && quoted && index + 1 < text.length) {
      current += char + text[index + 1]!;
      index += 1;
      continue;
    }
    if (char === '"') quoted = !quoted;
    if (/\s/.test(char) && !quoted) {
      if (current) tokens.push(current);
      current = "";
    } else {
      current += char;
    }
  }
  if (current) tokens.push(current);
  return tokens;
}

/** Replaces the `sort:` directive of a query. */
export function withSort(text: string, column: string, direction: "asc" | "desc"): string {
  const kept = tokenize(text).filter((token) => !token.startsWith("sort:"));
  return [...kept, `sort:${column}:${direction}`].join(" ");
}

/** Current sort of a query (first key), if any. */
export function currentSort(text: string): { column: string; direction: "asc" | "desc" } | null {
  const token = tokenize(text).find((t) => t.startsWith("sort:"));
  const first = token?.slice(5).split(",")[0];
  if (!first) return null;
  const [column, direction] = first.split(":");
  if (!column) return null;
  return { column, direction: direction === "asc" ? "asc" : "desc" };
}
