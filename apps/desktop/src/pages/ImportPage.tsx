import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { FileSearch, FileUp, FolderOpen, Square, Trash2, X } from "lucide-react";

import { api } from "../api/commands";
import type { CommandError, FileImportReport, ImportEvent, ImportReport, InspectReport } from "../api/types";
import { ConfirmDialog } from "../components/Modal";
import { SiteSettingsCard } from "../components/SiteSettingsCard";
import { useToast } from "../components/Toasts";
import { Badge, Button, Card, Checkbox, ErrorBanner, Spinner } from "../components/ui";
import { formatBytes, formatDateTime, formatDuration, formatNumber, formatPercent } from "../lib/format";
import { describeSpec, formatInputFromSpec } from "../lib/formatSpec";
import { useStored } from "../lib/storage";
import { useI18n } from "../i18n";
import { keys } from "../state";

interface Progress {
  path: string;
  index: number;
  total: number;
  lines: number;
  bytes: number;
  startedAt: number;
}

export function ImportPage({ site }: { site: string }) {
  const { t, locale } = useI18n();
  const notify = useToast();
  const queryClient = useQueryClient();
  const overview = useQuery({ queryKey: keys.overview(site), queryFn: () => api.siteOverview(site) });
  const [paths, setPaths] = useState<string[]>([]);
  const [dns, setDns] = useStored("import-dns", true);
  const [over, setOver] = useState(false);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [finished, setFinished] = useState<FileImportReport[]>([]);
  const [deleting, setDeleting] = useState<{ id: number; path: string } | null>(null);
  const importId = useRef<string | null>(null);

  // Native drag and drop delivers file system paths (the browser API does not).
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    try {
      getCurrentWebview()
        .onDragDropEvent((event) => {
          if (event.payload.type === "over" || event.payload.type === "enter") setOver(true);
          else if (event.payload.type === "leave") setOver(false);
          else if (event.payload.type === "drop") {
            setOver(false);
            const dropped = event.payload.paths;
            setPaths((current) => [...new Set([...current, ...dropped])]);
          }
        })
        .then((stop) => {
          if (cancelled) stop();
          else unlisten = stop;
        })
        .catch(() => undefined);
    } catch {
      // Outside the desktop shell (browser preview) there is no native drag and drop.
    }
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const choose = async (directory: boolean) => {
    const picked = await open({ multiple: true, directory });
    if (!picked) return;
    const list = Array.isArray(picked) ? picked : [picked];
    setPaths((current) => [...new Set([...current, ...list])]);
  };

  const onEvent = (event: ImportEvent) => {
    switch (event.event) {
      case "fileStarted":
        setProgress({ path: event.path, index: event.index, total: event.total, lines: 0, bytes: 0, startedAt: Date.now() });
        break;
      case "progress":
        setProgress((current) => (current ? { ...current, lines: event.lines, bytes: event.bytes } : current));
        break;
      case "fileFinished":
        setFinished((current) => [...current, event.report]);
        break;
    }
  };

  const runImport = useMutation<ImportReport, CommandError>({
    mutationFn: () => {
      const id = crypto.randomUUID();
      importId.current = id;
      setFinished([]);
      setProgress(null);
      return api.importLogs(id, site, paths, dns, onEvent);
    },
    onSuccess: (report) => {
      setPaths([]);
      notify(
        "success",
        t("import.done", {
          inserted: formatNumber(report.inserted_rows, locale),
          duplicates: formatNumber(report.duplicate_rows, locale),
          time: formatDuration(report.elapsed_ms, locale),
        }),
      );
    },
    onError: (error) => {
      if (error.kind === "cancelled") notify("info", t("import.cancelled"));
    },
    onSettled: async () => {
      importId.current = null;
      setProgress(null);
      await queryClient.invalidateQueries({ queryKey: keys.site(site) });
    },
  });

  const removeImport = useMutation<void, CommandError, number>({
    mutationFn: (id) => api.deleteImport(site, id),
    onSuccess: async () => {
      setDeleting(null);
      notify("success", t("import.deleted"));
      await queryClient.invalidateQueries({ queryKey: keys.site(site) });
    },
  });

  const inspect = useMutation<InspectReport, CommandError>({
    mutationFn: () => api.inspectFiles(paths, formatInputFromSpec(overview.data?.format ?? "auto")),
  });

  const running = runImport.isPending;
  const elapsed = progress ? Math.max((Date.now() - progress.startedAt) / 1000, 0.001) : 1;

  return (
    <div className="page">
      <Card
        title={t("import.title", { site })}
        actions={
          overview.data && (
            <Badge>{t("import.format", { format: describeSpec(overview.data.format, t("site.formatAuto")) })}</Badge>
          )
        }
      >
        <div className="stack">
          <div className={`oc-dropzone ${over ? "over" : ""}`}>
            <FileUp size={28} />
            <p>{t("import.drop")}</p>
            <div className="row" style={{ justifyContent: "center", marginTop: 10 }}>
              <Button icon={<FileUp size={15} />} disabled={running} onClick={() => void choose(false)}>
                {t("import.chooseFiles")}
              </Button>
              <Button icon={<FolderOpen size={15} />} disabled={running} onClick={() => void choose(true)}>
                {t("import.chooseFolder")}
              </Button>
            </div>
          </div>
          {paths.length > 0 && (
            <div className="stack">
              <div className="row between">
                <strong>{t("import.selected", { count: paths.length })}</strong>
                <Button small variant="ghost" disabled={running} onClick={() => setPaths([])}>
                  {t("import.clear")}
                </Button>
              </div>
              <div className="oc-chips">
                {paths.map((path) => (
                  <span className="oc-chip" key={path} title={path}>
                    <span>{path}</span>
                    <button type="button" disabled={running} onClick={() => setPaths(paths.filter((p) => p !== path))}>
                      <X size={12} />
                    </button>
                  </span>
                ))}
              </div>
            </div>
          )}
          <Checkbox checked={dns} onChange={setDns}>
            {t("import.dns")}
          </Checkbox>
          <div className="row">
            <Button
              variant="primary"
              icon={<FileUp size={15} />}
              busy={running}
              disabled={paths.length === 0}
              onClick={() => runImport.mutate()}
            >
              {t("import.start")}
            </Button>
            <Button
              icon={<FileSearch size={15} />}
              busy={inspect.isPending}
              disabled={paths.length === 0 || running}
              onClick={() => inspect.mutate()}
            >
              {t("import.inspect")}
            </Button>
            {running && (
              <Button
                variant="danger"
                icon={<Square size={14} />}
                onClick={() => importId.current && void api.cancelImport(importId.current)}
              >
                {t("import.cancel")}
              </Button>
            )}
          </div>
          {running && (
            <div className="stack">
              <div className={`oc-progress ${progress ? "" : "indeterminate"}`}>
                <div style={progress ? { width: `${((progress.index + 0.5) / progress.total) * 100}%` } : undefined} />
              </div>
              {progress && (
                <div className="small">
                  <div>{t("import.running", { index: progress.index + 1, total: progress.total })}</div>
                  <div className="mono muted path">{progress.path}</div>
                  <div className="muted">
                    {t("import.progress", {
                      lines: formatNumber(progress.lines, locale),
                      bytes: formatBytes(progress.bytes, locale),
                      rate: formatNumber(progress.lines / elapsed, locale),
                    })}
                  </div>
                </div>
              )}
            </div>
          )}
          <ErrorBanner error={runImport.error?.kind === "cancelled" ? null : runImport.error} />
          <ErrorBanner error={inspect.error} />
          {inspect.data && <InspectResult report={inspect.data} />}
          {finished.length > 0 && <FileResults files={finished} />}
        </div>
      </Card>
      <SiteSettingsCard site={site} />
      <Card title={t("import.history")} actions={overview.isFetching ? <Spinner /> : undefined}>
        <ErrorBanner error={removeImport.error} />
        {(overview.data?.history.length ?? 0) === 0 ? (
          <p className="muted">{t("import.noHistory")}</p>
        ) : (
          <div className="oc-table-wrap">
            <table className="oc-table">
              <thead>
                <tr>
                  <th />
                  <th>{t("import.col.file")}</th>
                  <th>{t("import.col.finished")}</th>
                  <th className="num">{t("import.col.hits")}</th>
                  <th className="num">{t("import.col.duplicates")}</th>
                  <th className="num">{t("import.col.lines")}</th>
                  <th>{t("import.col.range")}</th>
                </tr>
              </thead>
              <tbody>
                {overview.data?.history.map((entry) => (
                  <tr key={entry.id}>
                    <td className="cell-action">
                      <Button
                        small
                        variant="ghost"
                        title={t("import.delete")}
                        icon={<Trash2 size={14} />}
                        disabled={running}
                        onClick={() => setDeleting({ id: entry.id, path: entry.path })}
                      />
                    </td>
                    <td className="path mono" title={entry.path}>
                      {entry.path}
                    </td>
                    <td>{formatDateTime(entry.finished_at, locale)}</td>
                    <td className="num">{formatNumber(entry.inserted_rows, locale)}</td>
                    <td className="num">{formatNumber(entry.duplicate_rows, locale)}</td>
                    <td className="num">
                      {formatNumber(entry.parsed_lines, locale)} / {formatNumber(entry.total_lines, locale)}
                    </td>
                    <td className="small muted">
                      {formatDateTime(entry.first_request, locale)} – {formatDateTime(entry.last_request, locale)}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>
      {deleting && (
        <ConfirmDialog
          title={t("import.delete")}
          message={t("import.deleteConfirm", { path: deleting.path })}
          confirmLabel={t("common.delete")}
          busy={removeImport.isPending}
          onConfirm={() => removeImport.mutate(deleting.id)}
          onClose={() => setDeleting(null)}
        />
      )}
    </div>
  );
}

function FileResults({ files }: { files: FileImportReport[] }) {
  const { t, locale } = useI18n();
  return (
    <table className="oc-table">
      <tbody>
        {files.map((file) => (
          <tr key={file.path}>
            <td className="path mono" title={file.path}>
              {file.path}
            </td>
            <td>
              {file.status === "already_imported" && <Badge>{t("import.alreadyImported")}</Badge>}
              {file.status === "unrecognized" && <Badge tone="danger">{t("import.unrecognized")}</Badge>}
              {file.status === "imported" && (
                <Badge tone="success">
                  {t("import.fileResult", {
                    inserted: formatNumber(file.inserted_rows, locale),
                    skipped: formatNumber(file.total_lines - file.parsed_lines, locale),
                  })}
                </Badge>
              )}
              {file.status === "imported" && file.filtered_lines > 0 && (
                <Badge>{t("import.filtered", { count: formatNumber(file.filtered_lines, locale) })}</Badge>
              )}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function InspectResult({ report }: { report: InspectReport }) {
  const { t, locale } = useI18n();
  const ok = report.parsed_lines > 0;
  return (
    <div className={`oc-banner ${ok ? "info" : "error"}`}>
      <FileSearch size={18} />
      <div className="grow stack">
        <div>
          <strong>
            {t("import.inspectResult", {
              format: report.format_name,
              parsed: formatNumber(report.parsed_lines, locale),
              total: formatNumber(report.total_lines, locale),
            })}
          </strong>
          {report.detection_confidence !== null && (
            <span className="muted"> · {t("import.confidence", { confidence: formatPercent(report.detection_confidence, locale) })}</span>
          )}
        </div>
        {report.error_samples.length > 0 && (
          <div>
            <div className="small">{t("import.skippedExamples")}</div>
            <pre>
              {report.error_samples
                .map((sample) => `${sample.line_number}: ${sample.reason}\n  ${sample.excerpt}`)
                .join("\n")}
            </pre>
          </div>
        )}
      </div>
    </div>
  );
}
