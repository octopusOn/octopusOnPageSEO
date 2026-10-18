import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ScrollText, Settings2 } from "lucide-react";

import { api } from "./api/commands";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { Sidebar } from "./components/Sidebar";
import { SiteSettingsDialog } from "./components/SiteDialogs";
import { Button, Empty } from "./components/ui";
import { formatDateTime, formatNumber } from "./lib/format";
import { describeSpec } from "./lib/formatSpec";
import { useI18n } from "./i18n";
import { BotsPage } from "./pages/BotsPage";
import { ImportPage } from "./pages/ImportPage";
import { OverviewPage } from "./pages/OverviewPage";
import { PeriodsPage } from "./pages/PeriodsPage";
import { ReportsPage } from "./pages/ReportsPage";
import { RobotsPage } from "./pages/RobotsPage";
import { SettingsPage } from "./pages/SettingsPage";
import { UrlComparePage } from "./pages/UrlComparePage";
import { ALL_PAGES } from "./components/nav";
import { keys, useAppState } from "./state";

export function App() {
  const { site, page } = useAppState();
  const sites = useQuery({ queryKey: keys.sites, queryFn: api.listSites });
  const current = sites.data?.find((s) => s.name === site) ?? null;

  let content;
  if (page === "settings") {
    content = (
      <>
        <PageTitle />
        <SettingsPage />
      </>
    );
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
    <div className="oc-shell oc-ground">
      <Sidebar />
      <main className="oc-work">
        <div className="oc-titlebar" data-tauri-drag-region />
        <ErrorBoundary resetKey={`${site}:${page}`}>{content}</ErrorBoundary>
      </main>
    </div>
  );
}

/** The page title on its own (pages without a site, such as Settings). */
function PageTitle() {
  const { t } = useI18n();
  const { page } = useAppState();
  const nav = ALL_PAGES.find((item) => item.page === page);
  return (
    <header className="oc-pagehead" data-tauri-drag-region>
      <div className="oc-pagehead-text">
        <h1 className="oc-pagehead-title">{nav ? t(nav.label) : ""}</h1>
      </div>
    </header>
  );
}

/** The page title of a site module, with the site and its data as the lead. */
function SiteHeader({ site }: { site: string }) {
  const { t, locale } = useI18n();
  const { page } = useAppState();
  const sites = useQuery({ queryKey: keys.sites, queryFn: api.listSites });
  const overview = useQuery({ queryKey: keys.overview(site), queryFn: () => api.siteOverview(site) });
  const [editing, setEditing] = useState(false);
  const info = sites.data?.find((s) => s.name === site);
  const data = overview.data;
  const nav = ALL_PAGES.find((item) => item.page === page);
  return (
    <header className="oc-pagehead" data-tauri-drag-region>
      <div className="oc-pagehead-text">
        <h1 className="oc-pagehead-title">{nav ? t(nav.label) : site}</h1>
        <p className="oc-pagehead-lead">
          <b>{site}</b> · {info?.client} · {describeSpec(info?.format ?? "auto", t("site.formatAuto"))}
          {data && data.hits > 0 && (
            <>
              <br />
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
        </p>
      </div>
      <div className="oc-pagehead-actions">
        <Button icon={<Settings2 size={16} />} onClick={() => setEditing(true)}>
          {t("site.settings")}
        </Button>
      </div>
      {editing && info && <SiteSettingsDialog site={info} onClose={() => setEditing(false)} />}
    </header>
  );
}

function Welcome() {
  const { t } = useI18n();
  return (
    <div className="page">
      <Empty icon={<ScrollText size={36} />} title={t("welcome.title")}>
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
