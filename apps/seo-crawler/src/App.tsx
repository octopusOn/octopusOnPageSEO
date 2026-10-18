import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Activity,
  ArrowDownToLine,
  ArrowRight,
  Check,
  ChevronDown,
  CircleAlert,
  Plus,
  Search,
  Settings2,
  ShieldCheck,
  X,
  Zap,
  ScanLine,
  RefreshCw,
} from "lucide-react";
import { api, errorText, isDemo } from "./api/client";
import type { CrawlConfig, CrawlMeta, Job, ResponseGroup } from "./api/types";
import { useI18n, stored, persist, number } from "./i18n";
import { Sidebar } from "./components/Sidebar";
import { LiveCrawl } from "./components/LiveCrawl";
import type { Page } from "./lib/navigation";
import { Button, ErrorState, Loading } from "./components/ui";
import { NewCrawl } from "./components/NewCrawl";
import { Overview } from "./pages/Overview";
import { Explorer, tabs } from "./pages/Explorer";
import { Issues } from "./pages/Issues";
import { Sessions } from "./pages/Sessions";
import { Settings } from "./pages/Settings";
import { Comparison } from "./pages/Comparison";
import { SitemapBuilder } from "./pages/SitemapBuilder";
import { Opportunities } from "./pages/Opportunities";
import { SchemaValidator } from "./pages/SchemaValidator";
import { loadConfig, validConfig } from "./lib/config";
import { isActiveJob, jobStatusLabel, mergeJobs, updateJob } from "./lib/jobs";
import { prepareRenderingRetry } from "./lib/renderRetry";
import { exportSelection, type ExplorerExportView } from "./lib/exports";
import { isLinkEvidenceIssue } from "./lib/links";
import { prepareRepeat, savedCrawlState, type JobsStatus } from "./lib/sessions";

