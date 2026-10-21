import { useMemo, useState, type FormEvent } from "react";
import { Download, Link2, Search } from "lucide-react";
import { Button, DataTable, Segmented, Stat, type Column } from "@octopusseo/ui";
import type {
  AnchorRow,
  BacklinkReport,
  BacklinkRow,
  BacklinkSummary,
  LinkStatus,
  LinkedPageRow,
  NewLostPoint,
  ProjectRow,
  RankRow,
  ReferringDomainRow,
  Share,
} from "../../api/types";
import { useI18n } from "../../i18n";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { formatCompact, formatNumber } from "../../lib/format";
import { PaidFrame } from "../../lib/PaidFrame";
import { download, fold, toCsv } from "../../lib/report";
import { shortUrl, tableLabels } from "../../lib/tables";

const TABS = ["summary", "backlinks", "referring_domains", "anchors", "pages", "bulk"] as const;
type Tab = (typeof TABS)[number];

/** Share of links passing authority, in percent, from the referring domains. */
export function followShare(summary: Pick<BacklinkSummary, "referring_domains" | "referring_domains_nofollow">): number | null {
  if (summary.referring_domains === 0) return null;
  return Math.round(((summary.referring_domains - summary.referring_domains_nofollow) / summary.referring_domains) * 100);
}

/** The authority in a coloured badge. */
export function Authority({ rank }: { rank: number | null }) {
  if (rank === null) return <span className="faint">—</span>;
  return <span className={`oa oa-${Math.min(4, Math.floor(rank / 20))}`}>{rank}</span>;
}

function ShareBars({ title, shares, label }: { title: string; shares: Share[]; label?: (name: string) => string }) {
  const { locale } = useI18n();
  const top = shares.slice(0, 8);
  const most = Math.max(1, ...top.map((share) => share.count));
  return (
    <section className="sub-panel">
      <h4>{title}</h4>
      <ol className="bars">
        {top.map((share) => (
          <li key={share.name}>
            <span className="bar-label">{label ? label(share.name) : share.name || "—"}</span>
            <span className="bar-track" aria-hidden="true">
              <span className="bar-fill" style={{ width: `${(share.count / most) * 100}%` }} />
            </span>
            <span className="bar-value">{formatCompact(share.count, locale)}</span>
          </li>
        ))}
      </ol>
    </section>
  );
}

function NewLostChart({ points }: { points: NewLostPoint[] }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    return {
      grid: { left: 48, right: 12, top: 32, bottom: 24 },
      legend: { top: 0, left: 0 },
      tooltip: { trigger: "axis" },
      xAxis: {
        type: "category",
        data: points.map((point) => new Date(`${point.date}T00:00:00Z`).toLocaleDateString(locale, { month: "short", year: "2-digit" })),
        ...axis,
        splitLine: { show: false },
      },
      yAxis: { type: "value", ...axis, axisLabel: { ...axis.axisLabel, formatter: (value: number) => formatCompact(Math.abs(value), locale) } },
      series: [
        { name: t("backlinks.newDomains"), type: "bar", stack: "domains", barMaxWidth: 18, itemStyle: { color: tokens.good }, data: points.map((point) => point.new_referring_domains) },
        { name: t("backlinks.lostDomains"), type: "bar", stack: "domains", barMaxWidth: 18, itemStyle: { color: tokens.warning }, data: points.map((point) => -point.lost_referring_domains) },
      ],
    };
  }, [points, tokens, t, locale]);
  if (points.length === 0) return <p className="faint">{t("backlinks.noHistory")}</p>;
  return <EChart option={option} height={240} />;
}

