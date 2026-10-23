import { useEffect, useState, type ChangeEvent, type CSSProperties } from "react";
import { keepPreviousData, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowDown, ArrowUp, ChevronDown, ImagePlus, RotateCcw, SlidersHorizontal, Trash2 } from "lucide-react";
import { Button, Field, Modal, Segmented } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { Branding, ProjectRow, ReportColors, ReportDesign, ReportLanguage, ReportRow, ReportSchedule, ReportSpec, ReportWidget, ReportsSetup } from "../../api/types";
import { LANGUAGES as UI_LANGUAGES, useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { explain } from "../../lib/errors";
import { blankReport, COLOR_KEYS, DAYS, LANGUAGES, logoProblem, MAX_RECIPIENTS, moveWidget, NO_COLORS, orderedChoices, ownColors, parseRecipients, shownColors, validEmail } from "./report";
import { ScaledPage } from "./ScaledPage";

/** Milliseconds the live preview waits after the last change before drawing. */
const PREVIEW_WAIT = 350;

/** Optional colours of the report's own: each shows the design's until changed. */
function CustomizePanel({ design, colors, brandAccent, onChange }: { design: ReportDesign; colors: ReportColors; brandAccent: string | null; onChange: (colors: ReportColors) => void }) {
  const { t } = useI18n();
  const own = ownColors(colors);
  const [open, setOpen] = useState(own > 0);
  const shown = shownColors(design, colors, brandAccent);
  return (
    <section className={`customize ${open ? "open" : ""}`}>
      <button type="button" className="customize-head" aria-expanded={open} onClick={() => setOpen(!open)}>
        <SlidersHorizontal size={15} aria-hidden="true" />
        <strong>{t("reports.customize")}</strong>
        <span className="faint small">{own > 0 ? t("reports.customized", { n: own }) : t("reports.customizeOptional")}</span>
        <ChevronDown size={15} className="customize-chevron" aria-hidden="true" />
      </button>
      {open && (
        <div className="customize-body">
          <p className="faint small">{t("reports.customizeHint")}</p>
          <ul className="color-list">
            {COLOR_KEYS.map((key) => {
              const mine = colors[key] !== null;
              return (
                <li key={key} className={mine ? "mine" : undefined}>
                  <label className="color-swatch" title={t(`reports.color_${key}`)}>
                    <input type="color" value={shown[key]} onChange={(event) => onChange({ ...colors, [key]: event.target.value })} aria-label={t(`reports.color_${key}`)} />
                    <span style={{ background: shown[key] }} aria-hidden="true" />
                  </label>
                  <span className="color-text">
                    <strong>{t(`reports.color_${key}`)}</strong>
                    <small className="faint">{t(`reports.colorHint_${key}`)}</small>
                  </span>
                  <span className="color-state small">
                    {mine ? (
                      <>
                        <code>{shown[key]}</code>
                        <Button small className="icon ghost" title={t("reports.colorReset")} aria-label={t("reports.colorReset")} onClick={() => onChange({ ...colors, [key]: null })}>
                          <RotateCcw size={13} />
                        </Button>
                      </>
                    ) : (
                      <span className="faint">{t("reports.colorDesign")}</span>
                    )}
                  </span>
                </li>
              );
            })}
          </ul>
          {own > 0 && (
            <Button small className="ghost" onClick={() => onChange({ ...NO_COLORS })}>
              <RotateCcw size={14} />
              {t("reports.customizeReset")}
            </Button>
          )}
        </div>
      )}
    </section>
  );
}

/** A value that follows `value` once it has stopped changing for `wait` milliseconds. */
function useSettled<T>(value: T, wait: number): T {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setSettled(value), wait);
    return () => clearTimeout(timer);
  }, [value, wait]);
  return settled;
}