export function App() {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const [page, setPage] = useState<Page>("overview");
  const [schemaUrl,setSchemaUrl]=useState("");
  const [selected, setSelected] = useState(() => stored("spider.selected", ""));
  const [selectedJob, setSelectedJob] = useState("");
  const [tab, setTab] = useState("internal");
  const [issueId, setIssueId] = useState("");
  const [responseGroup, setResponseGroup] = useState<ResponseGroup | "">("");
  const [newCrawl, setNewCrawl] = useState(false);
  const [retryConfig, setRetryConfig] = useState<CrawlConfig | null>(null);
  const [repeatPlan, setRepeatPlan] = useState<{config:CrawlConfig;source:Pick<CrawlMeta,"name"|"created_at_ms">} | null>(null);
  const [preparingRetry, setPreparingRetry] = useState(false);
  const retryAbort = useRef<AbortController | undefined>(undefined);
  const [url, setUrl] = useState("");
  const [exportOpen, setExportOpen] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [exportScope, setExportScope] = useState<"filtered" | "all">("filtered");
  const [exportView, setExportView] = useState<ExplorerExportView | null>(null);
  const [config, setConfig] = useState(loadConfig);
  const [exclusions, setExclusions] = useState<Record<string, string[]>>(() =>
    stored("spider.exclusions", {}),
  );
  const [toast, setToast] = useState<{ text: string; error: boolean } | null>(
    null,
  );
  const toastTimer = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  );
  const notify = useCallback((text: string, error = false) => {
    setToast({ text, error });
    clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setToast(null), 5000);
  }, []);
  useEffect(() => () => { clearTimeout(toastTimer.current); retryAbort.current?.abort(); }, []);
  const jobsQuery = useQuery({
    queryKey: ["jobs"],
    queryFn: async () => {
      const incoming = await api.jobs();
      return mergeJobs(queryClient.getQueryData<Job[]>(["jobs"]) ?? [], incoming);
    },
    refetchInterval: 2000,
  });
  const jobs = jobsQuery.data ?? [];
  const activeJobs = jobs.filter(isActiveJob);
  const crawls = useQuery({
    queryKey: ["crawls"],
    queryFn: api.crawls,
    refetchInterval: activeJobs.length ? 3000 : false,
  });
  const storage = useQuery({
    queryKey: ["crawl-storage"],
    queryFn: api.storage,
    enabled: page === "sessions",
    refetchInterval: page === "sessions" && activeJobs.length ? 10000 : false,
  });
  const meta = crawls.data?.find((c) => c.id === selected) ?? crawls.data?.[0];
  useEffect(() => setResponseGroup(""), [meta?.id]);
  const job = jobs.find((j) => j.id === selectedJob);
  const jobsStatus:JobsStatus=jobsQuery.isError ? "unavailable" : jobsQuery.isSuccess ? "ready" : "loading";
  const savedState=meta ? savedCrawlState(meta,jobs,jobsStatus) : undefined;
  const running=job && isActiveJob(job) ? job : savedState?.active;
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void api
      .watch((update) => {
        queryClient.setQueryData<Job[]>(["jobs"], (old) => updateJob(old ?? [], update));
        if (update.meta || update.error) {
          void queryClient.invalidateQueries({ queryKey: ["crawls"] });
          void queryClient.invalidateQueries({ queryKey: ["crawl-storage"] });
        }
      }, () => notify(t("closeWhileCrawling"), true))
      .then((unlisten) => {
        if (disposed) unlisten();
        else stop = unlisten;
      })
      .catch((error) => { if (!disposed) notify(errorText(error), true); });
    return () => {
      disposed = true;
      stop?.();
    };
  }, [queryClient, notify, t]);
  useEffect(() => {
    if (job?.crawl_id && isActiveJob(job)) {
      setSelected(job.crawl_id);
      persist("spider.selected", job.crawl_id);
    }
  }, [job?.crawl_id, job?.id, job?.status]);
  useEffect(() => {
    if (!selectedJob && meta?.status === "running") {
      const active = activeJobs.find((candidate) => candidate.crawl_id === meta.id);
      if (active) setSelectedJob(active.id);
    }
  }, [selectedJob, meta?.id, meta?.status, activeJobs]);
  useEffect(() => {
    if (job?.meta) {
      setSelected(job.meta.id);
      persist("spider.selected", job.meta.id);
      setSelectedJob("");
      notify(t(job.meta.status === "stopped" ? "crawlStopped" : "crawlComplete"));
    } else if (job?.status === "failed") {
      notify(job.error ?? t("crawlFailed"), true);
      setSelectedJob("");
    }
  }, [job, notify, t]);
  const overview = useQuery({
    queryKey: ["overview", meta?.id],
    queryFn: () => api.overview(meta!.id),
    enabled: !!meta && meta.status !== "running" && !savedState?.active,
  });
  const excluded = exclusions[meta?.id ?? ""] ?? [];
  const renderingFailures = overview.data?.issues.find((issue) => issue.id === "render-error")?.count ?? 0;
  const issueCount = useMemo(
    () =>
      overview.data?.issues.filter((i) => !excluded.includes(i.id)).length ?? 0,
    [overview.data, excluded],
  );
  const onSelect = (id: string) => {
    retryAbort.current?.abort();
    setPreparingRetry(false);
    setSelectedJob("");
    setSelected(id);
    setResponseGroup("");
    persist("spider.selected", id);
  };
  const inspectIssue = (id: string) => {
    setResponseGroup("");
    setIssueId(id);
    setTab(
      isLinkEvidenceIssue(id) ? "links" : id === "content-near-duplicate" ? "content-matches" : id.startsWith("render-")
        ? "rendering"
        : id.startsWith("external-")
          ? "external"
          : id.startsWith("robots-") || id.startsWith("ai-")
            ? "robots"
            : [
                  "sitemap-error",
                  "sitemap-too-large",
                  "sitemap-missing",
                  "sitemap-not-in-robots",
                ].includes(id)
              ? "sitemaps"
              : "internal",
    );
    setPage("explorer");
  };
  async function start(next: CrawlConfig) {
    if (!validConfig(next)) throw new Error(t("invalidNumbers"));
    const id = await api.start(next);
    setSelectedJob(id);
    setPage("overview");
    setNewCrawl(false);
    setRetryConfig(null);
    setRepeatPlan(null);
    await jobsQuery.refetch();
  }
  async function control(action: "pause" | "resume" | "stop") {
    if (!running) return;
    try {
      await api.control(running.id, action);
      await jobsQuery.refetch();
    } catch (error) {
      notify(errorText(error), true);
    }
  }
  async function exportTab(format: "csv" | "xlsx" | "json") {
    if (!meta) return;
    const selection = page === "explorer" && exportScope === "filtered" ? exportSelection(exportView, meta.id, tab) : undefined;
    if (selection === null) { notify(t("exportUpdating"), true); return; }
    setExportOpen(false);
    setExporting(true);
    try {
      const backend =
        page === "issues"
          ? "issues"
          : page === "explorer"
            ? (tabs.find((item) => item.id === tab)?.backend ?? "internal")
            : "internal";
      const rows = await api.export(meta.id, backend, format, selection);
      if (rows !== null) notify(`${t("exportDone")} · ${number(rows, lang)} ${t("rows")}`);
    } catch (error) {
      notify(errorText(error), true);
    } finally {
      setExporting(false);
    }
  }
  const closeNew = useCallback(() => { retryAbort.current?.abort(); setNewCrawl(false); setRetryConfig(null); setRepeatPlan(null); }, []);
  const openNew = () => { retryAbort.current?.abort(); setPreparingRetry(false); setRetryConfig(null); setRepeatPlan(null); setNewCrawl(true); };
  function repeatSaved(crawl:CrawlMeta) {
    if(!savedCrawlState(crawl,jobs,jobsStatus).canRepeat) {notify(t("crawlStatusRetry"),true);return;}
    retryAbort.current?.abort();
    setPreparingRetry(false);
    setRetryConfig(null);
    setRepeatPlan({config:prepareRepeat(crawl),source:{name:crawl.name,created_at_ms:crawl.created_at_ms}});
    setNewCrawl(true);
  }
  async function retryRendering() {
    if (!meta) return;
    setRepeatPlan(null);
    retryAbort.current?.abort();
    const controller = new AbortController();
    retryAbort.current = controller;
    setPreparingRetry(true);
    try {
      const prepared = await prepareRenderingRetry(meta, (request) => api.page(meta.id, request), controller.signal);
      if (!prepared) { notify(t("noRenderingFailures")); return; }
      setRetryConfig(prepared);
      setNewCrawl(true);
    } catch (error) {
      if (!controller.signal.aborted) notify(`${t("retryPreparationFailed")} ${errorText(error)}`, true);
    } finally {
      if (retryAbort.current === controller) { setPreparingRetry(false); retryAbort.current = undefined; }
    }
  }
  const switchExclusion = (id: string) => {
    if (!meta) return;
    setExclusions((old) => {
      const existing = old[meta.id] ?? [];
      const next = {
        ...old,
        [meta.id]: existing.includes(id)
          ? existing.filter((v) => v !== id)
          : [...existing, id],
      };
      persist("spider.exclusions", next);
      return next;
    });
  };
  const subtitle =
    page === "opportunities" ? t("opportunitiesDescription") : page === "schemaValidator" ? t("schemaValidatorDescription") : page === "sitemapBuilder" ? t("sitemapBuilderDescription") : page === "comparison" ? t("compareDescription") : page === "sessions"
      ? t("savedDescription")
      : page === "settings"
        ? t("configDescription")
        : job && selectedJob
          ? `${job.url} · ${t(jobStatusLabel(job))}`
          : meta
            ? `${meta.name} · ${t("lastCrawl")} ${new Intl.DateTimeFormat(lang === "tr" ? "tr-TR" : "en-US", { dateStyle: "medium", timeStyle: "short" }).format(meta.created_at_ms)}`
            : t("subtitle");
  return (
    <div className="oc-shell oc-ground spider-shell">
      <Sidebar
        page={page}
        onPage={setPage}
        crawls={crawls.data ?? []}
        selectedId={meta?.id ?? ""}
        selectedJob={selectedJob}
        onSelect={onSelect}
        activeJobs={activeJobs}
        onJob={setSelectedJob}
        issueCount={selectedJob ? 0 : issueCount}
        renderMode={job?.rendering ?? meta?.config.rendering?.mode ?? "html"}
        jobsStatus={jobsStatus}
      />

      <main className="oc-work">
        <div className="oc-titlebar top-titlebar" data-tauri-drag-region>
          <span>
            OctopusOn SEO <span className="faint">/</span> SEO Crawler
          </span>
          <span className="row faint">
            <ShieldCheck size={12} />
            {t("local")}
          </span>
        </div>
        <div className="crawl-bar">
          <div className="crawl-input">
            <Search size={18} />
            <input
              aria-label={t("crawlUrlLabel")}
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder={t("crawlUrl")}
              onKeyDown={(e) => e.key === "Enter" && openNew()}
            />
            <span className="crawl-mode">{t("spider")}</span>
          </div>
          <Button primary onClick={openNew}>
            <Plus size={17} />
            {t("newCrawl")}
          </Button>
          <Button
            className="icon ghost"
            title={t("settings")}
            aria-label={t("settings")}
            onClick={() => setPage("settings")}
          >
            <Settings2 size={19} />
          </Button>
        </div>
        <div
          className={`page-scroll ${page === "explorer" ? "explorer-scroll" : ""}`}
        >
          <header className="page-header">
            <div>
              <div className="row page-eyebrow">
                <span className="eyebrow">SEO CRAWLER</span>
                {isDemo && (
                  <span className="oc-badge demo-badge">{t("demo")}</span>
                )}
              </div>
              <h1>{t(page)}</h1>
              <p>{subtitle}</p>
            </div>
            <div className="page-actions">
              {meta && !running && meta.status !== "running" && page === "explorer" && tab === "rendering" &&
                renderingFailures > 0 && (
                  <Button small busy={preparingRetry} onClick={() => void retryRendering()}>
                    <RefreshCw size={15} />{t("retryRendering")} ({number(renderingFailures, lang)})
                  </Button>
                )}
              {meta && !running && savedState && page!=="schemaValidator" && page!=="opportunities" && (
                <span
                  className={`oc-badge ${savedState.inactive || meta.status!=="completed" ? "warning" : "good"}`}
                >
                  <span className="oc-dot" />
                  {t(savedState.label)}
                </span>
              )}
              {meta &&
                page !== "settings" &&
                page !== "sessions" &&
                page !== "comparison" &&
                page !== "sitemapBuilder" &&
                page !== "schemaValidator" && page !== "opportunities" &&
                !running &&
                meta.status !== "running" && (
                  <div className="export-menu-wrap">
                    <Button
                      small
                      busy={exporting}
                      onClick={() => setExportOpen(!exportOpen)}
                      aria-expanded={exportOpen}
                    >
                      <ArrowDownToLine size={15} />
                      {t("export")}
                      <ChevronDown size={12} />
                    </Button>
                    {exportOpen && (
                      <>
                        <button
                          className="menu-dismiss"
                          aria-label={t("close")}
                          onClick={() => setExportOpen(false)}
                        />
                        <div className="oc-menu export-menu">
                          {page === "explorer" ? <>
                            <div className="export-scope" role="group" aria-label={t("exportScope")}>
                              <button className="oc-menu-item" aria-pressed={exportScope === "filtered"} onClick={() => setExportScope("filtered")}>{t("exportFiltered")}</button>
                              <button className="oc-menu-item" aria-pressed={exportScope === "all"} onClick={() => setExportScope("all")}>{t("exportAll")}</button>
                            </div>
                            <small>{t(exportScope === "filtered" ? "exportFilteredNote" : "exportTab")}</small>
                          </> : <small>{t("exportTab")}</small>}
                          {(["csv", "xlsx", "json"] as const).map((format) => (
                            <button
                              key={format}
                              className="oc-menu-item"
                              disabled={(isDemo && format === "xlsx") || (page === "explorer" && exportScope === "filtered" && !exportSelection(exportView, meta.id, tab))}
                              onClick={() => void exportTab(format)}
                            >
                              {format.toUpperCase()}
                            </button>
                          ))}
                        </div>
                      </>
                    )}
                  </div>
                )}
            </div>
          </header>
          {isDemo && (
            <div className="demo-strip">
              <span className="dot" />
              <span>{t("demoNote")}</span>
            </div>
          )}
          {page === "opportunities" ? (
            <Opportunities crawls={crawls.data??[]} seed={meta} notify={notify}/>
          ) : page === "schemaValidator" ? (
            <SchemaValidator initialUrl={schemaUrl||meta?.start_urls[0]||url} config={config}/>
          ) : page === "settings" ? (
            <Settings
              config={config}
              onChange={setConfig}
              onSave={() => {
                if (!validConfig(config)) {
                  notify(t("invalidNumbers"), true);
                  return;
                }
                persist("spider.config", config);
                notify(t("configurationSaved"));
              }}
            />
          ) : page === "sessions" ? (
            crawls.isPending ? <Loading/> : crawls.error ? <ErrorState message={errorText(crawls.error)} retry={()=>void crawls.refetch()}/> :
            <Sessions
              crawls={crawls.data ?? []}
              jobs={jobs}
              jobsStatus={jobsStatus}
              storage={storage.data}
              storageStatus={storage.isError ? "unavailable" : storage.isSuccess ? "ready" : "loading"}
              storageFetching={storage.isFetching}
              onRefreshStorage={() => { void Promise.all([crawls.refetch(), storage.refetch()]); }}
              onRepeat={repeatSaved}
              onOpen={(id) => {
                onSelect(id);
                setPage("overview");
              }}
              onDelete={async (id) => {
                try {
                  await api.remove(id);
                  await Promise.all([
                    crawls.refetch(),
                    queryClient.invalidateQueries({ queryKey: ["crawl-storage"] }),
                  ]);
                  if (id === selected) setSelected("");
                  notify(t("deleteDone"));
                } catch (error) {
                  notify(errorText(error), true);
                  throw error;
                }
              }}
            />
          ) : page === "comparison" ? (
            crawls.isPending ? <Loading/> : crawls.error ? <ErrorState message={errorText(crawls.error)} retry={()=>void crawls.refetch()}/> :
            <Comparison crawls={crawls.data??[]} jobs={jobs} jobsStatus={jobsStatus} onSessions={()=>setPage("sessions")} onRetryStatus={()=>void jobsQuery.refetch()} notify={notify}/>
          ) : page === "sitemapBuilder" && meta ? (
            <SitemapBuilder meta={meta} active={!!savedState?.active} jobsStatus={jobsStatus} onRetryStatus={()=>void jobsQuery.refetch()} notify={notify}/>
          ) : running ? (
            <LiveCrawl key={running.id} running={running} config={crawls.data?.find(crawl => crawl.id === running.crawl_id)?.config} onControl={control} />
          ) : crawls.isPending ? (
            <Loading />
          ) : crawls.error ? (
            <ErrorState
              message={errorText(crawls.error)}
              retry={() => void crawls.refetch()}
            />
          ) : !meta ? (
            <div className="welcome-panel">
              <span className="oc-tile large teal">
                <ScanLine size={27} />
              </span>
              <h2>{t("noCrawls")}</h2>
              <p>{t("welcome")}</p>
              <Button primary onClick={openNew}>
                {t("start")}
                <ArrowRight size={16} />
              </Button>
              <div className="row welcome-benefits">
                {(["unlimited", "local", "openSource"] as const).map((k) => (
                  <span key={k}>
                    <Check size={14} />
                    {t(k)}
                  </span>
                ))}
              </div>
            </div>
          ) : meta.status === "running" ? (
            jobsStatus==="unavailable" ? <ErrorState message={t("crawlStatusRetry")} retry={()=>void jobsQuery.refetch()} /> : jobsStatus==="loading" ? <Loading /> :
            <section className="interrupted-panel oc-panel">
              <span className="oc-tile large orange"><Activity size={26}/></span>
              <h2>{t("interrupted")}</h2><p>{t("interruptedNote")}</p>
              <div className="row"><Button primary onClick={()=>repeatSaved(meta)}><RefreshCw size={15}/>{t("repeatCrawl")}</Button><Button onClick={()=>setPage("sessions")}>{t("sessions")}</Button></div>
            </section>
          ) : overview.isPending ? (
            <Loading />
          ) : overview.error ? (
            <ErrorState
              message={errorText(overview.error)}
              retry={() => void overview.refetch()}
            />
          ) : (
            overview.data && (
              <>
                {meta.limit_reached && (
                  <div className="oc-banner warning">{t("stoppedLimit")}</div>
                )}
                {meta.error && (
                  <div className="oc-banner error">{meta.error}</div>
                )}
                {meta.config.mode === "list" && <div className="oc-banner info">{t("listCrawlScope")}</div>}
                {page === "overview" && (
                  <Overview
                    meta={meta}
                    data={overview.data}
                    excluded={excluded}
                    onIssue={inspectIssue}
                    onExplore={() => {
                      setIssueId("");
                      setResponseGroup("");
                      setTab("internal");
                      setPage("explorer");
                    }}
                    onResponse={(group) => {
                      setIssueId("");
                      setResponseGroup(group);
                      setTab("response-codes");
                      setPage("explorer");
                    }}
                    onIssues={() => setPage("issues")}
                  />
                )}{" "}
                {page === "issues" && (
                  <Issues
                    data={overview.data}
                    excluded={excluded}
                    onExclude={switchExclusion}
                    onInspect={inspectIssue}
                  />
                )}{" "}
                {page === "explorer" && (
                  <Explorer
                    key={meta.id}
                    id={meta.id}
                    tabId={tab}
                    onTab={(next) => { setResponseGroup(""); setTab(next); }}
                    responseGroup={responseGroup}
                    onResponseGroup={setResponseGroup}
                    issueId={issueId}
                    onClearIssue={() => setIssueId("")}
                    onExportView={setExportView}
                    excluded={excluded}
                    onIssue={inspectIssue}
                    onSchema={(url)=>{setSchemaUrl(url);setPage("schemaValidator");}}
                  />
                )}
              </>
            )
          )}
        </div>
        <footer className="app-status">
          <span>
            <span className="engine-dot" />
            {running
              ? t(jobStatusLabel(running))
              : t(
                  (job?.rendering ?? meta?.config.rendering?.mode) ===
                    "javascript"
                    ? "javascript"
                    : "rawHtml",
                )}
          </span>
          <span>
            {running
              ? `${number(running.progress.crawled, lang)} ${t("urls")}`
              : meta
                ? `${number(meta.progress.crawled, lang)} ${t("urls")}`
                : t("unlimited")}
            <i /> <Zap size={11} />
            {t("local")}
          </span>
        </footer>
      </main>
      {newCrawl && (
        <NewCrawl
          config={repeatPlan?.config ?? retryConfig ?? config}
          initialUrl={repeatPlan?.config.start_urls.join("\n") ?? retryConfig?.start_urls.join("\n") ?? url}
          retryRendering={!!retryConfig}
          repeatSource={repeatPlan?.source}
          onClose={closeNew}
          onStart={start}
        />
      )}
      {toast && (
        <div
          className={`sp-toast ${toast.error ? "error" : ""}`}
          role={toast.error ? "alert" : "status"}
        >
          {toast.error ? <CircleAlert size={18} /> : <Check size={18} />}
          <span>{toast.text}</span>
          <button aria-label={t("close")} onClick={() => setToast(null)}>
            <X size={16} />
          </button>
        </div>
      )}
    </div>
  );
}
