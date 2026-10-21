import type { ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { RefreshCw } from "lucide-react";
import { Button, Empty, ErrorState, Loading, Segmented } from "@octopusseo/ui";
import { paid, type PaidModule } from "../api/client";
import type { Estimate, Fetched } from "../api/types";
import { useI18n } from "../i18n";
import { useShell } from "./context";
import { explain } from "./errors";
import { formatCredits, formatUsd, relativeTime } from "./format";

export const ROW_CHOICES = [100, 500, 1000] as const;

/** What fetching would cost, in the way it would be paid. */
export function CostLabel({ estimate }: { estimate: Estimate }) {
  const { t, locale } = useI18n();
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

/**
 * One paid report: what is kept, what fetching costs, the fetch button and the rows choice; the
 * report's own view renders the data. `ask` is the question as the module's commands take it.
 */
export function PaidFrame<T>({
  module,
  ask,
  title,
  hint,
  limit,
  onLimit,
  extra,
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
  children: (data: T, fetched: Fetched<T>) => ReactNode;
}) {
  const { t, locale } = useI18n();
  const { notify, go } = useShell();
  const queryClient = useQueryClient();
  const question = { ...ask, ...(limit ? { limit } : {}) };
  const key = [module, JSON.stringify(question)];
  const kept = useQuery({ queryKey: key, queryFn: () => paid.kept<T>(module, question) });
  const estimate = useQuery({ queryKey: [`${module}-estimate`, ...key.slice(1)], queryFn: () => paid.estimate(module, question) });
  const fetch = useMutation({
    mutationFn: (force: boolean) => paid.fetch<T>(module, { ...question, force }),
    onSuccess: (fetched) => {
      queryClient.setQueryData(key, fetched);
      void queryClient.invalidateQueries({ queryKey: [`${module}-estimate`] });
      refreshSpending(queryClient);
      if (fetched.cached) notify(t("overview.freeFromCache"));
      else notify(t("overview.spentNow", { amount: formatUsd(fetched.spent_usd, locale) }));
    },
    onError: (thrown) => notify(explain(thrown, t).message, true),
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
      {estimate.error && <div className="oc-banner error">{explain(estimate.error, t).message}</div>}
      {kept.isLoading && <Loading label={t("common.loading")} />}
      {kept.error && !estimate.error && <ErrorState title={t("common.loadFailed")} message={explain(kept.error, t).message} retry={() => void kept.refetch()} retryLabel={t("common.retry")} />}
      {kept.isSuccess && !fetched && (
        <Empty title={t("overview.reportNothing")}>
          <p>{t("overview.reportNothingText")}</p>
        </Empty>
      )}
      {fetched && children(fetched.data, fetched)}
    </section>
  );
}
