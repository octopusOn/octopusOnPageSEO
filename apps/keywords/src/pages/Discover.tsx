import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FileUp, Gem, Plus, Sparkles } from "lucide-react";
import { api, errorText } from "../api/client";
import type {
  DiscoverEstimate,
  DiscoverRequest,
  DiscoverySource,
  EnrichEstimate,
  EnrichRequest,
  Enrichment,
  Expansion,
  ImportPreview,
  Job,
  ProjectRow,
  Settings,
} from "../api/types";
import { JobCard } from "../components/JobCard";
import { Button, Check, Field, Modal } from "../components/ui";
import { number, usd, useI18n, type Key } from "../i18n";
import { lines, roleKey } from "../lib/format";

const GROUPS: { title: Key; sources: DiscoverySource[] }[] = [
  { title: "groupPaid", sources: ["wordstat", "related", "suggestions", "labs-ideas", "ads-ideas", "competitors", "autocomplete", "serpapi-autocomplete"] },
  { title: "groupGoogle", sources: ["google-ads"] },
  { title: "groupFree", sources: ["google-suggest", "yandex-suggest"] },
];
const EXPANSIONS: Expansion[] = ["questions", "commercial", "alphabet", "turkish-letters"];
const AUTOCOMPLETE: DiscoverySource[] = ["autocomplete", "serpapi-autocomplete", "google-suggest", "yandex-suggest"];

