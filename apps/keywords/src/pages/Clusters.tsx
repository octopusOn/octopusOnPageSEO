import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CircleCheck, CircleHelp, Download, Network, Scissors, Search, Undo2 } from "lucide-react";
import { api, errorText, type ExportFormat } from "../api/client";
import type {
  ClusterQuery,
  ClusterRecord,
  ClusterRequest,
  ClusterSort,
  Job,
  MemberRow,
  Method,
  ProjectRow,
  SerpEstimate,
  SerpRequest,
  Settings,
} from "../api/types";
import { EChart, type ChartOption } from "../components/EChart";
import { JobCard } from "../components/JobCard";
import { Button, Check, Empty, Field, Loading } from "../components/ui";
import { date, number, usd, useI18n, type Key } from "../i18n";
import { difficultyBand } from "../lib/format";

const INTENT_COLORS: Record<string, string> = {
  informational: "var(--tile-blue-1)",
  commercial: "var(--tile-violet-1)",
  transactional: "var(--tile-orange-1)",
  navigational: "var(--tile-teal-1)",
  local: "var(--tile-pink-1)",
};

function ClusterSettings({
  project,
  settings,
  notify,
}: {
  project: ProjectRow;
  settings: Settings | undefined;
  notify: (text: string, error?: boolean) => void;
}) {
  const { t, lang } = useI18n();
  const defaults = settings?.clustering;
  const [request, setRequest] = useState<ClusterRequest>({
    method: defaults?.method ?? "hybrid",
    threshold: defaults?.threshold ?? 0.6,
    serp_threshold: defaults?.serp_threshold ?? 3,
    mode: defaults?.mode ?? "soft",
    level: defaults?.level ?? "page",
    engine: null,
    device: "desktop",
  });
  const [serp, setSerp] = useState<SerpRequest>({ device: "desktop", top: null, per_cluster: 3, refresh: false });
  const [estimate, setEstimate] = useState<SerpEstimate | null>(null);
  const cluster = useMutation({
    mutationFn: () => api.cluster(project.id, request),
    onError: (error) => notify(errorText(error), true),
  });
  const estimating = useMutation({
    mutationFn: () => api.estimateSerps(project.id, serp),
    onSuccess: setEstimate,
    onError: (error) => notify(errorText(error), true),
  });
  const fetch = useMutation({
    mutationFn: async () => {
      const current = await api.estimateSerps(project.id, serp);
      setEstimate(current);
      if (current.keywords === 0) throw new Error(t("allFresh"));
      if (!current.provider) throw new Error(t("noSerpProvider"));
      if (current.cost_usd > (settings?.confirm_above_usd ?? 1) && !window.confirm(t("costConfirm", { usd: usd(current.cost_usd, lang) }))) return null;
      return api.fetchSerps(project.id, serp);
    },
    onError: (error) => notify(errorText(error), true),
  });
  const usesSerps = request.method !== "lexical";
  return (
    <div className="two-columns">
      <section className="oc-panel">
        <h2 className="oc-panel-title">
          <Network size={16} />
          {t("clusterTitle")}
        </h2>
        <div className="oc-segmented" role="radiogroup" aria-label={t("method")}>
          {(["lexical", "hybrid", "serp"] as Method[]).map((method) => (
            <button key={method} role="radio" aria-checked={request.method === method} className={request.method === method ? "active" : ""} onClick={() => setRequest({ ...request, method })}>
              {t(`method.${method}` as Key)}
            </button>
          ))}
        </div>
        <p className="faint small">{t(`methodHint.${request.method}` as Key)}</p>
        <div className="form-row">
          <Field label={t("wordingThreshold", { value: number(request.threshold, lang, 2) })} hint={t("wordingThresholdHint")}>
            <input type="range" min={0.3} max={0.9} step={0.05} value={request.threshold} onChange={(e) => setRequest({ ...request, threshold: Number(e.target.value) })} />
          </Field>
          {usesSerps && (
            <Field label={t("sharedThreshold", { value: request.serp_threshold })} hint={t("sharedThresholdHint")}>
              <input type="range" min={1} max={9} step={1} value={request.serp_threshold} onChange={(e) => setRequest({ ...request, serp_threshold: Number(e.target.value) })} />
            </Field>
          )}
        </div>
        {usesSerps && (
          <div className="row wrap">
            <select className="oc-select" value={request.mode} onChange={(e) => setRequest({ ...request, mode: e.target.value as ClusterRequest["mode"] })} aria-label={t("mode")}>
              <option value="soft">{t("modeSoft")}</option>
              <option value="hard">{t("modeHard")}</option>
            </select>
            <select className="oc-select" value={request.level} onChange={(e) => setRequest({ ...request, level: e.target.value as ClusterRequest["level"] })} aria-label={t("level")}>
              <option value="page">{t("levelPage")}</option>
              <option value="site">{t("levelSite")}</option>
            </select>
          </div>
        )}
        <div className="row end">
          <Button primary busy={cluster.isPending} onClick={() => cluster.mutate()}>
            {t("runClustering")}
          </Button>
        </div>
      </section>
      <section className="oc-panel">
        <h2 className="oc-panel-title">{t("serpTitle")}</h2>
        <p className="faint small">{t("serpLead")}</p>
        <div className="form-row">
          <Field label={t("perCluster")} hint={t("perClusterHint")}>
            <input className="oc-input" type="number" min={1} value={serp.per_cluster ?? ""} onChange={(e) => { setEstimate(null); setSerp({ ...serp, per_cluster: e.target.value ? Number(e.target.value) : null }); }} />
          </Field>
          <Field label={t("topKeywords")} hint={t("topKeywordsHint")}>
            <input className="oc-input" type="number" min={1} value={serp.top ?? ""} onChange={(e) => { setEstimate(null); setSerp({ ...serp, top: e.target.value ? Number(e.target.value) : null }); }} />
          </Field>
        </div>
        <Check checked={serp.refresh} onChange={(refresh) => { setEstimate(null); setSerp({ ...serp, refresh }); }}>
          {t("refreshSerps")}
        </Check>
        {estimate && (
          <p className={estimate.provider ? "faint" : "warning-text"}>
            {t("serpEstimate", {
              keywords: number(estimate.keywords, lang, 0),
              cached: number(estimate.cached, lang, 0),
              provider: estimate.provider ?? "–",
              usd: usd(estimate.cost_usd, lang),
            })}
            {!estimate.provider && ` · ${estimate.skipped.map(([id, reason]) => `${id}: ${reason}`).join("; ")}`}
          </p>
        )}
        {estimate?.provider === "browser" && settings && (
          <p className="faint small">
            {t("serpBrowserNote", {
              seconds: settings.browser.delay_secs,
              minutes: number(Math.ceil((estimate.keywords * settings.browser.delay_secs) / 60), lang, 0),
            })}
          </p>
        )}
        <div className="row end">
          <Button busy={estimating.isPending} onClick={() => estimating.mutate()}>
            {t("estimate")}
          </Button>
          <Button primary busy={fetch.isPending} onClick={() => fetch.mutate()}>
            {t("fetchSerps")}
          </Button>
        </div>
      </section>
    </div>
  );
}

