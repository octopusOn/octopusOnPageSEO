// Links from the project's own site to other sites, as the latest site audit found them, and the
// broken ones when the audit checked them.

import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ExternalLink } from "lucide-react";
import { Button, DataTable, Empty, Segmented, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { ProjectRow } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { Entity } from "../../lib/Entity";
import { formatNumber } from "../../lib/format";
import { tableLabels } from "../../lib/tables";

type StatusClass = "0" | "4" | "5";

interface OutboundRow {
  address: string;
  status: string;
  error: string;
  inlinks: number;
}

export function OutboundTab({ project }: { project: ProjectRow }) {
  const { t, locale } = useI18n();
  const { go } = useShell();
  const [statusClass, setStatusClass] = useState<StatusClass>("0");
  const outbound = useQuery({ queryKey: ["audit-outbound", project.id, statusClass], queryFn: () => api.auditOutbound(project.id, Number(statusClass)) });
  const data = outbound.data;
  const index = (name: string) => data?.columns.indexOf(name) ?? -1;
  const rows: OutboundRow[] = (data?.rows ?? []).map((row) => ({
    address: row[index("address")] ?? "",
    status: row[index("status_code")] ?? "",
    error: row[index("error")] ?? "",
    inlinks: Number(row[index("inlinks")] ?? 0),
  }));
  const columns: Column<OutboundRow>[] = [
    { key: "address", label: t("backlinks.colOutbound"), render: (row) => (
        <Entity kind="url" value={row.address}>
          <span className="cell-url">{row.address.replace(/^https?:\/\/(www\.)?/, "")}</span>
        </Entity>
      ), sort: (row) => row.address },
    { key: "status", label: t("backlinks.colHttp"), render: (row) => <span className={Number(row.status) >= 400 ? "warn" : undefined}>{row.status && row.status !== "null" ? row.status : row.error || "—"}</span>, sort: (row) => Number(row.status) || 0, numeric: true },
    { key: "inlinks", label: t("backlinks.colLinkingPages"), render: (row) => formatNumber(row.inlinks, locale), sort: (row) => row.inlinks, numeric: true },
  ];
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("backlinks.outboundTitle")}</h3>
          <span className="faint">{t("backlinks.outboundHint")}</span>
        </div>
        <div className="report-actions">
          <Segmented<StatusClass>
            label={t("backlinks.colHttp")}
            value={statusClass}
            onChange={setStatusClass}
            options={[
              { value: "0", label: t("backlinks.outboundAll") },
              { value: "4", label: "4xx" },
              { value: "5", label: "5xx" },
            ]}
          />
        </div>
      </header>
      {outbound.isSuccess && !data && (
        <Empty icon={<ExternalLink size={26} />} title={t("backlinks.outboundNone")}>
          <p>{t("backlinks.outboundNoneText")}</p>
          <Button small onClick={() => go({ page: "audit" })}>
            {t("backlinks.goToAudit")}
          </Button>
        </Empty>
      )}
      {data && !data.checked && (
        <div className="oc-banner info" role="status">
          <span>{t("backlinks.outboundUnchecked")}</span>
          <Button small onClick={() => go({ page: "audit" })}>
            {t("backlinks.goToAudit")}
          </Button>
        </div>
      )}
      {data && (
        <>
          <p className="faint small">{t("backlinks.outboundCount", { n: formatNumber(data.total, locale) })}</p>
          <DataTable rows={rows} columns={columns} rowKey={(row) => row.address} initialSort={{ key: "inlinks", direction: "desc" }} empty={t("overview.emptyRows")} labels={tableLabels(t, locale)} />
        </>
      )}
    </section>
  );
}
