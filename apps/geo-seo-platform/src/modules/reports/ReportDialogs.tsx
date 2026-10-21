import { useState, type ChangeEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { ArrowDown, ArrowUp, ImagePlus, Trash2 } from "lucide-react";
import { Button, Field, Modal, Segmented } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { Branding, ProjectRow, ReportLanguage, ReportRow, ReportSchedule, ReportSpec, ReportWidget, ReportsSetup } from "../../api/types";
import { LANGUAGES as UI_LANGUAGES, useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { explain } from "../../lib/errors";
import { blankReport, DAYS, LANGUAGES, logoProblem, MAX_RECIPIENTS, moveWidget, orderedChoices, parseRecipients, validEmail } from "./report";

/** A report's name, language, period, parts in order, and who gets it how often. */
export function ReportDialog({ project, setup, report, onClose, onSaved }: { project: ProjectRow; setup: ReportsSetup; report: ReportRow | null; onClose: () => void; onSaved: (report: ReportRow) => void }) {
  const { t } = useI18n();
  const { notify } = useShell();
  const queryClient = useQueryClient();
  const [spec, setSpec] = useState<ReportSpec>(() => (report ? { ...report } : { ...blankReport(project.language, setup.widgets), name: t("reports.defaultName") }));
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
    onError: (thrown) => notify(explain(thrown, t).message, true),
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
      className="wide"
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
            ]}
          />
        </Field>
        {setup.can_email && (
          <Field label={t("reports.recipients")} hint={t("reports.recipientsHint", { n: MAX_RECIPIENTS })}>
            <textarea className="oc-input recipients-input" value={recipients} onChange={(event) => setRecipients(event.target.value)} placeholder="musteri@example.com" spellCheck={false} />
          </Field>
        )}
      </div>
    </Modal>
  );
}

/** The agency's look on every report: name, logo, colour, website, footer. */
export function BrandingDialog({ branding, onClose }: { branding: Branding; onClose: () => void }) {
  const { t } = useI18n();
  const { notify } = useShell();
  const queryClient = useQueryClient();
  const [draft, setDraft] = useState<Branding>(branding);
  const save = useMutation({
    mutationFn: () => api.saveReportBranding(draft),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["reports"] });
      notify(t("reports.brandingSaved"));
      onClose();
    },
    onError: (thrown) => notify(explain(thrown, t).message, true),
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