function Treemap({ clusters, onSelect }: { clusters: ClusterRecord[]; onSelect: (number: number) => void }) {
  const { t, lang } = useI18n();
  const option: ChartOption = useMemo(() => {
    const style = getComputedStyle(document.documentElement);
    const color = (intent: string | null) => {
      const value = intent ? INTENT_COLORS[intent] ?? "" : "";
      const name = value.match(/var\((--[^)]+)\)/)?.[1];
      return name ? style.getPropertyValue(name).trim() : style.getPropertyValue("--tile-slate-1").trim();
    };
    return {
      tooltip: {
        formatter: (info: { data?: { name: string; value: number; size: number } }) =>
          info.data ? `${info.data.name}<br/>${t("volume")}: ${number(info.data.value, lang, 0)}<br/>${t("keywords")}: ${info.data.size}` : "",
      },
      series: [
        {
          type: "treemap",
          roam: false,
          nodeClick: false,
          breadcrumb: { show: false },
          label: { show: true, formatter: "{b}", fontSize: 12 },
          upperLabel: { show: false },
          itemStyle: { borderColor: "rgba(0,0,0,0.25)", borderWidth: 1, gapWidth: 2 },
          data: clusters.map((cluster) => ({
            name: cluster.label,
            value: Math.max(cluster.volume, cluster.size),
            size: cluster.size,
            number: cluster.number,
            itemStyle: { color: color(cluster.intent), opacity: cluster.verified ? 0.95 : 0.7 },
          })),
        },
      ],
    };
  }, [clusters, t, lang]);
  return (
    <div className="treemap">
      <EChart option={option} height={320} onClick={(data) => typeof data?.number === "number" && onSelect(data.number)} />
    </div>
  );
}

