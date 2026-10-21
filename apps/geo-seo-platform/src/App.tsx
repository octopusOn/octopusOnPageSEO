import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckCircle2, CircleAlert, FolderPlus, Hourglass } from "lucide-react";
import { Button, Empty } from "@octopusseo/ui";
import { api } from "./api/client";
import { watch, type Mode } from "./api/transport";
import type { Job } from "./api/types";
import { useI18n } from "./i18n";
import { ShellContext, type Shell } from "./lib/context";
import { explain } from "./lib/errors";
import { isRunning, mergeJob } from "./lib/jobs";
import { refreshSpending } from "./lib/PaidFrame";
import { useRoute } from "./lib/route";
import { Dashboard } from "./modules/dashboard/Dashboard";
import { Backlinks } from "./modules/backlinks/Backlinks";
import { Gap } from "./modules/gap/Gap";
import { KeywordExplorer } from "./modules/keywords/KeywordExplorer";
import { Overview } from "./modules/overview/Overview";
import { RankTracker } from "./modules/rank/RankTracker";
import { SiteAudit } from "./modules/audit/SiteAudit";
import { AiVisibility } from "./modules/ai/AiVisibility";
import { SearchConsole } from "./modules/gsc/SearchConsole";
import { Reports } from "./modules/reports/Reports";
import { moduleOf } from "./modules/registry";
import { Sidebar } from "./modules/shell/Sidebar";
import { Settings } from "./modules/settings/Settings";
import { Sources } from "./modules/sources/Sources";

