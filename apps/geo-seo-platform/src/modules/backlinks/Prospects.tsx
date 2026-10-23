// Link prospects: sites that could link to the project, gathered from the link gap, lost and
// broken links and competitors' best-linked pages, with their outreach state and notes.

import { useState, type FormEvent } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Handshake, Plus, Trash2 } from "lucide-react";
import { Button, DataTable, Empty, Segmented, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { LinkedPageRow, LinkProspect, ProjectRow, ProspectSource, ProspectStatus } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { domainOf, Entity } from "../../lib/Entity";
import { formatNumber, relativeTime } from "../../lib/format";
import { PaidFrame } from "../../lib/PaidFrame";
import { shortUrl, tableLabels } from "../../lib/tables";

const STATES: ProspectStatus[] = ["prospect", "contacted", "won", "lost"];

/** Adds sites to the prospects (or updates them), from any list. */
export function useAddProspect(projectId: number) {
  const { notify, fail } = useShell();
  const { t } = useI18n();
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (prospect: Pick<LinkProspect, "domain" | "url" | "source">) => api.saveLinkProspect(projectId, { ...prospect, status: "prospect" }),
    onSuccess: (saved) => {
      void queryClient.invalidateQueries({ queryKey: ["link-prospects", projectId] });
      notify(t("backlinks.prospectAdded", { domain: saved.domain }));
    },
    onError: (thrown) => fail(thrown),
  });
}

/** A row's "add to prospects" button; shows a tick once the site is a prospect. */
export function ProspectButton({ projectId, domain, url, source }: { projectId: number; domain: string | null; url?: string | null; source: ProspectSource }) {
  const { t } = useI18n();
  const prospects = useQuery({ queryKey: ["link-prospects", projectId], queryFn: () => api.linkProspects(projectId) });
  const add = useAddProspect(projectId);
  if (!domain) return null;
  const site = domainOf(domain);
  if (prospects.data?.some((prospect) => prospect.domain === site)) {
    return (
      <span className="faint small row-done">
        <Check size={13} aria-hidden="true" /> {t("backlinks.isProspect")}
      </span>
    );
  }
  return (
    <Button small className="ghost" busy={add.isPending} onClick={() => add.mutate({ domain: site, url: url ?? null, source })} title={t("backlinks.addProspectHint")}>
      <Handshake size={13} aria-hidden="true" />
      {t("backlinks.addProspect")}
    </Button>
  );
}

function NoteInput({ value, onSave }: { value: string; onSave: (note: string) => void }) {
  const { t } = useI18n();
  const [text, setText] = useState(value);
  return (
    <input
      className="oc-input note-input"
      value={text}
      placeholder={t("backlinks.notePlaceholder")}
      aria-label={t("backlinks.note")}
      onChange={(event) => setText(event.target.value)}
      onBlur={() => {
        if (text !== value) onSave(text);
      }}
    />
  );
}

