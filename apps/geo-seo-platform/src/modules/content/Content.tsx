// Content optimisation: briefs made from the pages ranking for a keyword, and the project's
// pages checked against the pages ranking for their keyword. A brief opens on its own (the
// `brief` address field) with its editor and scored pages.

import { useState, type FormEvent } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FilePlus2, FileSearch, PenLine, Smartphone, Trash2 } from "lucide-react";
import { Button, DataTable, Empty, Segmented, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { BriefSummary, CheckerRow, ProjectRow } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { Entity } from "../../lib/Entity";
import { formatNumber, formatUsd, relativeTime } from "../../lib/format";
import { MarketPicker } from "../../lib/MarketPicker";
import { blockedText, CostLabel, refreshSpending } from "../../lib/PaidFrame";
import { tableLabels } from "../../lib/tables";
import { BriefDetail } from "./BriefDetail";
import { ScoreChip } from "./ScoreChip";

const TABS = ["briefs", "checker"] as const;
type Tab = (typeof TABS)[number];

/** Starts a brief for a keyword in the market chosen above. */
function NewBrief({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const { params, go, fail, notify } = useShell();
  const queryClient = useQueryClient();
  const [keyword, setKeyword] = useState(params.keyword ?? "");
  const [mobile, setMobile] = useState(false);
  const asked = keyword.trim().replace(/\s+/g, " ");
  const ask = { project_id: project.id, keyword: asked, mobile, ...(params.market ? { market: params.market } : {}) };
  const estimate = useQuery({ queryKey: ["content", "estimate", ask], queryFn: () => api.contentBriefEstimate(ask), enabled: asked.length > 0 });
  const blocked = blockedText(estimate.data, t);
  const kept = estimate.data?.kept?.fresh ?? false;
  const start = useMutation({
    mutationFn: () => api.startContentBrief(ask),
    onSuccess: (started) => {
      notify(t("content.started", { keyword: asked }));
      void queryClient.invalidateQueries({ queryKey: ["content"] });
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
      refreshSpending(queryClient);
      go({ params: { market: params.market, brief: String(started.brief_id) } });
    },
    onError: (thrown) => fail(thrown),
  });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (asked) start.mutate();
  };
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("content.newTitle")}</h3>
          <span className="faint">{t("content.newHint")}</span>
        </div>
        <div className="report-actions">
          <MarketPicker project={project} />
        </div>
      </header>
      <form className="new-brief" onSubmit={submit}>
        <input className="oc-input" value={keyword} onChange={(event) => setKeyword(event.target.value)} placeholder={t("content.keywordPlaceholder")} aria-label={t("content.keyword")} maxLength={200} />
        <Segmented<string>
          label={t("keywords.device")}
          value={mobile ? "mobile" : "desktop"}
          onChange={(value) => setMobile(value === "mobile")}
          options={[
            { value: "desktop", label: t("keywords.desktop") },
            { value: "mobile", label: t("keywords.mobile") },
          ]}
        />
        {estimate.data && !blocked && (kept ? <span className="cost-label">{t("content.serpKept")}</span> : <CostLabel estimate={estimate.data} />)}
        <Button type="submit" primary small busy={start.isPending} disabled={!asked || (Boolean(blocked) && !kept)} title={blocked ?? undefined}>
          <FilePlus2 size={14} />
          {t("content.create")}
        </Button>
      </form>
      {blocked && !kept && (
        <div className="oc-banner warning" role="status">
          <span>{blocked}</span>
          <Button small onClick={() => go({ page: "sources" })}>
            {t("common.goToSources")}
          </Button>
        </div>
      )}
      <p className="faint small">{t("content.howItWorks")}</p>
    </section>
  );
}

