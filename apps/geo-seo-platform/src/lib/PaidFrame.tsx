import type { ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight, LifeBuoy, RefreshCw } from "lucide-react";
import { Button, Empty, Loading, Segmented } from "@octopusseo/ui";
import { paid, type PaidModule } from "../api/client";
import type { Estimate, Fetched } from "../api/types";
import { useI18n } from "../i18n";
import { useShell } from "./context";
import { explain } from "./errors";
import { formatCredits, formatNumber, formatUsd, relativeTime } from "./format";
import { askedQuery, isEmptyQuery, pageRange, type ListQuery } from "./query";

export const ROW_CHOICES = [100, 500, 1000] as const;

/** What fetching would cost, in the way it would be paid. */
export function CostLabel({ estimate }: { estimate: Estimate }) {
  const { t, locale } = useI18n();
  const shared = estimate.shared;
  if (estimate.via === "credit" && shared && shared.full_credits > 0) {
    // Someone asked the same shortly before: the gateway sells its kept answer at a discount.
    return (
      <span className="cost-label shared">
        {t("overview.costShared", {
          credits: formatCredits(shared.credits, locale),
          full: formatCredits(shared.full_credits, locale),
          pct: Math.round((1 - shared.credits / shared.full_credits) * 100),
        })}
      </span>
    );
  }
  if (estimate.via === "credit" && estimate.credits !== null) {
    return <span className="cost-label">{t("overview.costCredits", { credits: formatCredits(estimate.credits, locale) })}</span>;
  }
  return <span className="cost-label">{t("overview.costOwn", { usd: formatUsd(estimate.usd, locale) })}</span>;
}

/** Why a question cannot be asked now, in the reader's language. */
export function blockedText(estimate: Estimate | undefined, t: (key: string) => string): string | null {
  const reason = estimate?.blocked;
  if (!reason) return null;
  return reason.code === "budget" ? `${reason.message} ${t("common.errorBudget")}` : t("common.errorNoSource");
}

/** Queries of every paid report, refreshed after spending. */
export function refreshSpending(queryClient: ReturnType<typeof useQueryClient>) {
  void queryClient.invalidateQueries({ queryKey: ["account"] });
  void queryClient.invalidateQueries({ queryKey: ["providers"] });
  void queryClient.invalidateQueries({ queryKey: ["usage"] });
}

/** A failure in words, with a retry and, when support can act on it, a report to support. */
export function ProblemBanner({ error, retry }: { error: unknown; retry?: () => void }) {
  const { t } = useI18n();
  const { report } = useShell();
  const explained = explain(error, t);
  return (
    <div className="oc-banner error" role="alert">
      <span>{explained.message}</span>
      {retry && (
        <Button small onClick={retry}>
          {t("common.retry")}
        </Button>
      )}
      {explained.support && (
        <Button small onClick={() => report(explained.support?.detail ?? "")}>
          <LifeBuoy size={14} />
          {t("common.reportProblem")}
        </Button>
      )}
    </div>
  );
}

/** Pages of a list DataForSEO holds more rows of than one question returns. */
function ServerPager({ offset, rows, total, onOffset }: { offset: number; rows: number; total: number; onOffset: (offset: number) => void }) {
  const { t, locale } = useI18n();
  const { from, to } = pageRange(offset, rows, total);
  if (total <= rows && offset === 0) return null;
  return (
    <nav className="server-pager" aria-label={t("query.pages")}>
      <span>{t("query.pageOf", { from: formatNumber(from, locale), to: formatNumber(to, locale), total: formatNumber(total, locale) })}</span>
      <Button small disabled={offset === 0} onClick={() => onOffset(Math.max(0, offset - rows))}>
        <ChevronLeft size={14} aria-hidden="true" />
        {t("query.previousRows", { n: formatNumber(rows, locale) })}
      </Button>
      <Button small disabled={offset + rows >= total} onClick={() => onOffset(offset + rows)}>
        {t("query.nextRows", { n: formatNumber(rows, locale) })}
        <ChevronRight size={14} aria-hidden="true" />
      </Button>
    </nav>
  );
}

/**
 * One paid report: what is kept, what fetching costs, the fetch button and the rows choice; the
 * report's own view renders the data. `ask` is the question as the module's commands take it;
 * `query` adds a list's filters, order and page, asked of the source.
 */