/** Competitors' pages with the most links: content worth matching, and their linking sites. */
function CompetitorPages({ project }: { project: ProjectRow }) {
  const { t, locale } = useI18n();
  const { go } = useShell();
  const [competitor, setCompetitor] = useState(project.competitors[0] ?? "");
  if (project.competitors.length === 0) return null;
  const columns: Column<LinkedPageRow>[] = [
    { key: "url", label: t("overview.colPage"), render: (row) => (
        <Entity kind="url" value={row.url}>
          <span className="cell-url">{shortUrl(row.url)}</span>
        </Entity>
      ), sort: (row) => row.url },
    { key: "domains", label: t("backlinks.referringDomains"), render: (row) => formatNumber(row.referring_domains, locale), sort: (row) => row.referring_domains, numeric: true },
    { key: "backlinks", label: t("backlinks.backlinks"), render: (row) => formatNumber(row.backlinks, locale), sort: (row) => row.backlinks, numeric: true },
    {
      key: "linkers",
      label: "",
      render: (row) => (
        <Button small className="ghost" onClick={() => go({ page: "backlinks", params: { url: row.url, tab: "backlinks" } })}>
          {t("backlinks.seeLinkers")}
        </Button>
      ),
    },
  ];
  return (
    <PaidFrame<LinkedPageRow[]>
      module="backlinks"
      ask={{ project_id: project.id, report: "pages", target: competitor }}
      title={t("backlinks.competitorPagesTitle")}
      hint={t("backlinks.competitorPagesHint")}
      extra={
        <select className="oc-select" value={competitor} onChange={(event) => setCompetitor(event.target.value)} aria-label={t("backlinks.competitor")}>
          {project.competitors.map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </select>
      }
    >
      {(rows) => <DataTable rows={rows.filter((row) => (row.status ?? 200) < 400)} columns={columns} rowKey={(row) => row.url} initialSort={{ key: "domains", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />}
    </PaidFrame>
  );
}

export function ProspectsTab({ project }: { project: ProjectRow }) {
  const { t, locale } = useI18n();
  const { go, fail } = useShell();
  const queryClient = useQueryClient();
  const prospects = useQuery({ queryKey: ["link-prospects", project.id], queryFn: () => api.linkProspects(project.id) });
  const [status, setStatus] = useState<ProspectStatus | "all">("all");
  const [domain, setDomain] = useState("");
  const [url, setUrl] = useState("");
  const save = useMutation({
    mutationFn: (prospect: LinkProspect) => api.saveLinkProspect(project.id, prospect),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["link-prospects", project.id] }),
    onError: (thrown) => fail(thrown),
  });
  const remove = useMutation({
    mutationFn: (site: string) => api.removeLinkProspect(project.id, site),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["link-prospects", project.id] }),
    onError: (thrown) => fail(thrown),
  });
  const add = (event: FormEvent) => {
    event.preventDefault();
    if (!domain.trim()) return;
    save.mutate({ domain: domainOf(domain), url: url.trim() || null, source: "manual", status: "prospect" });
    setDomain("");
    setUrl("");
  };
  const rows = prospects.data ?? [];
  const count = (state: ProspectStatus) => rows.filter((row) => row.status === state).length;
  const shown = status === "all" ? rows : rows.filter((row) => row.status === status);
  const columns: Column<LinkProspect>[] = [
    {
      key: "domain",
      label: t("overview.colDomain"),
      render: (row) => (
        <span className="link-cell">
          <Entity kind="domain" value={row.domain}>
            <strong className="cell-main">{row.domain}</strong>
          </Entity>
          {row.url && (
            <Entity kind="url" value={row.url}>
              <span className="cell-url">{row.url.replace(/^https?:\/\/(www\.)?/, "")}</span>
            </Entity>
          )}
        </span>
      ),
      sort: (row) => row.domain,
    },
    { key: "source", label: t("backlinks.colSource"), render: (row) => t(`backlinks.source_${row.source}`), sort: (row) => row.source },
    {
      key: "status",
      label: t("backlinks.colStatus"),
      render: (row) => (
        <select className={`oc-select status-select status-${row.status}`} value={row.status} aria-label={t("backlinks.statusFor", { domain: row.domain })} onChange={(event) => save.mutate({ ...row, status: event.target.value as ProspectStatus })}>
          {STATES.map((state) => (
            <option key={state} value={state}>
              {t(`backlinks.prospect_${state}`)}
            </option>
          ))}
        </select>
      ),
      sort: (row) => STATES.indexOf(row.status),
    },
    { key: "note", label: t("backlinks.note"), render: (row) => <NoteInput value={row.note ?? ""} onSave={(note) => save.mutate({ ...row, note })} /> },
    { key: "updated", label: t("backlinks.colUpdated"), render: (row) => (row.updated_at ? relativeTime(row.updated_at, locale) : ""), sort: (row) => row.updated_at ?? "" },
    {
      key: "remove",
      label: "",
      render: (row) => (
        <Button small className="icon ghost" aria-label={t("backlinks.removeProspect", { domain: row.domain })} onClick={() => remove.mutate(row.domain)}>
          <Trash2 size={14} />
        </Button>
      ),
    },
  ];
  return (
    <div className="page-stack">
      <section className="oc-panel report">
        <header className="report-head">
          <div className="report-title">
            <h3 className="oc-panel-title">{t("backlinks.prospectsTitle")}</h3>
            <span className="faint">{t("backlinks.prospectsHint")}</span>
          </div>
          <div className="report-actions">
            <Segmented<ProspectStatus | "all">
              label={t("backlinks.colStatus")}
              value={status}
              onChange={setStatus}
              options={[{ value: "all" as const, label: `${t("backlinks.prospectAll")} ${rows.length}` }, ...STATES.map((state) => ({ value: state, label: `${t(`backlinks.prospect_${state}`)} ${count(state)}` }))]}
            />
          </div>
        </header>
        <form className="list-bar" onSubmit={add}>
          <input className="oc-input" value={domain} onChange={(event) => setDomain(event.target.value)} placeholder={t("backlinks.prospectDomain")} aria-label={t("backlinks.prospectDomain")} spellCheck={false} />
          <input className="oc-input" value={url} onChange={(event) => setUrl(event.target.value)} placeholder={t("backlinks.prospectUrl")} aria-label={t("backlinks.prospectUrl")} spellCheck={false} />
          <Button type="submit" small disabled={!domain.trim()} busy={save.isPending}>
            <Plus size={14} />
            {t("backlinks.addProspect")}
          </Button>
        </form>
        {rows.length === 0 ? (
          <Empty icon={<Handshake size={26} />} title={t("backlinks.prospectsNone")}>
            <p>{t("backlinks.prospectsNoneText")}</p>
            <div className="row-actions">
              <Button small onClick={() => go({ page: "gap", params: { tab: "links" } })}>
                {t("backlinks.fromGap")}
              </Button>
              <Button small onClick={() => go({ page: "backlinks", params: { tab: "backlinks" } })}>
                {t("backlinks.fromLost")}
              </Button>
            </div>
          </Empty>
        ) : (
          <DataTable rows={shown} columns={columns} rowKey={(row) => row.domain} initialSort={{ key: "updated", direction: "desc" }} tableId="link-prospects" empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
        )}
      </section>
      <CompetitorPages project={project} />
    </div>
  );
}