function BriefList({ project }: { project: ProjectRow }) {
  const { t, locale } = useI18n();
  const { go, params, fail, jobs } = useShell();
  const queryClient = useQueryClient();
  const briefs = useQuery({
    queryKey: ["content", "briefs", project.id],
    queryFn: () => api.contentBriefs(project.id),
    refetchInterval: (state) => (state.state.data?.some((row) => row.status === "running") ? 4000 : false),
  });
  const remove = useMutation({
    mutationFn: (id: number) => api.deleteContentBrief(id),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["content"] }),
    onError: (thrown) => fail(thrown),
  });
  const running = jobs.find((job) => job.kind === "content-brief" && job.project_id === project.id && job.status === "running");
  const open = (row: BriefSummary) => go({ params: { ...params, brief: String(row.id), keyword: undefined } });
  const columns: Column<BriefSummary>[] = [
    {
      key: "keyword",
      label: t("content.colKeyword"),
      render: (row) => (
        <button type="button" className="link-button cell-main" onClick={() => open(row)}>
          {row.keyword}
          {row.mobile && <Smartphone size={12} aria-label={t("keywords.mobile")} />}
        </button>
      ),
      sort: (row) => row.keyword,
    },
    { key: "market", label: t("content.colMarket"), render: (row) => `${row.country.toUpperCase()} · ${row.language}`, sort: (row) => `${row.country}-${row.language}`, optional: true },
    {
      key: "status",
      label: t("content.colStatus"),
      render: (row) =>
        row.status === "running" ? (
          <span className="oc-chip small accent">{running ? t("content.reading", { done: running.done, total: running.total || 10 }) : t("content.status_running")}</span>
        ) : row.status === "done" ? (
          <span className="oc-chip small">{t("content.status_done")}</span>
        ) : (
          <span className="oc-chip small warning" title={row.error ?? undefined}>
            {t(`content.status_${row.status}`)}
          </span>
        ),
      sort: (row) => row.status,
    },
    { key: "words", label: t("content.colWords"), render: (row) => (row.words === null ? "—" : formatNumber(Math.round(row.words), locale)), sort: (row) => row.words, numeric: true },
    { key: "pages", label: t("content.colPages"), render: (row) => formatNumber(row.pages, locale), sort: (row) => row.pages, numeric: true, optional: true },
    { key: "written", label: t("content.colWritten"), render: (row) => (row.written ? <PenLine size={14} aria-label={t("content.written")} /> : <span className="faint">—</span>), sort: (row) => Number(row.written), optional: true },
    { key: "cost", label: t("content.colCost"), render: (row) => (row.cost_usd > 0 ? formatUsd(row.cost_usd, locale) : <span className="faint">{t("content.free")}</span>), sort: (row) => row.cost_usd, numeric: true, optional: true },
    { key: "created", label: t("content.colCreated"), render: (row) => relativeTime(row.created_at, locale), sort: (row) => row.created_at },
    {
      key: "actions",
      label: "",
      render: (row) => (
        <Button small className="ghost" disabled={row.status === "running"} aria-label={t("content.delete")} title={t("content.delete")} busy={remove.isPending && remove.variables === row.id} onClick={() => remove.mutate(row.id)}>
          <Trash2 size={14} />
        </Button>
      ),
    },
  ];
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("content.listTitle")}</h3>
          <span className="faint">{t("content.listHint")}</span>
        </div>
      </header>
      {(briefs.data?.length ?? 0) === 0 ? (
        <Empty icon={<PenLine size={26} />} title={t("content.noBriefsTitle")}>
          <p>{t("content.noBriefsText")}</p>
        </Empty>
      ) : (
        <DataTable rows={briefs.data ?? []} columns={columns} rowKey={(row) => String(row.id)} initialSort={{ key: "created", direction: "desc" }} tableId="content-briefs" empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
      )}
    </section>
  );
}

