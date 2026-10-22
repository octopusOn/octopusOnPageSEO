import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ExternalLink, MapPin, Plus, Sparkles, Trash2 } from "lucide-react";
import { Button, Field, Loading, Modal, Segmented } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { Device, KeywordRank, NewKeyword, ProjectRow, RankSetup, Schedule, SearchEngine, TrackerRow } from "../../api/types";
import { useI18n } from "../../i18n";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { useShell } from "../../lib/context";
import { explain } from "../../lib/errors";
import { relativeTime } from "../../lib/format";
import { featureName } from "../../lib/tables";
import { ENGINES, parseKeywords, parseTags, placeLabel } from "./rank";

/** Keywords typed or taken from a list; the first ones also choose how often they are checked. */
export function AddKeywordsDialog({ project, setup, onClose }: { project: ProjectRow; setup: RankSetup; onClose: () => void }) {
  const { t } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const first = setup.keywords === 0;
  const [text, setText] = useState("");
  const [tags, setTags] = useState("");
  const [listId, setListId] = useState("");
  const [schedule, setSchedule] = useState<Schedule>("daily");
  const lists = useQuery({ queryKey: ["keyword-lists", project.id], queryFn: () => api.keywordLists(project.id) });
  const listed = useQuery({ queryKey: ["list-keywords", Number(listId)], queryFn: () => api.listKeywords(Number(listId)), enabled: listId !== "" });
  const keywords = useMemo<NewKeyword[]>(() => {
    const typed = parseKeywords(text);
    const known = new Set(typed.map((entry) => entry.keyword));
    for (const entry of listed.data ?? []) {
      if (known.has(entry.keyword)) continue;
      known.add(entry.keyword);
      const volume = entry.data.volume;
      typed.push({ keyword: entry.keyword, volume: typeof volume === "number" ? volume : null });
    }
    return typed;
  }, [text, listed.data]);
  const add = useMutation({
    mutationFn: async () => {
      const tracked = await api.trackKeywords(project.id, keywords, parseTags(tags));
      if (first && schedule !== "manual") await api.saveRankSettings(project.id, { schedule, depth: setup.settings.depth, ai_overview: setup.settings.ai_overview });
      return tracked;
    },
    onSuccess: (tracked) => {
      void queryClient.invalidateQueries({ queryKey: ["rank"] });
      notify(tracked.added ? t("rank.added", { n: tracked.added }) : t("rank.alreadyTracked"));
      onClose();
    },
    onError: (thrown) => fail(thrown),
  });
  return (
    <Modal
      title={t("rank.addTitle")}
      closeLabel={t("common.close")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button primary busy={add.isPending} disabled={keywords.length === 0} onClick={() => add.mutate()}>
            <Plus size={15} />
            {t("rank.addConfirm", { n: keywords.length })}
          </Button>
        </>
      }
    >
      <div className="form-grid">
        <Field label={t("rank.addText")}>
          <textarea className="oc-input rank-keywords-input" value={text} onChange={(event) => setText(event.target.value)} placeholder={t("rank.addPlaceholder")} />
        </Field>
        <Field label={t("rank.addFromList")}>
          <select className="oc-select" value={listId} onChange={(event) => setListId(event.target.value)}>
            <option value="">{t("rank.noList")}</option>
            {lists.data?.map((list) => (
              <option key={list.id} value={list.id}>
                {list.name} ({list.keywords})
              </option>
            ))}
          </select>
        </Field>
        <Field label={t("rank.tags")}>
          <input className="oc-input" value={tags} onChange={(event) => setTags(event.target.value)} placeholder={t("rank.tagsPlaceholder")} />
        </Field>
        {first && (
          <Field label={t("rank.firstSchedule")} hint={t("rank.firstScheduleHint")}>
            <Segmented<Schedule>
              label={t("rank.schedule")}
              value={schedule}
              onChange={setSchedule}
              options={[
                { value: "daily", label: t("rank.scheduleDaily") },
                { value: "weekly", label: t("rank.scheduleWeekly") },
                { value: "manual", label: t("rank.scheduleManual") },
              ]}
            />
          </Field>
        )}
      </div>
    </Modal>
  );
}

