import { useQuery } from "@tanstack/react-query";
import {
  Activity,
  TrendingUp,
  ChevronDown,
  CircleAlert,
  FolderClock,
  FileCode2,
  Braces,
  GitCompareArrows,
  Globe2,
  LayoutDashboard,
  ListTree,
  Settings2,
  ShieldCheck,
} from "lucide-react";
import { api, isDemo } from "../api/client";
import type { CrawlMeta, Job } from "../api/types";
import { useI18n, number, type Key } from "../i18n";
import type { Page } from "../lib/navigation";
import { jobStatusLabel } from "../lib/jobs";
import { BrandMark } from "./BrandMark";
import { savedCrawlState, type JobsStatus } from "../lib/sessions";

const nav: { id: Page; label: Key; icon: typeof Activity; tone: string }[] = [
  { id: "overview", label: "overview", icon: LayoutDashboard, tone: "teal" },
  { id: "explorer", label: "explorer", icon: ListTree, tone: "blue" },
  { id: "issues", label: "issues", icon: CircleAlert, tone: "orange" },
  { id: "opportunities", label: "opportunities", icon: TrendingUp, tone: "teal" },
  { id: "comparison", label: "comparison", icon: GitCompareArrows, tone: "violet" },
];

export function Sidebar({
  page,
  onPage,
  crawls,
  selectedId,
  selectedJob,
  onSelect,
  activeJobs,
  onJob,
  issueCount,
  renderMode,
  jobsStatus,
}: {
  page: Page;
  onPage: (page: Page) => void;
  crawls: CrawlMeta[];
  selectedId: string;
  selectedJob: string;
  onSelect: (id: string) => void;
  activeJobs: Job[];
  onJob: (id: string) => void;
  issueCount: number;
  renderMode: "html" | "javascript";
  jobsStatus: JobsStatus;
}) {
  const { t, lang, setLang } = useI18n();
  const memory = useQuery({
    queryKey: ["memory"],
    queryFn: api.memory,
    refetchInterval: 5000,
    enabled: !isDemo,
  });
  return (
    <aside className="oc-rail">
      <div className="oc-titlebar" data-tauri-drag-region />
      <div className="oc-brand">
        <BrandMark />
        <div>
          <div className="oc-brand-name">
            OctopusOn <span>SEO</span>
          </div>
          <div className="oc-brand-product">SEO Crawler <span className="product-tag">BETA</span></div>
        </div>
      </div>
      <div className="project-select">
        <Globe2 size={17} />
        <div>
          <small>{t("workspace")}</small>
          <select
            aria-label={t("noSelection")}
            value={selectedJob ? `job:${selectedJob}` : selectedId}
            onChange={(e) => {
              if (e.target.value.startsWith("job:"))
                onJob(e.target.value.slice(4));
              else onSelect(e.target.value);
              onPage("overview");
            }}
          >
            {!crawls.length && !activeJobs.length && (
              <option value="">{t("noSelection")}</option>
            )}
            {activeJobs.map((job) => (
              <option key={job.id} value={`job:${job.id}`}>
                {job.url} · {t(jobStatusLabel(job))}
              </option>
            ))}
            {crawls.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name} · {new Intl.DateTimeFormat(lang==="tr"?"tr-TR":"en-US",{month:"short",day:"numeric",hour:"2-digit",minute:"2-digit"}).format(c.created_at_ms)} · {t(savedCrawlState(c,activeJobs,jobsStatus).label)}
              </option>
            ))}
          </select>
        </div>
        <ChevronDown size={14} />
      </div>
      <span className="nav-section">{t("analysis")}</span>
      <nav className="oc-nav">
        {nav.map((item) => (
          <button
            key={item.id}
            className={`oc-nav-item ${page === item.id ? "active" : ""}`}
            onClick={() => onPage(item.id)}
          >
            <span className={`oc-tile ${item.tone}`}>
              <item.icon size={16} />
            </span>
            {t(item.label)}
            {item.id === "issues" && issueCount > 0 && (
              <span className="nav-count">{issueCount}</span>
            )}
            {item.id === "overview" && page === item.id && (
              <span className="active-dot" />
            )}
          </button>
        ))}
        <span className="nav-section tools-label">{t("tools")}</span>
        <button className={`oc-nav-item ${page === "schemaValidator" ? "active" : ""}`} onClick={()=>onPage("schemaValidator")}><span className="oc-tile blue"><Braces size={16}/></span>{t("schemaValidator")}</button>
        <button
          className={`oc-nav-item ${page === "sessions" ? "active" : ""}`}
          onClick={() => onPage("sessions")}
        >
          <span className="oc-tile violet">
            <FolderClock size={16} />
          </span>
          {t("sessions")}
          <span className="nav-count faint">{crawls.length ?? 0}</span>
        </button>
        <button className={`oc-nav-item ${page === "sitemapBuilder" ? "active" : ""}`} onClick={()=>onPage("sitemapBuilder")}><span className="oc-tile teal"><FileCode2 size={16}/></span>{t("sitemapBuilder")}</button>
        {activeJobs.length > 0 && (
          <div className="rail-jobs">
            {activeJobs.map((job) => (
              <button
                key={job.id}
                onClick={() => {
                  onJob(job.id);
                  onPage("overview");
                }}
              >
                <span className="loading-dot" />
                <span>{job.url.replace(/^https?:\/\//, "").split("/")[0]}</span>
                <small>{number(job.progress.crawled, lang)}</small>
              </button>
            ))}
          </div>
        )}
      </nav>
      <div className="rail-engine">
        <div className="row">
          <span className="engine-dot" />
          <b>{t(renderMode === "javascript" ? "javascript" : "rawHtml")}</b>
          <span className="oc-badge">Rust</span>
        </div>
        <p>
          <ShieldCheck size={12} />
          {t("nativeNote")}
        </p>
        {!isDemo && (
          <div className="resource-stats">
            <div>
              <span>{t("memory")}</span>
              <b>
                {memory.data === null || memory.data === undefined
                  ? "—"
                  : `${number(memory.data / 1048576, lang)} MB`}
              </b>
            </div>
            <div>
              <span>{t("activeCrawls")}</span>
              <b>{activeJobs.length}</b>
            </div>
          </div>
        )}
      </div>
      <div className="oc-rail-foot">
        <button
          className={`oc-nav-item ${page === "settings" ? "active" : ""}`}
          onClick={() => onPage("settings")}
        >
          <Settings2 size={18} />
          {t("settings")}
        </button>
        <div className="rail-meta">
          <span>{t("version")}</span>
          <button
            onClick={() => setLang(lang === "tr" ? "en" : "tr")}
            aria-label={t("language")}
          >
            {lang.toUpperCase()}
            <ChevronDown size={11} />
          </button>
        </div>
      </div>
    </aside>
  );
}
