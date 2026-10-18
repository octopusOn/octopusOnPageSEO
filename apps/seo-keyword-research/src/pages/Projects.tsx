import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FolderPlus, Pencil, Trash2 } from "lucide-react";
import { api, errorText } from "../api/client";
import type { Engine, ProjectRow, ProjectSpec, Settings } from "../api/types";
import { Button, Check, Empty, ErrorState, Field, Loading, Modal } from "../components/ui";
import { date, number, usd, useI18n, type Key } from "../i18n";

const ENGINES: Engine[] = ["google", "yandex", "bing"];

function ProjectModal({
  project,
  settings,
  onClose,
  onSaved,
  notify,
}: {
  project: ProjectRow | null;
  settings: Settings | undefined;
  onClose: () => void;
  onSaved: (project: ProjectRow) => void;
  notify: (text: string, error?: boolean) => void;
}) {
  const { t } = useI18n();
  const [spec, setSpec] = useState<ProjectSpec>(() =>
    project
      ? {
          name: project.name,
          country: project.country,
          language: project.language,
          engines: project.engines,
          site: project.site,
          competitors: project.competitors,
        }
      : {
          name: "",
          country: settings?.projects.country ?? "tr",
          language: settings?.projects.language ?? "tr",
          engines: settings?.projects.engines ?? ["google"],
          site: "",
          competitors: [],
        },
  );
  const [competitors, setCompetitors] = useState(spec.competitors.join("\n"));
  const save = useMutation({
    mutationFn: () => {
      const next = {
        ...spec,
        site: spec.site?.trim() ? spec.site.trim() : null,
        competitors: competitors.split(/[\n,]/).map((line) => line.trim()).filter(Boolean),
      };
      return project ? api.updateProject(project.id, next) : api.createProject(next);
    },
    onSuccess: (saved) => {
      notify(t("saved"));
      onSaved(saved);
    },
    onError: (error) => notify(errorText(error), true),
  });
  const toggle = (engine: Engine, on: boolean) =>
    setSpec((old) => ({
      ...old,
      engines: on ? [...old.engines.filter((item) => item !== engine), engine] : old.engines.filter((item) => item !== engine),
    }));
  return (
    <Modal
      title={project ? t("editProject") : t("newProject")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("cancel")}</Button>
          <Button primary busy={save.isPending} disabled={!spec.name.trim() || spec.engines.length === 0} onClick={() => save.mutate()}>
            {t("save")}
          </Button>
        </>
      }
    >
      <div className="form-grid">
        <Field label={t("projectName")}>
          <input className="oc-input" value={spec.name} onChange={(e) => setSpec({ ...spec, name: e.target.value })} placeholder={t("projectNamePlaceholder")} />
        </Field>
        <div className="form-row">
          <Field label={t("country")} hint={t("countryHint")}>
            <input className="oc-input" maxLength={2} value={spec.country} onChange={(e) => setSpec({ ...spec, country: e.target.value.toLowerCase() })} />
          </Field>
          <Field label={t("languageCode")} hint={t("languageHint")}>
            <input className="oc-input" maxLength={2} value={spec.language} onChange={(e) => setSpec({ ...spec, language: e.target.value.toLowerCase() })} />
          </Field>
        </div>
        <Field label={t("engines")} hint={t("enginesHint")}>
          <div className="row wrap">
            {ENGINES.map((engine) => (
              <Check key={engine} checked={spec.engines.includes(engine)} onChange={(on) => toggle(engine, on)}>
                {t(engine)}
                {spec.engines[0] === engine && <span className="oc-badge">{t("primary")}</span>}
              </Check>
            ))}
          </div>
        </Field>
        <Field label={t("yourSite")} hint={t("yourSiteHint")}>
          <input className="oc-input" value={spec.site ?? ""} onChange={(e) => setSpec({ ...spec, site: e.target.value })} placeholder="ornek.com.tr" />
        </Field>
        <Field label={t("competitors")} hint={t("competitorsHint")}>
          <textarea className="oc-textarea" rows={3} value={competitors} onChange={(e) => setCompetitors(e.target.value)} placeholder={"rakip.com\ndiger.com.tr"} />
        </Field>
      </div>
    </Modal>
  );
}

