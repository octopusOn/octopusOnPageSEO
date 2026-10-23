// One brief: what the ranking pages have (length, headings, terms, questions, structured data,
// titles), the pages themselves, pages of the site to link to, the brief an assistant wrote, an
// editor that scores a draft as it is written, and the site's pages scored against the brief
// with improvements an assistant suggests.

import { useEffect, useMemo, useState, type FormEvent } from "react";
import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, Check, ClipboardCopy, Download, Hourglass, Link2, ListChecks, Save, Smartphone, Sparkles, Trash2, X } from "lucide-react";
import { Button, DataTable, Empty, ErrorState, Field, Loading, Stat, Tip, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { BriefCompetitor, BriefView, ContentBrief, ContentScore, ProjectRow, TermUse, WriterEngine } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { Entity } from "../../lib/Entity";
import { explain } from "../../lib/errors";
import { formatNumber, formatUsd, relativeTime } from "../../lib/format";
import { refreshSpending } from "../../lib/PaidFrame";
import { download } from "../../lib/report";
import { tableLabels } from "../../lib/tables";
import { briefMarkdown, countWords, rangePlace, scoreBand } from "./brief";
import { Markdown } from "./Markdown";
import { ScoreChip } from "./ScoreChip";
import { WriterPicker } from "./Writers";

/** Score parts the page names, in the model's order. */
const PARTS = ["placement", "coverage", "depth", "structure", "questions", "media", "schema", "snippet", "freshness"] as const;

function rangeText(spread: { low: number; high: number } | null, locale: string): string {
  return spread ? `${formatNumber(Math.round(spread.low), locale)}–${formatNumber(Math.round(spread.high), locale)}` : "—";
}

/** The score as a ring. */
function ScoreRing({ score, label }: { score: number; label: string }) {
  const radius = 34;
  const length = 2 * Math.PI * radius;
  const value = Math.max(0, Math.min(100, score));
  return (
    <div className={`score-ring ${scoreBand(score)}`} role="img" aria-label={`${label}: ${Math.round(score)}`}>
      <svg viewBox="0 0 80 80" aria-hidden="true">
        <circle cx="40" cy="40" r={radius} className="track" />
        <circle cx="40" cy="40" r={radius} className="fill" strokeDasharray={`${(value / 100) * length} ${length}`} transform="rotate(-90 40 40)" />
      </svg>
      <strong>{Math.round(score)}</strong>
    </div>
  );
}

/** Terms as chips: used ones with their count against the ranking pages' average. */
function TermChips({ terms, used }: { terms: TermUse[]; used: boolean }) {
  const { t, locale } = useI18n();
  if (!terms.length) return <p className="faint small">{used ? t("content.noTermsUsed") : t("content.noTermsMissing")}</p>;
  return (
    <div className="term-chips">
      {terms.map((term) => (
        <span key={term.term} className={`oc-chip small ${used ? (term.count >= term.average * 0.5 ? "good" : "thin") : "missing"}`} title={t("content.termTip", { share: Math.round(term.share * 100), average: new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(term.average) })}>
          {term.term}
          {used && <span className="faint">{term.count}</span>}
        </span>
      ))}
    </div>
  );
}

/** What a page or draft scored: the ring, its parts, length and headings against the brief, and the gaps. */
function ScorePanel({ score, brief }: { score: ContentScore; brief: ContentBrief }) {
  const { t, locale } = useI18n();
  const parts = PARTS.map((id) => score.breakdown.components.find((part) => part.id === id)).filter((part) => part !== undefined);
  const words = rangePlace(score.words, brief.words);
  const h2 = rangePlace(score.h2, brief.h2);
  return (
    <div className="score-panel">
      <div className="score-head">
        <ScoreRing score={score.breakdown.score} label={t("content.score")} />
        <div className="score-facts">
          <span className={`fact ${words ?? ""}`}>
            {t("content.factWords", { n: formatNumber(score.words, locale), range: rangeText(brief.words, locale) })}
          </span>
          <span className={`fact ${h2 ?? ""}`}>{t("content.factH2", { n: score.h2, range: rangeText(brief.h2, locale) })}</span>
          <span className={`fact ${score.title_chars > 0 && score.title_chars <= 60 ? "within" : "below"}`}>{t("content.factTitle", { n: score.title_chars })}</span>
          <span className={`fact ${score.description_chars >= 70 && score.description_chars <= 160 ? "within" : "below"}`}>{t("content.factDescription", { n: score.description_chars })}</span>
          <span className={`fact ${score.keyword_in_title ? "within" : "below"}`}>{score.keyword_in_title ? t("content.keywordInTitle") : t("content.keywordNotInTitle")}</span>
          {score.readability !== null && <span className="fact">{t("content.factReading", { n: Math.round(score.readability) })}</span>}
        </div>
      </div>
      <ol className="bars score-parts">
        {parts.map((part) => (
          <li key={part.id} className={part.normalized === null ? "unknown" : undefined}>
            <span className="bar-label">
              {t(`content.part_${part.id}`)}
              <Tip text={t(`content.partHelp_${part.id}`)} label={t(`content.part_${part.id}`)} />
            </span>
            <span className="bar-track" aria-hidden="true">
              <span className={`bar-fill ${scoreBand((part.normalized ?? 0) * 100)}`} style={{ width: `${(part.normalized ?? 0) * 100}%` }} />
            </span>
            <span className="bar-value">{part.normalized === null ? "—" : Math.round(part.normalized * 100)}</span>
          </li>
        ))}
      </ol>
      <h5>{t("content.missingTerms", { n: score.missing.length })}</h5>
      <TermChips terms={score.missing} used={false} />
      <h5>{t("content.usedTerms", { n: score.terms.length })}</h5>
      <TermChips terms={score.terms} used />
      {score.unanswered.length > 0 && (
        <>
          <h5>{t("content.unanswered")}</h5>
          <ul className="plain-list small">
            {score.unanswered.map((question) => (
              <li key={question}>{question}</li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}

/** A value that settles `delay` milliseconds after it stops changing. */
function useSettled<T>(value: T, delay: number): T {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const timer = window.setTimeout(() => setSettled(value), delay);
    return () => window.clearTimeout(timer);
  }, [value, delay]);
  return settled;
}

/** A draft written against the brief, scored as it is written. */
function Editor({ view, brief }: { view: BriefView; brief: ContentBrief }) {
  const { t, locale } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const kept = view.scores.find((row) => row.kind === "draft");
  const [draft, setDraft] = useState({ title: kept?.title ?? "", description: kept?.description ?? "", body: kept?.body ?? "" });
  const settled = useSettled(draft, 800);
  const live = useQuery({
    queryKey: ["content", "draft-score", view.id, settled],
    queryFn: () => api.scoreContent({ brief_id: view.id, ...settled }),
    enabled: settled.body.trim().length > 0,
    placeholderData: keepPreviousData,
  });
  const save = useMutation({
    mutationFn: () => api.scoreContent({ brief_id: view.id, ...draft, save: true }),
    onSuccess: () => {
      notify(t("content.draftSaved"));
      void queryClient.invalidateQueries({ queryKey: ["content", "brief", view.id] });
    },
    onError: (thrown) => fail(thrown),
  });
  const words = countWords(draft.body);
  const changed = !kept || kept.title !== draft.title || kept.description !== draft.description || kept.body !== draft.body;
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("content.editorTitle")}</h3>
          <span className="faint">{t("content.editorHint")}</span>
        </div>
        <div className="report-actions">
          <span className="faint small">{t("content.wordsTyped", { n: formatNumber(words, locale) })}</span>
          <Button small primary busy={save.isPending} disabled={!draft.body.trim() || !changed} onClick={() => save.mutate()}>
            <Save size={14} />
            {t("content.saveDraft")}
          </Button>
        </div>
      </header>
      <div className="editor-grid">
        <div className="editor-fields">
          <Field label={t("content.draftTitle")} hint={t("content.draftTitleHint", { n: draft.title.length, median: Math.round(brief.title.median_chars) })}>
            <input className="oc-input" value={draft.title} maxLength={200} onChange={(event) => setDraft({ ...draft, title: event.target.value })} />
          </Field>
          <Field label={t("content.draftDescription")} hint={t("content.draftDescriptionHint", { n: draft.description.length })}>
            <textarea className="oc-input" rows={2} value={draft.description} maxLength={400} onChange={(event) => setDraft({ ...draft, description: event.target.value })} />
          </Field>
          <Field label={t("content.draftBody")} hint={t("content.draftBodyHint")}>
            <textarea className="oc-input draft-body" rows={22} value={draft.body} onChange={(event) => setDraft({ ...draft, body: event.target.value })} placeholder={brief.outline.map((heading) => `## ${heading}`).join("\n\n")} />
          </Field>
        </div>
        <aside className="editor-score" aria-live="polite">
          {live.data ? (
            <ScorePanel score={live.data} brief={brief} />
          ) : live.error ? (
            <p className="warn small">{explain(live.error, t).message}</p>
          ) : (
            <p className="faint small">{t("content.startWriting")}</p>
          )}
          {live.isFetching && <span className="faint small">{t("content.scoring")}</span>}
        </aside>
      </div>
    </section>
  );
}

/** Pages of the site scored against the brief, and improvements an assistant suggests. */
function ScoredPages({ view, brief, project }: { view: BriefView; brief: ContentBrief; project: ProjectRow }) {
  const { t, locale } = useI18n();
  const { notify, fail, jobs } = useShell();
  const queryClient = useQueryClient();
  const [url, setUrl] = useState("");
  const [open, setOpen] = useState<number | null>(null);
  const pages = view.scores.filter((row) => row.kind === "url");
  const draft = view.scores.find((row) => row.kind === "draft");
  const rows = draft ? [draft, ...pages] : pages;
  const score = useMutation({
    mutationFn: (address: string) => api.scoreContent({ brief_id: view.id, url: address }),
    onSuccess: (scored) => {
      setUrl("");
      if (scored.id !== null) setOpen(scored.id);
      void queryClient.invalidateQueries({ queryKey: ["content", "brief", view.id] });
    },
    onError: (thrown) => fail(thrown),
  });
  const writers = useQuery({ queryKey: ["content", "writers", view.id, "improve"], queryFn: () => api.contentWritingEstimate(view.id, "improve") });
  const writing = jobs.some((job) => job.kind === "content-writing" && job.project_id === project.id && job.status === "running");
  const improve = useMutation({
    mutationFn: ({ engine, id }: { engine: WriterEngine; id: number }) => api.startContentWriting(view.id, "improve", engine, id),
    onSuccess: () => {
      notify(t("content.writingStarted"));
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
      refreshSpending(queryClient);
    },
    onError: (thrown) => fail(thrown),
  });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (url.trim()) score.mutate(url.trim());
  };
  const shown = rows.find((row) => row.id === open) ?? null;
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("content.scoredTitle")}</h3>
          <span className="faint">{t("content.scoredHint")}</span>
        </div>
      </header>
      <form className="new-brief" onSubmit={submit}>
        <input className="oc-input" type="url" value={url} onChange={(event) => setUrl(event.target.value)} placeholder={`https://${project.domain}/…`} aria-label={t("content.pageUrl")} />
        <Button type="submit" small primary busy={score.isPending} disabled={!url.trim()}>
          <ListChecks size={14} />
          {t("content.scorePage")}
        </Button>
      </form>
      {rows.length === 0 ? (
        <p className="faint small">{t("content.noScores")}</p>
      ) : (
        <ul className="scored-list">
          {rows.map((row) => (
            <li key={row.id} className={open === row.id ? "open" : undefined}>
              <button type="button" className="scored-row" aria-expanded={open === row.id} onClick={() => setOpen(open === row.id ? null : row.id)}>
                <ScoreChip score={row.score} />
                <span className="cell-main">{row.kind === "draft" ? t("content.draft") : (row.url ?? "").replace(/^https?:\/\/(www\.)?/, "")}</span>
                <span className="faint small">
                  {t("content.scoredLine", { words: formatNumber(row.result.words, locale), missing: row.result.missing.length })} · {relativeTime(row.scored_at, locale)}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
      {shown && (
        <div className="scored-detail">
          <div className="scored-actions">
            {shown.url && (
              <Entity kind="url" value={shown.url}>
                <span className="cell-url">{shown.url}</span>
              </Entity>
            )}
            {shown.url && (
              <Button small className="ghost" busy={score.isPending} onClick={() => score.mutate(shown.url!)}>
                {t("content.scoreAgain")}
              </Button>
            )}
            <WriterPicker writers={writers.data?.writers} busy={improve.isPending || writing} disabled={writing} label={t("content.suggest")} onWrite={(engine) => improve.mutate({ engine, id: shown.id })} />
          </div>
          <div className="two-columns">
            <ScorePanel score={shown.result} brief={brief} />
            <div className="sub-panel">
              <h4>
                <Sparkles size={14} aria-hidden="true" /> {t("content.suggestions")}
              </h4>
              {shown.suggestions ? <Markdown text={shown.suggestions} /> : <p className="faint small">{writing ? t("content.writing") : t("content.noSuggestions")}</p>}
            </div>
          </div>
        </div>
      )}
    </section>
  );
}

/** What the brief found, laid out for a writer. */
function BriefFindings({ brief }: { brief: ContentBrief }) {
  const { t, locale } = useI18n();
  const percent = (share: number) => `${Math.round(share * 100)}%`;
  const columns: Column<BriefCompetitor>[] = [
    { key: "position", label: "#", render: (row) => row.position, sort: (row) => row.position, numeric: true },
    {
      key: "page",
      label: t("content.colPage"),
      render: (row) => (
        <span className="link-cell">
          <strong className="cell-main">{row.title ?? row.domain}</strong>
          <Entity kind="url" value={row.url}>
            <span className="cell-url faint small">{row.url.replace(/^https?:\/\/(www\.)?/, "")}</span>
          </Entity>
        </span>
      ),
      sort: (row) => row.title ?? row.url,
    },
    { key: "words", label: t("content.colWords"), render: (row) => (row.score === null ? "—" : formatNumber(row.words, locale)), sort: (row) => row.words, numeric: true },
    { key: "h2", label: "H2", render: (row) => (row.score === null ? "—" : row.h2), sort: (row) => row.h2, numeric: true, optional: true },
    {
      key: "score",
      label: t("content.colScore"),
      render: (row) => (row.score === null ? <span className="faint small">{row.status === null ? t("content.noAnswer") : `HTTP ${row.status}`}</span> : <ScoreChip score={row.score} />),
      sort: (row) => row.score,
      numeric: true,
    },
  ];
  return (
    <>
      <div className="oc-stats">
        <Stat label={t("content.targetWords")} value={brief.words ? formatNumber(Math.round(brief.words.median), locale) : "—"} hint={t("content.rangeHint", { range: rangeText(brief.words, locale) })} />
        <Stat label={t("content.targetH2")} value={brief.h2 ? Math.round(brief.h2.median) : "—"} hint={t("content.rangeHint", { range: rangeText(brief.h2, locale) })} />
        <Stat label={t("content.intent")} value={brief.intent ? t(`overview.intent_${brief.intent}`) : "—"} />
        <Stat label={t("content.titleLength")} value={Math.round(brief.title.median_chars)} hint={t("content.titleHint", { first: percent(brief.title.keyword_first), year: percent(brief.title.with_year) })} />
      </div>
      {brief.short_answer && (
        <div className="oc-banner info" role="status">
          <span>{t("content.shortAnswer")}</span>
        </div>
      )}
      <div className="two-columns">
        <section className="sub-panel">
          <h4>{t("content.outline")}</h4>
          {brief.outline.length ? (
            <ol className="outline">
              {brief.outline.map((heading) => (
                <li key={heading} className={brief.questions.includes(heading) ? "question" : undefined}>
                  {heading}
                </li>
              ))}
            </ol>
          ) : (
            <p className="faint small">{t("content.noOutline")}</p>
          )}
          {brief.schema.length > 0 && (
            <p className="profile-chips">
              <span className="faint small">{t("content.schema")}</span>
              {brief.schema.map((schema) => (
                <span key={schema} className="oc-chip small">
                  {schema}
                </span>
              ))}
            </p>
          )}
        </section>
        <section className="sub-panel">
          <h4>{t("content.terms", { n: brief.terms.length })}</h4>
          <ol className="bars term-bars">
            {brief.terms.slice(0, 30).map((term) => (
              <li key={term.term}>
                <span className="bar-label">{term.term}</span>
                <span className="bar-track" aria-hidden="true">
                  <span className="bar-fill" style={{ width: `${term.share * 100}%` }} />
                </span>
                <span className="bar-value" title={t("content.termTip", { share: Math.round(term.share * 100), average: new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(term.average) })}>
                  {percent(term.share)}
                </span>
              </li>
            ))}
          </ol>
        </section>
      </div>
      {(brief.questions.length > 0 || brief.ideas.length > 0) && (
        <div className="two-columns">
          <section className="sub-panel">
            <h4>{t("content.questions")}</h4>
            {brief.questions.length ? (
              <ul className="plain-list">
                {brief.questions.map((question) => (
                  <li key={question}>{question}</li>
                ))}
              </ul>
            ) : (
              <p className="faint small">{t("content.noQuestions")}</p>
            )}
          </section>
          <section className="sub-panel">
            <h4>{t("content.ideas")}</h4>
            <div className="term-chips">
              {brief.ideas.map((idea) => (
                <Entity key={idea} kind="keyword" value={idea}>
                  <span className="oc-chip small">{idea}</span>
                </Entity>
              ))}
            </div>
          </section>
        </div>
      )}
      <section className="sub-panel">
        <h4>{t("content.competitors")}</h4>
        <DataTable rows={brief.competitors} columns={columns} rowKey={(row) => row.url} initialSort={{ key: "position", direction: "asc" }} tableId="content-competitors" empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
      </section>
      <section className="sub-panel">
        <h4>
          <Link2 size={14} aria-hidden="true" /> {t("content.links")}
        </h4>
        {brief.internal_links.length ? (
          <ul className="plain-list">
            {brief.internal_links.map((link) => (
              <li key={link.url}>
                <Entity kind="url" value={link.url}>
                  <span>{link.title || link.url}</span>
                </Entity>{" "}
                <span className="faint small">{link.url.replace(/^https?:\/\/(www\.)?/, "")}</span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="faint small">{t("content.noLinks")}</p>
        )}
      </section>
    </>
  );
}

export function BriefDetail({ project, briefId, onBack }: { project: ProjectRow; briefId: number; onBack: () => void }) {
  const { t, locale } = useI18n();
  const { notify, fail, jobs } = useShell();
  const queryClient = useQueryClient();
  const view = useQuery({
    queryKey: ["content", "brief", briefId],
    queryFn: () => api.contentBrief(briefId),
    refetchInterval: (state) => (state.state.data?.status === "running" ? 3000 : false),
  });
  const writers = useQuery({ queryKey: ["content", "writers", briefId, "brief"], queryFn: () => api.contentWritingEstimate(briefId, "brief"), enabled: view.data?.status === "done" });
  const writing = jobs.some((job) => job.kind === "content-writing" && job.project_id === project.id && job.status === "running");
  const reading = jobs.find((job) => job.kind === "content-brief" && job.project_id === project.id && job.status === "running");
  const write = useMutation({
    mutationFn: (engine: WriterEngine) => api.startContentWriting(briefId, "brief", engine),
    onSuccess: () => {
      notify(t("content.writingStarted"));
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
      refreshSpending(queryClient);
    },
    onError: (thrown) => fail(thrown),
  });
  const remove = useMutation({
    mutationFn: () => api.deleteContentBrief(briefId),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["content"] });
      onBack();
    },
    onError: (thrown) => fail(thrown),
  });
  const labels = useMemo(
    () => ({
      title: t("content.titleLength"),
      words: t("content.targetWords"),
      h2: t("content.targetH2"),
      outline: t("content.outline"),
      terms: t("content.termsPlain"),
      questions: t("content.questions"),
      schema: t("content.schema"),
      links: t("content.links"),
      competitors: t("content.competitors"),
    }),
    [t],
  );
  if (view.isLoading) return <Loading label={t("common.loading")} />;
  if (view.error || !view.data) return <ErrorState title={t("common.loadFailed")} message={explain(view.error, t).message} retry={() => void view.refetch()} retryLabel={t("common.retry")} />;
  const data = view.data;
  const brief = data.brief;
  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      notify(t("content.copied"));
    } catch {
      notify(t("local.replyCopyFailed"), true);
    }
  };
  return (
    <div className="page-stack">
      <section className="oc-panel report">
        <header className="report-head">
          <div className="report-title">
            <Button small className="ghost" onClick={onBack} aria-label={t("content.back")}>
              <ArrowLeft size={14} />
              {t("content.back")}
            </Button>
            <h3 className="oc-panel-title">
              {data.keyword} {data.mobile && <Smartphone size={14} aria-label={t("keywords.mobile")} />}
            </h3>
            <span className="faint small">
              {data.country.toUpperCase()} · {data.language} · {relativeTime(data.created_at, locale)}
              {data.cost_usd > 0 && <> · {formatUsd(data.cost_usd, locale)}</>}
            </span>
          </div>
          <div className="report-actions">
            {brief && (
              <>
                <Button small onClick={() => void copy(briefMarkdown(brief, labels))}>
                  <ClipboardCopy size={14} />
                  {t("content.copyBrief")}
                </Button>
                <Button small onClick={() => download(`brief-${data.keyword.replace(/\s+/g, "-")}.md`, briefMarkdown(brief, labels), "text/markdown;charset=utf-8")}>
                  <Download size={14} />
                  {t("content.downloadBrief")}
                </Button>
              </>
            )}
            <Button small className="ghost" disabled={data.status === "running"} busy={remove.isPending} onClick={() => remove.mutate()} aria-label={t("content.delete")} title={t("content.delete")}>
              <Trash2 size={14} />
            </Button>
          </div>
        </header>
        {data.status === "running" && (
          <div className="oc-banner info" role="status">
            <Hourglass size={14} aria-hidden="true" />
            <span>{reading ? t("content.reading", { done: reading.done, total: reading.total || 10 }) : t("content.status_running")}</span>
          </div>
        )}
        {(data.status === "failed" || data.status === "interrupted") && (
          <div className="oc-banner error" role="status">
            <X size={14} aria-hidden="true" />
            <span>{data.error ?? t(`content.status_${data.status}`)}</span>
          </div>
        )}
        {brief && <BriefFindings brief={brief} />}
      </section>
      {brief && (
        <section className="oc-panel report">
          <header className="report-head">
            <div className="report-title">
              <h3 className="oc-panel-title">
                <Sparkles size={15} aria-hidden="true" /> {t("content.aiTitle")}
              </h3>
              <span className="faint">{data.ai_model ? t("content.aiBy", { model: data.ai_model }) : t("content.aiHint")}</span>
            </div>
            <div className="report-actions">
              {data.ai_text && (
                <Button small onClick={() => void copy(data.ai_text!)}>
                  <Check size={14} />
                  {t("content.copyText")}
                </Button>
              )}
              <WriterPicker writers={writers.data?.writers} busy={write.isPending || writing} disabled={writing} label={data.ai_text ? t("content.writeAgain") : t("content.write")} onWrite={(engine) => write.mutate(engine)} />
            </div>
          </header>
          {data.ai_text ? <Markdown text={data.ai_text} /> : writing ? <p className="faint">{t("content.writing")}</p> : <p className="faint small">{t("content.aiEmpty")}</p>}
        </section>
      )}
      {brief && <Editor view={data} brief={brief} />}
      {brief && <ScoredPages view={data} brief={brief} project={project} />}
      {!brief && data.status !== "running" && (
        <div className="oc-card">
          <Empty icon={<X size={26} />} title={t("content.notMadeTitle")}>
            <p>{t("content.notMadeText")}</p>
          </Empty>
        </div>
      )}
    </div>
  );
}
