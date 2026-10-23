import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Copy, Download, FileSpreadsheet, FileText, Link2, Link2Off, Mail, Palette, Pencil, Plus, Send, Trash2 } from "lucide-react";
import { Button, Empty, ErrorState, Loading } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { ProjectRow, ReportRow, ReportsSetup } from "../../api/types";
import { LANGUAGES as UI_LANGUAGES, useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { explain } from "../../lib/errors";
import { formatNumber, relativeTime } from "../../lib/format";
import { refreshSpending } from "../../lib/PaidFrame";
import { download, downloadBase64 } from "../../lib/report";
import { BrandingDialog, ReportDialog } from "./ReportDialogs";
import { ScaledPage } from "./ScaledPage";
import { shareUrl } from "./report";

/** The report as its page: the same page the PDF and the link show. */
function Preview({ project, report }: { project: ProjectRow; report: ReportRow }) {
  const { t } = useI18n();
  const page = useQuery({ queryKey: ["reports", "html", project.id, report.id, report.updated_at], queryFn: () => api.reportHtml(project.id, report.id) });
  if (page.isLoading) return <Loading label={t("common.loading")} />;
  if (page.error || !page.data) {
    return <ErrorState title={t("common.loadFailed")} message={explain(page.error, t).message} retry={() => void page.refetch()} retryLabel={t("common.retry")} />;
  }
  return <ScaledPage title={t("reports.previewTitle", { name: report.name })} html={page.data.html} />;
}

function ShareBox({ project, setup, report }: { project: ProjectRow; setup: ReportsSetup; report: ReportRow }) {
  const { t, locale } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const done = () => void queryClient.invalidateQueries({ queryKey: ["reports", "setup", project.id] });
  const share = useMutation({ mutationFn: () => api.shareReport(project.id, report.id), onSuccess: done, onError: (thrown) => fail(thrown) });
  const stop = useMutation({
    mutationFn: () => api.unshareReport(project.id, report.id),
    onSuccess: () => {
      notify(t("reports.unshared"));
      done();
    },
    onError: (thrown) => fail(thrown),
  });
  const link = report.share_token ? shareUrl(setup.public_url, report.share_token, window.location.origin) : null;
  return (
    <div className="share-box">
      <h4>{t("reports.shareTitle")}</h4>
      {link ? (
        <>
          <div className="share-link">
            <input className="oc-input" readOnly value={link} onFocus={(event) => event.target.select()} aria-label={t("reports.shareTitle")} />
            <Button
              small
              onClick={() =>
                void navigator.clipboard
                  .writeText(link)
                  .then(() => notify(t("reports.copied")))
                  .catch(() => notify(t("reports.copyFailed"), true))
              }
            >
              <Copy size={14} />
              {t("reports.copy")}
            </Button>
          </div>
          <p className="faint small">{t("reports.shareViews", { n: formatNumber(report.share_views, locale) })}</p>
          <Button small className="ghost" busy={stop.isPending} onClick={() => stop.mutate()}>
            <Link2Off size={14} />
            {t("reports.unshare")}
          </Button>
        </>
      ) : (
        <>
          <p className="faint small">{t("reports.shareHint")}</p>
          <Button small busy={share.isPending} onClick={() => share.mutate()}>
            <Link2 size={14} />
            {t("reports.share")}
          </Button>
        </>
      )}
    </div>
  );
}

function Deliveries({ project, report }: { project: ProjectRow; report: ReportRow }) {
  const { t, locale } = useI18n();
  const deliveries = useQuery({ queryKey: ["reports", "deliveries", project.id, report.id, report.last_sent_at], queryFn: () => api.reportDeliveries(project.id, report.id) });
  if (!deliveries.data?.length) return null;
  return (
    <div className="share-box">
      <h4>{t("reports.deliveriesTitle")}</h4>
      <ul className="run-list compact">
        {deliveries.data.map((delivery) => (
          <li key={delivery.id}>
            <span className={`oc-badge ${delivery.status === "sent" ? "good" : "critical"}`}>{t(`reports.delivery_${delivery.status}`)}</span>
            <span>{relativeTime(delivery.sent_at, locale)}</span>
            <span className="faint small">{delivery.scheduled ? t("reports.scheduledSend") : t("reports.manualSend")}</span>
            {delivery.error && <span className="faint small run-error">{delivery.error}</span>}
          </li>
        ))}
      </ul>
    </div>
  );
}

/** One report: what it shows, its files, its link and its e-mails. */
function ReportCard({ project, setup, report, selected, onSelect, onEdit }: { project: ProjectRow; setup: ReportsSetup; report: ReportRow; selected: boolean; onSelect: () => void; onEdit: () => void }) {
  const { t, locale } = useI18n();
  const { notify, jobs, fail } = useShell();
  const queryClient = useQueryClient();
  const failed = (thrown: unknown) => fail(thrown);
  const pdf = useMutation({
    mutationFn: () => api.reportPdf(project.id, report.id),
    onSuccess: (file) => {
      downloadBase64(file.name, file.data, file.content_type);
      refreshSpending(queryClient);
    },
    onError: failed,
  });
  const xlsx = useMutation({ mutationFn: () => api.reportXlsx(project.id, report.id), onSuccess: (file) => downloadBase64(file.name, file.data, file.content_type), onError: failed });
  const html = useMutation({ mutationFn: () => api.reportHtml(project.id, report.id), onSuccess: (page) => download(page.name, page.html, "text/html;charset=utf-8"), onError: failed });
  const send = useMutation({
    mutationFn: () => api.sendReport(project.id, report.id),
    onSuccess: () => {
      notify(t("reports.sending"));
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
    },
    onError: failed,
  });
  const remove = useMutation({
    mutationFn: () => api.deleteReport(project.id, report.id),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["reports"] }),
    onError: failed,
  });
  const sending = jobs.some((job) => job.kind === "report-email" && job.project_id === project.id && job.status === "running");
  const language = UI_LANGUAGES.find((entry) => entry.code === report.language)?.name ?? report.language;
  return (
    <article className={`oc-card report-card ${selected ? "selected" : ""}`}>
      <button type="button" className="report-card-head" onClick={onSelect} aria-pressed={selected}>
        <span className="oc-tile violet">
          <FileText size={16} />
        </span>
        <span className="report-card-title">
          <strong>{report.name}</strong>
          <small className="faint">
            {language} · {t("reports.daysOption", { n: report.days })} · {t("reports.partsCount", { n: report.widgets.length })} · {t(`reports.design_${report.design}`)}
          </small>
        </span>
        {report.schedule !== "manual" && (
          <span className="oc-badge info">
            <Mail size={12} aria-hidden="true" /> {t(`reports.schedule_${report.schedule}`)}
          </span>
        )}
      </button>
      {report.last_sent_at && <p className="faint small">{t("reports.lastSent", { time: relativeTime(report.last_sent_at, locale) })}</p>}
      <div className="report-actions-row">
        <Button small primary busy={pdf.isPending} disabled={!setup.can_print} title={setup.can_print ? undefined : t("reports.noBrowser")} onClick={() => pdf.mutate()}>
          <Download size={14} />
          PDF
        </Button>
        <Button small busy={xlsx.isPending} onClick={() => xlsx.mutate()}>
          <FileSpreadsheet size={14} />
          Excel
        </Button>
        <Button small busy={html.isPending} onClick={() => html.mutate()}>
          <FileText size={14} />
          HTML
        </Button>
        {setup.can_email && (
          <Button small busy={send.isPending} disabled={sending || report.recipients.length === 0} title={report.recipients.length === 0 ? t("reports.recipientsMissing") : undefined} onClick={() => send.mutate()}>
            <Send size={14} />
            {t("reports.sendNow")}
          </Button>
        )}
        <Button small className="ghost" onClick={onEdit}>
          <Pencil size={14} />
          {t("reports.edit")}
        </Button>
        <Button
          small
          className="ghost"
          busy={remove.isPending}
          onClick={() => {
            if (window.confirm(t("reports.deleteConfirm", { name: report.name }))) remove.mutate();
          }}
        >
          <Trash2 size={14} />
          <span className="sr-only">{t("reports.delete")}</span>
        </Button>
      </div>
      {selected && setup.can_share && <ShareBox project={project} setup={setup} report={report} />}
      {selected && setup.can_email && <Deliveries project={project} report={report} />}
    </article>
  );
}

