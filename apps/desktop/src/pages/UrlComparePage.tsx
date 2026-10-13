import { useMemo, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Download, FileUp, GitCompare, Link, X } from "lucide-react";

import { api } from "../api/commands";
import type { AnalysisTable, CommandError, QueryMode, UrlListReport } from "../api/types";
import { DataGrid } from "../components/DataGrid";
import { useToast } from "../components/Toasts";
import { Button, Card, Checkbox, ErrorBanner, Field, Kpi, Segmented, Select, TextInput } from "../components/ui";
import { formatNumber } from "../lib/format";
import { TRAFFIC_PRESETS, type TrafficPreset } from "../lib/queryBuilder";
import { useStored } from "../lib/storage";
import { useI18n } from "../i18n";

type Presence = "all" | "both" | "not-in-logs" | "not-in-list";

export function UrlComparePage({ site }: { site: string }) {
  const { t, locale } = useI18n();
  const notify = useToast();
  // Remembered per site: crawl exports and sitemaps are usually compared again later.
  const [sources, setSources] = useStored<string[]>(`urls-sources-${site}`, []);
  const [url, setUrl] = useState("");
  const [traffic, setTraffic] = useStored<TrafficPreset>("urls-traffic", "search-verified");
  const [queryMode, setQueryMode] = useStored<QueryMode>("urls-query", "drop-tracking");
  const [trailingSlash, setTrailingSlash] = useStored("urls-slash", false);
  const [ignoreCase, setIgnoreCase] = useStored("urls-case", false);
  const [host, setHost] = useState("");
  const [presence, setPresence] = useState<Presence>("all");

  const compare = useMutation<AnalysisTable<UrlListReport>, CommandError>({
    mutationFn: () =>
      api.compareUrls(
        site,
        sources,
        TRAFFIC_PRESETS[traffic],
        { query: queryMode, ignore_trailing_slash: trailingSlash, ignore_case: ignoreCase },
        host.trim() || null,
      ),
  });

  const addFiles = async () => {
    const picked = await open({
      multiple: true,
      filters: [{ name: "URL lists", extensions: ["txt", "csv", "tsv", "xml", "gz"] }],
    });
    if (!picked) return;
    const list = Array.isArray(picked) ? picked : [picked];
    setSources([...new Set([...sources, ...list])]);
  };

  const addUrl = () => {
    const value = url.trim();
    if (!value) return;
    setSources([...new Set([...sources, value])]);
    setUrl("");
  };

  const data = compare.data;
  const presenceIndex = data?.columns.findIndex((c) => c.name === "presence") ?? -1;
  const rows = useMemo(
    () => (data && presence !== "all" ? data.rows.filter((row) => row[presenceIndex] === presence) : data?.rows ?? []),
    [data, presence, presenceIndex],
  );

  const exportRows = async () => {
    if (!data) return;
    const path = await save({
      defaultPath: `${site}-url-comparison.xlsx`,
      filters: [
        { name: "Excel", extensions: ["xlsx"] },
        { name: "CSV", extensions: ["csv"] },
        { name: "JSON", extensions: ["json"] },
      ],
    });
    if (!path) return;
    try {
      await api.saveTable(path, { columns: data.columns, rows });
      notify("success", t("common.saved", { path }));
    } catch (error) {
      notify("error", (error as CommandError).message);
    }
  };

  const n = (value: number) => formatNumber(value, locale);
  return (
    <div className="page">
      <Card title={t("urls.title")}>
        <div className="stack">
          <p className="muted">{t("urls.intro")}</p>
          <div className="row">
            <Button icon={<FileUp size={15} />} onClick={() => void addFiles()}>
              {t("urls.addFile")}
            </Button>
            <TextInput
              className="input grow"
              value={url}
              placeholder={t("urls.urlPlaceholder")}
              onChange={(e) => setUrl(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addUrl()}
            />
            <Button icon={<Link size={15} />} disabled={!url.trim()} onClick={addUrl}>
              {t("urls.addUrl")}
            </Button>
          </div>
          {sources.length > 0 && (
            <div className="chips">
              {sources.map((source) => (
                <span className="chip" key={source} title={source}>
                  <span>{source}</span>
                  <button type="button" onClick={() => setSources(sources.filter((s) => s !== source))}>
                    <X size={12} />
                  </button>
                </span>
              ))}
            </div>
          )}
          <div className="row">
            <Field label={t("urls.traffic")}>
              <Select value={traffic} onChange={(e) => setTraffic(e.target.value as TrafficPreset)}>
                {(Object.keys(TRAFFIC_PRESETS) as TrafficPreset[]).map((preset) => (
                  <option key={preset} value={preset}>
                    {t(`traffic.${preset}`)}
                  </option>
                ))}
              </Select>
            </Field>
            <Field label={t("urls.queryMode")}>
              <Select value={queryMode} onChange={(e) => setQueryMode(e.target.value as QueryMode)}>
                {(["drop-tracking", "keep", "drop-all"] as QueryMode[]).map((mode) => (
                  <option key={mode} value={mode}>
                    {t(`urls.query.${mode}`)}
                  </option>
                ))}
              </Select>
            </Field>
            <Field label={t("urls.host")}>
              <TextInput value={host} placeholder="www.example.com" onChange={(e) => setHost(e.target.value)} />
            </Field>
          </div>
          <div className="row">
            <Checkbox checked={trailingSlash} onChange={setTrailingSlash}>
              {t("urls.trailingSlash")}
            </Checkbox>
            <Checkbox checked={ignoreCase} onChange={setIgnoreCase}>
              {t("urls.ignoreCase")}
            </Checkbox>
            <span className="grow" />
            <Button
              variant="primary"
              icon={<GitCompare size={15} />}
              busy={compare.isPending}
              disabled={sources.length === 0}
              onClick={() => compare.mutate()}
            >
              {t("urls.run")}
            </Button>
          </div>
          <ErrorBanner error={compare.error} />
        </div>
      </Card>
      {data && (
        <>
          <div className="grid-kpi">
            <Kpi label={t("urls.listed")} value={n(data.report.summary.listed)} />
            <Kpi label={t("urls.logged")} value={n(data.report.summary.logged)} />
            <Kpi label={t("urls.both")} value={n(data.report.summary.both)} tone="success" />
            <Kpi label={t("urls.notInLogs")} value={n(data.report.summary.not_in_logs)} />
            <Kpi label={t("urls.notInList")} value={n(data.report.summary.not_in_list)} tone="danger" />
          </div>
          {data.report.skipped > 0 && <p className="muted small">{t("urls.skipped", { count: n(data.report.skipped) })}</p>}
          <div className="row between">
            <Segmented<Presence>
              value={presence}
              onChange={setPresence}
              options={[
                { value: "all", label: t("common.all") },
                { value: "both", label: t("urls.both") },
                { value: "not-in-logs", label: t("urls.notInLogs") },
                { value: "not-in-list", label: t("urls.notInList") },
              ]}
            />
            <Button icon={<Download size={15} />} onClick={() => void exportRows()}>
              {t("reports.export")}
            </Button>
          </div>
          <div style={{ height: 520, display: "flex" }}>
            <DataGrid columns={data.columns} rows={rows} />
          </div>
        </>
      )}
    </div>
  );
}
