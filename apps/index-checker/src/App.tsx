import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { save } from "@tauri-apps/plugin-dialog";
import { ArrowDownToLine, Copy, Pause, Play, Square, X } from "lucide-react";
import { api, errorText, isDemo, isDemoScanId } from "./api/client";
import {
  buildLiveOverviewFromJob,
  computeScanProgress,
  defaultConfig,
  downloadTextFile,
  formatVerifiedUrlsAsCsv,
  queryLiveRows,
} from "./api/demo";
import type {
  FinderJob,
  Page,
  RedirectServerFormat,
  ScanConfig,
  VerifiedUrl,
} from "./api/types";
import { useI18n, percent, persist, stored } from "./i18n";
import { Sidebar } from "./components/Sidebar";
import { NewScanModal } from "./components/NewScanModal";
import { Overview } from "./pages/Overview";
import { Explorer } from "./pages/Explorer";
import { EnginesPage } from "./pages/EnginesPage";
import { Sessions } from "./pages/Sessions";
import { Settings } from "./pages/Settings";

export function App() {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const [page, setPage] = useState<Page>("overview");
  const [selectedId, setSelectedId] = useState<string>(() =>
    stored("finder.selected", ""),
  );
  const [config, setConfig] = useState<ScanConfig>(() =>
    stored("finder.config.v2", defaultConfig),
  );
  const [category, setCategory] = useState<string>("issues");
  const [engine, setEngine] = useState<string>("");
  const [search, setSearch] = useState<string>("");
  const [maxWords, setMaxWords] = useState<number | null>(null);
  const [sortBy, setSortBy] = useState<string>("priority");
  const [offset, setOffset] = useState<number>(0);
  const pageLimit = 100;
  const [selectedRow, setSelectedRow] = useState<VerifiedUrl | null>(null);
  const [newScanOpen, setNewScanOpen] = useState<boolean>(false);
  const [startingScan, setStartingScan] = useState<boolean>(false);
  const [redirectsModal, setRedirectsModal] = useState<{
    server: RedirectServerFormat;
    rules: string;
  } | null>(null);
  const [toast, setToast] = useState<{ text: string; error: boolean } | null>(
    null,
  );
  const toastTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );

  const notify = useCallback((text: string, error = false) => {
    setToast({ text, error });
    clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setToast(null), 4500);
  }, []);

  useEffect(() => () => clearTimeout(toastTimer.current), []);

  const scansQuery = useQuery({
    queryKey: ["finder-scans"],
    queryFn: api.scans,
  });
  const scans = scansQuery.data ?? [];
  const activeScan =
    scans.find((s) => s.id === selectedId) ?? scans[0] ?? null;

  const jobsQuery = useQuery({
    queryKey: ["finder-jobs"],
    queryFn: api.jobs,
    refetchInterval: isDemo ? false : 2000,
  });
  const jobs = jobsQuery.data ?? [];
  const activeJobs = jobs.filter(
    (j) => j.status === "running" || j.status === "paused",
  );
  const runningJob = activeJobs[0] ?? null;
  const isScanning = Boolean(runningJob) || startingScan;
  const runningProgress = runningJob ? computeScanProgress(runningJob, t) : null;

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void api
      .watch((job: FinderJob) => {
        queryClient.setQueryData<FinderJob[]>(["finder-jobs"], (prev = []) => {
          const exists = prev.some((p) => p.id === job.id);
          return exists
            ? prev.map((p) => (p.id === job.id ? job : p))
            : [job, ...prev];
        });
        if (job.meta) {
          setSelectedId(job.meta.id);
          persist("finder.selected", job.meta.id);
          void queryClient.invalidateQueries({ queryKey: ["finder-scans"] });
          void queryClient.invalidateQueries({ queryKey: ["finder-overview"] });
          void queryClient.invalidateQueries({ queryKey: ["finder-page"] });
        } else if (job.error) {
          notify(job.error, true);
        }
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [queryClient, notify]);

  const overviewQuery = useQuery({
    queryKey: ["finder-overview", activeScan?.id],
    queryFn: () => api.overview(activeScan!.id),
    enabled: !!activeScan,
  });

  const pageQuery = useQuery({
    queryKey: [
      "finder-page",
      activeScan?.id,
      category,
      engine,
      search,
      maxWords,
      sortBy,
      offset,
    ],
    queryFn: () =>
      api.page(activeScan!.id, {
        category,
        engine,
        search,
        max_words: maxWords,
        sort_by: sortBy,
        offset,
        limit: pageLimit,
      }),
    enabled: !!activeScan,
  });

  const effectiveOverview = runningJob
    ? buildLiveOverviewFromJob(runningJob, config)
    : overviewQuery.data;

  const effectivePageData = runningJob
    ? queryLiveRows(runningJob.progress.live_rows ?? [], {
        category,
        engine,
        search,
        max_words: maxWords,
        sort_by: sortBy,
        offset,
        limit: pageLimit,
      })
    : (pageQuery.data ?? { total: 0, rows: [] });

  const issueCount = runningJob
    ? runningJob.progress.found_404 +
      runningJob.progress.found_4xx +
      runningJob.progress.found_5xx +
      runningJob.progress.found_thin_soft404
    : activeScan
      ? activeScan.count_404 +
        activeScan.count_4xx +
        activeScan.count_5xx +
        activeScan.count_soft_404 +
        activeScan.count_thin_200
      : 0;

  const handleStartScan = async (nextConfig: ScanConfig) => {
    if (isScanning) return;
    setStartingScan(true);
    setConfig(nextConfig);
    persist("finder.config.v2", nextConfig);
    setNewScanOpen(false);
    try {
      await api.start(nextConfig);
      await jobsQuery.refetch();
      setPage("overview");
    } catch (err) {
      notify(isDemo ? t("demoScanBlocked") : errorText(err), true);
    } finally {
      setStartingScan(false);
    }
  };

  const handleOpenRedirects = async (
    server: RedirectServerFormat = "nginx",
  ) => {
    if (!activeScan) return;
    try {
      const rules = await api.redirectRules(activeScan.id, server);
      setRedirectsModal({ server, rules });
    } catch (err) {
      notify(errorText(err), true);
    }
  };

  const handleReverify = async (url?: string) => {
    if (!activeScan) return;
    try {
      await api.reverifyScan(activeScan.id, url);
      await queryClient.invalidateQueries({ queryKey: ["finder-scans"] });
      await queryClient.invalidateQueries({ queryKey: ["finder-overview"] });
      await queryClient.invalidateQueries({ queryKey: ["finder-page"] });
      notify(t("reverifiedToast"));
    } catch (err) {
      notify(errorText(err), true);
    }
  };

  const handleUpdateTarget = async (url: string, target: string) => {
    if (!activeScan) return;
    try {
      await api.updateRedirectTarget(activeScan.id, url, target);
      if (selectedRow && selectedRow.url === url) {
        setSelectedRow({ ...selectedRow, suggested_target: target.trim() });
      }
      await queryClient.invalidateQueries({ queryKey: ["finder-overview"] });
      await queryClient.invalidateQueries({ queryKey: ["finder-page"] });
      notify(t("targetSavedToast"));
    } catch (err) {
      notify(errorText(err), true);
    }
  };

  const handleExport = async () => {
    if (!activeScan) return;
    if (isDemoScanId(activeScan.id)) {
      const fullPage = await api.page(activeScan.id, {
        category,
        engine,
        search,
        max_words: maxWords,
        sort_by: sortBy,
        offset: 0,
        limit: 5000,
      });
      const csv = formatVerifiedUrlsAsCsv(fullPage.rows);
      downloadTextFile(
        `${activeScan.domain}-404-report.csv`,
        csv,
        "text/csv;charset=utf-8",
      );
      notify(t("exportedUrls", { count: fullPage.rows.length }));
      return;
    }
    try {
      const destination = await save({
        defaultPath: `${activeScan.domain}-404-report.xlsx`,
        filters: [
          { name: "Excel", extensions: ["xlsx"] },
          { name: "CSV", extensions: ["csv"] },
          { name: "JSON", extensions: ["json"] },
        ],
      });
      if (!destination) return;
      const rows = await api.exportScan(
        activeScan.id,
        {
          category,
          engine,
          search,
          max_words: maxWords,
          sort_by: sortBy,
          offset: 0,
          limit: 5000,
        },
        destination,
      );
      notify(t("exportedUrls", { count: rows }));
    } catch (err) {
      notify(errorText(err), true);
    }
  };

  const copyText = (text: string) => {
    void navigator.clipboard?.writeText(text);
    notify(t("copied"));
  };

  return (
    <div className="oc-shell oc-ground finder-shell">
      <Sidebar
        page={page}
        onPage={setPage}
        scans={scans}
        selectedId={activeScan?.id ?? ""}
        onSelect={(id) => {
          setSelectedId(id);
          persist("finder.selected", id);
        }}
        activeJobs={activeJobs}
        issueCount={issueCount}
      />

      <main className="oc-work">
        <div className="oc-titlebar" data-tauri-drag-region />

        {isDemo && (
          <div className="oc-banner info" style={{ margin: "0 28px 12px" }}>
            <span>{t("demoBanner")}</span>
          </div>
        )}

        {runningJob && runningProgress && (
          <div className="live-job-bar oc-panel" style={{ margin: "0 28px 14px" }}>
            <div className="row between">
              <div>
                <b>
                  {runningJob.domain} — {runningProgress.phaseTitle} (
                  {percent(runningProgress.percent, lang)})
                </b>
                <div className="muted mono" style={{ fontSize: 12 }}>
                  {runningJob.progress.current_item || "..."} ·{" "}
                  {runningProgress.detailText}
                </div>
              </div>
              <div className="row" style={{ gap: 12 }}>
                <span className="oc-badge info">
                  {t("liveIndexed")}: {runningJob.progress.discovered_urls}
                </span>
                <span className="oc-badge">
                  {t("liveChecked")}: {runningJob.progress.verified_urls}
                </span>
                <span className="oc-badge critical">
                  404: {runningJob.progress.found_404}
                </span>
                <span className="oc-badge warning">
                  {t("liveThinSoft")}: {runningJob.progress.found_thin_soft404}
                </span>
                {runningJob.status === "paused" ? (
                  <button
                    className="oc-btn small"
                    onClick={() => void api.control(runningJob.id, "resume")}
                  >
                    <Play size={13} />
                    {t("resume")}
                  </button>
                ) : (
                  <button
                    className="oc-btn small"
                    onClick={() => void api.control(runningJob.id, "pause")}
                  >
                    <Pause size={13} />
                    {t("pause")}
                  </button>
                )}
                <button
                  className="oc-btn danger small"
                  onClick={() => void api.control(runningJob.id, "stop")}
                >
                  <Square size={13} />
                  {t("stop")}
                </button>
              </div>
            </div>
            <div
              className="scan-progress-track"
              role="progressbar"
              aria-valuenow={runningProgress.percent}
              aria-valuemin={0}
              aria-valuemax={100}
            >
              <div
                className="scan-progress-fill"
                style={{ width: `${runningProgress.percent}%` }}
              />
            </div>
          </div>
        )}

        <div className="finder-content">
          {page === "overview" &&
            (effectiveOverview ? (
              <Overview
                overview={effectiveOverview}
                config={config}
                onChangeConfig={(next) => {
                  setConfig(next);
                  persist("finder.config.v2", next);
                }}
                runningJob={runningJob}
                isScanning={isScanning}
                onOpenExplorer={(cat, eng, searchPrefix) => {
                  setCategory(cat);
                  if (eng !== undefined) setEngine(eng);
                  if (searchPrefix !== undefined) setSearch(searchPrefix);
                  setOffset(0);
                  setPage("explorer");
                }}
                onInspectUrl={(row) => {
                  setCategory("issues");
                  setSelectedRow(row);
                  setPage("explorer");
                }}
                onNewScan={(domainOverride) => {
                  if (isScanning) return;
                  if (domainOverride) {
                    setConfig((prev) => ({ ...prev, domain: domainOverride }));
                  }
                  setNewScanOpen(true);
                }}
                onQuickStartScan={(domain) =>
                  void handleStartScan({ ...config, domain })
                }
                onOpenRedirects={() => void handleOpenRedirects("nginx")}
                onReverify={() => void handleReverify()}
                onUpdateTarget={(url, target) =>
                  void handleUpdateTarget(url, target)
                }
              />
            ) : (
              <div className="finder-page">
                <div className="oc-panel" style={{ padding: 28 }}>
                  <h1 className="oc-pagehead-title" style={{ marginBottom: 8 }}>
                    {t("overview")}
                  </h1>
                  <p className="muted" style={{ marginBottom: 18 }}>
                    {overviewQuery.isLoading || scansQuery.isLoading
                      ? t("loadingScans")
                      : t("noScansLead")}
                  </p>
                  <button
                    className="oc-btn primary"
                    disabled={isScanning}
                    onClick={() => {
                      if (!isScanning) setNewScanOpen(true);
                    }}
                  >
                    <Play size={15} />
                    {isScanning ? t("scanRunning") : t("newScan")}
                  </button>
                </div>
              </div>
            ))}

          {page === "explorer" && (
            <Explorer
              scanId={runningJob?.scan_id ?? activeScan?.id ?? ""}
              category={category}
              onCategory={(c) => {
                setCategory(c);
                setOffset(0);
              }}
              engine={engine}
              onEngine={(e) => {
                setEngine(e);
                setOffset(0);
              }}
              search={search}
              onSearch={(s) => {
                setSearch(s);
                setOffset(0);
              }}
              maxWords={maxWords}
              onMaxWords={(v) => {
                setMaxWords(v);
                setOffset(0);
              }}
              sortBy={sortBy}
              onSortBy={(s) => {
                setSortBy(s);
                setOffset(0);
              }}
              offset={offset}
              limit={pageLimit}
              onOffset={setOffset}
              pageData={effectivePageData}
              selectedRow={selectedRow}
              onSelectRow={setSelectedRow}
              onExport={() => void handleExport()}
              onCopyText={copyText}
              onReverify={(url) => void handleReverify(url)}
              onUpdateTarget={(url, target) => void handleUpdateTarget(url, target)}
            />
          )}

          {page === "engines" && (
            <EnginesPage
              config={config}
              isScanning={isScanning}
              onChangeConfig={setConfig}
              onSave={() => {
                persist("finder.config.v2", config);
                notify(t("savedToast"));
              }}
              onStartScan={() => {
                if (!isScanning) setNewScanOpen(true);
              }}
              notify={notify}
            />
          )}

          {page === "sessions" && (
            <Sessions
              scans={scans}
              selectedId={activeScan?.id ?? ""}
              onSelect={(id) => {
                setSelectedId(id);
                persist("finder.selected", id);
                setPage("overview");
              }}
              onDelete={async (id) => {
                await api.deleteScan(id);
                await scansQuery.refetch();
              }}
              onCopyText={copyText}
            />
          )}

          {page === "settings" && (
            <Settings
              config={config}
              onChangeConfig={setConfig}
              onSave={() => {
                persist("finder.config.v2", config);
                notify(t("savedToast"));
              }}
            />
          )}
        </div>
      </main>

      {newScanOpen && (
        <NewScanModal
          initialConfig={config}
          isScanning={isScanning}
          onClose={() => setNewScanOpen(false)}
          onStart={(next) => void handleStartScan(next)}
        />
      )}

      {redirectsModal && (
        <div className="oc-scrim" onClick={() => setRedirectsModal(null)}>
          <div
            className="oc-modal"
            style={{ maxWidth: 880, width: "94vw" }}
            onClick={(e) => e.stopPropagation()}
          >
            <div className="row between" style={{ marginBottom: 14 }}>
              <h2 className="type-card-title">{t("redirectRules")}</h2>
              <div className="row" style={{ gap: 6, flexWrap: "wrap" }}>
                <button
                  className={`oc-btn small ${redirectsModal.server === "nginx" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("nginx")}
                >
                  Nginx (301)
                </button>
                <button
                  className={`oc-btn small ${redirectsModal.server === "nginx_cluster" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("nginx_cluster")}
                >
                  {t("formatNginxCluster")}
                </button>
                <button
                  className={`oc-btn small ${redirectsModal.server === "apache" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("apache")}
                >
                  Apache (.htaccess)
                </button>
                <button
                  className={`oc-btn small ${redirectsModal.server === "apache_410" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("apache_410")}
                >
                  Apache (410 Gone)
                </button>
                <button
                  className={`oc-btn small ${redirectsModal.server === "cloudflare" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("cloudflare")}
                >
                  Cloudflare (_redirects)
                </button>
                <button
                  className={`oc-btn small ${redirectsModal.server === "cloudflare_csv" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("cloudflare_csv")}
                >
                  Cloudflare Bulk (CSV)
                </button>
                <button
                  className={`oc-btn small ${redirectsModal.server === "vercel" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("vercel")}
                >
                  Vercel (vercel.json)
                </button>
                <button
                  className={`oc-btn small ${redirectsModal.server === "nextjs" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("nextjs")}
                >
                  Next.js (next.config.mjs)
                </button>
                <button
                  className={`oc-btn small ${redirectsModal.server === "caddy" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("caddy")}
                >
                  Caddy (Caddyfile)
                </button>
                <button
                  className={`oc-btn small ${redirectsModal.server === "iis" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("iis")}
                >
                  IIS (web.config)
                </button>
                <button
                  className={`oc-btn small ${redirectsModal.server === "nginx_410" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("nginx_410")}
                >
                  Nginx (410 Gone)
                </button>
                <button
                  className={`oc-btn small ${redirectsModal.server === "wordpress_csv" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("wordpress_csv")}
                >
                  WordPress (CSV)
                </button>
                <button
                  className={`oc-btn small ${redirectsModal.server === "shopify_csv" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("shopify_csv")}
                >
                  Shopify (CSV)
                </button>
                <button
                  className={`oc-btn small ${redirectsModal.server === "removal_list" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("removal_list")}
                >
                  {t("formatRemovalList")}
                </button>
                <button
                  className={`oc-btn small ${redirectsModal.server === "clean_sitemap" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("clean_sitemap")}
                >
                  {t("formatCleanSitemap")}
                </button>
                <button
                  className={`oc-btn small ${redirectsModal.server === "redirect_chains_csv" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("redirect_chains_csv")}
                >
                  {t("formatRedirectChains")}
                </button>
                <button
                  className={`oc-btn small ${redirectsModal.server === "indexnow_json" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("indexnow_json")}
                >
                  IndexNow (Yandex/Bing JSON)
                </button>
                <button
                  className={`oc-btn small ${redirectsModal.server === "markdown_report" ? "primary" : ""}`}
                  onClick={() => void handleOpenRedirects("markdown_report")}
                >
                  {t("formatMarkdownReport")}
                </button>
                <button
                  className="oc-btn ghost icon"
                  onClick={() => setRedirectsModal(null)}
                >
                  <X size={16} />
                </button>
              </div>
            </div>
            <textarea
              className="oc-textarea mono"
              rows={12}
              readOnly
              value={redirectsModal.rules}
            />
            <div className="row end" style={{ marginTop: 14, gap: 8 }}>
              <button
                className="oc-btn"
                onClick={() => {
                  const extMap: Record<RedirectServerFormat, string> = {
                    nginx: "nginx-301.conf",
                    nginx_cluster: "nginx-cluster-301.conf",
                    apache: ".htaccess",
                    apache_410: "apache-410.htaccess",
                    cloudflare: "_redirects",
                    cloudflare_csv: "cloudflare-bulk-redirects.csv",
                    vercel: "vercel.json",
                    nextjs: "next.config.mjs",
                    caddy: "Caddyfile",
                    iis: "web.config",
                    nginx_410: "nginx-410.conf",
                    wordpress_csv: "wordpress-redirection.csv",
                    shopify_csv: "shopify-redirects.csv",
                    removal_list: "gsc-yandex-removal-urls.txt",
                    clean_sitemap: "sitemap-clean.xml",
                    redirect_chains_csv: "redirect-chains.csv",
                    indexnow_json: "indexnow-payload.json",
                    markdown_report: "seo-404-audit-report.md",
                  };
                  downloadTextFile(
                    `${activeScan?.domain ?? "domain"}-${extMap[redirectsModal.server]}`,
                    redirectsModal.rules,
                  );
                }}
              >
                <ArrowDownToLine size={14} />
                {t("downloadFileBtn")}
              </button>
              <button
                className="oc-btn primary"
                onClick={() => copyText(redirectsModal.rules)}
              >
                <Copy size={14} />
                {t("copyRules")}
              </button>
            </div>
          </div>
        </div>
      )}

      {toast && (
        <div className="oc-toasts">
          <div className={`oc-toast ${toast.error ? "error" : "success"}`}>
            <span>{toast.text}</span>
          </div>
        </div>
      )}
    </div>
  );
}