export function Projects({
  selected,
  onSelect,
  settings,
  notify,
}: {
  selected: number | null;
  onSelect: (id: number | null) => void;
  settings: Settings | undefined;
  notify: (text: string, error?: boolean) => void;
}) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const projects = useQuery({ queryKey: ["projects"], queryFn: api.projects });
  const runs = useQuery({ queryKey: ["runs", selected], queryFn: () => api.runs(selected) });
  const [editing, setEditing] = useState<ProjectRow | null | "new">(null);
  const remove = useMutation({
    mutationFn: (id: number) => api.deleteProject(id),
    onSuccess: (_, id) => {
      if (selected === id) onSelect(null);
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
    },
    onError: (error) => notify(errorText(error), true),
  });
  if (projects.isLoading) return <Loading />;
  if (projects.error) return <ErrorState message={errorText(projects.error)} retry={() => void projects.refetch()} />;
  const list = projects.data ?? [];
  return (
    <div className="stack">
      <div className="row between">
        <span className="faint">{t("projectCount", { count: list.length })}</span>
        <Button primary onClick={() => setEditing("new")}>
          <FolderPlus size={16} />
          {t("newProject")}
        </Button>
      </div>
      {list.length === 0 ? (
        <Empty icon={<FolderPlus size={28} />}>
          <b>{t("noProjects")}</b>
          <p>{t("noProjectsLead")}</p>
        </Empty>
      ) : (
        <div className="oc-cards project-cards">
          {list.map((project) => (
            <article key={project.id} className={`oc-card project-card ${selected === project.id ? "selected" : ""}`}>
              <header className="row between">
                <h3 className="oc-card-title">{project.name}</h3>
                <span className="oc-badge">
                  {project.language}-{project.country}
                </span>
              </header>
              <p className="faint">
                {project.engines.map((engine) => t(engine)).join(" · ")}
                {project.site && ` · ${project.site}`}
              </p>
              <div className="oc-stats compact">
                <div className="oc-stat">
                  <span className="oc-stat-label">{t("keywords")}</span>
                  <span className="oc-stat-value">{number(project.keywords, lang, 0)}</span>
                </div>
                <div className="oc-stat">
                  <span className="oc-stat-label">{t("clusters")}</span>
                  <span className="oc-stat-value">{project.clusters === null ? "–" : number(project.clusters, lang, 0)}</span>
                </div>
                <div className="oc-stat">
                  <span className="oc-stat-label">{t("excluded")}</span>
                  <span className="oc-stat-value">{number(project.excluded, lang, 0)}</span>
                </div>
              </div>
              <footer className="oc-card-foot row between">
                <Button primary={selected !== project.id} small onClick={() => onSelect(project.id)}>
                  {selected === project.id ? t("selected") : t("open")}
                </Button>
                <span className="row">
                  <Button small className="icon ghost" aria-label={t("edit")} onClick={() => setEditing(project)}>
                    <Pencil size={15} />
                  </Button>
                  <Button
                    small
                    className="icon ghost"
                    aria-label={t("remove")}
                    onClick={() => {
                      if (window.confirm(t("confirmDeleteProject", { name: project.name, count: project.keywords + project.excluded }))) remove.mutate(project.id);
                    }}
                  >
                    <Trash2 size={15} />
                  </Button>
                </span>
              </footer>
            </article>
          ))}
        </div>
      )}
      {selected !== null && (runs.data?.length ?? 0) > 0 && (
        <section className="oc-panel">
          <h2 className="oc-panel-title">{t("recentRuns")}</h2>
          <div className="oc-table-wrap">
            <table className="oc-table">
              <thead>
                <tr>
                  <th>{t("started")}</th>
                  <th>{t("kind")}</th>
                  <th>{t("status")}</th>
                  <th className="num">{t("steps")}</th>
                  <th className="num">{t("spent")}</th>
                  <th>{t("problem")}</th>
                </tr>
              </thead>
              <tbody>
                {(runs.data ?? []).slice(0, 12).map((run) => (
                  <tr key={run.id}>
                    <td>{date(run.started_at, lang)}</td>
                    <td>{t(`run.${run.kind}` as Key)}</td>
                    <td>{t(`status.${run.status}` as Key)}</td>
                    <td className="num">
                      {run.done}/{run.planned}
                    </td>
                    <td className="num">{usd(run.cost_usd, lang)}</td>
                    <td className="faint clip">{run.error ?? ""}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      )}
      {editing !== null && (
        <ProjectModal
          project={editing === "new" ? null : editing}
          settings={settings}
          notify={notify}
          onClose={() => setEditing(null)}
          onSaved={(project) => {
            setEditing(null);
            onSelect(project.id);
            void queryClient.invalidateQueries({ queryKey: ["projects"] });
          }}
        />
      )}
    </div>
  );
}