/** The report as it will look with the dialog's settings, before saving. */
function LivePreview({ project, reportId, spec }: { project: ProjectRow; reportId: number | null; spec: ReportSpec }) {
  const { t } = useI18n();
  const settled = useSettled(JSON.stringify({ ...spec, recipients: [] }), PREVIEW_WAIT);
  const page = useQuery({
    queryKey: ["reports", "preview", project.id, reportId, settled],
    queryFn: () => api.reportPreview(project.id, reportId, JSON.parse(settled) as ReportSpec),
    placeholderData: keepPreviousData,
  });
  return (
    <aside className="report-editor-preview" aria-label={t("reports.previewLive")}>
      <div className="report-editor-preview-head">
        <strong>{t("reports.previewLive")}</strong>
        <span className="faint small">{page.isFetching ? t("common.loading") : t("reports.previewLiveHint")}</span>
      </div>
      {page.data ? <ScaledPage title={t("reports.previewLive")} html={page.data.html} dim={page.isFetching} /> : page.error ? <p className="faint small">{explain(page.error, t).message}</p> : <div className="report-preview" />}
    </aside>
  );
}

/** The report's look: each design as a small cover. */
function DesignPicker({ designs, value, accent, onChange }: { designs: ReportDesign[]; value: ReportDesign; accent: string | null; onChange: (design: ReportDesign) => void }) {
  const { t } = useI18n();
  return (
    <div className="oc-field">
      <span id="report-design">{t("reports.design")}</span>
      <div className="design-picker" role="radiogroup" aria-labelledby="report-design">
        {designs.map((design) => (
          <button
            key={design}
            type="button"
            role="radio"
            aria-checked={value === design}
            className={`design-choice ${value === design ? "on" : ""}`}
            onClick={() => onChange(design)}
          >
            <span className={`design-thumb thumb-${design}`} style={accent ? ({ "--thumb-accent": accent } as CSSProperties) : undefined} aria-hidden="true">
              <i className="thumb-kicker" />
              <i className="thumb-title" />
              <i className="thumb-line" />
              <span className="thumb-tiles">
                <i />
                <i />
                <i />
              </span>
            </span>
            <strong>{t(`reports.design_${design}`)}</strong>
            <small className="faint">{t(`reports.designHint_${design}`)}</small>
          </button>
        ))}
      </div>
      <small className="oc-field-hint">{t("reports.designHint")}</small>
    </div>
  );
}

