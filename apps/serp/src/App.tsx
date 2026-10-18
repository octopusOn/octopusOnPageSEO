import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckCircle2, CircleAlert, ShieldAlert, ShieldCheck } from "lucide-react";
import { api, errorText, isDemo } from "./api/client";
import type { Job } from "./api/types";
import { Sidebar } from "./components/Sidebar";
import { persist, stored, useI18n, type Key } from "./i18n";
import { isActive, mergeJob } from "./lib/jobs";
import type { Page } from "./lib/navigation";
import { Analyze } from "./pages/Analyze";
import { Providers } from "./pages/Providers";
import { Settings } from "./pages/Settings";
import { Snapshots } from "./pages/Snapshots";
import { Tracking } from "./pages/Tracking";

const pages: Page[] = ["analyze", "tracking", "snapshots", "providers"];
const leads: Record<Page, Key> = {
  analyze: "analyzeLead",
  tracking: "trackingLead",
  snapshots: "snapshotsLead",
  providers: "providersLead",
  settings: "settingsLead",
};

export function App() {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [page, setPageState] = useState<Page>(() => {
    const saved = stored<Page>("serp.page", "analyze");
    return pages.includes(saved) || saved === "settings" ? saved : "analyze";
  });
  const [selected, setSelectedState] = useState<number | null>(() => stored("serp.analysis", null));
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
    persist("serp.page", next);
  };
  const setSelected = useCallback((id: number | null) => {
    setSelectedState(id);
    persist("serp.analysis", id);
  }, []);

  const info = useQuery({ queryKey: ["info"], queryFn: api.info });
  const settings = useQuery({ queryKey: ["settings"], queryFn: api.settings });
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
        (update) => queryClient.setQueryData<Job[]>(["jobs"], (old) => mergeJob(old ?? [], update)),
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
  const challenged = jobList.find((job) => job.status === "challenge");
  return (
    <div className="oc-shell oc-ground serp-shell">
      <Sidebar page={page} onPage={setPage} jobs={jobList} pages={pages} version={info.data?.version} />
      <main className="oc-work">
        <div className="oc-titlebar top-titlebar" data-tauri-drag-region>
          <span>
            OctopusOn SEO <span className="faint">/</span> {t("product")}
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
                <span className="type-label">SERP CHECKER</span>
                {isDemo && <span className="oc-badge accent">{t("demo")}</span>}
              </div>
              <h1 className="oc-pagehead-title">{t(page)}</h1>
              <p className="oc-pagehead-lead">{t(leads[page])}</p>
            </div>
          </header>
          {challenged && page !== "analyze" && page !== "tracking" && (
            <div className="oc-banner warning challenge-banner" role="alert">
              <ShieldAlert size={18} />
              <div>
                <strong>{t("challengeTitle")}</strong>
                <p>
                  {challenged.label} · {t(challenged.challenge?.kind === "consent" ? "challengeConsent" : challenged.challenge?.kind === "captcha" ? "challengeCaptcha" : "challengeBlocked")}
                </p>
              </div>
            </div>
          )}
          {page === "analyze" && (
            <Analyze jobs={jobList} settings={settings.data} selected={selected} onSelect={setSelected} notify={notify} />
          )}
          {page === "tracking" && <Tracking jobs={jobList} settings={settings.data} notify={notify} />}
          {page === "snapshots" && <Snapshots notify={notify} />}
          {page === "providers" && <Providers settings={settings.data} info={info.data} jobs={jobList} notify={notify} />}
          {page === "settings" && <Settings settings={settings.data} info={info.data} notify={notify} />}
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
