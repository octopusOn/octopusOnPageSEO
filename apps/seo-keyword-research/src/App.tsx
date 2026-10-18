import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckCircle2, CircleAlert, ShieldCheck } from "lucide-react";
import { api, errorText, isDemo } from "./api/client";
import type { Job } from "./api/types";
import { CheckBanner } from "./components/JobCard";
import { Sidebar } from "./components/Sidebar";
import { persist, stored, useI18n, type Key } from "./i18n";
import { isActive, mergeJob } from "./lib/jobs";
import type { Page } from "./lib/navigation";
import { Clusters } from "./pages/Clusters";
import { Discover } from "./pages/Discover";
import { Keywords } from "./pages/Keywords";
import { Mapping } from "./pages/Mapping";
import { Projects } from "./pages/Projects";
import { Settings } from "./pages/Settings";
import { Sources } from "./pages/Sources";

const PAGES: Page[] = ["projects", "discover", "keywords", "clusters", "mapping", "sources", "settings"];
const NEEDS_PROJECT: Page[] = ["discover", "keywords", "clusters", "mapping"];
const leads: Record<Page, Key> = {
  projects: "projectsLead",
  discover: "discoverLead",
  keywords: "keywordsLead",
  clusters: "clustersLead",
  mapping: "mappingLead",
  sources: "sourcesLead",
  settings: "settingsLead",
};

export function App() {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [page, setPageState] = useState<Page>(() => {
    const saved = stored<Page>("keywords.page", "projects");
    return PAGES.includes(saved) ? saved : "projects";
  });
  const [projectId, setProjectIdState] = useState<number | null>(() => stored("keywords.project", null));
  const [clusterFilter, setClusterFilter] = useState<number | null>(null);
  const [toast, setToast] = useState<{ text: string; error: boolean } | null>(null);
  const toastTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const notify = useCallback((text: string, error = false) => {
    setToast({ text, error });
    clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setToast(null), 5000);
  }, []);
  useEffect(() => () => clearTimeout(toastTimer.current), []);
  const setPage = (next: Page) => {
    setPageState(next);
    persist("keywords.page", next);
    if (next !== "keywords") setClusterFilter(null);
  };
  const setProjectId = (id: number | null) => {
    setProjectIdState(id);
    persist("keywords.project", id);
  };

  const info = useQuery({ queryKey: ["info"], queryFn: api.info });
  const settings = useQuery({ queryKey: ["settings"], queryFn: api.settings });
  const projects = useQuery({ queryKey: ["projects"], queryFn: api.projects });
  const project = projects.data?.find((row) => row.id === projectId) ?? (projectId === null ? projects.data?.[0] : undefined);
  useEffect(() => {
    if (project && project.id !== projectId) setProjectId(project.id);
  }, [project, projectId]);
  const jobs = useQuery({
    queryKey: ["jobs"],
    queryFn: async () => {
      const incoming = await api.jobs();
      return incoming.reduce(mergeJob, queryClient.getQueryData<Job[]>(["jobs"]) ?? []);
    },
    refetchInterval: (query) => (query.state.data?.some(isActive) ? 3000 : 15000),
  });
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void api
      .watch(
        (update) => {
          queryClient.setQueryData<Job[]>(["jobs"], (old) => mergeJob(old ?? [], update));
          if (update.status !== "running") {
            void queryClient.invalidateQueries({ queryKey: ["projects"] });
            void queryClient.invalidateQueries({ queryKey: ["keywords"] });
            void queryClient.invalidateQueries({ queryKey: ["facets"] });
            void queryClient.invalidateQueries({ queryKey: ["usage"] });
            void queryClient.invalidateQueries({ queryKey: ["providers"] });
          }
        },
        () => notify(t("closeWhileRunning"), true),
      )
      .then((unlisten) => {
        if (disposed) unlisten();
        else stop = unlisten;
      })
      .catch((error) => {
        if (!disposed) notify(errorText(error), true);
      });
    return () => {
      disposed = true;
      stop?.();
    };
    // Subscribe once; `t` only changes with the language.
  }, [queryClient, notify]);

  const jobList = jobs.data ?? [];
  const shown: Page = NEEDS_PROJECT.includes(page) && !project ? "projects" : page;
  // A check page waits for the person: shown on every page but the one with the job card.
  const checking = jobList.find((job) => job.status === "running" && job.check);
  return (
    <div className="oc-shell oc-ground kw-shell">
      <Sidebar page={shown} onPage={setPage} jobs={jobList} project={project} version={info.data?.version} />
      <main className="oc-work">
        <div className="oc-titlebar top-titlebar" data-tauri-drag-region>
          <span>
            OctopusOn SEO <span className="faint">/</span> {t("product")}
            {project && NEEDS_PROJECT.includes(shown) && (
              <>
                {" "}
                <span className="faint">/</span> {project.name}
              </>
            )}
          </span>
          <span className="row faint">
            <ShieldCheck size={12} />
            {t("local")}
          </span>
        </div>
        <div className="page-scroll">
          <header className="oc-pagehead">
            <div className="oc-pagehead-text">
              <div className="row">
                <span className="type-label">SEO KEYWORD RESEARCH</span>
                {isDemo && <span className="oc-badge accent">{t("demo")}</span>}
              </div>
              <h1 className="oc-pagehead-title">{t(shown)}</h1>
              <p className="oc-pagehead-lead">{t(leads[shown])}</p>
            </div>
          </header>
          {checking?.check && shown !== "clusters" && (
            <div className="check-banner">
              <CheckBanner check={checking.check} label={checking.label} />
            </div>
          )}
          {shown === "projects" && (
            <Projects
              selected={project?.id ?? null}
              onSelect={(id) => {
                setProjectId(id);
                if (id !== null) setPage("discover");
              }}
              settings={settings.data}
              notify={notify}
            />
          )}
          {shown === "discover" && project && <Discover key={project.id} project={project} settings={settings.data} jobs={jobList} notify={notify} />}
          {shown === "keywords" && project && <Keywords key={`${project.id}-${clusterFilter ?? ""}`} project={project} notify={notify} initialCluster={clusterFilter} />}
          {shown === "clusters" && project && (
            <Clusters
              key={project.id}
              project={project}
              settings={settings.data}
              jobs={jobList}
              notify={notify}
              onKeywords={(cluster) => {
                setPage("keywords");
                setClusterFilter(cluster);
              }}
            />
          )}
          {shown === "mapping" && project && <Mapping key={project.id} project={project} jobs={jobList} notify={notify} />}
          {shown === "sources" && <Sources settings={settings.data} jobs={jobList} notify={notify} />}
          {shown === "settings" && <Settings settings={settings.data} info={info.data} notify={notify} />}
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
  );
}
