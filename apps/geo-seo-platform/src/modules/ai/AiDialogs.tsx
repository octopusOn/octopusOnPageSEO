import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ExternalLink, Plus, Trash2 } from "lucide-react";
import { Button, Field, Loading, Modal, Segmented } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { AiAnswer, AiEngine, AiSchedule, AiSetup, Brand, ProjectRow, Rival } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { explain } from "../../lib/errors";
import { parseTags } from "../rank/rank";
import { ENGINE_NAMES, highlight, parsePrompts, toneOf } from "./ai";

/** Questions typed or picked from the suggestions; the first ones also choose how often they are asked. */
export function AddPromptsDialog({ project, setup, onClose }: { project: ProjectRow; setup: AiSetup; onClose: () => void }) {
  const { t } = useI18n();
  const { notify } = useShell();
  const queryClient = useQueryClient();
  const first = setup.prompts.length === 0;
  const [text, setText] = useState("");
  const [tags, setTags] = useState("");
  const [schedule, setSchedule] = useState<AiSchedule>("manual");
  const suggestions = useQuery({ queryKey: ["ai", "suggestions", project.id], queryFn: () => api.suggestAiPrompts(project.id) });
  const prompts = useMemo(() => parsePrompts(text), [text]);
  const add = useMutation({
    mutationFn: async () => {
      const added = await api.addAiPrompts(project.id, prompts, parseTags(tags));
      if (first && schedule !== setup.settings.schedule) {
        const { engines, models, rivals } = setup.settings;
        await api.saveAiSettings(project.id, { schedule, engines, models, rivals });
      }
      return added;
    },
    onSuccess: (added) => {
      void queryClient.invalidateQueries({ queryKey: ["ai"] });
      notify(added.added ? t("ai.added", { n: added.added }) : t("ai.alreadyAsked"));
      onClose();
    },
    onError: (thrown) => notify(explain(thrown, t).message, true),
  });
  const take = (suggestion: string) => setText((previous) => (previous.trim() ? `${previous.trimEnd()}\n${suggestion}` : suggestion));
  const offered = (suggestions.data ?? []).filter((suggestion) => !prompts.includes(suggestion));
  return (
    <Modal
      title={t("ai.addTitle")}
      closeLabel={t("common.close")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button primary busy={add.isPending} disabled={prompts.length === 0} onClick={() => add.mutate()}>
            <Plus size={15} />
            {t("ai.addConfirm", { n: prompts.length })}
          </Button>
        </>
      }
    >
      <div className="form-grid">
        <Field label={t("ai.addText")} hint={t("ai.addHint")}>
          <textarea className="oc-input rank-keywords-input" value={text} onChange={(event) => setText(event.target.value)} placeholder={t("ai.addPlaceholder")} />
        </Field>
        {offered.length > 0 && (
          <Field label={t("ai.suggestions")} hint={t("ai.suggestionsHint")}>
            <div className="feature-chips">
              {offered.map((suggestion) => (
                <button key={suggestion} type="button" className="oc-chip chip-button" onClick={() => take(suggestion)}>
                  <Plus size={12} aria-hidden="true" />
                  {suggestion}
                </button>
              ))}
            </div>
          </Field>
        )}
        <Field label={t("ai.tags")}>
          <input className="oc-input" value={tags} onChange={(event) => setTags(event.target.value)} placeholder={t("ai.tagsPlaceholder")} />
        </Field>
        {first && (
          <Field label={t("ai.firstSchedule")} hint={t("ai.firstScheduleHint")}>
            <Segmented<AiSchedule>
              label={t("ai.schedule")}
              value={schedule}
              onChange={setSchedule}
              options={[
                { value: "weekly", label: t("ai.scheduleWeekly") },
                { value: "monthly", label: t("ai.scheduleMonthly") },
                { value: "manual", label: t("ai.scheduleManual") },
              ]}
            />
          </Field>
        )}
      </div>
    </Modal>
  );
}

interface RivalDraft {
  domain: string;
  names: string;
}

const draftOf = (rival: Rival): RivalDraft => ({ domain: rival.domain, names: rival.names.join(", ") });

