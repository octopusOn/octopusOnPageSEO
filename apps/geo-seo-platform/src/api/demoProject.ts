// Sample opportunities and dashboard of the first sample project's shoe shop, for the interface
// outside the program. Decisions taken on the page are kept until it is reloaded.

import type { DatedValue, Effort, Opportunity, OpportunityList, OpportunityRule, OpportunityState, ProjectDashboard, ProjectRow } from "./types";
import type { PageId } from "../lib/route";

const SITE = "https://ornekmagaza.com";
const day = (back: number) => new Date(Date.now() - back * 86_400_000).toISOString().slice(0, 10);
const at = (ahead: number) => new Date(Date.now() + ahead * 3_600_000).toISOString();

function make(rule: OpportunityRule, page: PageId, subject: string, effort: Effort, worth: { gain?: number; impact?: number }, detail: Opportunity["detail"], params: Record<string, string> = {}): Opportunity {
  const impact = worth.gain !== undefined ? Math.max(5, Math.min(100, Math.round((100 * Math.log(1 + worth.gain)) / Math.log(5_001) * 10) / 10)) : (worth.impact ?? 30);
  const factor = effort === "low" ? 1 : effort === "medium" ? 0.75 : 0.5;
  return { key: `${rule}|${subject}`, rule, page, params, subject, detail, gain: worth.gain ?? null, impact, effort, priority: Math.round(impact * factor * 10) / 10, state: "open", until: null, returned: false };
}

const ALL: Opportunity[] = [
  make("low_ctr", "gsc", "kışlık bot", "low", { gain: 310 }, { position: 8.4, ctr: 1.5, expected: 3.4 }, { keyword: "kışlık bot" }),
  make("near_top", "rank", "spor ayakkabı", "medium", { gain: 2_740 }, { position: 6.8, volume: 90_500, source: "rank" }, { keyword: "spor ayakkabı" }),
  make("near_top", "gsc", "beyaz sneaker", "medium", { gain: 410 }, { position: 4.1, impressions: 9_800, source: "gsc" }, { keyword: "beyaz sneaker" }),
  make("dropped", "rank", "deri bot erkek", "medium", { gain: 180 }, { before: 12, volume: 4_400, lost: true }, { keyword: "deri bot erkek" }),
  make("losing_page", "gsc", `${SITE}/kosu-ayakkabisi`, "medium", { gain: 95 }, { clicks: 455, before: 585, drop: 22.2 }, { url: `${SITE}/kosu-ayakkabisi` }),
  make("cannibal", "gsc", "spor ayakkabı", "medium", { gain: 770 }, { pages: 2, impressions: 38_500 }, { keyword: "spor ayakkabı" }),
  make("audit_issue", "audit", "structured-data-missing", "medium", { impact: 69.1 }, { title: "Yapısal veri yok", count: 402 }, { tab: "issues" }),
  make("vitals", "audit", SITE, "high", { impact: 60 }, { metrics: "LCP, INP" }, { tab: "vitals" }),
  make("slow_page", "audit", `${SITE}/spor-ayakkabi`, "medium", { impact: 29.5 }, { score: 41 }, { tab: "vitals" }),
  make("orphans", "audit", "orphans", "medium", { impact: 33.5 }, { count: 6 }, { tab: "links" }),
  make("weak_links", "audit", "weak", "low", { impact: 33.8 }, { count: 23 }, { tab: "links" }),
  make("ai_gap", "ai", "En iyi koşu ayakkabısı markası hangisi?", "high", { impact: 50 }, { engines: 2, rivals: "rakipmagaza.com, sporpazari.com" }),
  make("low_content", "content", `${SITE}/kosu-ayakkabisi`, "medium", { impact: 26 }, { score: 48, keyword: "koşu ayakkabısı" }, { tab: "checker" }),
  make("local_weak", "local", "koşu ayakkabısı", "medium", { impact: 41 }, { solv: 18, arp: 9.6 }, { keyword: "koşu ayakkabısı" }),
  make("rival_growth", "gap", "rakipmagaza.com", "high", { impact: 35.6 }, { gained: 420, growth: 14, own: 3 }, { domain: "rakipmagaza.com" }),
  make("lost_links", "backlinks", "lost", "medium", { impact: 31 }, { count: 3 }, { tab: "prospects" }),
].sort((a, b) => b.priority - a.priority);

const decisions = new Map<string, { state: OpportunityState; until: string | null }>();

