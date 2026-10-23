import { useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Button, Field, Modal } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { ProjectRow, ProjectSpec } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { explain } from "../../lib/errors";
import { COUNTRIES, LANGUAGE_CODES } from "../../lib/market";

function lines(text: string): string[] {
  return text
    .split(/[\n,]/)
    .map((line) => line.trim())
    .filter(Boolean);
}

export function ProjectDialog({ project, onClose, onSaved }: { project: ProjectRow | null; onClose: () => void; onSaved: (row: ProjectRow) => void }) {
  const { t, locale } = useI18n();
  const { notify } = useShell();
  const queryClient = useQueryClient();
  const [domain, setDomain] = useState(project?.domain ?? "");
  const [name, setName] = useState(project?.name ?? "");
  const [country, setCountry] = useState(project?.country ?? "tr");
  const [language, setLanguage] = useState(project?.language ?? "tr");
  const [competitors, setCompetitors] = useState((project?.competitors ?? []).join("\n"));
  const [brands, setBrands] = useState((project?.brands ?? []).join(", "));
  const [error, setError] = useState<string | null>(null);
  const regions = new Intl.DisplayNames([locale], { type: "region" });
  const languages = new Intl.DisplayNames([locale], { type: "language" });
  const save = useMutation({
    mutationFn: (spec: ProjectSpec) => (project ? api.updateProject(project.id, spec) : api.createProject(spec)),
    onSuccess: (row) => {
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
      notify(t(project ? "projects.updated" : "projects.created"));
      onSaved(row);
    },
    onError: (thrown) => setError(explain(thrown, t).message),
  });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    setError(null);
    save.mutate({ name, domain, country, language, competitors: lines(competitors), brands: lines(brands) });
  };
  return (
    <Modal
      title={t(project ? "projects.editTitle" : "projects.newTitle")}
      closeLabel={t("common.close")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button primary busy={save.isPending} type="submit" form="project-form">
            {t(project ? "common.save" : "projects.create")}
          </Button>
        </>
      }
    >
      <form id="project-form" className="form-grid" onSubmit={submit}>
        <Field label={t("projects.domain")} hint={t("projects.domainHint")}>
          <input className="oc-input" value={domain} onChange={(event) => setDomain(event.target.value)} required autoComplete="off" spellCheck={false} placeholder="ornek.com" />
        </Field>
        <Field label={t("projects.name")} hint={t("projects.nameHint")}>
          <input className="oc-input" value={name} onChange={(event) => setName(event.target.value)} maxLength={120} />
        </Field>
        <div className="form-row">
          <Field label={t("projects.country")}>
            <select className="oc-select" value={country} onChange={(event) => setCountry(event.target.value)}>
              {COUNTRIES.map((code) => (
                <option key={code} value={code}>
                  {regions.of(code.toUpperCase()) ?? code}
                </option>
              ))}
            </select>
          </Field>
          <Field label={t("projects.language")}>
            <select className="oc-select" value={language} onChange={(event) => setLanguage(event.target.value)}>
              {LANGUAGE_CODES.map((code) => (
                <option key={code} value={code}>
                  {languages.of(code) ?? code}
                </option>
              ))}
            </select>
          </Field>
        </div>
        <Field label={t("projects.competitors")} hint={t("projects.competitorsHint")}>
          <textarea className="oc-textarea" rows={4} value={competitors} onChange={(event) => setCompetitors(event.target.value)} spellCheck={false} placeholder={"rakip.com\nbaska-rakip.com.tr"} />
        </Field>
        <Field label={t("projects.brands")} hint={t("projects.brandsHint")}>
          <input className="oc-input" value={brands} onChange={(event) => setBrands(event.target.value)} />
        </Field>
        {error && (
          <div className="oc-banner error" role="alert">
            {error}
          </div>
        )}
      </form>
    </Modal>
  );
}