/** The rival brands watched in the answers and the model each assistant is asked with. */
export function BrandsDialog({ project, setup, onClose }: { project: ProjectRow; setup: AiSetup; onClose: () => void }) {
  const { t } = useI18n();
  const { notify, go } = useShell();
  const queryClient = useQueryClient();
  const { settings } = setup;
  const own = setup.brands.find((brand) => brand.own);
  const [rivals, setRivals] = useState<RivalDraft[]>(() =>
    settings.rivals.length
      ? settings.rivals.map(draftOf)
      : setup.brands.filter((brand) => !brand.own).map((brand) => ({ domain: brand.key, names: "" })),
  );
  const [models, setModels] = useState<Partial<Record<AiEngine, string>>>(settings.models);
  const save = useMutation({
    mutationFn: () =>
      api.saveAiSettings(project.id, {
        schedule: settings.schedule,
        engines: settings.engines,
        models: Object.fromEntries(Object.entries(models).filter(([, model]) => model?.trim())) as Partial<Record<AiEngine, string>>,
        rivals: rivals
          .filter((rival) => rival.domain.trim())
          .map((rival) => ({ domain: rival.domain.trim(), names: rival.names.split(",").map((name) => name.trim()).filter(Boolean) })),
      }),
    onSuccess: (next) => {
      queryClient.setQueryData(["ai", "setup", project.id], next);
      void queryClient.invalidateQueries({ queryKey: ["ai"] });
      notify(t("ai.brandsSaved"));
      onClose();
    },
    onError: (thrown) => notify(explain(thrown, t).message, true),
  });
  const change = (index: number, patch: Partial<RivalDraft>) => setRivals((previous) => previous.map((rival, at) => (at === index ? { ...rival, ...patch } : rival)));
  return (
    <Modal
      title={t("ai.brandsTitle")}
      className="wide"
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
      <div className="form-grid brand-form">
        <section>
          <h4>{t("ai.ownBrand")}</h4>
          <div className="feature-chips">
            {own?.names.map((name) => (
              <span key={name} className="oc-chip accent">
                {name}
              </span>
            ))}
          </div>
          <p className="faint small">
            {t("ai.ownBrandHint")}{" "}
            <button
              type="button"
              className="link-button"
              onClick={() => {
                onClose();
                go({ page: "dashboard" });
              }}
            >
              {t("ai.editProject")}
            </button>
          </p>
        </section>
        <section>
          <h4>{t("ai.rivals")}</h4>
          <p className="faint small">{t("ai.rivalsHint")}</p>
          <ul className="rival-rows">
            {rivals.map((rival, index) => (
              <li key={index}>
                <input className="oc-input" value={rival.domain} placeholder="rakip.com" aria-label={t("ai.rivalDomain")} onChange={(event) => change(index, { domain: event.target.value })} />
                <input className="oc-input" value={rival.names} placeholder={t("ai.rivalNamesPlaceholder")} aria-label={t("ai.rivalNames")} onChange={(event) => change(index, { names: event.target.value })} />
                <Button small className="ghost" title={t("ai.removeRival")} onClick={() => setRivals((previous) => previous.filter((_, at) => at !== index))}>
                  <Trash2 size={14} />
                  <span className="sr-only">{t("ai.removeRival")}</span>
                </Button>
              </li>
            ))}
          </ul>
          <Button small disabled={rivals.length >= 10} onClick={() => setRivals((previous) => [...previous, { domain: "", names: "" }])}>
            <Plus size={14} />
            {t("ai.addRival")}
          </Button>
        </section>
        <section>
          <h4>{t("ai.models")}</h4>
          <p className="faint small">{t("ai.modelsHint")}</p>
          <div className="model-grid">
            {setup.engines.map((engine) => (
              <Field key={engine.engine} label={ENGINE_NAMES[engine.engine]}>
                <input
                  className="oc-input"
                  value={models[engine.engine] ?? ""}
                  disabled={engine.engine === "google_ai_mode"}
                  placeholder={settings.models[engine.engine] ? "" : (engine.model ?? "")}
                  onChange={(event) => setModels((previous) => ({ ...previous, [engine.engine]: event.target.value }))}
                />
              </Field>
            ))}
          </div>
        </section>
      </div>
    </Modal>
  );
}

/** An answer with the brands it names marked. */
function AnswerText({ text, brands }: { text: string; brands: Brand[] }) {
  const segments = useMemo(() => highlight(text, brands), [text, brands]);
  return (
    <div className="answer-text">
      {segments.map((segment, index) =>
        segment.brand ? (
          <mark key={index} className={segment.brand.own ? "own" : "rival"} title={segment.brand.key}>
            {segment.text}
          </mark>
        ) : (
          <span key={index}>{segment.text}</span>
        ),
      )}
    </div>
  );
}

