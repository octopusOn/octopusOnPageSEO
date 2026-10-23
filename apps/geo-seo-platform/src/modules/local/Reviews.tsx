// The business's Google reviews: figures, stars, the rating over time, every review with the
// business's answer, and a reply drafted for those still waiting for one (from a template, or
// by an assistant with the user's own key).

import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Camera, Copy, ExternalLink, Hourglass, MessageSquareReply, Search, Star } from "lucide-react";
import { Button, Empty, Field, Modal, Segmented, Stat } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { LocalReview, LocalSetup, ProjectRow, RatingDay, WriterEngine } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { formatNumber, relativeTime } from "../../lib/format";
import { blockedText, CostLabel, refreshSpending } from "../../lib/PaidFrame";
import { WriterPicker } from "../content/Writers";
import { filterReviews, REVIEW_DEPTHS, replyDraft, type ReviewFilter } from "./local";

function one(value: number, locale: string): string {
  return new Intl.NumberFormat(locale, { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(value);
}

function Stars({ value }: { value: number | null }) {
  const { t } = useI18n();
  if (value === null) return null;
  return (
    <span className="review-stars" role="img" aria-label={t("local.stars", { n: value })}>
      {[1, 2, 3, 4, 5].map((star) => (
        <Star key={star} size={13} aria-hidden="true" className={star <= value ? "on" : "off"} />
      ))}
    </span>
  );
}

/** The rating and the number of reviews day by day. */
function RatingChart({ days }: { days: RatingDay[] }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    return {
      grid: { left: 44, right: 52, top: 36, bottom: 28 },
      legend: { top: 0, left: 0 },
      tooltip: { trigger: "axis" },
      xAxis: { type: "category", data: days.map((day) => new Date(`${day.day}T12:00:00Z`).toLocaleDateString(locale, { day: "numeric", month: "short" })), ...axis, splitLine: { show: false } },
      yAxis: [
        { type: "value", min: (extent: { min: number }) => Math.max(1, Math.floor(extent.min * 2) / 2 - 0.5), max: 5, ...axis },
        { type: "value", minInterval: 1, ...axis, splitLine: { show: false } },
      ],
      series: [
        { name: t("local.ratingSeries"), type: "line", smooth: true, symbolSize: 6, data: days.map((day) => day.rating) },
        { name: t("local.votesSeries"), type: "line", step: "end", symbol: "none", yAxisIndex: 1, lineStyle: { type: "dashed", width: 1.5 }, data: days.map((day) => day.votes) },
      ],
    };
  }, [days, tokens, t, locale]);
  return <EChart option={option} height={220} />;
}

/** A reply to a review, drafted in the reader's language, to copy and post on Google. */
function ReplyDialog({ projectId, review, business, onClose }: { projectId: number; review: LocalReview; business: string; onClose: () => void }) {
  const { t, locale } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const writers = useQuery({ queryKey: ["local", "reply-writers", projectId], queryFn: () => api.replyEstimate(projectId) });
  const draft = useMutation({
    mutationFn: (engine: WriterEngine) => api.draftReviewReply(projectId, review.review_id, engine),
    onSuccess: (drafted) => {
      setText(drafted.text);
      notify(t("local.replyDrafted", { cost: new Intl.NumberFormat(locale, { style: "currency", currency: "USD", maximumFractionDigits: 4 }).format(drafted.cost_usd) }));
      refreshSpending(queryClient);
    },
    onError: (thrown) => fail(thrown),
  });
  const [text, setText] = useState(() =>
    replyDraft(review, business, {
      greeting: t("local.replyGreeting"),
      greetingNamed: t("local.replyGreetingNamed"),
      positive: t("local.replyPositive"),
      neutral: t("local.replyNeutral"),
      negative: t("local.replyNegative"),
      signature: t("local.replySignature"),
    }),
  );
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      notify(t("local.replyCopied"));
    } catch {
      notify(t("local.replyCopyFailed"));
    }
  };
  return (
    <Modal
      title={t("local.replyTitle")}
      closeLabel={t("common.close")}
      onClose={onClose}
      footer={
        <>
          {review.url && (
            <a className="oc-btn small" href={review.url} target="_blank" rel="noreferrer noopener">
              <ExternalLink size={14} />
              {t("local.openOnGoogle")}
            </a>
          )}
          <Button primary small onClick={() => void copy()}>
            <Copy size={14} />
            {t("local.replyCopy")}
          </Button>
        </>
      }
    >
      <blockquote className="review-quote">
        <Stars value={review.rating} /> {review.text ?? <span className="faint">{t("local.noText")}</span>}
      </blockquote>
      <Field label={t("local.replyLabel")} hint={t("local.replyHint")}>
        <textarea className="oc-input reply-text" rows={8} value={text} onChange={(event) => setText(event.target.value)} />
      </Field>
      <div className="reply-ai">
        <span className="faint small">{t("local.replyAi")}</span>
        <WriterPicker writers={writers.data} busy={draft.isPending} label={t("local.replyWrite")} onWrite={(engine) => draft.mutate(engine)} />
      </div>
    </Modal>
  );
}