/** The places a project is checked in: add one (engine, device, country, city) or remove one. */
export function PlacesDialog({ project, setup, onClose }: { project: ProjectRow; setup: RankSetup; onClose: () => void }) {
  const { t } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const [engine, setEngine] = useState<SearchEngine>("google");
  const [device, setDevice] = useState<Device>("desktop");
  const [country, setCountry] = useState(project.country);
  const [language, setLanguage] = useState(project.language);
  const [location, setLocation] = useState("");
  const done = (next: RankSetup) => {
    queryClient.setQueryData(["rank", "setup", project.id], next);
    void queryClient.invalidateQueries({ queryKey: ["rank"] });
  };
  const add = useMutation({
    mutationFn: () => api.addRankTracker(project.id, { engine, device, country, language, ...(location.trim() ? { location } : {}) }),
    onSuccess: (next) => {
      done(next);
      setLocation("");
      notify(t("rank.placeAdded"));
    },
    onError: (thrown) => fail(thrown),
  });
  const remove = useMutation({
    mutationFn: (tracker: TrackerRow) => api.removeRankTracker(project.id, tracker.id),
    onSuccess: done,
    onError: (thrown) => fail(thrown),
  });
  const deviceName = (name: string) => t(`rank.${name}`);
  return (
    <Modal title={t("rank.managePlaces")} closeLabel={t("common.close")} onClose={onClose} footer={<Button onClick={onClose}>{t("common.close")}</Button>}>
      <div className="form-grid">
        <ul className="place-list">
          {setup.trackers.map((tracker) => (
            <li key={tracker.id}>
              <MapPin size={15} aria-hidden="true" />
              <span>
                <strong>{placeLabel(tracker, deviceName)}</strong>
                <small className="faint">
                  {tracker.location ?? t("rank.wholeCountry")} · {tracker.language}-{tracker.country}
                </small>
              </span>
              <Button
                small
                busy={remove.isPending && remove.variables?.id === tracker.id}
                disabled={setup.trackers.length === 1 && setup.keywords > 0}
                title={t("rank.removePlace")}
                onClick={() => {
                  if (window.confirm(t("rank.removePlaceConfirm", { place: placeLabel(tracker, deviceName) }))) remove.mutate(tracker);
                }}
              >
                <Trash2 size={14} />
                <span className="sr-only">{t("rank.removePlace")}</span>
              </Button>
            </li>
          ))}
        </ul>
        <h4>{t("rank.addPlace")}</h4>
        <div className="place-form">
          <Field label={t("rank.engine")}>
            <select className="oc-select" value={engine} onChange={(event) => setEngine(event.target.value as SearchEngine)}>
              {ENGINES.map((name) => (
                <option key={name} value={name}>
                  {name === "google" ? "Google" : name === "bing" ? "Bing" : "Yandex"}
                </option>
              ))}
            </select>
          </Field>
          <Field label={t("rank.device")}>
            <Segmented<Device>
              label={t("rank.device")}
              value={device}
              onChange={setDevice}
              options={[
                { value: "desktop", label: t("rank.desktop") },
                { value: "mobile", label: t("rank.mobile") },
              ]}
            />
          </Field>
          <Field label={t("rank.country")}>
            <input className="oc-input narrow" maxLength={2} value={country} onChange={(event) => setCountry(event.target.value.toLowerCase())} />
          </Field>
          <Field label={t("rank.language")}>
            <input className="oc-input narrow" maxLength={3} value={language} onChange={(event) => setLanguage(event.target.value.toLowerCase())} />
          </Field>
        </div>
        <Field label={t("rank.location")} hint={t("rank.locationHint")}>
          <input className="oc-input" value={location} onChange={(event) => setLocation(event.target.value)} placeholder="Istanbul,Istanbul,Turkey" />
        </Field>
        <div>
          <Button primary busy={add.isPending} disabled={country.length !== 2 || language.length < 2} onClick={() => add.mutate()}>
            <Plus size={15} />
            {t("rank.addPlace")}
          </Button>
        </div>
      </div>
    </Modal>
  );
}