export function App({ mode }: { mode: Mode }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [route, go] = useRoute();
  const [toast, setToast] = useState<{ text: string; error: boolean } | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const notify = useCallback((text: string, error = false) => {
    setToast({ text, error });
    clearTimeout(timer.current);
    timer.current = setTimeout(() => setToast(null), 5000);
  }, []);
  useEffect(() => () => clearTimeout(timer.current), []);

  // A new page starts at its top.
  const scroller = useRef<HTMLDivElement>(null);
  useEffect(() => {
    scroller.current?.scrollTo({ top: 0 });
  }, [route.page, route.project]);

  const info = useQuery({ queryKey: ["info"], queryFn: api.info });
  const projects = useQuery({ queryKey: ["projects"], queryFn: api.projects });
  const jobs = useQuery({
    queryKey: ["jobs"],
    queryFn: async () => (await api.jobs()).reduce(mergeJob, queryClient.getQueryData<Job[]>(["jobs"]) ?? []),
    refetchInterval: (query) => (query.state.data?.some(isRunning) ? 3000 : 20_000),
  });
  useEffect(() => {
    let stop: (() => void) | undefined;
    let disposed = false;
    void watch((job) => {
      queryClient.setQueryData<Job[]>(["jobs"], (old) => mergeJob(old ?? [], job));
      if (job.status === "running") return;
      if (job.kind === "account-login") {
        void queryClient.invalidateQueries({ queryKey: ["account"] });
        void queryClient.invalidateQueries({ queryKey: ["providers"] });
      }
      if (job.kind === "rank-check") {
        void queryClient.invalidateQueries({ queryKey: ["rank"] });
        refreshSpending(queryClient);
      }
      if (job.kind === "site-audit") {
        void queryClient.invalidateQueries({ queryKey: ["audit"] });
        refreshSpending(queryClient);
      }
      if (job.kind === "ai-visibility") {
        void queryClient.invalidateQueries({ queryKey: ["ai"] });
        refreshSpending(queryClient);
      }
      if (job.kind === "search-console" || job.kind === "google-login") {
        void queryClient.invalidateQueries({ queryKey: ["gsc"] });
      }
      if (job.kind === "report-email") {
        void queryClient.invalidateQueries({ queryKey: ["reports"] });
        refreshSpending(queryClient);
      }
      if (job.status === "failed" && job.error) notify(explain(job.error, t).message, true);
    }).then((unlisten) => {
      if (disposed) unlisten();
      else stop = unlisten;
    });
    return () => {
      disposed = true;
      stop?.();
    };
    // Subscribed once; `t` only changes with the language.
  }, [queryClient, notify]);

  const project = useMemo(() => {
    const list = projects.data ?? [];
    return list.find((row) => row.id === route.project) ?? (route.project === null ? (list[0] ?? null) : null);
  }, [projects.data, route.project]);
  const entry = moduleOf(route.page);
  const jobList = jobs.data ?? [];
  const shell: Shell = { mode, notify, go, project, jobs: jobList };

  let content: React.ReactNode;
  if (!entry.ready) {
    content = (
      <div className="oc-card">
        <Empty icon={<Hourglass size={28} />} title={t("common.soonTitle")}>
          <p>{t("common.soonText")}</p>
        </Empty>
      </div>
    );
  } else if (entry.project && !project) {
    content = projects.isLoading ? null : (
      <div className="oc-card">
        <Empty icon={<FolderPlus size={28} />} title={t("common.noProjectTitle")}>
          <p>{t("common.noProjectText")}</p>
          <Button primary onClick={() => go({ page: "dashboard" })}>
            {t("common.toDashboard")}
          </Button>
        </Empty>
      </div>
    );
  } else if (route.page === "overview" && project) {
    content = <Overview key={project.id} project={project} />;
  } else if (route.page === "keywords" && project) {
    content = <KeywordExplorer key={project.id} project={project} />;
  } else if (route.page === "backlinks" && project) {
    content = <Backlinks key={project.id} project={project} />;
  } else if (route.page === "gap" && project) {
    content = <Gap key={project.id} project={project} />;
  } else if (route.page === "rank" && project) {
    content = <RankTracker key={project.id} project={project} />;
  } else if (route.page === "audit" && project) {
    content = <SiteAudit key={project.id} project={project} />;
  } else if (route.page === "ai" && project) {
    content = <AiVisibility key={project.id} project={project} />;
  } else if (route.page === "gsc" && project) {
    content = <SearchConsole key={project.id} project={project} />;
  } else if (route.page === "reports" && project) {
    content = <Reports key={project.id} project={project} />;
  } else if (route.page === "sources") {
    content = <Sources jobs={jobList} />;
  } else if (route.page === "settings") {
    content = <Settings info={info.data} />;
  } else {
    content = <Dashboard />;
  }

  return (
    <ShellContext.Provider value={shell}>
      <a className="skip-link" href="#main">
        {t("common.skipToContent")}
      </a>
      <div className="oc-shell oc-ground platform-shell">
        <Sidebar
          page={route.page}
          project={project}
          jobs={jobList}
          web={mode === "web"}
          version={info.data?.version}
          onPage={(page) => go({ page, project: project?.id ?? null })}
        />
        <main className="oc-work" id="main">
          <div className="oc-titlebar top-titlebar" data-tauri-drag-region>
            <span>
              OctopusOn SEO <span className="faint">/</span> {t("common.product")}
              {project && entry.project && (
                <>
                  {" "}
                  <span className="faint">/</span> {project.name}
                </>
              )}
            </span>
          </div>
          <div className="page-scroll" ref={scroller}>
            <header className="oc-pagehead">
              <div className="oc-pagehead-text">
                <div className="row">
                  <span className="type-label">GEO-SEO PLATFORM</span>
                  {mode === "demo" && <span className="oc-badge accent">{t("common.demo")}</span>}
                  {!entry.ready && <span className="oc-badge">{t("common.soon")}</span>}
                </div>
                <h1 className="oc-pagehead-title">{t(entry.label)}</h1>
                <p className="oc-pagehead-lead">{t(entry.lead)}</p>
              </div>
            </header>
            {content}
          </div>
        </main>
        {toast && (
          <div className="oc-toasts" role="status" aria-live="polite">
            <div className={`oc-toast ${toast.error ? "error" : "success"}`}>
              {toast.error ? <CircleAlert size={17} /> : <CheckCircle2 size={17} />}
              {toast.text}
            </div>
          </div>
        )}
      </div>
    </ShellContext.Provider>
  );
}