/** A report's name, language, period, parts in order, and who gets it how often. */
export function ReportDialog({
  project,
  setup,
  report,
  preset,
  onClose,
  onSaved,
}: {
  project: ProjectRow;
  setup: ReportsSetup;
  report: ReportRow | null;
  /** A new report's choices made elsewhere (the Snapshots page). */
  preset?: Partial<ReportSpec>;
  onClose: () => void;
  onSaved: (report: ReportRow) => void;
}) {
  const { t } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const [spec, setSpec] = useState<ReportSpec>(() =>
    report ? { ...report, colors: { ...NO_COLORS, ...report.colors } } : { ...blankReport(project.language, setup.widgets), name: t("reports.defaultName"), ...preset },
  );
  const [recipients, setRecipients] = useState((report?.recipients ?? []).join("\n"));
  const typed = parseRecipients(recipients);
  const wrong = typed.filter((address) => !validEmail(address));
  const save = useMutation({
    mutationFn: () => api.saveReport(project.id, report?.id ?? null, { ...spec, recipients: typed }),
    onSuccess: (saved) => {
      void queryClient.invalidateQueries({ queryKey: ["reports"] });
      notify(t("reports.saved"));
      onSaved(saved);
      onClose();
    },
    onError: (thrown) => fail(thrown),
  });
  const toggle = (widget: ReportWidget) =>
    setSpec((previous) => ({
      ...previous,
      widgets: previous.widgets.includes(widget) ? previous.widgets.filter((known) => known !== widget) : [...previous.widgets, widget],
    }));
  const scheduled = spec.schedule !== "manual";
  const problem = !spec.name.trim()
    ? t("reports.nameMissing")
    : spec.widgets.length === 0
      ? t("reports.partsMissing")
      : wrong.length > 0
        ? t("reports.recipientsWrong", { list: wrong.join(", ") })
        : typed.length > MAX_RECIPIENTS
          ? t("reports.recipientsMany", { n: MAX_RECIPIENTS })
          : scheduled && typed.length === 0
            ? t("reports.recipientsMissing")
            : null;
  const languageName = (code: ReportLanguage) => UI_LANGUAGES.find((language) => language.code === code)?.name ?? code;
  return (
    <Modal
      title={report ? t("reports.editTitle") : t("reports.newTitle")}
      className="wide report-editor"
      closeLabel={t("common.close")}
      onClose={onClose}
      footer={
        <>
          {problem && <span className="faint small dialog-problem">{problem}</span>}
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button primary busy={save.isPending} disabled={Boolean(problem)} onClick={() => save.mutate()}>
            {t("common.save")}
          </Button>
        </>
      }
    >
      <div className="report-editor-grid">
      <div className="form-grid">
        <Field label={t("reports.name")}>
          <input className="oc-input" value={spec.name} maxLength={120} onChange={(event) => setSpec({ ...spec, name: event.target.value })} />
        </Field>
        <div className="form-row">
          <Field label={t("reports.language")}>
            <select className="oc-select" value={spec.language} onChange={(event) => setSpec({ ...spec, language: event.target.value as ReportLanguage })}>
              {LANGUAGES.map((code) => (
                <option key={code} value={code}>
                  {languageName(code)}
                </option>
              ))}
            </select>
          </Field>
          <Field label={t("reports.days")}>
            <Segmented<string>
              label={t("reports.days")}
              value={String(spec.days)}
              onChange={(value) => setSpec({ ...spec, days: Number(value) })}
              options={DAYS.map((days) => ({ value: String(days), label: t("reports.daysOption", { n: days }) }))}
            />
          </Field>
        </div>
        <DesignPicker designs={setup.designs} value={spec.design} accent={spec.colors.accent ?? setup.branding.accent} onChange={(design) => setSpec({ ...spec, design })} />
        <CustomizePanel design={spec.design} colors={spec.colors} brandAccent={setup.branding.accent} onChange={(colors) => setSpec({ ...spec, colors })} />
        <Field label={t("reports.parts")} hint={t("reports.partsHint")}>
          <ul className="part-list">
            {orderedChoices(setup.widgets, spec.widgets).map((widget) => {
              const on = spec.widgets.includes(widget);
              const index = spec.widgets.indexOf(widget);
              return (
                <li key={widget} className={on ? "on" : undefined}>
                  <label className="oc-check">
                    <input type="checkbox" checked={on} onChange={() => toggle(widget)} />
                    <span>
                      <strong>{t(`reports.part_${widget}`)}</strong>
                      <small className="faint">{t(`reports.partHint_${widget}`)}</small>
                    </span>
                  </label>
                  {on && (
                    <span className="part-moves">
                      <Button small className="ghost" disabled={index === 0} title={t("reports.moveUp")} onClick={() => setSpec({ ...spec, widgets: moveWidget(spec.widgets, widget, -1) })}>
                        <ArrowUp size={14} />
                        <span className="sr-only">{t("reports.moveUp")}</span>
                      </Button>
                      <Button small className="ghost" disabled={index === spec.widgets.length - 1} title={t("reports.moveDown")} onClick={() => setSpec({ ...spec, widgets: moveWidget(spec.widgets, widget, 1) })}>
                        <ArrowDown size={14} />
                        <span className="sr-only">{t("reports.moveDown")}</span>
                      </Button>
                    </span>
                  )}
                </li>
              );
            })}
          </ul>
        </Field>
        <Field label={t("reports.schedule")} hint={setup.can_email ? t("reports.scheduleHint") : t("reports.noEmail")}>
          <Segmented<ReportSchedule>
            label={t("reports.schedule")}
            value={spec.schedule}
            disabled={!setup.can_email}
            onChange={(schedule) => setSpec({ ...spec, schedule })}
            options={[
              { value: "manual", label: t("reports.scheduleManual") },
              { value: "weekly", label: t("reports.scheduleWeekly") },
              { value: "monthly", label: t("reports.scheduleMonthly") },
              { value: "snapshot", label: t("reports.scheduleSnapshot") },
            ]}
          />
        </Field>
        {setup.can_email && (
          <Field label={t("reports.recipients")} hint={t("reports.recipientsHint", { n: MAX_RECIPIENTS })}>
            <textarea className="oc-input recipients-input" value={recipients} onChange={(event) => setRecipients(event.target.value)} placeholder="musteri@example.com" spellCheck={false} />
          </Field>
        )}
      </div>
      <LivePreview project={project} reportId={report?.id ?? null} spec={spec} />
      </div>
    </Modal>
  );
}

