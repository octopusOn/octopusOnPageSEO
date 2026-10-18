// robots.txt compliance: bots requesting disallowed URLs and bots the robots.txt blocks.

import { useMemo, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Download, FileUp, ShieldBan } from "lucide-react";

import { api } from "../api/commands";
import type { AnalysisTable, CommandError, RobotsReport } from "../api/types";
import { DataGrid } from "../components/DataGrid";
import { useToast } from "../components/Toasts";
import { Badge, Button, Card, ErrorBanner, Field, Kpi, Select, TextInput } from "../components/ui";
import { formatNumber } from "../lib/format";
import { useStored } from "../lib/storage";
import { useI18n } from "../i18n";
import type { MessageKey } from "../i18n/en";

const PERIODS = ["", "last7d", "last30d", "last90d"] as const;
const PERIOD_LABELS: Record<(typeof PERIODS)[number], MessageKey> = {
  "": "overview.allData",
  last7d: "overview.last7",
  last30d: "overview.last30",
  last90d: "overview.last90",
};

export function RobotsPage({ site }: { site: string }) {
  const { t, locale } = useI18n();
  const notify = useToast();
  const [source, setSource] = useStored(`robots-source-${site}`, "");
  const [period, setPeriod] = useStored<(typeof PERIODS)[number]>("robots-period", "last30d");
  const [verifiedOnly, setVerifiedOnly] = useStored("robots-verified", false);
  const [bot, setBot] = useState("");

  const check = useMutation<AnalysisTable<RobotsReport>, CommandError>({
    mutationFn: () => {
      const filter = [period ? `date:${period}` : "", verifiedOnly ? "verification:verified" : ""].filter(Boolean).join(" ");
      return api.robotsCompliance(site, source.trim() || null, filter || null);
    },
  });
  const data = check.data;
  const rows = useMemo(() => {
    if (!data) return [];
    const botIndex = data.columns.findIndex((c) => c.name === "bot");
    return bot ? data.rows.filter((row) => row[botIndex] === bot) : data.rows;
  }, [data, bot]);

  const pickFile = async () => {
    const picked = await open({ multiple: false, filters: [{ name: "robots.txt", extensions: ["txt"] }] });
    if (typeof picked === "string") setSource(picked);
  };

  const exportRows = async () => {
    if (!data) return;
    const path = await save({ defaultPath: `${site}-robots-violations.xlsx`, filters: [{ name: "Excel", extensions: ["xlsx"] }, { name: "CSV", extensions: ["csv"] }] });
    if (!path) return;
    try {
      await api.saveTable(path, { columns: data.columns, rows });
      notify("success", t("common.saved", { path }));
    } catch (error) {
      notify("error", (error as CommandError).message);
    }
  };

  const n = (value: number) => formatNumber(value, locale);
  const totals = data?.report.bots.reduce(
    (sum, b) => ({ disallowed: sum.disallowed + b.disallowed_hits, verified: sum.verified + b.verified_disallowed_hits }),
    { disallowed: 0, verified: 0 },
  );

  return (
    <div className="page">
      <Card title={t("robots.title")}>
        <div className="stack">
          <p className="muted">{t("robots.intro")}</p>
          <div className="row">
            <Field label={t("robots.source")}>
              <TextInput
                className="oc-input mono"
                style={{ minWidth: 320 }}
                value={source}
                placeholder={`https://${site}/robots.txt`}
                onChange={(e) => setSource(e.target.value)}
              />
            </Field>
            <Button icon={<FileUp size={15} />} onClick={() => void pickFile()}>
              {t("robots.file")}
            </Button>
            <Field label={t("overview.period")}>
              <Select value={period} onChange={(e) => setPeriod(e.target.value as (typeof PERIODS)[number])}>
                {PERIODS.map((p) => (
                  <option key={p} value={p}>
                    {t(PERIOD_LABELS[p])}
                  </option>
                ))}
              </Select>
            </Field>
            <label className="row" style={{ gap: 6, marginTop: 18 }}>
              <input type="checkbox" checked={verifiedOnly} onChange={(e) => setVerifiedOnly(e.target.checked)} />
              {t("robots.verifiedOnly")}
            </label>
            <span className="grow" />
            <Button variant="primary" icon={<ShieldBan size={15} />} busy={check.isPending} onClick={() => check.mutate()}>
              {t("robots.run")}
            </Button>
          </div>
          <ErrorBanner error={check.error} />
        </div>
      </Card>
      {data && totals && (
        <>
          <div className="oc-stats">
            <Kpi label={t("robots.bots")} value={n(data.report.bots.length)} />
            <Kpi label={t("robots.disallowedHits")} value={n(totals.disallowed)} tone={totals.disallowed > 0 ? "danger" : "success"} />
            <Kpi label={t("robots.verifiedDisallowed")} value={n(totals.verified)} tone={totals.verified > 0 ? "danger" : undefined} />
            <Kpi label={t("robots.blockedBots")} value={n(data.report.bots.filter((b) => b.blocked).length)} />
          </div>
          <Card title={t("robots.perBot")}>
            <div className="oc-table-wrap" style={{ maxHeight: 360 }}>
              <table className="oc-table">
                <thead>
                  <tr>
                    <th>{t("bots.name")}</th>
                    <th>{t("bots.category")}</th>
                    <th>{t("robots.group")}</th>
                    <th>{t("robots.blocked")}</th>
                    <th className="num">{t("metric.hits")}</th>
                    <th className="num">{t("robots.disallowedHits")}</th>
                    <th className="num">{t("robots.verifiedDisallowed")}</th>
                    <th className="num">{t("robots.disallowedUrls")}</th>
                  </tr>
                </thead>
                <tbody>
                  {data.report.bots.map((b) => (
                    <tr key={b.bot} className={bot === b.bot ? "selected" : undefined} onClick={() => setBot(bot === b.bot ? "" : b.bot)} style={{ cursor: "pointer" }}>
                      <td>
                        {b.name} <span className="mono muted small">{b.bot}</span>
                      </td>
                      <td>{t(`category.${b.category || "other"}` as MessageKey)}</td>
                      <td className="mono">{b.group ?? "–"}</td>
                      <td>{b.blocked ? <Badge tone="warning">{t("robots.blockedYes")}</Badge> : ""}</td>
                      <td className="num">{n(b.hits)}</td>
                      <td className="num">
                        {b.disallowed_hits > 0 ? <Badge tone={b.user_triggered ? "warning" : "danger"}>{n(b.disallowed_hits)}</Badge> : "0"}
                      </td>
                      <td className="num">{n(b.verified_disallowed_hits)}</td>
                      <td className="num">{n(b.disallowed_urls)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <p className="muted small" style={{ marginTop: 8 }}>
              {t("robots.legend")}
            </p>
          </Card>
          <div className="row between">
            <strong>{bot ? t("robots.violationsOf", { bot }) : t("robots.violations")}</strong>
            <Button icon={<Download size={15} />} disabled={rows.length === 0} onClick={() => void exportRows()}>
              {t("reports.export")}
            </Button>
          </div>
          <div style={{ height: 460, display: "flex" }}>
            <DataGrid columns={data.columns} rows={rows} />
          </div>
          {data.report.sitemaps.length > 0 && (
            <p className="muted small">
              {t("robots.sitemaps")}: <span className="mono">{data.report.sitemaps.join(" · ")}</span>
            </p>
          )}
        </>
      )}
    </div>
  );
}