function AnswerFacts({ answer }: { answer: AiAnswer }) {
  const { t } = useI18n();
  const tone = toneOf(answer.sentiment);
  return (
    <div className="feature-chips">
      <span className={`oc-badge ${answer.mentioned ? "good" : "critical"}`}>{answer.mentioned ? t("ai.mentioned") : t("ai.notMentioned")}</span>
      {answer.position !== null && <span className="oc-badge info">{t("ai.positionBadge", { n: answer.position })}</span>}
      {answer.cited && <span className="oc-badge good">{t("ai.citedBadge")}</span>}
      {tone && <span className={`oc-badge ${tone === "positive" ? "good" : tone === "negative" ? "critical" : "info"}`}>{t(`ai.tone_${tone}`)}</span>}
    </div>
  );
}

/** One question on one assistant: its latest answers, the brands named and the sources cited. */
export function AnswerDialog({ project, promptId, engine, onClose }: { project: ProjectRow; promptId: number; engine: AiEngine; onClose: () => void }) {
  const { t, locale } = useI18n();
  const detail = useQuery({ queryKey: ["ai", "answer", project.id, promptId, engine], queryFn: () => api.aiAnswer(project.id, promptId, engine) });
  const [index, setIndex] = useState(0);
  const data = detail.data;
  const answer = data?.answers[index];
  const ownSite = data?.brands.find((brand) => brand.own)?.key ?? project.domain;
  const isOwn = (domain: string) => domain === ownSite || domain.endsWith(`.${ownSite}`);
  return (
    <Modal
      title={data?.prompt.prompt ?? t("common.loading")}
      className="wide"
      closeLabel={t("common.close")}
      onClose={onClose}
      footer={<Button onClick={onClose}>{t("common.close")}</Button>}
    >
      {detail.isLoading && <Loading label={t("common.loading")} />}
      {detail.error && <div className="oc-banner error">{explain(detail.error, t).message}</div>}
      {data && data.answers.length === 0 && <p className="faint">{t("ai.noAnswer")}</p>}
      {data && answer && (
        <div className="form-grid">
          <div className="answer-head">
            <strong>{ENGINE_NAMES[engine]}</strong>
            <span className="faint small">
              {answer.model ?? ""} · {t(`ai.via_${answer.via}`)}
            </span>
            {data.answers.length > 1 && (
              <select className="oc-select" value={index} aria-label={t("ai.whichAnswer")} onChange={(event) => setIndex(Number(event.target.value))}>
                {data.answers.map((entry, at) => (
                  <option key={`${entry.run_id}-${at}`} value={at}>
                    {new Date(entry.checked_at).toLocaleString(locale, { dateStyle: "medium", timeStyle: "short" })}
                  </option>
                ))}
              </select>
            )}
          </div>
          {answer.error ? (
            <div className="oc-banner warning">{t("ai.answerFailed", { error: answer.error })}</div>
          ) : (
            <>
              <AnswerFacts answer={answer} />
              <AnswerText text={answer.text} brands={data.brands} />
              {answer.brands.length > 0 && (
                <section>
                  <h4>{t("ai.brandsNamed")}</h4>
                  <ol className="check-list">
                    {[...answer.brands]
                      .sort((left, right) => (left.position ?? 99) - (right.position ?? 99))
                      .map((brand) => (
                        <li key={brand.key}>
                          <strong>{brand.position ?? "—"}</strong>
                          <span className={brand.own ? "good" : undefined}>{brand.key}</span>
                          {brand.cited && <span className="oc-badge good">{t("ai.citedBadge")}</span>}
                        </li>
                      ))}
                  </ol>
                </section>
              )}
              <section>
                <h4>{t("ai.sourcesCited", { n: answer.citations.length })}</h4>
                {answer.citations.length === 0 ? (
                  <p className="faint small">{t("ai.noSources")}</p>
                ) : (
                  <ul className="citation-list">
                    {answer.citations.map((citation) => (
                      <li key={citation.url} className={isOwn(citation.domain) ? "own" : undefined}>
                        <a href={citation.url} target="_blank" rel="noreferrer noopener">
                          <span>{citation.title || citation.domain}</span>
                          <small className="faint">{citation.domain}</small>
                          <ExternalLink size={12} aria-hidden="true" />
                        </a>
                      </li>
                    ))}
                  </ul>
                )}
              </section>
            </>
          )}
        </div>
      )}
    </Modal>
  );
}
