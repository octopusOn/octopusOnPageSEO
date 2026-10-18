import { ChevronDown, Gauge, Layers, LineChart, PlugZap, Settings2, ShieldCheck } from "lucide-react";
import type { Job } from "../api/types";
import { useI18n, type Key } from "../i18n";
import type { Page } from "../lib/navigation";
import { BrandMark } from "./BrandMark";

const nav: { id: Page; label: Key; icon: typeof Gauge; tone: string }[] = [
  { id: "analyze", label: "analyze", icon: Gauge, tone: "teal" },
  { id: "tracking", label: "tracking", icon: LineChart, tone: "blue" },
  { id: "snapshots", label: "snapshots", icon: Layers, tone: "violet" },
  { id: "providers", label: "providers", icon: PlugZap, tone: "orange" },
];

export function Sidebar({
  page,
  onPage,
  jobs,
  pages,
  version,
}: {
  page: Page;
  onPage: (page: Page) => void;
  jobs: Job[];
  pages: Page[];
  version?: string;
}) {
  const { t, lang, setLang } = useI18n();
  const running = jobs.filter((job) => job.status === "running" || job.status === "challenge");
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
      <span className="nav-section">{t("analysis")}</span>
      <nav className="oc-nav">
        {nav
          .filter((item) => pages.includes(item.id))
          .map((item) => (
            <button
              key={item.id}
              className={`oc-nav-item ${page === item.id ? "active" : ""}`}
              aria-current={page === item.id ? "page" : undefined}
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
                className={job.status === "challenge" ? "attention" : ""}
                onClick={() => onPage(job.kind === "track" ? "tracking" : job.kind === "account" ? "providers" : "analyze")}
              >
                <span className="loading-dot" />
                <span>{job.label}</span>
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
