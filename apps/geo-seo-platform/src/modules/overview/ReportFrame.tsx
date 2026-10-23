import type { ReactNode } from "react";
import type { Fetched, OverviewReport, ReportData } from "../../api/types";
import { useShell } from "../../lib/context";
import { PaidFrame } from "../../lib/PaidFrame";
import type { ListQuery } from "../../lib/query";

/** A Site Explorer report of a project's domain (or another one), in the project's market or another. */
export function ReportFrame<R extends OverviewReport>({
  report,
  projectId,
  domain,
  market,
  more,
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
  report: R;
  projectId: number;
  domain: string | undefined;
  /** Another market than the project's: `country-language`; the address's by default. */
  market?: string;
  /** More of the question (months of history, the keyword set, domains to compare). */
  more?: Record<string, unknown>;
  title: string;
  hint?: string;
  limit?: number;
  onLimit?: (limit: number) => void;
  /** More controls in the header. */
  extra?: ReactNode;
  query?: ListQuery;
  toolbar?: ReactNode;
  onOffset?: (offset: number) => void;
  children: (data: ReportData[R], fetched: Fetched<ReportData[R]>) => ReactNode;
}) {
  const { params } = useShell();
  const asked = market ?? params.market;
  return (
    <PaidFrame<ReportData[R]>
      module="overview"
      ask={{ project_id: projectId, report, ...(domain ? { domain } : {}), ...(asked ? { market: asked } : {}), ...more }}
      title={title}
      hint={hint}
      limit={limit}
      onLimit={onLimit}
      extra={extra}
      query={query}
      toolbar={toolbar}
      onOffset={onOffset}
    >
      {children}
    </PaidFrame>
  );
}