function Brief({ project, pillar }: { project: ProjectRow; pillar: MemberRow | undefined }) {
  const { t } = useI18n();
  const engine = project.engines[0] ?? "google";
  const serp = useQuery({
    queryKey: ["serp", pillar?.keyword_id, engine],
    queryFn: () => api.keywordSerp(pillar?.keyword_id ?? 0, engine),
    enabled: pillar !== undefined,
  });
  if (!pillar) return null;
  if (!serp.data)
    return <p className="faint small">{t("briefNeedsSerp")}</p>;
  return (
    <div className="brief">
      {serp.data.questions.length > 0 && (
        <>
          <h4>{t("briefQuestions")}</h4>
          <ul>
            {serp.data.questions.slice(0, 8).map((question) => (
              <li key={question}>{question}</li>
            ))}
          </ul>
        </>
      )}
      {serp.data.related.length > 0 && (
        <>
          <h4>{t("briefRelated")}</h4>
          <p className="faint">{serp.data.related.slice(0, 10).join(" · ")}</p>
        </>
      )}
      {serp.data.features.length > 0 && (
        <p className="faint small">
          {t("briefFeatures")}: {serp.data.features.map((feature) => t(`feature.${feature}` as Key)).join(", ")}
        </p>
      )}
      <h4>{t("briefTopPages")}</h4>
      <ol className="small">
        {serp.data.urls.slice(0, 5).map((url) => (
          <li key={url} className="clip">
            {url}
          </li>
        ))}
      </ol>
    </div>
  );
}

