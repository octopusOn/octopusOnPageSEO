import { useMemo, useState } from "react";
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Bookmark, ChevronDown, ChevronRight, Download, FileDown, FileUp, Play, Search, Trash2 } from "lucide-react";

import { api } from "../api/commands";
import type { CommandError, QueryPageResult, QuerySpec, SavedQueryInfo } from "../api/types";
import { DataGrid } from "../components/DataGrid";
import { FilterBuilder } from "../components/FilterBuilder";
import { ConfirmDialog } from "../components/Modal";
import { SaveQueryDialog } from "../components/SaveQueryDialog";
import { useFilterForm } from "../components/useFilterForm";
import { useToast } from "../components/Toasts";
import { Button, ErrorBanner, Field, Select, TextInput } from "../components/ui";
import { formatDuration, formatNumber } from "../lib/format";
import {
  composeQuery,
  currentSort,
  TRAFFIC_PRESETS,
  withSort,
  type DateFilter,
  type DatePreset,
  type TrafficPreset,
} from "../lib/queryBuilder";
import { useStored } from "../lib/storage";
import { useI18n } from "../i18n";
import { keys } from "../state";

const PAGE_SIZE = 500;
const CUSTOM = "__custom__";

export function ReportsPage({ site }: { site: string }) {
  const { t, locale, template: localized } = useI18n();
  const notify = useToast();
  const templates = useQuery({ queryKey: keys.templates, queryFn: api.listTemplates, staleTime: Infinity });
  const [selected, setSelected] = useStored<string>("report-template", "top-urls");
  const [text, setText] = useStored<string>("report-text", "group:url metric:hits,bytes,last_seen sort:hits:desc limit:500");
  const [date, setDate] = useStored<DateFilter>("report-date", { preset: "last30d" });
  const [traffic, setTraffic] = useStored<TrafficPreset>("report-traffic", "all");
  const [search, setSearch] = useState("");
  const [showForm, setShowForm] = useStored("report-form-open", true);
  // Editing the query (form or text) turns a template into a custom query.
  const editText = (next: string) => {
    setText(next);
    setSelected(CUSTOM);
  };
  const { form, changeForm, schema, kinds } = useFilterForm(text, editText);
  const [submitted, setSubmitted] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const queryClient = useQueryClient();
  const savedQueries = useQuery({ queryKey: keys.saved, queryFn: api.listSavedQueries });
  // The saved query last opened; saving under its name updates it without asking.
  const [loaded, setLoaded] = useStored<SavedQueryInfo | null>("report-saved", null);
  const [saving, setSaving] = useState(false);
  const [removing, setRemoving] = useState<string | null>(null);
  const removeSaved = useMutation<void, CommandError, string>({
    mutationFn: (name) => api.removeSavedQuery(name),
    onSuccess: async (_, name) => {
      if (loaded?.name === name) setLoaded(null);
      await queryClient.invalidateQueries({ queryKey: keys.saved });
    },
    onError: (error) => notify("error", error.message),
    onSettled: () => setRemoving(null),
  });

  const query = submitted ?? composeQuery(date, traffic, text);
  const spec: QuerySpec = { kind: "text", text: query };

  const results = useInfiniteQuery<QueryPageResult, CommandError>({
    queryKey: keys.query(site, query),
    queryFn: ({ pageParam }) => api.runQuery(site, spec, pageParam as number, PAGE_SIZE),
    initialPageParam: 0,
    getNextPageParam: (last, pages) => (last.hasMore ? pages.length * PAGE_SIZE : undefined),
    enabled: submitted !== null,
    retry: false,
  });

  const run = (nextText = text) => {
    setText(nextText);
    setSubmitted(composeQuery(date, traffic, nextText));
  };

  const pick = (id: string) => {
    setSelected(id);
    setLoaded(null);
    const found = templates.data?.find((tpl) => tpl.id === id);
    if (found) run(found.query);
  };

  // Saved queries already contain their dates and traffic filters, so the quick filters reset.
  const openSaved = (saved: SavedQueryInfo) => {
    setSelected(`saved:${saved.name}`);
    setLoaded(saved);
    setDate({ preset: "all" });
    setTraffic("all");
    setText(saved.query);
    setSubmitted(composeQuery({ preset: "all" }, "all", saved.query));
  };

  const filteredSaved = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return (savedQueries.data ?? []).filter(
      (q) => !needle || `${q.name} ${q.description} ${q.query}`.toLowerCase().includes(needle),
    );
  }, [savedQueries.data, search]);

  const exportSaved = async () => {
    const path = await save({ defaultPath: "octopuson-saved-queries.json", filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!path) return;
    try {
      const count = await api.exportSavedQueries(path);
      notify("success", t("saved.exported", { count, path }));
    } catch (error) {
      notify("error", (error as CommandError).message);
    }
  };

  const importSaved = async () => {
    const path = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
    if (typeof path !== "string") return;
    try {
      const report = await api.importSavedQueries(path, false);
      await queryClient.invalidateQueries({ queryKey: keys.saved });
      const details = report.skipped.slice(0, 3).map((s) => `${s.name}: ${s.reason}`);
      notify(
        report.skipped.length > 0 ? "info" : "success",
        [
          t("saved.imported", { added: report.added.length, replaced: report.replaced.length, skipped: report.skipped.length }),
          ...details,
        ].join(" · "),
      );
    } catch (error) {
      notify("error", (error as CommandError).message);
    }
  };

  const filteredTemplates = useMemo(() => {
    const needle = search.trim().toLowerCase();
    return (templates.data ?? [])
      .map((tpl) => ({ ...tpl, ...localized(tpl.id, tpl.title, tpl.description) }))
      .filter((tpl) => !needle || `${tpl.title} ${tpl.description} ${tpl.id}`.toLowerCase().includes(needle));
  }, [templates.data, search, localized]);

  const pages = results.data?.pages ?? [];
  const first = pages[0];
  const rows = useMemo(() => pages.flatMap((page) => page.rows), [pages]);

  const exportResults = async () => {
    const path = await save({
      defaultPath: `${site}-${selected === CUSTOM ? "query" : selected}.xlsx`,
      filters: [
        { name: "Excel", extensions: ["xlsx"] },
        { name: "CSV", extensions: ["csv"] },
        { name: "JSON", extensions: ["json"] },
      ],
    });
    if (!path) return;
    setExporting(true);
    try {
      const report = await api.exportQuery(site, spec, path);
      notify("success", t("reports.exported", { rows: formatNumber(report.rows, locale), path: report.path }));
    } catch (error) {
      notify("error", (error as CommandError).message);
    } finally {
      setExporting(false);
    }
  };

  // Changing a quick filter re-runs a query that is already shown.
  const updateFilters = (nextDate: DateFilter, nextTraffic: TrafficPreset) => {
    setDate(nextDate);
    setTraffic(nextTraffic);
    if (submitted !== null) setSubmitted(composeQuery(nextDate, nextTraffic, text));
  };
  const setPreset = (preset: DatePreset) => updateFilters({ ...date, preset }, traffic);

  return (
    <div className="page fill">
      <div className="reports">
        <div className="oc-panel reports-side">
          <div className="stack">
            <div className="row">
              <Search size={15} className="muted" />
              <input
                className="oc-input grow"
                value={search}
                placeholder={t("common.search")}
                onChange={(e) => setSearch(e.target.value)}
              />
            </div>
            <div className="list">
              <div className="list-heading">
                <Bookmark size={12} />
                <span className="grow">{t("saved.title")}</span>
                <Button variant="ghost" small title={t("saved.import")} icon={<FileUp size={13} />} onClick={() => void importSaved()} />
                <Button
                  variant="ghost"
                  small
                  title={t("saved.export")}
                  icon={<FileDown size={13} />}
                  disabled={(savedQueries.data ?? []).length === 0}
                  onClick={() => void exportSaved()}
                />
              </div>
              {(savedQueries.data ?? []).length === 0 && <p className="muted small" style={{ padding: "2px 10px 6px" }}>{t("saved.empty")}</p>}
              {filteredSaved.map((saved) => (
                <div className="list-entry" key={saved.name}>
                  <button
                    type="button"
                    className={`list-item ${selected === `saved:${saved.name}` ? "active" : ""}`}
                    onClick={() => openSaved(saved)}
                    title={saved.query}
                  >
                    <span className="title">{saved.name}</span>
                    <span className="desc">{saved.description}</span>
                    <span className="desc mono">{saved.query}</span>
                  </button>
                  <span className="entry-actions">
                    <Button variant="ghost" small title={t("saved.remove")} icon={<Trash2 size={13} />} onClick={() => setRemoving(saved.name)} />
                  </span>
                </div>
              ))}
              <div className="list-heading">
                <span className="grow">{t("saved.builtIn")}</span>
              </div>
              <button
                type="button"
                className={`list-item ${selected === CUSTOM ? "active" : ""}`}
                onClick={() => setSelected(CUSTOM)}
              >
                <span className="title">{t("reports.custom")}</span>
              </button>
              {filteredTemplates.map((tpl) => (
                <button
                  key={tpl.id}
                  type="button"
                  className={`list-item ${selected === tpl.id ? "active" : ""}`}
                  onClick={() => pick(tpl.id)}
                  title={tpl.query}
                >
                  <span className="title">{tpl.title}</span>
                  <span className="desc">{tpl.description}</span>
                </button>
              ))}
            </div>
          </div>
        </div>
        <div className="reports-main">
          <div className="oc-panel stack">
            <div className="row">
              <Field label={t("reports.date")}>
                <Select value={date.preset} onChange={(e) => setPreset(e.target.value as DatePreset)}>
                  <option value="last7d">{t("overview.last7")}</option>
                  <option value="last30d">{t("overview.last30")}</option>
                  <option value="last90d">{t("overview.last90")}</option>
                  <option value="all">{t("overview.allData")}</option>
                  <option value="custom">{t("reports.custom.dates")}</option>
                </Select>
              </Field>
              {date.preset === "custom" && (
                <>
                  <Field label={t("reports.from")}>
                    <TextInput type="date" value={date.from ?? ""} onChange={(e) => updateFilters({ ...date, from: e.target.value }, traffic)} />
                  </Field>
                  <Field label={t("reports.to")}>
                    <TextInput type="date" value={date.to ?? ""} onChange={(e) => updateFilters({ ...date, to: e.target.value }, traffic)} />
                  </Field>
                </>
              )}
              <Field label={t("reports.traffic")}>
                <Select value={traffic} onChange={(e) => updateFilters(date, e.target.value as TrafficPreset)}>
                  {(Object.keys(TRAFFIC_PRESETS) as TrafficPreset[]).map((preset) => (
                    <option key={preset} value={preset}>
                      {t(`traffic.${preset}`)}
                    </option>
                  ))}
                </Select>
              </Field>
              <span className="grow" />
              <button type="button" className="section-toggle" onClick={() => setShowForm(!showForm)}>
                {showForm ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
                {showForm ? t("builder.hide") : t("builder.show")}
              </button>
            </div>
            {showForm && (
              <div onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLElement).tagName === "INPUT" && run()}>
                <FilterBuilder
                  form={form}
                  schema={schema}
                  kinds={kinds}
                  onChange={changeForm}
                  onReset={() => editText("")}
                  directives
                  site={site}
                  context={composeQuery(date, traffic, "") || null}
                />
              </div>
            )}
            <div className="oc-field">
              <span>{t("builder.text")}</span>
              <div className="row">
                <textarea
                  className="oc-textarea grow"
                  rows={2}
                  value={text}
                  placeholder={t("reports.queryPlaceholder")}
                  title={t("reports.helpText")}
                  onChange={(e) => editText(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) run();
                  }}
                />
                <div className="stack">
                  <Button variant="primary" icon={<Play size={15} />} busy={results.isFetching && !results.isFetchingNextPage} onClick={() => run()}>
                    {t("reports.run")}
                  </Button>
                  <Button icon={<Download size={15} />} busy={exporting} disabled={submitted === null} onClick={exportResults}>
                    {t("reports.export")}
                  </Button>
                  <Button icon={<Bookmark size={15} />} onClick={() => setSaving(true)}>
                    {t("saved.save")}
                  </Button>
                </div>
              </div>
            </div>
          </div>
          <ErrorBanner error={results.error} />
          {first && (
            <DataGrid
              columns={first.outcome.columns}
              rows={rows}
              hasMore={results.hasNextPage}
              loadingMore={results.isFetchingNextPage}
              onLoadMore={() => void results.fetchNextPage()}
              sort={currentSort(text)}
              onSort={(sort) => run(withSort(text, sort.column, sort.direction))}
              footer={
                <>
                  <span className="mono" title={first.outcome.query}>
                    {t("reports.answeredFrom", {
                      source: first.outcome.source,
                      time: formatDuration(first.outcome.elapsed_ms, locale),
                    })}
                  </span>
                  {first.outcome.columns.some((c) => ["country", "asn", "as_org"].includes(c.name)) && (
                    <a className="small muted" href="https://db-ip.com" target="_blank" rel="noreferrer">
                      {t("geo.attribution")}
                    </a>
                  )}
                </>
              }
            />
          )}
        </div>
      </div>
      {saving && (
        <SaveQueryDialog
          query={composeQuery(date, traffic, text)}
          initialName={loaded?.name ?? ""}
          initialDescription={loaded?.description ?? ""}
          onSaved={(saved) => {
            notify("success", t("saved.saved", { name: saved.name }));
            openSaved(saved);
          }}
          onClose={() => setSaving(false)}
        />
      )}
      {removing && (
        <ConfirmDialog
          title={t("saved.remove")}
          message={t("saved.removeConfirm", { name: removing })}
          confirmLabel={t("common.delete")}
          busy={removeSaved.isPending}
          onConfirm={() => removeSaved.mutate(removing)}
          onClose={() => setRemoving(null)}
        />
      )}
    </div>
  );
}
