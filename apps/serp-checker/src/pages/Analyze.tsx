import { useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronDown, ChevronRight, ExternalLink, RefreshCw, Search, Sparkles } from "lucide-react";
import { api, errorText } from "../api/client";
import {
  engines,
  type AnalysisSummary,
  type AnalysisView,
  type AnalyzeRequest,
  type Device,
  type DomainFacts,
  type Job,
  type PageScore,
  type Settings,
} from "../api/types";
import { BreakdownList, ScorePill, ScoreRing } from "../components/Score";
import { JobCard } from "../components/JobCard";
import { ExportMenu } from "./Tracking";
import { Button, Empty, ErrorState, Loading, Modal } from "../components/ui";
import { date, hasKey, number, percent, persist, stored, usd, useI18n, type Key } from "../i18n";
import { requestOf, type Form } from "../lib/analyzeRequest";
import { bandKey, difficultyTone, sameSite, tone } from "../lib/score";

const initialForm = (settings?: Settings): Form => ({
  keyword: "",
  engine: "google",
  country: settings?.defaults.country ?? "tr",
  language: settings?.defaults.language ?? "tr",
  device: settings?.defaults.device ?? "desktop",
  target: "",
  targetUrl: "",
  pages: 10,
  volume: true,
  backlinks: false,
  javascript: false,
  ...stored<Partial<Form>>("serp.analyzeForm", {}),
});

