import { useEffect, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { save } from "@tauri-apps/plugin-dialog";
import { CalendarRange, Download } from "lucide-react";

import { api } from "../api/commands";
import type { AnalysisTable, CommandError, PeriodReport } from "../api/types";
import { DataGrid } from "../components/DataGrid";
import { FilterBuilder } from "../components/FilterBuilder";
import { useFilterForm } from "../components/useFilterForm";
import { useToast } from "../components/Toasts";
import { Button, Card, ErrorBanner, Field, Kpi, Select, TextInput } from "../components/ui";
import { formatNumber } from "../lib/format";
import { TRAFFIC_PRESETS, joinQuery, type TrafficPreset } from "../lib/queryBuilder";
import { useI18n } from "../i18n";
import { keys } from "../state";

const GROUPS = ["url", "directory", "path", "bot", "bot_category", "status", "status_class", "ua", "host"];

function isoDay(date: Date): string {
  return date.toISOString().slice(0, 10);
}

function shift(day: string, days: number): string {
  const date = new Date(`${day}T00:00:00Z`);
  date.setUTCDate(date.getUTCDate() + days);
  return isoDay(date);
}

interface Range {
  from: string;
  to: string;
}

export function PeriodsPage({ site }: { site: string }) {
  const { t, locale } = useI18n();
  const notify = useToast();
  const overview = useQuery({ queryKey: keys.overview(site), queryFn: () => api.siteOverview(site) });
  const [before, setBefore] = useState<Range>({ from: "", to: "" });
  const [after, setAfter] = useState<Range>({ from: "", to: "" });
  const [group, setGroup] = useState("url");
  const [traffic, setTraffic] = useState<TrafficPreset>("search-verified");
  const [filter, setFilter] = useState("");
  const { form, changeForm, schema, kinds } = useFilterForm(filter, setFilter);

  // Default: the last 30 days of stored data against the 30 days before.
  useEffect(() => {
    if (after.to || !overview.data) return;
    const last = overview.data.last_request ? overview.data.last_request.slice(0, 10) : isoDay(new Date());
    setAfter({ from: shift(last, -29), to: last });
    setBefore({ from: shift(last, -59), to: shift(last, -30) });
  }, [overview.data, after.to]);

  const compare = useMutation<AnalysisTable<PeriodReport>, CommandError>({
    mutationFn: () =>
      api.comparePeriods(
        site,
        `${before.from}..${before.to}`,
        `${after.from}..${after.to}`,
        group,
        joinQuery(TRAFFIC_PRESETS[traffic], filter) || null,
      ),
  });

  const exportRows = async () => {
    if (!compare.data) return;
    const path = await save({
      defaultPath: `${site}-periods-${group}.xlsx`,
      filters: [
        { name: "Excel", extensions: ["xlsx"] },
        { name: "CSV", extensions: ["csv"] },
        { name: "JSON", extensions: ["json"] },
      ],
    });
    if (!path) return;
    try {
      await api.saveTable(path, { columns: compare.data.columns, rows: compare.data.rows });
      notify("success", t("common.saved", { path }));
    } catch (error) {
      notify("error", (error as CommandError).message);
    }
  };

  const ready = before.from && before.to && after.from && after.to;
  const summary = compare.data?.report.summary;
  const n = (value: number) => formatNumber(value, locale);
  return (
    <div className="page">
      <Card title={t("periods.title")}>
        <div className="stack">
          <div className="row">
            <Field label={`${t("periods.before")} · ${t("reports.from")}`}>
              <TextInput type="date" value={before.from} onChange={(e) => setBefore({ ...before, from: e.target.value })} />
            </Field>
            <Field label={t("reports.to")}>
              <TextInput type="date" value={before.to} onChange={(e) => setBefore({ ...before, to: e.target.value })} />
            </Field>
            <Field label={`${t("periods.after")} · ${t("reports.from")}`}>
              <TextInput type="date" value={after.from} onChange={(e) => setAfter({ ...after, from: e.target.value })} />
            </Field>
            <Field label={t("reports.to")}>
              <TextInput type="date" value={after.to} onChange={(e) => setAfter({ ...after, to: e.target.value })} />
            </Field>
          </div>
          <div className="row">
            <Field label={t("periods.group")}>
              <Select value={group} onChange={(e) => setGroup(e.target.value)}>
                {GROUPS.map((g) => (
                  <option key={g} value={g}>
                    {g}
                  </option>
                ))}
              </Select>
            </Field>
            <Field label={t("reports.traffic")}>
              <Select value={traffic} onChange={(e) => setTraffic(e.target.value as TrafficPreset)}>
                {(Object.keys(TRAFFIC_PRESETS) as TrafficPreset[]).map((preset) => (
                  <option key={preset} value={preset}>
                    {t(`traffic.${preset}`)}
                  </option>
                ))}
              </Select>
            </Field>
            <span className="grow" />
            <Button
              variant="primary"
              icon={<CalendarRange size={15} />}
              busy={compare.isPending}
              disabled={!ready}
              onClick={() => compare.mutate()}
            >
              {t("periods.run")}
            </Button>
          </div>
          <div className="field">
            <span>{t("periods.filter")}</span>
            <FilterBuilder
              form={form}
              schema={schema}
              kinds={kinds}
              onChange={changeForm}
              onReset={() => setFilter("")}
              site={site}
              context={joinQuery(TRAFFIC_PRESETS[traffic], after.from && after.to ? `date:${after.from}..${after.to}` : null) || null}
            />
          </div>
          <Field label={t("builder.text")}>
            <TextInput className="input mono" value={filter} placeholder="status:4xx" onChange={(e) => setFilter(e.target.value)} />
          </Field>
          <ErrorBanner error={compare.error} />
        </div>
      </Card>
      {summary && compare.data && (
        <>
          <div className="grid-kpi">
            <Kpi label={t("periods.summary", { before: n(summary.before_hits), after: n(summary.after_hits) })} value={`${summary.after_hits >= summary.before_hits ? "+" : ""}${n(summary.after_hits - summary.before_hits)}`} />
            <Kpi label={t("periods.new")} value={n(summary.new)} tone="success" />
            <Kpi label={t("periods.lost")} value={n(summary.lost)} tone="danger" />
            <Kpi label={t("periods.up")} value={n(summary.up)} />
            <Kpi label={t("periods.down")} value={n(summary.down)} />
          </div>
          <div className="row end">
            <Button icon={<Download size={15} />} onClick={() => void exportRows()}>
              {t("reports.export")}
            </Button>
          </div>
          <div style={{ height: 520, display: "flex" }}>
            <DataGrid columns={compare.data.columns} rows={compare.data.rows} />
          </div>
        </>
      )}
    </div>
  );
}
