import {
  ChevronDown,
  FolderKanban,
  Gem,
  ListTree,
  Map,
  Network,
  PlugZap,
  Settings2,
  ShieldCheck,
} from "lucide-react";
import type { Job, ProjectRow } from "../api/types";
import { useI18n, type Key } from "../i18n";
import type { Page } from "../lib/navigation";
import { BrandMark } from "./BrandMark";

const nav: { id: Page; label: Key; icon: typeof Gem; tone: string; project: boolean }[] = [
  { id: "projects", label: "projects", icon: FolderKanban, tone: "slate", project: false },
  { id: "discover", label: "discover", icon: Gem, tone: "teal", project: true },
  { id: "keywords", label: "keywords", icon: ListTree, tone: "blue", project: true },
  { id: "clusters", label: "clusters", icon: Network, tone: "violet", project: true },
  { id: "mapping", label: "mapping", icon: Map, tone: "orange", project: true },
  { id: "sources", label: "sources", icon: PlugZap, tone: "amber", project: false },
];

export function Sidebar({
  page,
  onPage,
  jobs,
  project,
  version,
}: {
  page: Page;
  onPage: (page: Page) => void;
  jobs: Job[];
  project: ProjectRow | undefined;
  version?: string;
}) {
  const { t, lang, setLang } = useI18n();
  const running = jobs.filter((job) => job.status === "running");
  return (
    <aside className="oc-rail">
      <div className="oc-titlebar" data-tauri-drag-region />
      <div className="oc-brand">
        <BrandMark />
        <div>
          <div className="oc-brand-name">
            OctopusOn <span>SEO</span>
          </div>
          <div className="oc-brand-product">{t("product")} <span className="product-tag">BETA</span></div>
        </div>
      </div>
      {project && (
        <button className="rail-project" onClick={() => onPage("projects")} title={t("switchProject")}>
          <span className="faint">{t("project")}</span>
          <strong>{project.name}</strong>
          <small>
            {project.language}-{project.country} · {project.keywords} {t("keywordsShort")}
          </small>
        </button>
      )}
      <nav className="oc-nav">
        {nav.map((item) => (
          <button
            key={item.id}
            className={`oc-nav-item ${page === item.id ? "active" : ""}`}
            aria-current={page === item.id ? "page" : undefined}
            disabled={item.project && !project}
            onClick={() => onPage(item.id)}
          >
            <span className={`oc-tile ${item.tone}`}>
              <item.icon size={16} />
            </span>
            {t(item.label)}
          </button>
        ))}
        {running.length > 0 && (
          <div className="rail-jobs">
            {running.map((job) => (
              <button
                key={job.id}
                className={job.check ? "attention" : undefined}
                title={job.check ? t("checkTitle") : undefined}
                onClick={() =>
                  onPage(job.kind === "cluster" || job.kind === "serp" ? "clusters" : job.kind === "search-console" ? "mapping" : job.kind === "google-login" || job.kind === "account-login" ? "sources" : "discover")
                }
              >
                <span className="loading-dot" />
                <span>{t(`job.${job.kind}` as Key)}</span>
                {job.total > 0 && (
                  <small>
                    {job.done}/{job.total}
                  </small>
                )}
              </button>
            ))}
          </div>
        )}
      </nav>
      <div className="rail-note">
        <ShieldCheck size={13} />
        <p>{t("local")}</p>
      </div>
      <div className="oc-rail-foot">
        <button
          className={`oc-nav-item ${page === "settings" ? "active" : ""}`}
          aria-current={page === "settings" ? "page" : undefined}
          onClick={() => onPage("settings")}
        >
          <Settings2 size={18} />
          {t("settings")}
        </button>
        <div className="rail-meta">
          <span>
            {t("version")} {version ?? ""}
          </span>
          <button onClick={() => setLang(lang === "tr" ? "en" : "tr")} aria-label={t("language")}>
            {lang.toUpperCase()}
            <ChevronDown size={11} />
          </button>
        </div>
      </div>
    </aside>
  );
}
