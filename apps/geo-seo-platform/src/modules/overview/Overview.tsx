import { useEffect, useState, type FormEvent } from "react";
import { Search } from "lucide-react";
import { Button } from "@octopusseo/ui";
import type { ProjectRow } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { domainOf } from "../../lib/Entity";
import { MarketPicker } from "../../lib/MarketPicker";
import { KeywordsReport } from "./KeywordsReport";
import { ChangesReport, PaidReport } from "./ChangeReports";
import { Summary } from "./Summary";
import { CompetitorsReport, PagesReport, SubdomainsReport, SubfoldersReport } from "./TableReports";

const TABS = ["summary", "keywords", "changes", "pages", "folders", "competitors", "subdomains", "paid"] as const;
type Tab = (typeof TABS)[number];

const TAB_LABELS: Record<Tab, string> = {
  summary: "overview.tabSummary",
  keywords: "overview.tabKeywords",
  changes: "overview.tabChanges",
  pages: "overview.tabPages",
  folders: "overview.tabFolders",
  competitors: "overview.tabCompetitors",
  subdomains: "overview.tabSubdomains",
  paid: "overview.tabPaid",
};

function storedTab(): Tab {
  try {
    const saved = localStorage.getItem("platform.overview.tab");
    return (TABS as readonly string[]).includes(saved ?? "") ? (saved as Tab) : "summary";
  } catch {
    return "summary";
  }
}

export function Overview({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const { params, go } = useShell();
  // The domain and tab live in the address, so links and the back button reach them.
  const asked = params.domain ? domainOf(params.domain) : "";
  const domain = asked && asked !== project.domain ? asked : undefined;
  const tab: Tab = (TABS as readonly string[]).includes(params.tab ?? "") ? (params.tab as Tab) : storedTab();
  const [input, setInput] = useState(domain ?? "");
  useEffect(() => setInput(domain ?? ""), [domain]);
  const setTab = (next: Tab) => {
    try {
      localStorage.setItem("platform.overview.tab", next);
    } catch {
      // Not remembered without storage.
    }
    // A page's keywords belong to the keywords tab only.
    go({ params: { ...params, tab: next, url: undefined } });
  };
  const analyze = (next: string) => {
    const value = domainOf(next);
    setInput(value);
    go({ params: { ...params, domain: value && value !== project.domain ? value : undefined } });
  };
  const submit = (event: FormEvent) => {
    event.preventDefault();
    analyze(input);
  };
  const shown = domain ?? project.domain;
  return (
    <div className="page-stack">
      <form className="oc-card domain-bar" onSubmit={submit}>
        <label className="domain-input">
          <span className="sr-only">{t("overview.domainLabel")}</span>
          <Search size={16} aria-hidden="true" />
          <input
            className="oc-input"
            value={input}
            onChange={(event) => setInput(event.target.value)}
            placeholder={project.domain}
            spellCheck={false}
            autoComplete="off"
            list="overview-domains"
          />
          <datalist id="overview-domains">
            {[project.domain, ...project.competitors].map((value) => (
              <option key={value} value={value} />
            ))}
          </datalist>
        </label>
        <Button type="submit">{t("overview.analyze")}</Button>
        <span className="domain-bar-target">
          <strong>{shown}</strong>
          <small className="faint">{domain ? t("overview.competitor") : t("overview.projectDomain")}</small>
        </span>
        {domain && (
          <Button small className="ghost" onClick={() => analyze("")}>
            {t("overview.backToProject")}
          </Button>
        )}
        <MarketPicker project={project} />
      </form>
      <div className="oc-tabs" role="tablist" aria-label={t("overview.domainLabel")}>
        {TABS.map((entry) => (
          <button
            key={entry}
            type="button"
            role="tab"
            aria-selected={tab === entry}
            className={`oc-tab ${tab === entry ? "active" : ""}`}
            onClick={() => setTab(entry)}
          >
            {t(TAB_LABELS[entry])}
          </button>
        ))}
      </div>
      <div role="tabpanel">
        {tab === "summary" && <Summary key={`${shown}|${params.market ?? ""}`} project={project} domain={domain} />}
        {tab === "keywords" && <KeywordsReport key={shown} projectId={project.id} domain={domain} shownDomain={shown} />}
        {tab === "changes" && <ChangesReport key={shown} projectId={project.id} domain={domain} shownDomain={shown} />}
        {tab === "pages" && <PagesReport key={shown} projectId={project.id} domain={domain} shownDomain={shown} />}
        {tab === "folders" && <SubfoldersReport key={shown} projectId={project.id} domain={domain} shownDomain={shown} />}
        {tab === "competitors" && <CompetitorsReport key={shown} project={project} domain={domain} onAnalyze={(next) => { analyze(next); setTab("summary"); }} />}
        {tab === "subdomains" && <SubdomainsReport key={shown} projectId={project.id} domain={domain} />}
        {tab === "paid" && <PaidReport key={shown} projectId={project.id} domain={domain} shownDomain={shown} />}
      </div>
    </div>
  );
}
