import {
  ChevronDown,
  FolderClock,
  Globe2,
  KeyRound,
  LayoutDashboard,
  SearchX,
  Settings2,
  ShieldCheck,
} from "lucide-react";
import type { FinderJob, Page, ScanMeta } from "../api/types";
import { useI18n, number } from "../i18n";
import { BrandMark } from "./BrandMark";

export function Sidebar({
  page,
  onPage,
  scans,
  selectedId,
  onSelect,
  activeJobs,
  issueCount,
}: {
  page: Page;
  onPage: (page: Page) => void;
  scans: ScanMeta[];
  selectedId: string;
  onSelect: (id: string) => void;
  activeJobs: FinderJob[];
  issueCount: number;
}) {
  const { t, lang, setLang } = useI18n();
  return (
    <aside className="oc-rail">
      <div className="oc-titlebar" data-tauri-drag-region />
      <div className="oc-brand">
        <BrandMark />
        <div>
          <div className="oc-brand-name">
            OctopusOn <span>SEO</span>
          </div>
          <div className="oc-brand-product">{t("productName")} <span className="product-tag">BETA</span></div>
        </div>
      </div>

      <div className="project-select">
        <Globe2 size={17} />
        <div>
          <small>{t("workspace")}</small>
          <select
            aria-label={t("workspace")}
            value={selectedId}
            onChange={(e) => {
              onSelect(e.target.value);
              onPage("overview");
            }}
          >
            {!scans.length && <option value="">{t("noSelection")}</option>}
            {scans.map((s) => (
              <option key={s.id} value={s.id}>
                {s.domain} ·{" "}
                {t("urlCount", { count: number(s.verified_urls, lang) })}
              </option>
            ))}
          </select>
        </div>
        <ChevronDown size={14} />
      </div>

      <span className="nav-section">{t("analysis")}</span>
      <nav className="oc-nav">
        <button
          className={`oc-nav-item ${page === "overview" ? "active" : ""}`}
          onClick={() => onPage("overview")}
        >
          <span className="oc-tile orange">
            <LayoutDashboard size={16} />
          </span>
          {t("overview")}
        </button>

        <button
          className={`oc-nav-item ${page === "explorer" ? "active" : ""}`}
          onClick={() => onPage("explorer")}
        >
          <span className="oc-tile pink">
            <SearchX size={16} />
          </span>
          {t("explorer")}
          {issueCount > 0 && <span className="nav-count">{issueCount}</span>}
        </button>

        <span className="nav-section tools-label">{t("tools")}</span>
        <button
          className={`oc-nav-item ${page === "engines" ? "active" : ""}`}
          onClick={() => onPage("engines")}
        >
          <span className="oc-tile cyan">
            <KeyRound size={16} />
          </span>
          {t("engines")}
        </button>

        <button
          className={`oc-nav-item ${page === "sessions" ? "active" : ""}`}
          onClick={() => onPage("sessions")}
        >
          <span className="oc-tile violet">
            <FolderClock size={16} />
          </span>
          {t("sessions")}
          <span className="nav-count faint">{scans.length}</span>
        </button>
      </nav>

      <div className="rail-engine">
        <div className="row between">
          <b>Google · Yandex · Bing</b>
          <span className="oc-badge">Rust</span>
        </div>
        <p className="muted" style={{ fontSize: 11.5, marginTop: 6 }}>
          <ShieldCheck size={12} style={{ verticalAlign: "middle", marginRight: 4 }} />
          {t("railEngineLead")}
        </p>
        {activeJobs.length > 0 && (
          <div style={{ marginTop: 8, fontSize: 12 }}>
            <span className="oc-badge warning">
              {activeJobs[0]?.domain} ({activeJobs[0]?.progress.verified_urls}/
              {activeJobs[0]?.progress.discovered_urls})
            </span>
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