function ImportModal({
  project,
  path,
  preview,
  onClose,
  onDone,
}: {
  project: ProjectRow;
  path: string;
  preview: ImportPreview;
  onClose: () => void;
  onDone: (text: string, error?: boolean) => void;
}) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const run = useMutation({
    mutationFn: () => api.importFile(project.id, path),
    onSuccess: (outcome) => {
      void queryClient.invalidateQueries();
      onDone(
        t("importDone", {
          added: number(outcome.added, lang, 0),
          known: number(outcome.known, lang, 0),
          metrics: number(outcome.metrics, lang, 0),
        }),
      );
    },
    onError: (error) => onDone(errorText(error), true),
  });
  const read = preview.columns.filter((column) => column.role !== null);
  return (
    <Modal
      title={t("importTitle")}
      className="wide"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("cancel")}</Button>
          <Button primary busy={run.isPending} onClick={() => run.mutate()}>
            {t("importRows", { count: number(preview.total, lang, 0) })}
          </Button>
        </>
      }
    >
      <p className="faint">{path.split(/[\\/]/).pop()}</p>
      <p>{t("importColumns", { columns: read.map((column) => `${column.header || "#1"} → ${t(roleKey(column.role))}`).join(", ") })}</p>
      <div className="oc-table-wrap preview-table">
        <table className="oc-table">
          <thead>
            <tr>
              {preview.columns.map((column, index) => (
                <th key={index} className={column.role ? "" : "faint"}>
                  {column.header || `#${index + 1}`}
                  <small>{column.role ? t(roleKey(column.role)) : t("leftOut")}</small>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {preview.rows.map((row, index) => (
              <tr key={index}>
                {preview.columns.map((_, column) => (
                  <td key={column}>{row[column] ?? ""}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Modal>
  );
}

export function Discover({
  project,
  settings,
  jobs,
  notify,
}: {
  project: ProjectRow;
  settings: Settings | undefined;
  jobs: Job[];
  notify: (text: string, error?: boolean) => void;
}) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const [manual, setManual] = useState("");
  const [importing, setImporting] = useState<{ path: string; preview: ImportPreview } | null>(null);
  const [seeds, setSeeds] = useState("");
  const [sources, setSources] = useState<DiscoverySource[]>(["related", "suggestions"]);
  const [expansions, setExpansions] = useState<Expansion[]>(["questions", "commercial"]);
  const [competitors, setCompetitors] = useState(project.competitors.join("\n"));
  const [limit, setLimit] = useState(500);
  const [include, setInclude] = useState("");
  const [exclude, setExclude] = useState("");
  const [minVolume, setMinVolume] = useState("");
  const [maxWords, setMaxWords] = useState("");
  const [estimate, setEstimate] = useState<DiscoverEstimate | null>(null);
  const [asks, setAsks] = useState<Enrichment[]>(["volume"]);
  const [volumeSource, setVolumeSource] = useState<EnrichRequest["volume_source"]>("dataforseo");
  const [onlyMissing, setOnlyMissing] = useState(true);
  const [enrichEstimate, setEnrichEstimate] = useState<EnrichEstimate | null>(null);
  const [dismissed, setDismissed] = useState<string[]>([]);

  const request: DiscoverRequest = useMemo(
    () => ({
      seeds: lines(seeds),
      competitors: lines(competitors),
      sources,
      expansions: sources.some((source) => AUTOCOMPLETE.includes(source)) ? expansions : [],
      limit,
      filter: {
        include: lines(include),
        exclude: lines(exclude),
        min_volume: minVolume ? Number(minVolume) : null,
        max_words: maxWords ? Number(maxWords) : null,
      },
    }),
    [seeds, competitors, sources, expansions, limit, include, exclude, minVolume, maxWords],
  );
  const enrichRequest: EnrichRequest = { asks, volume_source: volumeSource, only_missing: onlyMissing };
  const google = useQuery({ queryKey: ["google"], queryFn: api.googleStatus });

  const add = useMutation({
    mutationFn: () => api.addKeywords(project.id, lines(manual)),
    onSuccess: (outcome) => {
      setManual("");
      void queryClient.invalidateQueries();
      notify(t("addDone", { added: outcome.added, known: outcome.known, rejected: outcome.rejected }));
    },
    onError: (error) => notify(errorText(error), true),
  });
  const pick = useMutation({
    mutationFn: async () => {
      const path = await api.pickImport();
      if (!path) return null;
      return { path, preview: await api.previewImport(path) };
    },
    onSuccess: (picked) => picked && setImporting(picked),
    onError: (error) => notify(errorText(error), true),
  });
  const estimating = useMutation({
    mutationFn: () => api.estimateDiscovery(project.id, request),
    onSuccess: setEstimate,
    onError: (error) => notify(errorText(error), true),
  });
  const start = useMutation({
    mutationFn: async () => {
      const current = await api.estimateDiscovery(project.id, request);
      setEstimate(current);
      if (current.requests === 0) throw new Error(t("nothingReady"));
      const limitUsd = settings?.confirm_above_usd ?? 1;
      if (current.cost_usd > limitUsd && !window.confirm(t("costConfirm", { usd: usd(current.cost_usd, lang) }))) return null;
      return api.discover(project.id, request);
    },
    onError: (error) => notify(errorText(error), true),
  });
  const estimateEnrich = useMutation({
    mutationFn: () => api.estimateEnrichment(project.id, enrichRequest),
    onSuccess: setEnrichEstimate,
    onError: (error) => notify(errorText(error), true),
  });
  const enrich = useMutation({
    mutationFn: async () => {
      const current = await api.estimateEnrichment(project.id, enrichRequest);
      setEnrichEstimate(current);
      if (current.reason) throw new Error(current.reason);
      if (current.requests === 0) throw new Error(t("nothingMissing"));
      const limitUsd = settings?.confirm_above_usd ?? 1;
      if (current.cost_usd > limitUsd && !window.confirm(t("costConfirm", { usd: usd(current.cost_usd, lang) }))) return null;
      return api.enrich(project.id, enrichRequest);
    },
    onError: (error) => notify(errorText(error), true),
  });

  const own = jobs.filter(
    (job) => job.project_id === project.id && ["discover", "enrich", "search-console"].includes(job.kind) && !dismissed.includes(job.id),
  );
  const toggleSource = (source: DiscoverySource, on: boolean) => {
    setEstimate(null);
    setSources((old) => (on ? [...old, source] : old.filter((item) => item !== source)));
  };
  return (
    <div className="stack">
      {own.slice(0, 3).map((job) => (
        <JobCard key={job.id} job={job} onStop={(id) => void api.cancel(id)} onDismiss={() => setDismissed((old) => [...old, job.id])} />
      ))}
      <div className="two-columns">
        <section className="oc-panel">
          <h2 className="oc-panel-title">
            <Plus size={16} />
            {t("addManually")}
          </h2>
          <textarea className="oc-textarea" rows={5} value={manual} onChange={(e) => setManual(e.target.value)} placeholder={t("manualPlaceholder")} />
          <div className="row end">
            <Button primary busy={add.isPending} disabled={!manual.trim()} onClick={() => add.mutate()}>
              {t("add")}
            </Button>
          </div>
        </section>
        <section className="oc-panel">
          <h2 className="oc-panel-title">
            <FileUp size={16} />
            {t("importFile")}
          </h2>
          <p className="faint">{t("importLead")}</p>
          <div className="row end">
            <Button busy={pick.isPending} onClick={() => pick.mutate()}>
              {t("chooseFile")}
            </Button>
          </div>
        </section>
      </div>

      <section className="oc-panel">
        <h2 className="oc-panel-title">
          <Gem size={16} />
          {t("discoverTitle")}
        </h2>
        <div className="form-grid">
          <Field label={t("seeds")} hint={t("seedsHint")}>
            <textarea className="oc-textarea" rows={4} value={seeds} onChange={(e) => { setSeeds(e.target.value); setEstimate(null); }} placeholder={t("seedsPlaceholder")} />
          </Field>
          {GROUPS.map((group) => (
            <fieldset key={group.title} className="source-group">
              <legend>{t(group.title)}</legend>
              <div className="source-grid">
                {group.sources.map((source) => (
                  <Check key={source} checked={sources.includes(source)} onChange={(on) => toggleSource(source, on)}>
                    <b>{t(`source.${source}` as Key)}</b>
                    <small className="faint">{t(`sourceHint.${source}` as Key)}</small>
                  </Check>
                ))}
              </div>
            </fieldset>
          ))}
          {(sources.includes("google-suggest") || sources.includes("yandex-suggest")) && (
            <div className="oc-banner warning">{t("freeSuggestWarning")}</div>
          )}
          {sources.some((source) => AUTOCOMPLETE.includes(source)) && (
            <Field label={t("expansions")} hint={t("expansionsHint")}>
              <div className="oc-chips">
                {EXPANSIONS.map((expansion) => (
                  <button
                    key={expansion}
                    className={`oc-chip ${expansions.includes(expansion) ? "active" : ""}`}
                    aria-pressed={expansions.includes(expansion)}
                    onClick={() => {
                      setEstimate(null);
                      setExpansions((old) => (old.includes(expansion) ? old.filter((item) => item !== expansion) : [...old, expansion]));
                    }}
                  >
                    {t(`expansion.${expansion}` as Key)}
                  </button>
                ))}
              </div>
            </Field>
          )}
          {sources.includes("competitors") && (
            <Field label={t("competitors")} hint={t("competitorsDiscoverHint")}>
              <textarea className="oc-textarea" rows={2} value={competitors} onChange={(e) => setCompetitors(e.target.value)} />
            </Field>
          )}
          <div className="form-row four">
            <Field label={t("resultsPerRequest")}>
              <input className="oc-input" type="number" min={1} max={2000} value={limit} onChange={(e) => setLimit(Math.max(1, Number(e.target.value) || 1))} />
            </Field>
            <Field label={t("minVolume")}>
              <input className="oc-input" type="number" min={0} value={minVolume} onChange={(e) => setMinVolume(e.target.value)} />
            </Field>
            <Field label={t("maxWords")}>
              <input className="oc-input" type="number" min={1} value={maxWords} onChange={(e) => setMaxWords(e.target.value)} />
            </Field>
          </div>
          <div className="form-row">
            <Field label={t("includeWords")} hint={t("includeHint")}>
              <textarea className="oc-textarea" rows={2} value={include} onChange={(e) => setInclude(e.target.value)} />
            </Field>
            <Field label={t("excludeWords")} hint={t("excludeHint")}>
              <textarea className="oc-textarea" rows={2} value={exclude} onChange={(e) => setExclude(e.target.value)} />
            </Field>
          </div>
        </div>
        {estimate && (
          <div className="oc-table-wrap">
            <table className="oc-table">
              <thead>
                <tr>
                  <th>{t("source")}</th>
                  <th className="num">{t("requests")}</th>
                  <th className="num">{t("cost")}</th>
                  <th>{t("state")}</th>
                </tr>
              </thead>
              <tbody>
                {estimate.sources.map((row) => (
                  <tr key={row.source}>
                    <td>{t(`source.${row.source}` as Key)}</td>
                    <td className="num">{number(row.requests, lang, 0)}</td>
                    <td className="num">{usd(row.cost_usd, lang)}</td>
                    <td className={row.ready ? "good" : "faint"}>{row.ready ? t("ready") : row.reason}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            <p className="faint">
              {t("estimateTotal", { requests: number(estimate.requests, lang, 0), usd: usd(estimate.cost_usd, lang) })}
              {estimate.wordstat_hours > 1 && ` · ${t("wordstatHours", { hours: number(estimate.wordstat_hours, lang) })}`}
            </p>
          </div>
        )}
        <div className="row end">
          <Button busy={estimating.isPending} disabled={sources.length === 0} onClick={() => estimating.mutate()}>
            {t("estimate")}
          </Button>
          <Button primary busy={start.isPending} disabled={sources.length === 0 || (request.seeds.length === 0 && !sources.includes("competitors"))} onClick={() => start.mutate()}>
            <Sparkles size={15} />
            {t("startDiscovery")}
          </Button>
        </div>
      </section>

      <section className="oc-panel">
        <h2 className="oc-panel-title">{t("enrichTitle")}</h2>
        <p className="faint">{t("enrichLead")}</p>
        <div className="row wrap">
          {(["volume", "difficulty", "intent"] as Enrichment[]).map((ask) => (
            <Check key={ask} checked={asks.includes(ask)} onChange={(on) => { setEnrichEstimate(null); setAsks((old) => (on ? [...old, ask] : old.filter((item) => item !== ask))); }}>
              {t(`enrich.${ask}` as Key)}
            </Check>
          ))}
          <select className="oc-select" value={volumeSource} onChange={(e) => { setEnrichEstimate(null); setVolumeSource(e.target.value as EnrichRequest["volume_source"]); }} aria-label={t("volumeSource")}>
            <option value="dataforseo">{t("volumeFromDataforseo")}</option>
            <option value="google-ads" disabled={!google.data?.connected}>
              {t("volumeFromAds")}
            </option>
          </select>
          <Check checked={onlyMissing} onChange={(on) => { setEnrichEstimate(null); setOnlyMissing(on); }}>
            {t("onlyMissing")}
          </Check>
        </div>
        {enrichEstimate && (
          <p className={enrichEstimate.reason ? "warning-text" : "faint"}>
            {t("enrichEstimate", {
              volume: enrichEstimate.keywords[0],
              difficulty: enrichEstimate.keywords[1],
              intent: enrichEstimate.keywords[2],
              requests: enrichEstimate.requests,
              usd: usd(enrichEstimate.cost_usd, lang),
            })}
            {enrichEstimate.reason && ` · ${enrichEstimate.reason}`}
          </p>
        )}
        <div className="row end">
          <Button busy={estimateEnrich.isPending} disabled={asks.length === 0} onClick={() => estimateEnrich.mutate()}>
            {t("estimate")}
          </Button>
          <Button primary busy={enrich.isPending} disabled={asks.length === 0} onClick={() => enrich.mutate()}>
            {t("startEnrich")}
          </Button>
        </div>
      </section>
      {importing && (
        <ImportModal
          project={project}
          path={importing.path}
          preview={importing.preview}
          onClose={() => setImporting(null)}
          onDone={(text, error) => {
            setImporting(null);
            notify(text, error);
          }}
        />
      )}
    </div>
  );
}