function list(projectId: number, all: boolean): OpportunityList {
  if (projectId !== 1) return { items: [], open: 0, hidden: {}, sources: { rank: false, gsc: false, audit: false, vitals: false, ai: false, content: false, local: false, snapshots: false }, gain: 0 };
  const hidden: Partial<Record<OpportunityState, number>> = {};
  const items: Opportunity[] = [];
  for (const item of ALL) {
    const decided = decisions.get(item.key);
    if (!decided) {
      items.push(item);
      continue;
    }
    hidden[decided.state] = (hidden[decided.state] ?? 0) + 1;
    if (all) items.push({ ...item, state: decided.state, until: decided.until });
  }
  const open = items.filter((item) => item.state === "open");
  return {
    items,
    open: open.length,
    hidden,
    sources: { rank: true, gsc: true, audit: true, vitals: true, ai: true, content: true, local: true, snapshots: true },
    gain: open.reduce((sum, item) => sum + (item.gain ?? 0), 0),
  };
}

function series(length: number, start: number, step: number, wobble: number, days = 7): DatedValue[] {
  return Array.from({ length }, (_, index) => ({ date: day((length - 1 - index) * days), value: Math.round((start + step * index + (index % 3 === 1 ? wobble : 0)) * 10) / 10 }));
}

function dashboard(project: ProjectRow): ProjectDashboard {
  const first = project.id === 1;
  const found = list(project.id, false);
  return {
    project,
    organic: first ? { metrics: { keywords: 12_400, traffic: 48_200, traffic_value_usd: 21_300, pos_1: 310, pos_2_3: 640, pos_4_10: 1_820 }, fetched_at: at(-26), traffic: series(12, 40_100, 720, -900, 30), keywords: series(12, 10_900, 130, -200, 30) } : null,
    links: first ? { links: { rank: 41, backlinks: 18_600, referring_domains: 1_240, broken_backlinks: 37 }, fetched_at: at(-50), referring_domains: series(12, 1_080, 14, -12, 30) } : null,
    rank: first ? { visibility: 23.4, visibility_change: 1.8, average_position: 9.6, top3: 8, top10: 21, ranked: 38, checked: 42, improved: 14, declined: 6, lost: 1, traffic: 6_820, series: series(12, 17.2, 0.55, -0.8) } : null,
    search: first ? { period: [day(30), day(3)], clicks: 4_520, clicks_before: 4_100, impressions: 136_400, impressions_before: 128_900, ctr: 3.3, position: 7.4, position_before: 8.1, series: series(28, 140, 1.3, 25, 1) } : null,
    audit: first ? { audit_id: 2, health: 78.4, health_before: 74.9, geo: 61.5, issues: 1_264, issues_before: 1_402, pages: 842, finished_at: at(-30), series: series(6, 68.2, 2, -1.2) } : null,
    ai: first ? { answers: 120, mention_rate: 42, citation_rate: 18, share_of_voice: 19.5, series: series(10, 30, 1.3, -2) } : null,
    local: first ? { arp: 4.2, solv: 46, top3: 22, points: 49, series: series(8, 34, 1.6, -3) } : null,
    vitals: first
      ? {
          devices: [
            { device: "phone", passes: false, lcp: 2_840, inp: 230, cls: 0.06, day: day(0) },
            { device: "desktop", passes: true, lcp: 1_760, inp: 90, cls: 0.04, day: day(0) },
          ],
        }
      : null,
    content: first ? { briefs: 2, scored_pages: 1, average_score: 48 } : { briefs: 0, scored_pages: 0, average_score: null },
    opportunities: found.items.slice(0, 10),
    opportunities_open: found.open,
    opportunities_gain: found.gain,
    schedule: first
      ? [
          { module: "gsc", at: at(5) },
          { module: "rank", at: at(19) },
          { module: "local", at: at(40) },
          { module: "audit", at: at(70) },
          { module: "ai", at: at(120) },
        ]
      : [],
    spend: first
      ? [
          ["rank", 1.12],
          ["ai", 0.84],
          ["overview", 0.42],
          ["local", 0.19],
        ]
      : [],
    spend_total: first ? 2.57 : 0,
  };
}

export function projectDemo(command: string, args: Record<string, unknown>, projects: ProjectRow[]): unknown {
  const projectId = Number(args.project_id);
  switch (command) {
    case "opportunities":
      return list(projectId, args.all === true);
    case "set_opportunity": {
      const key = String(args.key ?? "");
      const state = String(args.state ?? "open") as OpportunityState;
      if (state === "open") decisions.delete(key);
      else decisions.set(key, { state, until: state === "snoozed" ? at(24 * Number(args.days ?? 7)) : null });
      return null;
    }
    case "project_dashboard": {
      const project = projects.find((row) => row.id === projectId);
      return project ? dashboard(project) : null;
    }
    default:
      return undefined;
  }
}