export function Analyze({
  jobs,
  settings,
  selected,
  onSelect,
  notify,
}: {
  jobs: Job[];
  settings?: Settings;
  selected: number | null;
  onSelect: (id: number | null) => void;
  notify: (text: string, error?: boolean) => void;
}) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const [form, setForm] = useState<Form>(() => initialForm(settings));
  const [confirm, setConfirm] = useState<{ usd: number; provider: string } | null>(null);
  const [problem, setProblem] = useState("");
  const set = <K extends keyof Form>(key: K, value: Form[K]) => setForm((old) => ({ ...old, [key]: value }));
  useEffect(() => {
    const { keyword: _keyword, targetUrl: _targetUrl, ...kept } = form;
    persist("serp.analyzeForm", kept);
  }, [form]);

  const analyses = useQuery({ queryKey: ["analyses"], queryFn: () => api.analyses() });
  const current = selected ?? analyses.data?.[0]?.id ?? null;
  const view = useQuery({
    queryKey: ["analysis", current],
    queryFn: () => api.analysis(current as number),
    enabled: current !== null,
  });
  const estimate = useQuery({
    queryKey: ["estimate", form.engine, form.country, form.language, form.device],
    queryFn: () =>
      api.estimate({
        query: { engine: form.engine, keyword: "x", country: form.country || "tr", language: form.language || "tr", device: form.device, depth: 10 },
      }),
    retry: false,
  });
  const firstReady = estimate.data?.find((item) => item.ready);
  const start = useMutation({
    mutationFn: (request: AnalyzeRequest) => api.analyze(request),
    onSuccess: () => {
      setConfirm(null);
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
    },
    onError: (error) => notify(errorText(error), true),
  });

  const submit = () => {
    setProblem("");
    if (!form.keyword.trim()) return;
    if (estimate.data && !firstReady) {
      setProblem(t("noProvider", { engine: t(form.engine) }));
      return;
    }
    const cost = firstReady?.estimate.usd ?? 0;
    if (firstReady && settings && cost > settings.confirm_above_usd) {
      setConfirm({ usd: cost, provider: firstReady.provider });
      return;
    }
    start.mutate(requestOf(form));
  };

  const active = jobs.filter((job) => job.kind === "analysis");
  // Running analyses, and failures until they are dismissed; finished ones open instead.
  const shown = active.filter((job) => job.status !== "done" && job.status !== "cancelled").slice(0, 3);
  // Open an analysis when a job seen running finishes.
  const seen = useRef(new Set<string>());
  useEffect(() => {
    for (const job of active) {
      if (job.status === "running" || job.status === "challenge") seen.current.add(job.id);
      else if (seen.current.delete(job.id) && job.status === "done" && job.result !== undefined) {
        void queryClient.invalidateQueries({ queryKey: ["analyses"] });
        onSelect(job.result);
        notify(t("jobDone"));
      }
    }
  }, [active, queryClient, onSelect, notify, t]);

  return (
    <div className="analyze">
      <section className="oc-card analyze-form">
        <form
          onSubmit={(event) => {
            event.preventDefault();
            submit();
          }}
        >
          <div className="keyword-row">
            <label className="keyword-input">
              <Search size={18} />
              <input
                aria-label={t("keyword")}
                value={form.keyword}
                placeholder={t("keywordPlaceholder")}
                onChange={(event) => set("keyword", event.target.value)}
                maxLength={400}
              />
            </label>
            <Button primary type="submit" busy={start.isPending} disabled={!form.keyword.trim()}>
              <Sparkles size={16} />
              {t("runAnalysis")}
            </Button>
          </div>
          <div className="form-grid">
            <div className="oc-field">
              <span>{t("engine")}</span>
              <div className="oc-segmented" role="group" aria-label={t("engine")}>
                {engines.map((engine) => (
                  <button
                    type="button"
                    key={engine}
                    className={form.engine === engine ? "active" : ""}
                    aria-pressed={form.engine === engine}
                    onClick={() => set("engine", engine)}
                  >
                    {t(engine)}
                  </button>
                ))}
              </div>
            </div>
            <label className="oc-field narrow">
              <span>{t("country")}</span>
              <input className="oc-input" value={form.country} maxLength={2} onChange={(event) => set("country", event.target.value)} />
            </label>
            <label className="oc-field narrow">
              <span>{t("languageCode")}</span>
              <input className="oc-input" value={form.language} maxLength={2} onChange={(event) => set("language", event.target.value)} />
            </label>
            <label className="oc-field">
              <span>{t("device")}</span>
              <select className="oc-select" value={form.device} onChange={(event) => set("device", event.target.value as Device)}>
                <option value="desktop">{t("desktop")}</option>
                <option value="mobile">{t("mobile")}</option>
              </select>
            </label>
            <label className="oc-field wide">
              <span>{t("yourSite")}</span>
              <input
                className="oc-input"
                value={form.target}
                placeholder={t("yourSitePlaceholder")}
                onChange={(event) => set("target", event.target.value)}
              />
            </label>
            <label className="oc-field wide">
              <span>{t("yourPage")}</span>
              <input className="oc-input" value={form.targetUrl} placeholder="https://" onChange={(event) => set("targetUrl", event.target.value)} />
            </label>
            <label className="oc-field narrow">
              <span>{t("pagesToCrawl")}</span>
              <select className="oc-select" value={form.pages} onChange={(event) => set("pages", Number(event.target.value))}>
                {[5, 10, 20].map((count) => (
                  <option key={count} value={count}>
                    {count}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <div className="form-options">
            <label className="oc-check">
              <input type="checkbox" checked={form.volume} onChange={(event) => set("volume", event.target.checked)} />
              {t("monthlySearches")}
            </label>
            <label className="oc-check">
              <input type="checkbox" checked={form.javascript} onChange={(event) => set("javascript", event.target.checked)} />
              {t("renderJs")}
            </label>
            <label className="oc-check">
              <input type="checkbox" checked={form.backlinks} onChange={(event) => set("backlinks", event.target.checked)} />
              {t("backlinkData")}
            </label>
            <span className="cost-line">
              {firstReady
                ? firstReady.estimate.usd > 0
                  ? t("costEstimate", { usd: usd(firstReady.estimate.usd, lang), provider: firstReady.provider })
                  : t("costFree")
                : ""}
            </span>
          </div>
        </form>
        {problem && <div className="oc-banner warning">{problem}</div>}
      </section>

      {shown.length > 0 && (
        <div className="jobs">
          {shown.map((job) => (
            <JobCard
              key={job.id}
              job={job}
              onStop={(id) => void api.cancel(id)}
              onDismiss={() => void api.clearJobs().then(() => queryClient.invalidateQueries({ queryKey: ["jobs"] }))}
            />
          ))}
        </div>
      )}

      <div className="analyze-body">
        <History items={analyses.data ?? []} loading={analyses.isLoading} current={current} onSelect={onSelect} />
        <div className="analysis-main">
          {analyses.isError && <ErrorState message={errorText(analyses.error)} retry={() => void analyses.refetch()} />}
          {current === null && !analyses.isLoading && <Empty icon={<Sparkles size={28} />}>{t("noAnalyses")}</Empty>}
          {current !== null && view.isLoading && <Loading />}
          {view.isError && <ErrorState message={errorText(view.error)} retry={() => void view.refetch()} />}
          {view.data && <AnalysisResult view={view.data} notify={notify} />}
        </div>
      </div>

      {confirm && (
        <Modal
          title={t("runAnalysis")}
          onClose={() => setConfirm(null)}
          footer={
            <>
              <Button onClick={() => setConfirm(null)}>{t("cancel")}</Button>
              <Button primary busy={start.isPending} onClick={() => start.mutate(requestOf(form))}>
                {t("runAnalysis")}
              </Button>
            </>
          }
        >
          <p>{t("costConfirm", { usd: usd(confirm.usd, lang), provider: confirm.provider })}</p>
        </Modal>
      )}
    </div>
  );
}

/** An intent signal (`intent.modifier:nasıl`) in words. */
function signalText(signal: string, t: (key: Key) => string) {
  const [kind = "", value = ""] = signal.replace(/^intent\./, "").split(":");
  if (kind === "feature") {
    const key = `feature.${value}`;
    return hasKey(key) ? t(key) : value;
  }
  return kind === "modifier" ? `“${value}”` : value;
}

function History({
  items,
  loading,
  current,
  onSelect,
}: {
  items: AnalysisSummary[];
  loading: boolean;
  current: number | null;
  onSelect: (id: number) => void;
}) {
  const { t, lang } = useI18n();
  return (
    <aside className="oc-panel history">
      <div className="oc-panel-head">
        <h2>{t("history")}</h2>
      </div>
      {loading && <Loading />}
      <ul>
        {items.map((item) => (
          <li key={item.id}>
            <button className={item.id === current ? "active" : ""} aria-current={item.id === current} onClick={() => onSelect(item.id)}>
              <span className={`kd-chip ${difficultyTone(item.difficulty)}`}>{number(item.difficulty, lang, 0)}</span>
              <span className="history-text">
                <b>{item.keyword}</b>
                <small>
                  {t(item.engine)} · {date(item.created_at, lang)}
                </small>
              </span>
            </button>
          </li>
        ))}
      </ul>
    </aside>
  );
}

function AnalysisResult({ view, notify }: { view: AnalysisView; notify: (text: string, error?: boolean) => void }) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const { analysis, snapshot } = view;
  const kd = analysis.difficulty.score;
  const target = analysis.target;
  const rescore = useMutation({
    mutationFn: () => api.rescore(view.id),
    onSuccess: (fresh) => {
      queryClient.setQueryData<AnalysisView>(["analysis", view.id], (old) => (old ? { ...old, analysis: fresh } : old));
      void queryClient.invalidateQueries({ queryKey: ["analyses"] });
      notify(t("rescored"));
    },
    onError: (error) => notify(errorText(error), true),
  });
  const features = useMemo(() => {
    const kinds = new Map<string, number>();
    for (const feature of snapshot.features) kinds.set(feature.kind, (kinds.get(feature.kind) ?? 0) + Math.max(1, feature.items?.length ?? 0));
    return [...kinds.entries()];
  }, [snapshot.features]);
  const intentKey = (name: string) => `intent_${name}` as Key;

  return (
    <div className="analysis-result">
      <header className="result-head">
        <div>
          <h2>{snapshot.query.keyword}</h2>
          <p className="faint">
            {t(snapshot.query.engine)} · {snapshot.query.country.toUpperCase()}-{snapshot.query.language} · {t(snapshot.query.device ?? "desktop")} ·{" "}
            {t("serpOf", { provider: snapshot.provider, date: date(snapshot.fetched_at, lang) })}
            {snapshot.total_results ? ` · ${t("totalResults", { count: number(snapshot.total_results, lang, 0) })}` : ""}
          </p>
        </div>
        <div className="row">
          <ExportMenu
            onExport={(format) =>
              void api
                .exportAnalysis(view.id, `${snapshot.query.keyword}-${snapshot.query.engine}`, format)
                .then((rows) => rows !== null && notify(t("exported", { rows: number(rows, lang, 0) })))
                .catch((error: unknown) => notify(errorText(error), true))
            }
          />
          <Button small busy={rescore.isPending} onClick={() => rescore.mutate()}>
            <RefreshCw size={14} />
            {t("rescore")}
          </Button>
        </div>
      </header>

      <div className="headline-cards">
        <section className="oc-card kd-card">
          <ScoreRing value={kd} label={t("difficulty")} tone={difficultyTone(kd)} />
          <div>
            <span className="type-label">{t("difficulty")}</span>
            <h3 className={`band ${difficultyTone(kd)}`}>{t(bandKey(analysis.band))}</h3>
            <p>{t("difficultyLead")}</p>
          </div>
        </section>
        <section className="oc-card mini-card">
          <span className="type-label">{t("intent")}</span>
          <h3>{t(intentKey(analysis.intent.primary))}</h3>
          {analysis.intent.secondary && (
            <p className="faint">
              {t("secondaryIntent")} {t(intentKey(analysis.intent.secondary)).toLocaleLowerCase(lang)}
            </p>
          )}
          <div className="oc-chips">
            {analysis.intent.signals.slice(0, 4).map((signal) => (
              <span className="oc-chip" key={signal}>
                {signalText(signal, t)}
              </span>
            ))}
          </div>
        </section>
        <section className="oc-card mini-card">
          <span className="type-label">{t("volume")}</span>
          <h3 className="oc-num">{analysis.volume !== null && analysis.volume !== undefined ? number(analysis.volume, lang, 0) : "—"}</h3>
          {(analysis.volume === null || analysis.volume === undefined) && <p className="faint">{t("volumeUnknown")}</p>}
        </section>
        {target && (
          <section className="oc-card mini-card">
            <span className="type-label">{t("opportunity")}</span>
            <h3 className={`oc-num ${tone(target.opportunity.score)}`}>{number(target.opportunity.score, lang, 0)}</h3>
            <p className="faint">
              {t("yourPosition")}: {target.position ? `#${target.position}` : t("notRanking")}
            </p>
          </section>
        )}
      </div>

      <section className="oc-panel">
        <div className="oc-panel-head">
          <h2>{t("serpFeatures")}</h2>
        </div>
        <div className="oc-chips feature-chips">
          {features.length === 0 && <span className="faint">{t("noFeatures")}</span>}
          {features.map(([kind, count]) => (
            <span className="oc-chip" key={kind}>
              {t(`feature.${kind}` as Key)}
              {count > 1 && <span>{count}</span>}
            </span>
          ))}
        </div>
      </section>

      <ResultsTable view={view} />

      <div className="two-columns">
        <section className="oc-panel">
          <div className="oc-panel-head">
            <h2>{t("difficulty")}</h2>
          </div>
          <BreakdownList breakdown={analysis.difficulty} />
        </section>
        <Recommendations view={view} />
      </div>

      {target && (
        <div className="two-columns">
          <section className="oc-panel">
            <div className="oc-panel-head">
              <h2>{t("opportunity")}</h2>
            </div>
            <p className="panel-lead">{t("opportunityLead")}</p>
            <BreakdownList breakdown={target.opportunity} />
          </section>
          <section className="oc-panel">
            <div className="oc-panel-head">
              <h2>{t("gapTerms")}</h2>
            </div>
            <p className="panel-lead">{t("gapTermsLead")}</p>
            <div className="oc-chips">
              {target.gaps.map((term) => (
                <span className="oc-chip gap" key={term}>
                  {term}
                </span>
              ))}
            </div>
            {target.page?.content && (
              <>
                <h3 className="subhead">{t("contentScore")}</h3>
                <BreakdownList breakdown={target.page.content} compact />
              </>
            )}
          </section>
        </div>
      )}
    </div>
  );
}

function ResultsTable({ view }: { view: AnalysisView }) {
  const { t, lang } = useI18n();
  const [open, setOpen] = useState<number | null>(null);
  const facts = new Map<string, DomainFacts>(view.domains.map((fact) => [fact.domain, fact]));
  const scores = new Map<number, PageScore>(view.analysis.pages.map((page) => [page.position, page]));
  const ownDomain = view.analysis.target?.domain;
  const organic = view.snapshot.organic.filter((result) => result.position <= Math.max(10, view.analysis.pages.length));
  return (
    <section className="oc-panel results">
      <div className="oc-panel-head">
        <h2>{t("topResults")}</h2>
      </div>
      <div className="oc-table-wrap">
        <table className="oc-table">
          <thead>
            <tr>
              <th className="num">{t("position")}</th>
              <th>{t("result")}</th>
              <th className="num">{t("siteScore")}</th>
              <th className="num">{t("contentScore")}</th>
              <th className="num">{t("technicalScore")}</th>
              <th className="num">{t("strength")}</th>
              <th className="num">{t("domainAge")}</th>
              <th className="num">{t("pageRank")}</th>
              <th className="num">{t("referringDomains")}</th>
              <th className="num">{t("words")}</th>
            </tr>
          </thead>
          <tbody>
            {organic.map((result) => {
              const score = scores.get(result.position);
              const fact = facts.get(result.domain);
              const own = sameSite(result.domain, ownDomain);
              const expanded = open === result.position;
              return [
                <tr key={result.position} className={`${own ? "own" : ""} ${expanded ? "selected" : ""}`}>
                  <td className="num">
                    <button
                      className="expand"
                      aria-expanded={expanded}
                      aria-label={t("scoreDetails")}
                      disabled={!score}
                      onClick={() => setOpen(expanded ? null : result.position)}
                    >
                      {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                      {result.position}
                    </button>
                  </td>
                  <td className="result-cell">
                    <a href={result.url} target="_blank" rel="noreferrer" title={result.url}>
                      {result.title || result.url}
                      <ExternalLink size={11} />
                    </a>
                    <small>
                      {result.domain}
                      {own && <span className="oc-badge product">{t("yourSiteBadge")}</span>}
                      {result.site_only && (
                        <span className="oc-badge" title={t("siteOnlyHelp")}>
                          {t("siteOnly")}
                        </span>
                      )}
                    </small>
                  </td>
                  <td className="num">
                    <ScorePill value={score?.site?.score} tone={tone(score?.site?.score)} />
                  </td>
                  <td className="num">
                    {score?.content ? (
                      <ScorePill value={score.content.score} tone={tone(score.content.score)} />
                    ) : (
                      <span className="faint" title={result.site_only ? t("siteOnlyHelp") : undefined}>
                        {t("notCrawled")}
                      </span>
                    )}
                  </td>
                  <td className="num">
                    <ScorePill value={score?.technical?.score} tone={tone(score?.technical?.score)} />
                  </td>
                  <td className="num oc-num">{score?.strength !== null && score?.strength !== undefined ? number(score.strength, lang, 0) : "—"}</td>
                  <td className="num oc-num">{fact?.age_years !== undefined ? `${number(fact.age_years, lang)} ${t("years")}` : "—"}</td>
                  <td className="num oc-num">{fact?.open_page_rank !== undefined ? number(fact.open_page_rank, lang, 2) : "—"}</td>
                  <td className="num oc-num">{fact?.referring_domains !== undefined ? number(fact.referring_domains, lang, 0) : "—"}</td>
                  <td className="num oc-num">{score?.words ? number(score.words, lang, 0) : "—"}</td>
                </tr>,
                expanded && score ? (
                  <tr key={`${result.position}-details`} className="details-row">
                    <td colSpan={10}>
                      <div className="page-breakdowns">
                        {(
                          [
                            ["contentScore", score.content],
                            ["siteScore", score.site],
                            ["technicalScore", score.technical],
                          ] as const
                        ).map(([label, breakdown]) =>
                          breakdown ? (
                            <div key={label}>
                              <h3 className="subhead">
                                {t(label)} · <span className="oc-num">{number(breakdown.score, lang, 0)}</span>
                              </h3>
                              <BreakdownList breakdown={breakdown} compact />
                            </div>
                          ) : null,
                        )}
                      </div>
                    </td>
                  </tr>
                ) : null,
              ];
            })}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function Recommendations({ view }: { view: AnalysisView }) {
  const { t, lang } = useI18n();
  const recommendations = view.analysis.recommendations;
  const [copied, setCopied] = useState(false);
  const copyQuestions = () => {
    void navigator.clipboard?.writeText(recommendations.questions.join("\n")).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });
  };
  const range = (key: "recWordsValue" | "recH2Value", value?: { low: number; median: number; high: number } | null) =>
    value ? t(key, { median: number(value.median, lang, 0), low: number(value.low, lang, 0), high: number(value.high, lang, 0) }) : "—";
  return (
    <section className="oc-panel recommendations">
      <div className="oc-panel-head">
        <h2>{t("recommendations")}</h2>
      </div>
      <dl>
        <dt>{t("recWords")}</dt>
        <dd>{range("recWordsValue", recommendations.words)}</dd>
        <dt>{t("recH2")}</dt>
        <dd>{range("recH2Value", recommendations.h2)}</dd>
        <dt>{t("recTitle")}</dt>
        <dd>
          {t("recTitleValue", {
            chars: number(recommendations.title.median_chars, lang, 0),
            px: number(recommendations.title.median_px, lang, 0),
            first: percent(recommendations.title.keyword_first, lang),
          })}
        </dd>
        {recommendations.schema.length > 0 && (
          <>
            <dt>{t("recSchema")}</dt>
            <dd className="oc-chips">
              {recommendations.schema.map((type) => (
                <span key={type} className="oc-chip">
                  {type}
                </span>
              ))}
            </dd>
          </>
        )}
        {recommendations.headings.length > 0 && (
          <>
            <dt>{t("recHeadings")}</dt>
            <dd className="oc-chips">
              {recommendations.headings.map((heading) => (
                <span key={heading} className="oc-chip">
                  {heading}
                </span>
              ))}
            </dd>
          </>
        )}
      </dl>
      {recommendations.short_answer && <div className="oc-banner info">{t("recShortAnswer")}</div>}
      {recommendations.questions.length > 0 && (
        <>
          <h3 className="subhead row between">
            {t("recQuestions")}
            <Button small className="ghost" onClick={copyQuestions}>
              {copied ? t("copied") : t("copy")}
            </Button>
          </h3>
          <ul className="questions">
            {recommendations.questions.map((question) => (
              <li key={question}>{question}</li>
            ))}
          </ul>
        </>
      )}
      {recommendations.terms.length > 0 && (
        <>
          <h3 className="subhead">{t("consensusTerms")}</h3>
          <div className="oc-chips">
            {recommendations.terms.map((term) => (
              <span key={term} className="oc-chip">
                {term}
              </span>
            ))}
          </div>
        </>
      )}
      {recommendations.ideas.length > 0 && (
        <>
          <h3 className="subhead">{t("recIdeas")}</h3>
          <div className="oc-chips">
            {recommendations.ideas.map((idea) => (
              <span key={idea} className="oc-chip idea">
                {idea}
              </span>
            ))}
          </div>
        </>
      )}
    </section>
  );
}