function Detail({
  project,
  clusteringId,
  cluster,
  all,
  notify,
}: {
  project: ProjectRow;
  clusteringId: number;
  cluster: ClusterRecord;
  all: ClusterRecord[];
  notify: (text: string, error?: boolean) => void;
}) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const members = useQuery({
    queryKey: ["members", clusteringId, cluster.number],
    queryFn: () => api.members(clusteringId, cluster.number),
  });
  const [chosen, setChosen] = useState<Set<number>>(new Set());
  const [target, setTarget] = useState("");
  useEffect(() => setChosen(new Set()), [cluster.number, clusteringId]);
  const done = () => {
    setChosen(new Set());
    void queryClient.invalidateQueries({ queryKey: ["clusters"] });
    void queryClient.invalidateQueries({ queryKey: ["members"] });
    void queryClient.invalidateQueries({ queryKey: ["clusterings", project.id] });
    void queryClient.invalidateQueries({ queryKey: ["keywords", project.id] });
  };
  const place = useMutation({
    mutationFn: ({ ids, pillar }: { ids: number[]; pillar: number }) => api.place(project.id, ids, pillar),
    onSuccess: () => {
      notify(t("placed"));
      done();
    },
    onError: (error) => notify(errorText(error), true),
  });
  const release = useMutation({
    mutationFn: (ids: number[]) => api.unplace(project.id, ids),
    onSuccess: () => {
      notify(t("released"));
      done();
    },
    onError: (error) => notify(errorText(error), true),
  });
  const ids = [...chosen];
  const pillar = members.data?.find((member) => member.pillar);
  const others = all.filter((other) => other.number !== cluster.number);
  return (
    <aside className="oc-panel cluster-detail">
      <header>
        <span className="type-label">
          {t("cluster")} {cluster.number + 1}
        </span>
        <h2>{cluster.label}</h2>
        <div className="row wrap">
          {cluster.intent && <span className={`intent ${cluster.intent}`}>{t(`intent.${cluster.intent}` as Key)}</span>}
          {cluster.mixed_intent && <span className="oc-badge warning">{t("mixedIntent")}</span>}
          {cluster.verified ? (
            <span className="oc-badge good">
              <CircleCheck size={12} /> {t("verified")}
            </span>
          ) : (
            <span className="oc-badge">
              <CircleHelp size={12} /> {t("unverified")}
            </span>
          )}
          {cluster.gap && <span className="oc-badge critical">{t("gap")}</span>}
        </div>
      </header>
      <div className="oc-stats compact">
        <div className="oc-stat">
          <span className="oc-stat-label">{t("volume")}</span>
          <span className="oc-stat-value">{number(cluster.volume, lang, 0)}</span>
        </div>
        <div className="oc-stat">
          <span className="oc-stat-label">{t("keywords")}</span>
          <span className="oc-stat-value">{cluster.size}</span>
        </div>
        <div className="oc-stat">
          <span className="oc-stat-label">{t("difficultyShort")}</span>
          <span className="oc-stat-value">{cluster.difficulty === null ? "–" : number(cluster.difficulty, lang, 0)}</span>
        </div>
        <div className="oc-stat">
          <span className="oc-stat-label">{t("cpc")}</span>
          <span className="oc-stat-value">{cluster.cpc_usd === null ? "–" : usd(cluster.cpc_usd, lang)}</span>
        </div>
      </div>
      {cluster.target_url && (
        <p className="small">
          {t("targetPage")}: <span className="mono">{cluster.target_url}</span>
        </p>
      )}
      {cluster.competing.length > 0 && (
        <p className="small warning-text">
          {t("competingPages")}: {cluster.competing.join(", ")}
        </p>
      )}
      {cluster.competitors.length > 0 && (
        <p className="small faint">
          {t("topCompetitors")}: {cluster.competitors.map((competitor) => `${competitor.domain} (${competitor.keywords})`).join(", ")}
        </p>
      )}
      {members.isLoading ? (
        <Loading />
      ) : (
        <div className="oc-table-wrap members">
          <table className="oc-table">
            <thead>
              <tr>
                <th />
                <th>{t("keyword")}</th>
                <th className="num">{t("volume")}</th>
                <th className="num">{t("similarity")}</th>
              </tr>
            </thead>
            <tbody>
              {(members.data ?? []).map((member) => (
                <tr key={member.keyword_id} className={member.pillar ? "pillar" : ""}>
                  <td>
                    <input
                      type="checkbox"
                      checked={chosen.has(member.keyword_id)}
                      aria-label={member.keyword}
                      onChange={() =>
                        setChosen((old) => {
                          const next = new Set(old);
                          if (next.has(member.keyword_id)) next.delete(member.keyword_id);
                          else next.add(member.keyword_id);
                          return next;
                        })
                      }
                    />
                  </td>
                  <td>
                    {member.keyword}
                    {member.pinned && <span className="oc-badge">{t("placedByHand")}</span>}
                  </td>
                  <td className="num">{member.volume === null ? "–" : number(member.volume, lang, 0)}</td>
                  <td className="num">
                    {member.pillar ? t("pillar") : member.shared !== null ? t("sharedCount", { count: member.shared }) : number(member.score, lang, 2)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {ids.length > 0 && (
        <div className="selection-bar">
          <select className="oc-select" value={target} onChange={(e) => setTarget(e.target.value)} aria-label={t("moveTo")}>
            <option value="">{t("moveTo")}…</option>
            {others.slice(0, 300).map((other) => (
              <option key={other.number} value={other.pillar_id}>
                {other.number + 1}. {other.label}
              </option>
            ))}
          </select>
          <Button small disabled={!target} busy={place.isPending} onClick={() => place.mutate({ ids, pillar: Number(target) })}>
            {t("move")}
          </Button>
          <Button small busy={place.isPending} onClick={() => place.mutate({ ids, pillar: ids[0] ?? 0 })}>
            <Scissors size={14} />
            {t("split")}
          </Button>
          <Button small className="ghost" busy={release.isPending} onClick={() => release.mutate(ids)}>
            <Undo2 size={14} />
            {t("release")}
          </Button>
        </div>
      )}
      <section>
        <h3>{t("brief")}</h3>
        <Brief project={project} pillar={pillar} />
        <h4>{t("briefKeywords")}</h4>
        <p className="faint small">{(members.data ?? []).map((member) => member.keyword).join(" · ")}</p>
      </section>
    </aside>
  );
}

export function Clusters({
  project,
  settings,
  jobs,
  notify,
  onKeywords,
}: {
  project: ProjectRow;
  settings: Settings | undefined;
  jobs: Job[];
  notify: (text: string, error?: boolean) => void;
  onKeywords: (cluster: number) => void;
}) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const clusterings = useQuery({ queryKey: ["clusterings", project.id], queryFn: () => api.clusterings(project.id) });
  const [chosenClustering, setChosenClustering] = useState<number | null>(null);
  const clustering = clusterings.data?.find((row) => row.id === chosenClustering) ?? clusterings.data?.[0];
  const [search, setSearch] = useState("");
  const [intent, setIntent] = useState("");
  const [only, setOnly] = useState<"" | "verified" | "gap" | "multi">("");
  const [sort, setSort] = useState<ClusterSort>("volume");
  const [selected, setSelected] = useState<number | null>(null);
  const [dismissed, setDismissed] = useState<string[]>([]);
  const query: ClusterQuery = {
    search: search.trim() || null,
    intent: intent || null,
    verified: only === "verified" ? true : null,
    gap: only === "gap" ? true : null,
    min_size: only === "multi" ? 2 : null,
    sort,
    descending: sort !== "label" && sort !== "number",
    offset: 0,
    limit: 500,
  };
  const clusters = useQuery({
    queryKey: ["clusters", clustering?.id, query],
    queryFn: () => api.clusters(clustering?.id ?? 0, query),
    enabled: clustering !== undefined,
  });
  const own = jobs.filter((job) => job.project_id === project.id && (job.kind === "cluster" || job.kind === "serp") && !dismissed.includes(job.id));
  const finished = own.filter((job) => job.status !== "running").map((job) => job.id).join(",");
  useEffect(() => {
    if (finished) {
      void queryClient.invalidateQueries({ queryKey: ["clusterings", project.id] });
      void queryClient.invalidateQueries({ queryKey: ["clusters"] });
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
      setChosenClustering(null);
    }
  }, [finished, project.id, queryClient]);
  const exportClusters = useMutation({
    mutationFn: (format: ExportFormat) => api.exportClusters(project.id, clustering?.id ?? null, `${project.name}-clusters`, format),
    onSuccess: (written) => written !== null && notify(t("exported", { count: written })),
    onError: (error) => notify(errorText(error), true),
  });
  const rows = clusters.data?.rows ?? [];
  const current = rows.find((row) => row.number === selected) ?? rows[0];
  return (
    <div className="stack">
      {own.slice(0, 2).map((job) => (
        <JobCard key={job.id} job={job} onStop={(id) => void api.cancel(id)} onDismiss={() => setDismissed((old) => [...old, job.id])} />
      ))}
      <ClusterSettings project={project} settings={settings} notify={notify} />
      {clusterings.isLoading ? (
        <Loading />
      ) : !clustering ? (
        <Empty icon={<Network size={28} />}>
          <b>{t("noClusterings")}</b>
          <p>{t("noClusteringsLead")}</p>
        </Empty>
      ) : (
        <>
          <div className="toolbar">
            <select className="oc-select" value={clustering.id} onChange={(e) => setChosenClustering(Number(e.target.value))} aria-label={t("clustering")}>
              {(clusterings.data ?? []).map((row) => (
                <option key={row.id} value={row.id}>
                  {date(row.created_at, lang)} · {t(`method.${row.method}` as Key)} · {t("clusterCount", { count: row.clusters })}
                </option>
              ))}
            </select>
            <label className="oc-search">
              <Search size={15} />
              <input value={search} onChange={(e) => setSearch(e.target.value)} placeholder={t("searchClusters")} aria-label={t("searchClusters")} />
            </label>
            <select className="oc-select" value={intent} onChange={(e) => setIntent(e.target.value)} aria-label={t("intent")}>
              <option value="">{t("anyIntent")}</option>
              {["informational", "commercial", "transactional", "navigational", "local"].map((value) => (
                <option key={value} value={value}>
                  {t(`intent.${value}` as Key)}
                </option>
              ))}
            </select>
            <select className="oc-select" value={only} onChange={(e) => setOnly(e.target.value as typeof only)} aria-label={t("show")}>
              <option value="">{t("allClusters")}</option>
              <option value="multi">{t("multiOnly")}</option>
              <option value="verified">{t("verifiedOnly")}</option>
              <option value="gap">{t("gapsOnly")}</option>
            </select>
            <select className="oc-select" value={sort} onChange={(e) => setSort(e.target.value as ClusterSort)} aria-label={t("sort")}>
              <option value="volume">{t("byVolume")}</option>
              <option value="size">{t("bySize")}</option>
              <option value="difficulty">{t("byDifficulty")}</option>
              <option value="label">{t("byLabel")}</option>
            </select>
            <span className="toolbar-spacer" />
            <span className="faint">{t("clusterCount", { count: number(clusters.data?.total ?? 0, lang, 0) })}</span>
            <select className="oc-select" value="" aria-label={t("export")} onChange={(e) => e.target.value && exportClusters.mutate(e.target.value as ExportFormat)}>
              <option value="">{t("export")}…</option>
              <option value="xlsx">Excel</option>
              <option value="csv">CSV</option>
              <option value="json">JSON</option>
            </select>
          </div>
          {rows.length > 0 && <Treemap clusters={rows.slice(0, 150)} onSelect={setSelected} />}
          <div className="clusters-layout">
            <div className="oc-table-wrap cluster-list">
              <table className="oc-table">
                <thead>
                  <tr>
                    <th className="num">#</th>
                    <th>{t("pillar")}</th>
                    <th className="num">{t("keywords")}</th>
                    <th className="num">{t("volume")}</th>
                    <th>{t("intent")}</th>
                    <th className="num">{t("difficultyShort")}</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {rows.map((row) => (
                    <tr key={row.number} className={current?.number === row.number ? "selected" : ""} onClick={() => setSelected(row.number)}>
                      <td className="num faint">{row.number + 1}</td>
                      <td>
                        <button className="link" onClick={() => setSelected(row.number)}>
                          {row.label}
                        </button>
                      </td>
                      <td className="num">{row.size}</td>
                      <td className="num">{number(row.volume, lang, 0)}</td>
                      <td>{row.intent && <span className={`intent ${row.intent}`}>{t(`intent.${row.intent}` as Key)}</span>}</td>
                      <td className="num">{row.difficulty === null ? "" : <span className={`kd ${difficultyBand(row.difficulty)}`}>{number(row.difficulty, lang, 0)}</span>}</td>
                      <td>
                        {row.verified && <CircleCheck size={13} className="good" aria-label={t("verified")} />}
                        <button className="link small" onClick={(e) => { e.stopPropagation(); onKeywords(row.number); }} title={t("showInKeywords")}>
                          <Download size={12} style={{ transform: "rotate(-90deg)" }} />
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            {current && <Detail project={project} clusteringId={clustering.id} cluster={current} all={rows} notify={notify} />}
          </div>
        </>
      )}
    </div>
  );
}
