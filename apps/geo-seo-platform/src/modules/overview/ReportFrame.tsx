import type { ReactNode } from "react";
import type { Fetched, OverviewReport, ReportData } from "../../api/types";
import { PaidFrame } from "../../lib/PaidFrame";

/** A Site Explorer report of a project's domain (or another one in its market). */
export function ReportFrame<R extends OverviewReport>({
  report,
  projectId,
  domain,
  title,
  hint,
  limit,
  onLimit,
  children,
}: {
  report: R;
  projectId: number;
  domain: string | undefined;
  title: string;
  hint?: string;
  limit?: number;
  onLimit?: (limit: number) => void;
  children: (data: ReportData[R], fetched: Fetched<ReportData[R]>) => ReactNode;
}) {
  return (
    <PaidFrame<ReportData[R]>
      module="overview"
      ask={{ project_id: projectId, report, ...(domain ? { domain } : {}) }}
      title={title}
      hint={hint}
      limit={limit}
      onLimit={onLimit}
    >
      {children}
    </PaidFrame>
  );
}