/** The project's pages with the keyword each ranks for or gets clicks from, checked on demand. */
function Checker({ project }: { project: ProjectRow }) {
  const { t, locale } = useI18n();
  const { go, params, fail, notify, jobs } = useShell();
  const queryClient = useQueryClient();
  const pages = useQuery({ queryKey: ["content", "pages", project.id], queryFn: () => api.contentPages(project.id) });
  const running = jobs.some((job) => job.kind === "content-brief" && job.project_id === project.id && job.status === "running");
  const check = useMutation({
    mutationFn: (row: CheckerRow) => api.startContentBrief({ project_id: project.id, keyword: row.keyword, page: row.url, ...(params.market ? { market: params.market } : {}) }),
    onSuccess: (started, row) => {
      notify(t("content.checking", { url: row.url }));
      void queryClient.invalidateQueries({ queryKey: ["content"] });
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
      refreshSpending(queryClient);
      go({ params: { brief: String(started.brief_id) } });
    },
    onError: (thrown) => fail(thrown),
  });
  const columns: Column<CheckerRow>[] = [
    {
      key: "url",
      label: t("content.colPage"),
      render: (row) => (
        <Entity kind="url" value={row.url}>
          <span className="cell-url checker-url" title={row.url}>
            {row.url.replace(/^https?:\/\/(www\.)?/, "")}
          </span>
        </Entity>
      ),
      sort: (row) => row.url,
    },
    {
      key: "keyword",
      label: t("content.colKeyword"),
      render: (row) => (
        <Entity kind="keyword" value={row.keyword}>
          <span>{row.keyword}</span>
        </Entity>
      ),
      sort: (row) => row.keyword,
    },
    {
      key: "score",
      label: t("content.colScore"),
      render: (row) => (
        <span className="checker-score" title={row.scored_for ? t("content.scoredFor", { keyword: row.scored_for }) : undefined}>
          <ScoreChip score={row.score} />
          {row.scored_at && <span className="faint small">{relativeTime(row.scored_at, locale)}</span>}
        </span>
      ),
      sort: (row) => row.score,
      numeric: true,
    },
    {
      key: "check",
      label: "",
      render: (row) => (
        <Button small disabled={running} busy={check.isPending && check.variables?.url === row.url} onClick={() => check.mutate(row)}>
          <FileSearch size={14} />
          {t("content.check")}
        </Button>
      ),
    },
    { key: "source", label: t("content.colSource"), render: (row) => <span className="oc-chip small">{t(`content.source_${row.source}`)}</span>, sort: (row) => row.source, optional: true },
    { key: "position", label: t("content.colPosition"), render: (row) => (row.position === null ? "—" : new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(row.position)), sort: (row) => row.position, numeric: true },
    { key: "volume", label: t("overview.colVolume"), render: (row) => (row.volume === null ? "—" : formatNumber(row.volume, locale)), sort: (row) => row.volume, numeric: true, optional: true },
    { key: "clicks", label: t("content.colClicks"), render: (row) => (row.clicks === null ? "—" : formatNumber(Math.round(row.clicks), locale)), sort: (row) => row.clicks, numeric: true, optional: true },
  ];
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("content.checkerTitle")}</h3>
          <span className="faint">{t("content.checkerHint")}</span>
        </div>
        <div className="report-actions">
          <MarketPicker project={project} />
        </div>
      </header>
      {(pages.data?.length ?? 0) === 0 ? (
        <Empty icon={<FileSearch size={26} />} title={t("content.noPagesTitle")}>
          <p>{t("content.noPagesText")}</p>
          <div className="empty-actions">
            <Button small onClick={() => go({ page: "rank" })}>
              {t("common.navRank")}
            </Button>
            <Button small onClick={() => go({ page: "gsc" })}>
              {t("common.navGsc")}
            </Button>
          </div>
        </Empty>
      ) : (
        <>
          <p className="faint small">{t("content.checkerCost")}</p>
          <DataTable rows={pages.data ?? []} columns={columns} rowKey={(row) => row.url} initialSort={{ key: "score", direction: "asc" }} tableId="content-checker" empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
        </>
      )}
    </section>
  );
}

export function Content({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const { go, params } = useShell();
  const briefId = Number(params.brief);
  if (Number.isInteger(briefId) && briefId > 0) {
    return <BriefDetail key={briefId} project={project} briefId={briefId} onBack={() => go({ params: { market: params.market, tab: params.tab } })} />;
  }
  const tab: Tab = (TABS as readonly string[]).includes(params.tab ?? "") ? (params.tab as Tab) : "briefs";
  const labels: Record<Tab, string> = { briefs: t("content.tabBriefs"), checker: t("content.tabChecker") };
  return (
    <div className="page-stack">
      <div className="oc-tabs" role="tablist" aria-label={t("common.navContent")}>
        {TABS.map((entry) => (
          <button key={entry} type="button" role="tab" aria-selected={tab === entry} className={`oc-tab ${tab === entry ? "active" : ""}`} onClick={() => go({ params: { ...params, tab: entry } })}>
            {labels[entry]}
          </button>
        ))}
      </div>
      {tab === "briefs" && (
        <>
          <NewBrief project={project} />
          <BriefList project={project} />
        </>
      )}
      {tab === "checker" && <Checker project={project} />}
    </div>
  );
}