export function PaidFrame<T>({
  module,
  ask,
  title,
  hint,
  limit,
  onLimit,
  extra,
  query,
  toolbar,
  onOffset,
  children,
}: {
  module: PaidModule;
  ask: Record<string, unknown>;
  title: string;
  hint?: string;
  limit?: number;
  onLimit?: (limit: number) => void;
  /** More controls in the header (device choice …). */
  extra?: ReactNode;
  /** A list's filters, order and first row, asked of the source. */
  query?: ListQuery;
  /** Shown under the header whether or not anything is kept (filters). */
  toolbar?: ReactNode;
  /** Pages through every row the source holds; `limit` rows a page. */
  onOffset?: (offset: number) => void;
  children: (data: T, fetched: Fetched<T>) => ReactNode;
}) {
  const { t, locale } = useI18n();
  const { notify, go, fail } = useShell();
  const queryClient = useQueryClient();
  const question = { ...ask, ...(limit ? { limit } : {}), ...askedQuery(query) };
  const key = [module, JSON.stringify(question)];
  const kept = useQuery({ queryKey: key, queryFn: () => paid.kept<T>(module, question) });
  const estimate = useQuery({ queryKey: [`${module}-estimate`, ...key.slice(1)], queryFn: () => paid.estimate(module, question) });
  const fetch = useMutation({
    mutationFn: (force: boolean) => paid.fetch<T>(module, { ...question, force }),
    onSuccess: (fetched) => {
      queryClient.setQueryData(key, fetched);
      void queryClient.invalidateQueries({ queryKey: [`${module}-estimate`] });
      refreshSpending(queryClient);
      // Paid keywords leave their ads in the ad history.
      if (module === "overview") void queryClient.invalidateQueries({ queryKey: ["ads"] });
      const saved = fetched.saved_usd ?? 0;
      if (fetched.cached) notify(t("overview.freeFromCache"));
      else if (saved > 0)
        notify(
          t("overview.spentShared", {
            amount: formatUsd(fetched.spent_usd, locale),
            pct: Math.round((saved / (saved + fetched.spent_usd)) * 100),
          }),
        );
      else notify(t("overview.spentNow", { amount: formatUsd(fetched.spent_usd, locale) }));
    },
    onError: (thrown) => fail(thrown),
  });
  const fetched = kept.data ?? null;
  const blocked = blockedText(estimate.data, t);
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{title}</h3>
          {hint && <span className="faint">{hint}</span>}
        </div>
        <div className="report-actions">
          {fetched && (
            <span className={`oc-badge ${fetched.fresh ? "good" : "warning"}`}>
              {fetched.fresh ? t("common.fresh") : t("common.stale")} · {t("common.fetchedAt", { time: relativeTime(fetched.fetched_at, locale) })}
            </span>
          )}
          {extra}
          {onLimit && limit !== undefined && (
            <Segmented<string>
              label={t("overview.rows")}
              value={String(limit)}
              onChange={(value) => onLimit(Number(value))}
              options={ROW_CHOICES.map((rows) => ({ value: String(rows), label: String(rows) }))}
            />
          )}
          {estimate.data && !blocked && <CostLabel estimate={estimate.data} />}
          <Button primary small busy={fetch.isPending} disabled={Boolean(blocked) || estimate.isLoading || Boolean(estimate.error)} onClick={() => fetch.mutate(Boolean(fetched))} title={blocked ?? undefined}>
            {fetched && <RefreshCw size={14} />}
            {fetched ? t("overview.refresh") : t("overview.fetch")}
          </Button>
        </div>
      </header>
      {blocked && (
        <div className="oc-banner warning" role="status">
          <span>{blocked}</span>
          <Button small onClick={() => go({ page: "sources" })}>
            {t("common.goToSources")}
          </Button>
        </div>
      )}
      {toolbar}
      {estimate.error && <ProblemBanner error={estimate.error} />}
      {kept.isLoading && <Loading label={t("common.loading")} />}
      {kept.error && !estimate.error && <ProblemBanner error={kept.error} retry={() => void kept.refetch()} />}
      {kept.isSuccess && !fetched && (
        <Empty title={isEmptyQuery(query) ? t("overview.reportNothing") : t("query.nothing")}>
          <p>{isEmptyQuery(query) ? t("overview.reportNothingText") : t("query.nothingText")}</p>
        </Empty>
      )}
      {fetched && children(fetched.data, fetched)}
      {fetched && onOffset && limit && fetched.total !== undefined && (
        <ServerPager offset={query?.offset ?? 0} rows={limit} total={fetched.total} onOffset={onOffset} />
      )}
    </section>
  );
}