export function Reports({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const setup = useQuery({ queryKey: ["reports", "setup", project.id], queryFn: () => api.reportsSetup(project.id) });
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [editing, setEditing] = useState<ReportRow | "new" | null>(null);
  const [branding, setBranding] = useState(false);
  if (setup.isLoading) return <Loading label={t("common.loading")} />;
  if (setup.error || !setup.data) {
    return <ErrorState title={t("common.loadFailed")} message={explain(setup.error, t).message} retry={() => void setup.refetch()} retryLabel={t("common.retry")} />;
  }
  const data = setup.data;
  const selected = data.reports.find((report) => report.id === selectedId) ?? data.reports[0] ?? null;
  return (
    <div className="page-stack">
      <section className="oc-panel rank-setup">
        <div className="rank-run">
          <div className="rank-run-text">
            <span>{data.branding.company ? t("reports.brandedAs", { company: data.branding.company }) : t("reports.noBranding")}</span>
            <span className="faint small">{t("reports.freeNote")}</span>
          </div>
          <Button small onClick={() => setBranding(true)}>
            <Palette size={14} />
            {t("reports.branding")}
          </Button>
          <Button primary small onClick={() => setEditing("new")}>
            <Plus size={14} />
            {t("reports.new")}
          </Button>
        </div>
        {!data.can_print && <p className="oc-banner warning small">{t("reports.noBrowser")}</p>}
      </section>
      {data.reports.length === 0 ? (
        <div className="oc-card">
          <Empty icon={<FileText size={28} />} title={t("reports.emptyTitle")}>
            <p>{t("reports.emptyText", { domain: project.domain })}</p>
            <Button primary onClick={() => setEditing("new")}>
              <Plus size={15} />
              {t("reports.new")}
            </Button>
          </Empty>
        </div>
      ) : (
        <div className="reports-layout">
          <div className="report-list">
            {data.reports.map((report) => (
              <ReportCard
                key={report.id}
                project={project}
                setup={data}
                report={report}
                selected={report.id === selected?.id}
                onSelect={() => setSelectedId(report.id)}
                onEdit={() => setEditing(report)}
              />
            ))}
          </div>
          {selected && (
            <section className="oc-panel report report-preview-panel">
              <header className="report-head">
                <div className="report-title">
                  <h3 className="oc-panel-title">{selected.name}</h3>
                  <span className="faint">{t("reports.previewHint")}</span>
                </div>
              </header>
              <Preview project={project} report={selected} />
            </section>
          )}
        </div>
      )}
      {editing && <ReportDialog project={project} setup={data} report={editing === "new" ? null : editing} onClose={() => setEditing(null)} onSaved={(saved) => setSelectedId(saved.id)} />}
      {branding && <BrandingDialog branding={data.branding} onClose={() => setBranding(false)} />}
    </div>
  );
}