function SummaryTab({ projectId, target }: { projectId: number; target: string | undefined }) {
  const { t, locale } = useI18n();
  const regions = new Intl.DisplayNames([locale], { type: "region" });
  const country = (code: string) => {
    if (!code) return "—";
    try {
      return regions.of(code.toUpperCase()) ?? code;
    } catch {
      return code;
    }
  };
  const ask = { project_id: projectId, ...(target ? { target } : {}) };
  return (
    <div className="page-stack">
      <PaidFrame<BacklinkSummary> module="backlinks" ask={{ ...ask, report: "summary" }} title={t("backlinks.profileTitle")} hint={t("backlinks.profileHint")}>
        {(summary) => (
          <div className="page-stack">
            <div className="oc-stats">
              <Stat label={t("backlinks.authority")} value={<Authority rank={summary.rank} />} hint={t("backlinks.authorityHint")} />
              <Stat label={t("backlinks.backlinks")} value={formatCompact(summary.backlinks, locale)} hint={formatNumber(summary.backlinks, locale)} />
              <Stat label={t("backlinks.referringDomains")} value={formatCompact(summary.referring_domains, locale)} hint={t("backlinks.followShare", { share: followShare(summary) ?? "—" })} />
              <Stat label={t("backlinks.broken")} value={formatNumber(summary.broken_backlinks, locale)} hint={t("backlinks.brokenPages", { n: formatNumber(summary.broken_pages, locale) })} tone={summary.broken_backlinks > 0 ? "bad" : undefined} />
            </div>
            <div className="share-grid">
              <ShareBars title={t("backlinks.countries")} shares={summary.countries} label={country} />
              <ShareBars title={t("backlinks.tlds")} shares={summary.top_level_domains} label={(name) => `.${name}`} />
              <ShareBars title={t("backlinks.platforms")} shares={summary.platforms} />
              <ShareBars title={t("backlinks.linkTypes")} shares={[...summary.link_types, ...summary.attributes]} />
            </div>
            <p className="faint small">
              {t("backlinks.network", {
                ips: formatNumber(summary.referring_ips, locale),
                subnets: formatNumber(summary.referring_subnets, locale),
                spam: summary.spam_score ?? "—",
              })}
            </p>
          </div>
        )}
      </PaidFrame>
      <PaidFrame<NewLostPoint[]> module="backlinks" ask={{ ...ask, report: "new_lost" }} title={t("backlinks.newLostTitle")} hint={t("backlinks.newLostHint")}>
        {(points) => <NewLostChart points={points} />}
      </PaidFrame>
    </div>
  );
}

function linkState(row: BacklinkRow, t: (key: string) => string): string {
  if (row.is_broken) return t("backlinks.stateBroken");
  if (row.is_lost) return t("backlinks.stateLost");
  if (row.is_new) return t("backlinks.stateNew");
  return "";
}

