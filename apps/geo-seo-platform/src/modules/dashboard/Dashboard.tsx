import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FolderPlus, Globe, Pencil, Plus, Trash2, Wallet } from "lucide-react";
import { Button, Empty, ErrorState, Loading } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { ProjectRow, SnapshotProgress } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { explain } from "../../lib/errors";
import { creditsOf, formatCredits } from "../../lib/format";
import { ProjectDialog } from "../projects/ProjectDialog";
import { deltaText, toneOf } from "../snapshots/snapshot";

function AccountStrip() {
  const { t, locale } = useI18n();
  const { go } = useShell();
  const status = useQuery({ queryKey: ["account"], queryFn: api.account, refetchInterval: 30_000 });
  const account = status.data?.account;
  return (
    <button type="button" className="account-strip oc-card" onClick={() => go({ page: "sources" })}>
      <span className="oc-tile amber">
        <Wallet size={16} />
      </span>
      <span className="account-strip-text">
        <strong>{t("sources.accountTitle")}</strong>
        <small>
          {account
            ? t("sources.signedInAs", { email: account.email ?? "" })
            : status.data?.signed_in
              ? (status.data.error ?? "")
              : t("sources.notSignedIn")}
        </small>
      </span>
      {account && (
        <span className="account-strip-balance">
          {t("common.credits", { n: formatCredits(creditsOf(account.available_micros, account.credit_unit_micros), locale) })}
        </span>
      )}
    </button>
  );
}

/** Where the project stands against its first snapshot. */
function Progress({ project, progress }: { project: ProjectRow; progress: SnapshotProgress }) {
  const { t, locale } = useI18n();
  const { go } = useShell();
  const moved = progress.kpis.filter((kpi) => deltaText(kpi, locale) !== null).slice(0, 3);
  if (moved.length === 0) return null;
  return (
    <button type="button" className="project-progress chip-button-row" onClick={() => go({ page: "snapshots", project: project.id })} title={t("snapshots.openPage")}>
      <span className="faint small">{t("snapshots.since", { n: progress.days })}</span>
      {moved.map((kpi) => (
        <span key={kpi.key} className={`oc-chip ${toneOf(kpi) ?? ""}`}>
          {t(`snapshots.kpi_${kpi.key}`)} {deltaText(kpi, locale)}
        </span>
      ))}
    </button>
  );
}

function ProjectCard({ project, progress, onEdit }: { project: ProjectRow; progress: SnapshotProgress | undefined; onEdit: () => void }) {
  const { t, locale } = useI18n();
  const { go, notify, fail } = useShell();
  const queryClient = useQueryClient();
  const regions = new Intl.DisplayNames([locale], { type: "region" });
  const remove = useMutation({
    mutationFn: () => api.deleteProject(project.id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
      notify(t("projects.deleted"));
    },
    onError: (thrown) => fail(thrown),
  });
  return (
    <article className="oc-card project-card">
      <header className="project-card-head">
        <span className="oc-tile blue">
          <Globe size={16} />
        </span>
        <div>
          <h3>{project.name}</h3>
          <p className="faint">{project.domain}</p>
        </div>
      </header>
      <dl className="project-facts">
        <div>
          <dt>{t("projects.market")}</dt>
          <dd>
            {regions.of(project.country.toUpperCase()) ?? project.country} · {project.language}
          </dd>
        </div>
        <div>
          <dt>{t("projects.competitors")}</dt>
          <dd>{project.competitors.length ? t("projects.competitorsCount", { n: project.competitors.length }) : t("projects.noCompetitors")}</dd>
        </div>
      </dl>
      {progress && <Progress project={project} progress={progress} />}
      <footer className="project-card-foot">
        <Button primary small onClick={() => go({ page: "project", project: project.id })}>
          {t("projects.openDashboard")}
        </Button>
        <Button small className="ghost" onClick={() => go({ page: "overview", project: project.id })}>
          {t("projects.openOverview")}
        </Button>
        <Button small className="icon ghost" onClick={onEdit} aria-label={t("common.edit")} title={t("common.edit")}>
          <Pencil size={15} />
        </Button>
        <Button
          small
          className="icon ghost"
          busy={remove.isPending}
          aria-label={t("common.delete")}
          title={t("common.delete")}
          onClick={() => {
            if (window.confirm(t("projects.deleteConfirm", { name: project.name }))) remove.mutate();
          }}
        >
          <Trash2 size={15} />
        </Button>
      </footer>
    </article>
  );
}

export function Dashboard() {
  const { t } = useI18n();
  const { go } = useShell();
  const projects = useQuery({ queryKey: ["projects"], queryFn: api.projects });
  const progress = useQuery({ queryKey: ["snapshot-progress"], queryFn: api.snapshotProgress });
  const [editing, setEditing] = useState<ProjectRow | "new" | null>(null);
  return (
    <div className="page-stack">
      <AccountStrip />
      <section className="section">
        <div className="section-head">
          <h2>{t("projects.title")}</h2>
          <Button primary onClick={() => setEditing("new")}>
            <Plus size={16} />
            {t("projects.new")}
          </Button>
        </div>
        {projects.isLoading && <Loading label={t("common.loading")} />}
        {projects.error && (
          <ErrorState title={t("common.loadFailed")} message={explain(projects.error, t).message} retry={() => void projects.refetch()} retryLabel={t("common.retry")} />
        )}
        {projects.data?.length === 0 && (
          <div className="oc-card">
            <Empty icon={<FolderPlus size={28} />} title={t("projects.emptyTitle")}>
              <p>{t("projects.emptyText")}</p>
              <Button primary onClick={() => setEditing("new")}>
                <Plus size={16} />
                {t("projects.new")}
              </Button>
            </Empty>
          </div>
        )}
        {projects.data && projects.data.length > 0 && (
          <div className="project-grid">
            {projects.data.map((project) => (
              <ProjectCard key={project.id} project={project} progress={progress.data?.find((entry) => entry.project_id === project.id)} onEdit={() => setEditing(project)} />
            ))}
          </div>
        )}
      </section>
      {editing && (
        <ProjectDialog
          project={editing === "new" ? null : editing}
          onClose={() => setEditing(null)}
          onSaved={(row) => {
            setEditing(null);
            if (editing === "new") go({ page: "project", project: row.id });
          }}
        />
      )}
    </div>
  );
}