/** The agency's look on every report: name, logo, colour, website, footer. */
export function BrandingDialog({ branding, onClose }: { branding: Branding; onClose: () => void }) {
  const { t } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const [draft, setDraft] = useState<Branding>(branding);
  const save = useMutation({
    mutationFn: () => api.saveReportBranding(draft),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["reports"] });
      notify(t("reports.brandingSaved"));
      onClose();
    },
    onError: (thrown) => fail(thrown),
  });
  const pick = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    const problem = logoProblem(file);
    if (problem) {
      notify(t(problem === "type" ? "reports.logoType" : "reports.logoSize"), true);
      return;
    }
    const reader = new FileReader();
    reader.onload = () => setDraft((previous) => ({ ...previous, logo: typeof reader.result === "string" ? reader.result : null }));
    reader.readAsDataURL(file);
  };
  const text = (key: "company" | "website" | "footer") => (event: ChangeEvent<HTMLInputElement>) => setDraft({ ...draft, [key]: event.target.value || null });
  return (
    <Modal
      title={t("reports.brandingTitle")}
      closeLabel={t("common.close")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button primary busy={save.isPending} onClick={() => save.mutate()}>
            {t("common.save")}
          </Button>
        </>
      }
    >
      <div className="form-grid">
        <p className="faint small">{t("reports.brandingHint")}</p>
        <Field label={t("reports.company")}>
          <input className="oc-input" value={draft.company ?? ""} maxLength={120} onChange={text("company")} />
        </Field>
        <Field label={t("reports.logo")} hint={t("reports.logoHint")}>
          <div className="logo-row">
            {draft.logo ? <img className="logo-preview" src={draft.logo} alt="" /> : <span className="logo-preview empty" aria-hidden="true" />}
            <label className="oc-btn small">
              <ImagePlus size={14} />
              {t("reports.chooseLogo")}
              <input type="file" accept="image/png,image/jpeg,image/webp,image/svg+xml" className="sr-only" onChange={pick} />
            </label>
            {draft.logo && (
              <Button small className="ghost" onClick={() => setDraft({ ...draft, logo: null })}>
                <Trash2 size={14} />
                {t("reports.removeLogo")}
              </Button>
            )}
          </div>
        </Field>
        <div className="form-row">
          <Field label={t("reports.accent")}>
            <input className="oc-input color-input" type="color" value={draft.accent ?? "#1e6fd9"} onChange={(event) => setDraft({ ...draft, accent: event.target.value })} />
          </Field>
          <Field label={t("reports.website")}>
            <input className="oc-input" value={draft.website ?? ""} placeholder="https://" onChange={text("website")} />
          </Field>
        </div>
        <Field label={t("reports.footer")}>
          <input className="oc-input" value={draft.footer ?? ""} maxLength={300} onChange={text("footer")} />
        </Field>
        <label className="oc-check">
          <input type="checkbox" checked={draft.hide_octopuson} onChange={(event) => setDraft({ ...draft, hide_octopuson: event.target.checked })} />
          <span>{t("reports.hideOctopuson")}</span>
        </label>
      </div>
    </Modal>
  );
}
