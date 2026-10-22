import { CloudCog, ShieldCheck } from "lucide-react";
import { BrandMark } from "@octopusseo/ui";
import type { Job, ProjectRow } from "../../api/types";
import { useI18n } from "../../i18n";
import type { PageId } from "../../lib/route";
import { useSync } from "../../lib/sync";
import { GROUPS, MODULES } from "../registry";

export function Sidebar({
  page,
  project,
  jobs,
  web,
  version,
  onPage,
}: {
  page: PageId;
  project: ProjectRow | null;
  jobs: Job[];
  web: boolean;
  version?: string;
  onPage: (page: PageId) => void;
}) {
  const { t } = useI18n();
  const running = jobs.filter((job) => job.status === "running");
  const synced = useSync()?.active === true;
  return (
    <aside className="oc-rail platform-rail">
      <div className="oc-titlebar" data-tauri-drag-region />
      <div className="oc-brand">
        <BrandMark />
        <div>
          <div className="oc-brand-name">
            OctopusOn <span>SEO</span>
          </div>
          <div className="oc-brand-product">
            {t("common.product")} <span className="product-tag">{t("common.beta")}</span>
          </div>
        </div>
      </div>
      {project && (
        <button type="button" className="rail-project" onClick={() => onPage("dashboard")} title={t("common.switchProject")}>
          <span className="faint">{t("common.project")}</span>
          <strong>{project.name}</strong>
          <small>
            {project.domain} · {project.language}-{project.country}
          </small>
        </button>
      )}
      <nav className="oc-nav" aria-label={t("common.product")}>
        {GROUPS.map((group) => (
          <div key={group.id} className="rail-group">
            {group.label && <div className="rail-group-label">{t(group.label)}</div>}
            {MODULES.filter((entry) => entry.group === group.id).map((entry) => (
              <button
                key={entry.id}
                type="button"
                className={`oc-nav-item ${page === entry.id ? "active" : ""}`}
                aria-current={page === entry.id ? "page" : undefined}
                onClick={() => onPage(entry.id)}
              >
                <span className={`oc-tile ${entry.tone}`}>
                  <entry.icon size={16} />
                </span>
                <span className="rail-label">{t(entry.label)}</span>
                {!entry.ready && <span className="rail-soon">{t("common.soon")}</span>}
              </button>
            ))}
          </div>
        ))}
        {running.length > 0 && (
          <div className="rail-jobs" aria-live="polite">
            <div className="rail-group-label">{t("common.runningJobs")}</div>
            {running.map((job) => (
              <div key={job.id} className="rail-job">
                <span className="loading-dot" aria-hidden="true" />
                <span>{job.kind === "account-login" ? t("common.jobAccountLogin") : job.kind === "rank-check" ? t("common.jobRankCheck", { name: job.label }) : job.kind === "site-audit" ? t("common.jobSiteAudit", { name: job.label }) : job.kind === "ai-visibility" ? t("common.jobAiVisibility", { name: job.label }) : job.kind === "search-console" ? t("common.jobSearchConsole", { name: job.label }) : job.kind === "google-login" ? t("common.jobGoogleLogin") : job.kind === "report-email" ? t("common.jobReportEmail", { name: job.label }) : job.label}</span>
                {job.total > 0 && (
                  <small>
                    {job.done}/{job.total}
                  </small>
                )}
              </div>
            ))}
          </div>
        )}
      </nav>
      <div className="rail-note">
        {synced ? <CloudCog size={13} aria-hidden="true" /> : <ShieldCheck size={13} aria-hidden="true" />}
        <p>{web ? t("common.webNote") : synced ? t("common.syncNote") : t("common.localNote")}</p>
      </div>
      {version && <div className="rail-version">v{version}</div>}
    </aside>
  );
}