export function ReviewsTab({ project, setup }: { project: ProjectRow; setup: LocalSetup }) {
  const { t, locale } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const [filter, setFilter] = useState<ReviewFilter>("all");
  const [query, setQuery] = useState("");
  const [depth, setDepth] = useState<number>(100);
  const [replying, setReplying] = useState<LocalReview | null>(null);
  // While reviews wait in the queue, the page reads them again now and then.
  const reviews = useQuery({ queryKey: ["local", "reviews", project.id], queryFn: () => api.localReviews(project.id), refetchInterval: (state) => ((state.state.data?.waiting ?? 0) > 0 ? 60_000 : false) });
  const estimate = useQuery({ queryKey: ["local", "reviews-estimate", project.id, depth], queryFn: () => api.localReviewsEstimate(project.id, depth) });
  const ask = useMutation({
    mutationFn: () => api.fetchLocalReviews(project.id, depth),
    onSuccess: () => {
      notify(t("local.reviewsAsked"));
      void queryClient.invalidateQueries({ queryKey: ["local"] });
      refreshSpending(queryClient);
    },
    onError: (thrown) => fail(thrown),
  });
  const data = reviews.data;
  const blocked = blockedText(estimate.data, t);
  const waiting = (data?.waiting ?? 0) > 0 || setup.waiting_reviews > 0;
  const shown = useMemo(() => filterReviews(data?.reviews ?? [], filter, query), [data, filter, query]);
  const summary = data?.summary;
  const most = Math.max(1, ...(summary?.stars ?? [0]));
  return (
    <div className="page-stack">
      <section className="oc-panel report">
        <header className="report-head">
          <div className="report-title">
            <h3 className="oc-panel-title">{t("local.reviewsTitle")}</h3>
            <span className="faint">{t("local.reviewsHint")}</span>
          </div>
          <div className="report-actions">
            <select className="oc-select" value={depth} aria-label={t("local.reviewsDepth")} onChange={(event) => setDepth(Number(event.target.value))}>
              {REVIEW_DEPTHS.map((value) => (
                <option key={value} value={value}>
                  {t("local.reviewsDepthOption", { n: value })}
                </option>
              ))}
            </select>
            {estimate.data && !blocked && <CostLabel estimate={estimate.data} />}
            <Button primary small busy={ask.isPending} disabled={Boolean(blocked) || waiting} title={blocked ?? undefined} onClick={() => ask.mutate()}>
              {t("local.readReviews")}
            </Button>
          </div>
        </header>
        {waiting && (
          <div className="oc-banner info" role="status">
            <Hourglass size={14} aria-hidden="true" />
            <span>{t("local.reviewsWaiting")}</span>
          </div>
        )}
        {blocked && (
          <div className="oc-banner warning" role="status">
            <span>{blocked}</span>
          </div>
        )}
        {summary && summary.count > 0 && (
          <div className="page-stack">
            <div className="oc-stats">
              <Stat label={t("local.average")} value={summary.average === null ? "—" : one(summary.average, locale)} hint={t("local.averageHint", { n: formatNumber(summary.count, locale) })} />
              <Stat label={t("local.unanswered")} value={formatNumber(summary.unanswered, locale)} hint={t("local.unansweredNegative", { n: formatNumber(summary.unanswered_negative, locale) })} tone={summary.unanswered_negative > 0 ? "bad" : undefined} />
              <Stat label={t("local.recent")} value={formatNumber(summary.recent, locale)} hint={t("local.recentHint")} />
            </div>
            <div className="two-columns">
              <section className="sub-panel">
                <h4>{t("local.distribution")}</h4>
                <ol className="bars stars-bars">
                  {[5, 4, 3, 2, 1].map((star) => (
                    <li key={star}>
                      <span className="bar-label">{t("local.stars", { n: star })}</span>
                      <span className="bar-track" aria-hidden="true">
                        <span className="bar-fill" style={{ width: `${((summary.stars[star - 1] ?? 0) / most) * 100}%` }} />
                      </span>
                      <span className="bar-value">{formatNumber(summary.stars[star - 1] ?? 0, locale)}</span>
                    </li>
                  ))}
                </ol>
              </section>
              <section className="sub-panel">
                <h4>{t("local.ratingTrend")}</h4>
                {(data?.ratings.length ?? 0) > 1 ? <RatingChart days={data!.ratings} /> : <p className="faint">{t("local.ratingTrendOne")}</p>}
              </section>
            </div>
          </div>
        )}
      </section>
      <section className="oc-panel report">
        <header className="report-head">
          <div className="report-title">
            <h3 className="oc-panel-title">{t("local.reviewListTitle")}</h3>
          </div>
          <div className="report-actions">
            <Segmented<ReviewFilter>
              label={t("local.reviewFilter")}
              value={filter}
              onChange={setFilter}
              options={[
                { value: "all", label: t("local.filterAll") },
                { value: "unanswered", label: t("local.filterUnanswered") },
                { value: "negative", label: t("local.filterNegative") },
              ]}
            />
          </div>
        </header>
        {(data?.reviews.length ?? 0) === 0 ? (
          <Empty icon={<MessageSquareReply size={26} />} title={t("local.noReviewsTitle")}>
            <p>{t("local.noReviewsText")}</p>
          </Empty>
        ) : (
          <div className="page-stack">
            <div className="filters">
              <label className="filter-search">
                <Search size={15} aria-hidden="true" />
                <span className="sr-only">{t("local.reviewSearch")}</span>
                <input className="oc-input" value={query} placeholder={t("local.reviewSearch")} onChange={(event) => setQuery(event.target.value)} />
              </label>
            </div>
            <ul className="review-list">
              {shown.map((review) => (
                <li key={review.review_id} className={`review${review.rating !== null && review.rating <= 2 ? " negative" : ""}`}>
                  <div className="review-head">
                    <Stars value={review.rating} />
                    <strong>{review.author ?? t("local.anonymous")}</strong>
                    {review.local_guide && <span className="oc-chip small">{t("local.localGuide")}</span>}
                    {review.photos > 0 && (
                      <span className="faint small">
                        <Camera size={12} aria-hidden="true" /> {review.photos}
                      </span>
                    )}
                    <span className="faint small">{review.published_at ? relativeTime(review.published_at, locale) : ""}</span>
                  </div>
                  <p className="review-text">{review.text ?? <span className="faint">{t("local.noText")}</span>}</p>
                  {review.owner_answer ? (
                    <div className="review-answer">
                      <span className="faint small">{t("local.yourAnswer")}</span>
                      <p>{review.owner_answer}</p>
                    </div>
                  ) : (
                    <Button small className="ghost" onClick={() => setReplying(review)}>
                      <MessageSquareReply size={14} />
                      {t("local.draftReply")}
                    </Button>
                  )}
                </li>
              ))}
            </ul>
            {shown.length === 0 && <p className="faint">{t("overview.emptyRows")}</p>}
          </div>
        )}
      </section>
      {replying && <ReplyDialog projectId={project.id} review={replying} business={setup.business?.title ?? project.name} onClose={() => setReplying(null)} />}
    </div>
  );
}