function PositionChart({ checks }: { checks: { checked_at: string; position: number | null }[] }) {
  const { t, locale } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    return {
      grid: { left: 40, right: 16, top: 16, bottom: 28 },
      tooltip: { trigger: "axis" },
      xAxis: {
        type: "category",
        data: checks.map((check) => new Date(check.checked_at).toLocaleDateString(locale, { day: "numeric", month: "short" })),
        ...axis,
        splitLine: { show: false },
      },
      yAxis: { type: "value", inverse: true, min: 1, minInterval: 1, ...axis },
      series: [{ name: t("rank.colPosition"), type: "line", connectNulls: false, symbolSize: 6, data: checks.map((check) => check.position) }],
    };
  }, [checks, tokens, t, locale]);
  return <EChart option={option} height={220} />;
}

/** One keyword on one place: its positions, every check and the latest result page. */
export function KeywordDialog({ project, tracker, row, onClose }: { project: ProjectRow; tracker: TrackerRow; row: KeywordRank; onClose: () => void }) {
  const { t, locale } = useI18n();
  const detail = useQuery({
    queryKey: ["rank", "keyword", project.id, row.keyword_id, tracker.id],
    queryFn: () => api.rankKeyword(project.id, row.keyword_id, tracker.id),
  });
  const checks = detail.data?.checks ?? [];
  return (
    <Modal title={row.keyword} className="wide" closeLabel={t("common.close")} onClose={onClose} footer={<Button onClick={onClose}>{t("common.close")}</Button>}>
      {detail.isLoading && <Loading label={t("common.loading")} />}
      {detail.error && <div className="oc-banner error">{explain(detail.error, t).message}</div>}
      {detail.data && (
        <div className="form-grid">
          <p className="faint small">{placeLabel(tracker, (name) => t(`rank.${name}`))}</p>
          <section>
            <h4>{t("rank.detailPositions")}</h4>
            {checks.length > 1 ? <PositionChart checks={checks.filter((check) => !check.error)} /> : <p className="faint">{t("rank.detailOneCheck")}</p>}
          </section>
          <section>
            <h4>{t("rank.detailResults")}</h4>
            <ol className="serp-list">
              {detail.data.results.map((result) => {
                const own = row.pages.some(([, url]) => url === result.url);
                return (
                  <li key={`${result.position}-${result.url}`} className={own ? "own" : undefined}>
                    <span className="serp-position">{result.position}</span>
                    <span className="serp-text">
                      <a href={result.url} target="_blank" rel="noreferrer noopener">
                        {result.title ?? result.url} <ExternalLink size={12} aria-hidden="true" />
                      </a>
                      <small className="faint">{result.domain}</small>
                    </span>
                  </li>
                );
              })}
            </ol>
          </section>
          <section>
            <h4>{t("rank.detailChecks")}</h4>
            <ul className="check-list">
              {[...checks].reverse().map((check) => (
                <li key={check.checked_at}>
                  <span>{relativeTime(check.checked_at, locale)}</span>
                  <strong>{check.error ? "!" : (check.position ?? t("rank.notRanked"))}</strong>
                  <span className="faint small">
                    {check.error ??
                      [
                        check.ai_overview ? (check.ai_cited ? t("rank.aiCited") : t("rank.aiShownOnly")) : null,
                        check.snippet ? t("rank.snippetOwned") : null,
                        check.pages.length > 1 ? t("rank.cannibalised") : null,
                        check.features
                          .filter((feature) => feature !== "organic")
                          .map((feature) => featureName(feature, t))
                          .join(", ") || null,
                      ]
                        .filter(Boolean)
                        .join(" · ")}
                  </span>
                  {check.ai_cited && <Sparkles size={14} className="cited" aria-label={t("rank.aiCited")} />}
                </li>
              ))}
            </ul>
          </section>
        </div>
      )}
    </Modal>
  );
}
