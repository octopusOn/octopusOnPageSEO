// Sample alerts of the sample projects and their settings, for the interface outside the
// program. What is read, saved or found on the page is kept until it is reloaded.

import type { AlertList, AlertRow, AlertSettings, AlertSetup, ProjectRow } from "./types";

const ago = (hours: number) => new Date(Date.now() - hours * 3_600_000).toISOString();

const DEFAULTS: AlertSettings = {
  enabled: true,
  rules: { rank_drop: 5, top10_exit: true, visibility_drop: 10, gsc_drop: 20, health_drop: 5, audit_issues: true, ai_change: 10, local_drop: 10, reviews: true, rivals: true, links: false },
  email: "off",
  recipients: [],
  lang: "",
  last_digest_at: null,
  links_last_run: null,
};

let next = 1;
function row(projectId: number, project: string, hours: number, alert: Pick<AlertRow, "kind" | "severity" | "page" | "params" | "detail">, read = false): AlertRow {
  const id = next++;
  return { id, project_id: projectId, project, key: `${alert.kind}|${id}`, created_at: ago(hours), read_at: read ? ago(hours - 1) : null, ...alert };
}

let alerts: AlertRow[] = [
  row(1, "Örnek Mağaza", 2, { kind: "rank_drop", severity: "bad", page: "rank", params: {}, detail: { count: 3, top10: 1, keywords: "deri bot erkek (8→15), kışlık bot (12→19), yürüyüş ayakkabısı (6→11)", tracker: "Google · Türkiye · mobil" } }),
  row(1, "Örnek Mağaza", 5, { kind: "review", severity: "bad", page: "local", params: { tab: "reviews" }, detail: { rating: 2, author: "Ayşe K.", text: "Kargo geç geldi, numara küçük çıktı…" } }),
  row(1, "Örnek Mağaza", 26, { kind: "gsc_drop", severity: "bad", page: "gsc", params: {}, detail: { drop: 22.4, before: 5_820, clicks: 4_516 } }),
  row(1, "Örnek Mağaza", 30, { kind: "ai_gain", severity: "good", page: "ai", params: {}, detail: { change: 12, before: 30, now: 42 } }),
  row(1, "Örnek Mağaza", 52, { kind: "audit_issues", severity: "bad", page: "audit", params: { tab: "issues" }, detail: { count: 2, titles: "Kırık iç linkler, Yönlendirme zinciri" } }, true),
  row(1, "Örnek Mağaza", 74, { kind: "rival_growth", severity: "info", page: "gap", params: { domain: "rakipmagaza.com" }, detail: { domain: "rakipmagaza.com", growth: 14, gained: 420, own: 3 } }, true),
  row(1, "Örnek Mağaza", 170, { kind: "links", severity: "good", page: "backlinks", params: { tab: "prospects" }, detail: { new: 6, lost: 3, domains: "modablog.com, kosuforum.net, sporhaber.com.tr" } }, true),
  row(2, "example.org", 8, { kind: "visibility_drop", severity: "bad", page: "rank", params: {}, detail: { drop: 12.5, before: 18.4, now: 16.1, tracker: "Google · United States · desktop" } }),
];

const settings = new Map<number, AlertSettings>();

function list(projectId: number | null, unread: boolean, limit: number): AlertList {
  const byProject: Record<string, number> = {};
  for (const alert of alerts) if (!alert.read_at) byProject[alert.project_id] = (byProject[alert.project_id] ?? 0) + 1;
  const shown = alerts.filter((alert) => (projectId === null || alert.project_id === projectId) && (!unread || !alert.read_at)).slice(0, limit);
  return { alerts: shown, unread: Object.values(byProject).reduce((sum, n) => sum + n, 0), by_project: byProject };
}

function setup(projectId: number): AlertSetup {
  return { settings: settings.get(projectId) ?? DEFAULTS, can_email: true, can_notify: false, evaluates_here: true, links_usd: 0.05, links_estimate: { usd: 0.05, credits: 5, via: "credit", kept: null, blocked: null } };
}

export function alertsDemo(command: string, args: Record<string, unknown>, projects: ProjectRow[]): unknown {
  const projectId = args.project_id === undefined ? null : Number(args.project_id);
  switch (command) {
    case "list_alerts":
      return list(projectId, args.unread === true, Number(args.limit ?? 50));
    case "mark_alerts_read": {
      const ids = Array.isArray(args.ids) ? (args.ids as number[]) : null;
      let marked = 0;
      alerts = alerts.map((alert) => {
        if (alert.read_at || (projectId !== null && alert.project_id !== projectId) || (ids && !ids.includes(alert.id))) return alert;
        marked += 1;
        return { ...alert, read_at: new Date().toISOString() };
      });
      return marked;
    }
    case "alert_setup":
      return projectId !== null && projects.some((project) => project.id === projectId) ? setup(projectId) : null;
    case "save_alert_settings": {
      const saved = args.settings as AlertSettings;
      if (projectId !== null) settings.set(projectId, saved);
      return saved;
    }
    case "check_alerts":
      return 0;
    case "start_link_watch":
      return "demo-link-watch";
    default:
      return undefined;
  }
}