function BacklinksTab({ projectId, target, fileName }: { projectId: number; target: string | undefined; fileName: string }) {
  const { t, locale } = useI18n();
  const [status, setStatus] = useState<LinkStatus>("live");
  const [dofollow, setDofollow] = useState(false);
  const [onePerDomain, setOnePerDomain] = useState(true);
  const [limit, setLimit] = useState(100);
  const [query, setQuery] = useState("");
  const columns: Column<BacklinkRow>[] = [
    {
      key: "from",
      label: t("backlinks.colFrom"),
      render: (row) => (
        <span className="link-cell">
          <strong>{row.title ?? row.domain_from}</strong>
          <span className="cell-url" title={row.url_from}>
            {row.url_from}
          </span>
        </span>
      ),
      sort: (row) => row.domain_from,
    },
    { key: "page_rank", label: t("backlinks.colPageRank"), render: (row) => <Authority rank={row.page_rank} />, sort: (row) => row.page_rank, numeric: true },
    { key: "domain_rank", label: t("backlinks.colDomainRank"), render: (row) => <Authority rank={row.domain_rank} />, sort: (row) => row.domain_rank, numeric: true },
    {
      key: "anchor",
      label: t("backlinks.colAnchor"),
      render: (row) => (
        <span>
          {row.anchor ?? <span className="faint">{t("backlinks.noAnchor")}</span>}
          <span className="faint small"> · {row.dofollow ? "dofollow" : "nofollow"}{row.kind && row.kind !== "anchor" ? ` · ${row.kind}` : ""}</span>
        </span>
      ),
      sort: (row) => row.anchor,
    },
    { key: "to", label: t("backlinks.colTo"), render: (row) => <span className="cell-url" title={row.url_to ?? ""}>{shortUrl(row.url_to)}{row.url_to_status && row.url_to_status >= 400 ? ` (${row.url_to_status})` : ""}</span>, sort: (row) => row.url_to },
    { key: "seen", label: t("backlinks.colFirstSeen"), render: (row) => (row.first_seen ?? "").slice(0, 10), sort: (row) => row.first_seen },
    { key: "state", label: "", render: (row) => linkState(row, t) && <span className={`oc-badge ${row.is_new ? "success" : "warning"} small-badge`}>{linkState(row, t)}</span> },
  ];
  return (
    <PaidFrame<BacklinkRow[]>
      module="backlinks"
      ask={{ project_id: projectId, report: "backlinks", status, dofollow_only: dofollow, one_per_domain: onePerDomain, ...(target ? { target } : {}) }}
      title={t("backlinks.listTitle")}
      hint={t("backlinks.listHint")}
      limit={limit}
      onLimit={setLimit}
      extra={
        <Segmented<LinkStatus>
          label={t("backlinks.status")}
          value={status}
          onChange={setStatus}
          options={(["live", "new", "lost", "broken"] as const).map((value) => ({ value, label: t(`backlinks.status_${value}`) }))}
        />
      }
    >
      {(rows) => {
        const needle = fold(query.trim());
        const shown = needle ? rows.filter((row) => fold(`${row.url_from} ${row.anchor ?? ""}`).includes(needle)) : rows;
        return (
          <div className="page-stack">
            <div className="filters">
              <label className="filter-search">
                <Search size={15} aria-hidden="true" />
                <span className="sr-only">{t("backlinks.filterSearch")}</span>
                <input className="oc-input" value={query} placeholder={t("backlinks.filterSearch")} onChange={(event) => setQuery(event.target.value)} />
              </label>
              <label className="oc-check">
                <input type="checkbox" checked={dofollow} onChange={(event) => setDofollow(event.target.checked)} />
                <span>{t("backlinks.dofollowOnly")}</span>
              </label>
              <label className="oc-check">
                <input type="checkbox" checked={onePerDomain} onChange={(event) => setOnePerDomain(event.target.checked)} />
                <span>{t("backlinks.onePerDomain")}</span>
              </label>
              <Button
                small
                disabled={shown.length === 0}
                onClick={() =>
                  download(
                    `${fileName}-backlinks.csv`,
                    toCsv(
                      [t("backlinks.colFrom"), t("backlinks.colPageRank"), t("backlinks.colDomainRank"), t("backlinks.colAnchor"), "dofollow", t("backlinks.colTo"), t("backlinks.colFirstSeen")],
                      shown.map((row) => [row.url_from, row.page_rank, row.domain_rank, row.anchor, row.dofollow ? "yes" : "no", row.url_to, row.first_seen]),
                    ),
                  )
                }
              >
                <Download size={14} />
                {t("overview.export")}
              </Button>
            </div>
            <DataTable rows={shown} columns={columns} rowKey={(row) => `${row.url_from}|${row.url_to ?? ""}|${row.anchor ?? ""}`} initialSort={{ key: "page_rank", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
          </div>
        );
      }}
    </PaidFrame>
  );
}

function DomainsTab({ projectId, target }: { projectId: number; target: string | undefined }) {
  const { t, locale } = useI18n();
  const [limit, setLimit] = useState(100);
  const columns: Column<ReferringDomainRow>[] = [
    { key: "domain", label: t("overview.colDomain"), render: (row) => <strong className="cell-main">{row.domain}</strong>, sort: (row) => row.domain },
    { key: "rank", label: "OA", render: (row) => <Authority rank={row.rank} />, sort: (row) => row.rank, numeric: true },
    { key: "backlinks", label: t("backlinks.backlinks"), render: (row) => formatNumber(row.backlinks, locale), sort: (row) => row.backlinks, numeric: true },
    { key: "follow", label: t("backlinks.colFollow"), render: (row) => (row.nofollow_only ? "nofollow" : "dofollow"), sort: (row) => (row.nofollow_only ? 0 : 1) },
    { key: "spam", label: t("backlinks.colSpam"), render: (row) => row.spam_score ?? "—", sort: (row) => row.spam_score, numeric: true },
    { key: "seen", label: t("backlinks.colFirstSeen"), render: (row) => (row.first_seen ?? "").slice(0, 10), sort: (row) => row.first_seen },
  ];
  return (
    <PaidFrame<ReferringDomainRow[]> module="backlinks" ask={{ project_id: projectId, report: "referring_domains", ...(target ? { target } : {}) }} title={t("backlinks.domainsTitle")} hint={t("backlinks.domainsHint")} limit={limit} onLimit={setLimit}>
      {(rows) => <DataTable rows={rows} columns={columns} rowKey={(row) => row.domain} initialSort={{ key: "rank", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />}
    </PaidFrame>
  );
}

function AnchorsTab({ projectId, target }: { projectId: number; target: string | undefined }) {
  const { t, locale } = useI18n();
  const [limit, setLimit] = useState(100);
  const columns: Column<AnchorRow>[] = [
    { key: "anchor", label: t("backlinks.colAnchor"), render: (row) => (row.anchor ? <strong className="cell-main">{row.anchor}</strong> : <span className="faint">{t("backlinks.noAnchor")}</span>), sort: (row) => row.anchor },
    { key: "domains", label: t("backlinks.referringDomains"), render: (row) => formatNumber(row.referring_domains, locale), sort: (row) => row.referring_domains, numeric: true },
    { key: "backlinks", label: t("backlinks.backlinks"), render: (row) => formatNumber(row.backlinks, locale), sort: (row) => row.backlinks, numeric: true },
    { key: "nofollow", label: t("backlinks.colNofollowDomains"), render: (row) => formatNumber(row.referring_domains_nofollow, locale), sort: (row) => row.referring_domains_nofollow, numeric: true },
  ];
  return (
    <PaidFrame<AnchorRow[]> module="backlinks" ask={{ project_id: projectId, report: "anchors", ...(target ? { target } : {}) }} title={t("backlinks.anchorsTitle")} hint={t("backlinks.anchorsHint")} limit={limit} onLimit={setLimit}>
      {(rows) => <DataTable rows={rows} columns={columns} rowKey={(row) => row.anchor || "(empty)"} initialSort={{ key: "domains", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />}
    </PaidFrame>
  );
}

function PagesTab({ projectId, target }: { projectId: number; target: string | undefined }) {
  const { t, locale } = useI18n();
  const [limit, setLimit] = useState(100);
  const columns: Column<LinkedPageRow>[] = [
    {
      key: "url",
      label: t("overview.colPage"),
      render: (row) => (
        <span className="link-cell">
          <strong>{row.title ?? shortUrl(row.url)}</strong>
          <span className="cell-url" title={row.url}>
            {shortUrl(row.url)}
            {row.status && row.status >= 400 ? ` (${row.status})` : ""}
          </span>
        </span>
      ),
      sort: (row) => row.url,
    },
    { key: "domains", label: t("backlinks.referringDomains"), render: (row) => formatNumber(row.referring_domains, locale), sort: (row) => row.referring_domains, numeric: true },
    { key: "backlinks", label: t("backlinks.backlinks"), render: (row) => formatNumber(row.backlinks, locale), sort: (row) => row.backlinks, numeric: true },
    { key: "rank", label: "OA", render: (row) => <Authority rank={row.rank} />, sort: (row) => row.rank, numeric: true },
    { key: "broken", label: t("backlinks.broken"), render: (row) => formatNumber(row.broken_backlinks, locale), sort: (row) => row.broken_backlinks, numeric: true },
  ];
  return (
    <PaidFrame<LinkedPageRow[]> module="backlinks" ask={{ project_id: projectId, report: "pages", ...(target ? { target } : {}) }} title={t("backlinks.pagesTitle")} hint={t("backlinks.pagesHint")} limit={limit} onLimit={setLimit}>
      {(rows) => <DataTable rows={rows} columns={columns} rowKey={(row) => row.url} initialSort={{ key: "domains", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />}
    </PaidFrame>
  );
}

function BulkTab({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const tokens = useChartTokens();
  const [input, setInput] = useState("");
  const [targets, setTargets] = useState<string[]>([]);
  const submit = (event: FormEvent) => {
    event.preventDefault();
    setTargets(
      input
        .split(/[\n,;\s]+/)
        .map((line) => line.trim())
        .filter(Boolean),
    );
  };
  const option = (rows: RankRow[]): ChartOption => {
    const axis = axisStyle(tokens);
    return {
      grid: { left: 150, right: 24, top: 8, bottom: 24 },
      tooltip: { trigger: "axis", axisPointer: { type: "shadow" } },
      xAxis: { type: "value", max: 100, ...axis },
      yAxis: { type: "category", inverse: true, data: rows.map((row) => row.target), ...axis, splitLine: { show: false } },
      series: [{ name: "OA", type: "bar", barMaxWidth: 20, data: rows.map((row) => row.rank ?? 0) }],
    };
  };
  return (
    <div className="page-stack">
      <form className="oc-card list-bar" onSubmit={submit}>
        <textarea className="oc-input bulk-input" rows={3} value={input} onChange={(event) => setInput(event.target.value)} placeholder={t("backlinks.bulkPlaceholder")} spellCheck={false} />
        <Button type="submit">{t("backlinks.bulkCompare")}</Button>
        <span className="faint small">{t("backlinks.bulkHint")}</span>
      </form>
      <PaidFrame<RankRow[]> module="backlinks" ask={{ project_id: project.id, report: "bulk", targets }} title={t("backlinks.bulkTitle")} hint={targets.length ? t("backlinks.bulkCount", { n: targets.length }) : t("backlinks.bulkDefault")}>
        {(rows) => (
          <div className="page-stack">
            <EChart option={option(rows)} height={Math.max(140, 40 + rows.length * 30)} />
            <p className="faint small">{rows.map((row) => `${row.target}: ${row.rank ?? "—"}`).join(" · ")}</p>
            <p className="faint small">{t("backlinks.authorityHint")}</p>
          </div>
        )}
      </PaidFrame>
    </div>
  );
}

export function Backlinks({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const [input, setInput] = useState("");
  const [target, setTarget] = useState<string | undefined>(undefined);
  const [tab, setTab] = useState<Tab>("summary");
  const shown = target ?? project.domain;
  const labels: Record<Tab, string> = {
    summary: t("backlinks.tabSummary"),
    backlinks: t("backlinks.tabBacklinks"),
    referring_domains: t("backlinks.tabDomains"),
    anchors: t("backlinks.tabAnchors"),
    pages: t("backlinks.tabPages"),
    bulk: t("backlinks.tabBulk"),
  };
  const submit = (event: FormEvent) => {
    event.preventDefault();
    const value = input.trim();
    setTarget(value && value !== project.domain ? value : undefined);
  };
  const report: Record<Tab, BacklinkReport> = { summary: "summary", backlinks: "backlinks", referring_domains: "referring_domains", anchors: "anchors", pages: "pages", bulk: "bulk" };
  return (
    <div className="page-stack">
      {tab !== "bulk" && (
        <form className="oc-card domain-bar" onSubmit={submit}>
          <label className="domain-input">
            <span className="sr-only">{t("backlinks.targetLabel")}</span>
            <Link2 size={16} aria-hidden="true" />
            <input className="oc-input" value={input} onChange={(event) => setInput(event.target.value)} placeholder={project.domain} spellCheck={false} autoComplete="off" list="backlink-targets" />
            <datalist id="backlink-targets">
              {[project.domain, ...project.competitors].map((value) => (
                <option key={value} value={value} />
              ))}
            </datalist>
          </label>
          <Button type="submit">{t("overview.analyze")}</Button>
          <span className="domain-bar-target">
            <strong>{shown}</strong>
            <small className="faint">{target ? t("backlinks.otherTarget") : t("overview.projectDomain")}</small>
          </span>
          {target && (
            <Button small className="ghost" onClick={() => { setInput(""); setTarget(undefined); }}>
              {t("overview.backToProject")}
            </Button>
          )}
        </form>
      )}
      <div className="oc-tabs" role="tablist" aria-label={t("backlinks.targetLabel")}>
        {TABS.map((entry) => (
          <button key={entry} type="button" role="tab" aria-selected={tab === entry} className={`oc-tab ${tab === entry ? "active" : ""}`} onClick={() => setTab(entry)}>
            {labels[entry]}
          </button>
        ))}
      </div>
      <div role="tabpanel" data-report={report[tab]}>
        {tab === "summary" && <SummaryTab key={shown} projectId={project.id} target={target} />}
        {tab === "backlinks" && <BacklinksTab key={shown} projectId={project.id} target={target} fileName={shown} />}
        {tab === "referring_domains" && <DomainsTab key={shown} projectId={project.id} target={target} />}
        {tab === "anchors" && <AnchorsTab key={shown} projectId={project.id} target={target} />}
        {tab === "pages" && <PagesTab key={shown} projectId={project.id} target={target} />}
        {tab === "bulk" && <BulkTab project={project} />}
      </div>
    </div>
  );
}
