import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { BarChart3, Bot, CalendarRange, FileUp, GitCompare, LayoutDashboard, ScrollText, Settings2, ShieldBan } from "lucide-react";

import { api } from "./api/commands";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { Sidebar } from "./components/Sidebar";
import { SiteSettingsDialog } from "./components/SiteDialogs";
import { Button, Empty } from "./components/ui";
import { formatDateTime, formatNumber } from "./lib/format";
import { describeSpec } from "./lib/formatSpec";
import { useI18n } from "./i18n";
import type { MessageKey } from "./i18n/en";
import { BotsPage } from "./pages/BotsPage";
import { ImportPage } from "./pages/ImportPage";
import { OverviewPage } from "./pages/OverviewPage";
import { PeriodsPage } from "./pages/PeriodsPage";
import { ReportsPage } from "./pages/ReportsPage";
import { RobotsPage } from "./pages/RobotsPage";
import { SettingsPage, useTheme } from "./pages/SettingsPage";
import { UrlComparePage } from "./pages/UrlComparePage";
import { keys, useAppState, type Page } from "./state";

const TABS: { page: Exclude<Page, "settings">; label: MessageKey; icon: typeof BarChart3 }[] = [
  { page: "overview", label: "nav.overview", icon: LayoutDashboard },
  { page: "reports", label: "nav.reports", icon: BarChart3 },
  { page: "import", label: "nav.import", icon: FileUp },
  { page: "bots", label: "nav.bots", icon: Bot },
  { page: "robots", label: "nav.robots", icon: ShieldBan },
  { page: "urls", label: "nav.urls", icon: GitCompare },
  { page: "periods", label: "nav.periods", icon: CalendarRange },
];

export function App() {
  const [theme, setTheme] = useTheme();
  const { site, page } = useAppState();
  const sites = useQuery({ queryKey: keys.sites, queryFn: api.listSites });
  const current = sites.data?.find((s) => s.name === site) ?? null;

  let content;
  if (page === "settings") {
    content = <SettingsPage theme={theme} setTheme={setTheme} />;
  } else if (!current) {
    content = <Welcome />;
  } else {
    content = (
      <>
        <SiteHeader site={current.name} />
        {page === "overview" && <OverviewPage key={current.name} site={current.name} />}
        {page === "reports" && <ReportsPage key={current.name} site={current.name} />}
        {page === "import" && <ImportPage key={current.name} site={current.name} />}
        {page === "bots" && <BotsPage key={current.name} site={current.name} />}
        {page === "robots" && <RobotsPage key={current.name} site={current.name} />}
        {page === "urls" && <UrlComparePage key={current.name} site={current.name} />}
        {page === "periods" && <PeriodsPage key={current.name} site={current.name} />}
      </>
    );
  }
  return (
    <div className="app">
      <Sidebar />
      <main className="main">
        <ErrorBoundary resetKey={`${site}:${page}`}>{content}</ErrorBoundary>
      </main>
    </div>
  );
}

function SiteHeader({ site }: { site: string }) {
  const { t, locale } = useI18n();
  const { page, setPage } = useAppState();
  const sites = useQuery({ queryKey: keys.sites, queryFn: api.listSites });
  const overview = useQuery({ queryKey: keys.overview(site), queryFn: () => api.siteOverview(site) });
  const [editing, setEditing] = useState(false);
  const info = sites.data?.find((s) => s.name === site);
  const data = overview.data;
  return (
    <header className="header">
      <div className="header-top">
        <div className="grow">
          <h1>{site}</h1>
          <div className="header-meta">
            {info?.client} · {describeSpec(info?.format ?? "auto", t("site.formatAuto"))}
            {data && data.hits > 0 && (
              <>
                {" · "}
                {t("overview.dataLine", {
                  hits: formatNumber(data.hits, locale),
                  urls: formatNumber(data.urls, locale),
                  imports: formatNumber(data.imports, locale),
                })}
                {" · "}
                {t("overview.range", {
                  first: formatDateTime(data.first_request, locale),
                  last: formatDateTime(data.last_request, locale),
                })}
              </>
            )}
          </div>
        </div>
        <Button variant="ghost" icon={<Settings2 size={16} />} title={t("site.settings")} onClick={() => setEditing(true)} />
      </div>
      <nav className="tabs">
        {TABS.map(({ page: tab, label, icon: Icon }) => (
          <button key={tab} type="button" className={`tab ${page === tab ? "active" : ""}`} onClick={() => setPage(tab)}>
            <Icon size={15} />
            {t(label)}
          </button>
        ))}
      </nav>
      {editing && info && <SiteSettingsDialog site={info} onClose={() => setEditing(false)} />}
    </header>
  );
}

function Welcome() {
  const { t } = useI18n();
  return (
    <div className="page">
      <Empty icon={<ScrollText size={44} />} title={t("welcome.title")}>
        <p style={{ maxWidth: 560 }}>{t("welcome.body")}</p>
        <ol style={{ textAlign: "left" }}>
          <li>{t("welcome.step1")}</li>
          <li>{t("welcome.step2")}</li>
          <li>{t("welcome.step3")}</li>
        </ol>
      </Empty>
    </div>
  );
}
