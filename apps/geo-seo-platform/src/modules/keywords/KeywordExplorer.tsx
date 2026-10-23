import { useEffect, useMemo, useState, type FormEvent } from "react";
import { useQuery } from "@tanstack/react-query";
import { ExternalLink, KeyRound, PenLine, Search, Sparkles } from "lucide-react";
import { Button, Empty, Segmented, Stat } from "@octopusseo/ui";
import { api } from "../../api/client";
import { openExternal } from "../../api/transport";
import type { KeywordData, KeywordList, ProjectRow, SerpPage } from "../../api/types";
import { useI18n } from "../../i18n";
import { useGlossary } from "../../lib/glossary";
import { useShell } from "../../lib/context";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { formatCompact, formatNumber, formatUsd } from "../../lib/format";
import { FilterBar, type FilterPreset } from "../../lib/FilterBar";
import { ListSeen } from "../../lib/ListSeen";
import { marketOf } from "../../lib/market";
import { MarketPicker } from "../../lib/MarketPicker";
import { PaidFrame } from "../../lib/PaidFrame";
import { KEYWORD_FILTERS, withFilter, type QueryFilter } from "../../lib/query";
import { useListQuery } from "../../lib/useListQuery";
import { isQuestion, questionPattern } from "../../lib/questions";
import { featureName } from "../../lib/tables";
import { Difficulty, KeywordTable } from "./KeywordTable";
import { ListsTab } from "./ListsTab";
import { CountryVolumes, ParentTopicReport, SerpHistory, SerpWeights, VolumeHistory } from "./Research";
import { wordGroups, type WordGroup } from "./ideas";

const TABS = ["overview", "ideas", "serp", "wordstat", "lists"] as const;
type Tab = (typeof TABS)[number];
type IdeaKind = "suggestions" | "exact" | "questions" | "related" | "ideas";

/** Match types and kinds of ideas, broadest last. */
const IDEA_KINDS: IdeaKind[] = ["suggestions", "exact", "questions", "related", "ideas"];

/** Ideas sharing a word, a click away from a filter (the Keyword Magic groups). */
function WordGroups({ groups, active, onPick }: { groups: WordGroup[]; active: string | null; onPick: (word: string | null) => void }) {
  const { t, locale } = useI18n();
  if (groups.length === 0) return null;
  return (
    <aside className="word-groups" aria-label={t("keywords.groupsTitle")}>
      <h4>{t("keywords.groupsTitle")}</h4>
      <p className="faint small">{t("keywords.groupsHint")}</p>
      <button type="button" className={`word-group${active === null ? " on" : ""}`} onClick={() => onPick(null)}>
        <span>{t("keywords.groupsAll")}</span>
      </button>
      {groups.map((group) => (
        <button key={group.word} type="button" className={`word-group${active === group.word ? " on" : ""}`} aria-pressed={active === group.word} onClick={() => onPick(active === group.word ? null : group.word)}>
          <span>{group.word}</span>
          <span className="faint small">
            {formatNumber(group.keywords, locale)} · {formatCompact(group.volume, locale)}
          </span>
        </button>
      ))}
    </aside>
  );
}

/** Countries Yandex Wordstat knows well. */
const YANDEX_COUNTRIES = ["tr", "ru", "kz", "by", "uz", "az", "ge", "ua"];

function words(text: string): string[] {
  return text
    .split(/[\n,;]/)
    .map((line) => line.trim())
    .filter(Boolean);
}

function difficultyLabel(value: number | null, t: (key: string) => string): string {
  if (value === null) return "";
  if (value < 15) return t("keywords.kdVeryEasy");
  if (value < 30) return t("keywords.kdEasy");
  if (value < 50) return t("keywords.kdMedium");
  if (value < 70) return t("keywords.kdHard");
  return t("keywords.kdVeryHard");
}

function TrendChart({ keyword }: { keyword: KeywordData }) {
  const { locale, t } = useI18n();
  const tokens = useChartTokens();
  const option = useMemo<ChartOption>(() => {
    const axis = axisStyle(tokens);
    return {
      grid: { left: 48, right: 12, top: 12, bottom: 24 },
      tooltip: { trigger: "axis" },
      xAxis: {
        type: "category",
        data: keyword.monthly.map((month) => new Date(Date.UTC(month.year, month.month - 1, 1)).toLocaleDateString(locale, { month: "short", year: "2-digit" })),
        ...axis,
        splitLine: { show: false },
      },
      yAxis: { type: "value", ...axis, axisLabel: { ...axis.axisLabel, formatter: (value: number) => formatCompact(value, locale) } },
      series: [{ name: t("overview.colVolume"), type: "bar", barMaxWidth: 18, data: keyword.monthly.map((month) => month.volume) }],
    };
  }, [keyword, tokens, locale, t]);
  if (keyword.monthly.length === 0) return null;
  return <EChart option={option} height={200} />;
}

function KeywordCard({ keyword }: { keyword: KeywordData }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const percent = (value: number | null) => (value === null ? "—" : `${value > 0 ? "+" : ""}${value}%`);
  return (
    <div className="page-stack">
      <div className="oc-stats">
        <Stat label={t("overview.colVolume")} help={gloss("volume")} value={keyword.volume === null ? "—" : formatNumber(keyword.volume, locale)} hint={t("keywords.perMonth")} />
        <Stat
          label={t("keywords.difficulty")}
          value={
            <span className="kd-big">
              <Difficulty value={keyword.difficulty} />
            </span>
          }
          hint={difficultyLabel(keyword.difficulty, t)}
        />
        <Stat label={t("overview.colCpc")} help={gloss("cpc")} value={keyword.cpc === null ? "—" : formatUsd(keyword.cpc, locale)} hint={keyword.competition_level ? t(`keywords.competition_${keyword.competition_level.toLowerCase()}`) : undefined} />
        <Stat
          label={t("overview.colIntent")}
          value={keyword.intent ? t(`overview.intent_${keyword.intent}`) : "—"}
          hint={keyword.other_intents.map((intent) => t(`overview.intent_${intent}`)).join(", ") || undefined}
        />
      </div>
      <div className="two-columns">
        <section className="sub-panel">
          <h4>{t("keywords.trendTitle")}</h4>
          <TrendChart keyword={keyword} />
          <p className="faint small">
            {t("keywords.trendChange", { monthly: percent(keyword.trend.monthly), quarterly: percent(keyword.trend.quarterly), yearly: percent(keyword.trend.yearly) })}
          </p>
        </section>
        <section className="sub-panel">
          <h4>{t("keywords.serpTitle")}</h4>
          <div className="feature-chips">
            {keyword.serp_features.length === 0 && <span className="faint">—</span>}
            {keyword.serp_features.map((feature) => (
              <span key={feature} className={`oc-chip ${feature === "ai_overview" ? "accent" : ""}`}>
                {feature === "ai_overview" && <Sparkles size={12} aria-hidden="true" />} {featureName(feature, t)}
              </span>
            ))}
          </div>
          {keyword.top_referring_domains !== null && <p className="small">{t("keywords.topNeeds", { n: formatNumber(keyword.top_referring_domains, locale) })}</p>}
          {keyword.results !== null && <p className="faint small">{t("keywords.results", { n: formatCompact(keyword.results, locale) })}</p>}
        </section>
      </div>
    </div>
  );
}

/** Ready filters of keyword ideas. */
function ideaPresets(t: (key: string) => string): FilterPreset[] {
  return [
    { id: "easy", label: t("keywords.presetEasy"), filters: [{ field: "kd", op: "le", value: 29 }] },
    { id: "searched", label: t("keywords.presetSearched"), filters: [{ field: "volume", op: "ge", value: 100 }] },
    { id: "ai", label: t("query.presetAi"), filters: [{ field: "features", op: "has", value: "ai_overview" }] },
    { id: "buying", label: t("query.presetBuying"), filters: [{ field: "intent", op: "in", value: ["commercial", "transactional"] }] },
    { id: "info", label: t("overview.intent_informational"), filters: [{ field: "intent", op: "eq", value: "informational" }] },
  ];
}

function Autocomplete({ projectId, seed, market, onAnalyze }: { projectId: number; seed: string; market?: string; onAnalyze: (keyword: string) => void }) {
  const { t } = useI18n();
  const [engine, setEngine] = useState<"google" | "yandex">("google");
  const suggestions = useQuery({ queryKey: ["autocomplete", projectId, seed, engine, market], queryFn: () => api.keywordAutocomplete(projectId, seed, engine, market), staleTime: 3_600_000 });
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("keywords.autocompleteTitle")}</h3>
          <span className="faint">{t("keywords.autocompleteHint")}</span>
        </div>
        <Segmented<"google" | "yandex">
          label={t("keywords.engine")}
          value={engine}
          onChange={setEngine}
          options={[
            { value: "google", label: "Google" },
            { value: "yandex", label: "Yandex" },
          ]}
        />
      </header>
      {suggestions.error && <p className="faint small">{t("keywords.autocompleteFailed")}</p>}
      <div className="feature-chips">
        {suggestions.data?.map((keyword) => (
          <button key={keyword} type="button" className="oc-chip chip-button" onClick={() => onAnalyze(keyword)}>
            {keyword}
          </button>
        ))}
        {suggestions.data?.length === 0 && <span className="faint">—</span>}
      </div>
    </section>
  );
}

function SerpView({ page }: { page: SerpPage }) {
  const { t, locale } = useI18n();
  const features = page.features.filter((feature) => feature !== "organic");
  return (
    <div className="page-stack">
      <div className="feature-chips">
        {features.map((feature) => (
          <span key={feature} className={`oc-chip ${feature === "ai_overview" ? "accent" : ""}`}>
            {featureName(feature, t)}
          </span>
        ))}
        {page.results !== null && <span className="faint small">{t("keywords.results", { n: formatCompact(page.results, locale) })}</span>}
        {page.check_url && (
          <Button small className="ghost" onClick={() => openExternal(page.check_url ?? "", false)}>
            <ExternalLink size={13} />
            {t("keywords.openSerp")}
          </Button>
        )}
      </div>
      <ol className="serp-list">
        {page.organic.map((item) => (
          <li key={`${item.rank}-${item.url}`}>
            <span className="serp-rank">{item.rank}</span>
            <div>
              <strong>{item.title}</strong>
              <span className="serp-url">{item.url}</span>
              {item.description && <p className="faint small">{item.description}</p>}
            </div>
          </li>
        ))}
      </ol>
    </div>
  );
}

export function KeywordExplorer({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const { params, go } = useShell();
  // One keyword and the tab live in the address; a pasted list stays on the page.
  const [input, setInput] = useState(params.keyword ?? "");
  const [asked, setAsked] = useState<string[]>(() => (params.keyword ? [params.keyword] : []));
  useEffect(() => {
    if (!params.keyword) return;
    setAsked([params.keyword]);
    setInput(params.keyword);
  }, [params.keyword]);
  const market = params.market;
  const visibleTabs = TABS.filter((entry) => entry !== "wordstat" || YANDEX_COUNTRIES.includes(marketOf(project, market).country));
  const tab: Tab = (visibleTabs as readonly string[]).includes(params.tab ?? "") ? (params.tab as Tab) : "overview";
  const setTab = (next: Tab) => go({ params: { ...params, tab: next } });
  const [ideaKind, setIdeaKind] = useState<IdeaKind>("suggestions");
  const [limit, setLimit] = useState(100);
  const [mobile, setMobile] = useState(false);
  const seed = asked[0] ?? "";
  const ideas = useListQuery(JSON.stringify([project.id, ideaKind, asked, limit, market]));
  const presets = useMemo(() => ideaPresets(t), [t]);
  const inMarket = market ? { market } : {};
  // Questions are asked of DataForSEO as a pattern, so they reach every row it holds.
  const pattern = questionPattern(marketOf(project, market).language);
  const base: QueryFilter[] = ideaKind === "questions" && pattern ? [{ field: "keyword", op: "regex", value: pattern }] : [];
  const ideasAsked = base.length > 0 ? { ...ideas.asked, filters: [...base, ...ideas.asked.filters] } : ideas.asked;
  const pickedGroup = ideas.applied.filters.find((filter) => filter.field === "keyword" && filter.op === "contains");
  const pickGroup = (word: string | null) => {
    const others = ideas.applied.filters.filter((filter) => !(filter.field === "keyword" && filter.op === "contains"));
    ideas.change({ ...ideas.applied, filters: word ? withFilter(others, { field: "keyword", op: "contains", value: word }) : others });
  };
  const analyze = (keyword: string) => {
    setInput(keyword);
    setAsked([keyword]);
    go({ params: { ...params, keyword, tab: "overview" } });
  };
  const ask = (list: string[]) => {
    if (list.length === 0) return;
    setAsked(list);
    go({ params: { ...params, keyword: list.length === 1 ? list[0] : undefined } });
  };
  const submit = (event: FormEvent) => {
    event.preventDefault();
    ask(words(input));
  };
  const tabLabels: Record<Tab, string> = {
    overview: t("keywords.tabOverview"),
    ideas: t("keywords.tabIdeas"),
    serp: t("keywords.tabSerp"),
    wordstat: "Yandex Wordstat",
    lists: t("keywords.tabLists"),
  };
  const questions = (row: KeywordData) => isQuestion(row.keyword, project.language);
  return (
    <div className="page-stack">
      <form className="oc-card domain-bar" onSubmit={submit}>
        <label className="domain-input keyword-input">
          <span className="sr-only">{t("keywords.seedLabel")}</span>
          <Search size={16} aria-hidden="true" />
          <textarea
            className="oc-input"
            rows={Math.min(4, Math.max(1, input.split("\n").length))}
            value={input}
            onChange={(event) => setInput(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                ask(words(input));
              }
            }}
            placeholder={t("keywords.seedPlaceholder")}
            spellCheck={false}
          />
        </label>
        <Button type="submit" primary>
          {t("keywords.analyze")}
        </Button>
        <span className="faint small">{t("keywords.seedHint")}</span>
        <MarketPicker project={project} />
      </form>
      <div className="oc-tabs" role="tablist" aria-label={t("keywords.seedLabel")}>
        {visibleTabs.map((entry) => (
          <button key={entry} type="button" role="tab" aria-selected={tab === entry} className={`oc-tab ${tab === entry ? "active" : ""}`} onClick={() => setTab(entry)}>
            {tabLabels[entry]}
          </button>
        ))}
      </div>
      <div role="tabpanel">
        {tab !== "lists" && asked.length === 0 && (
          <div className="oc-card">
            <Empty icon={<KeyRound size={28} />} title={t("keywords.startTitle")}>
              <p>{t("keywords.startText")}</p>
            </Empty>
          </div>
        )}
        {tab === "overview" && asked.length > 0 && (
          <div className="page-stack">
            <PaidFrame<KeywordList>
              module="keywords"
              ask={{ project_id: project.id, report: "overview", keywords: asked, ...inMarket }}
              title={asked.length === 1 ? seed : t("keywords.bulkTitle", { n: asked.length })}
              hint={t("keywords.overviewHint")}
              extra={
                asked.length === 1 ? (
                  <Button small onClick={() => go({ page: "content", params: { keyword: seed, market: params.market } })}>
                    <PenLine size={14} />
                    {t("keywords.createBrief")}
                  </Button>
                ) : undefined
              }
            >
              {(data) =>
                asked.length === 1 && data.keywords[0] ? (
                  <KeywordCard keyword={data.keywords[0]} />
                ) : (
                  <KeywordTable rows={data.keywords} projectId={project.id} fileName={`${project.domain}-keywords`} />
                )
              }
            </PaidFrame>
            {asked.length === 1 && (
              <>
                <VolumeHistory projectId={project.id} seed={seed} market={market} />
                <ParentTopicReport projectId={project.id} seed={seed} market={market} />
                <CountryVolumes projectId={project.id} seed={seed} />
                <Autocomplete projectId={project.id} seed={seed} market={market} onAnalyze={analyze} />
              </>
            )}
          </div>
        )}
        {tab === "ideas" && asked.length > 0 && (
          <PaidFrame<KeywordList>
            module="keywords"
            ask={{
              project_id: project.id,
              report: ideaKind === "questions" || ideaKind === "exact" ? "suggestions" : ideaKind,
              ...(ideaKind === "ideas" ? { keywords: asked } : { seed }),
              ...(ideaKind === "related" ? { depth: 2 } : {}),
              ...(ideaKind === "exact" ? { exact: true } : {}),
              ...inMarket,
            }}
            title={t(`keywords.ideas_${ideaKind}`)}
            hint={t(`keywords.ideasHint_${ideaKind}`)}
            limit={limit}
            onLimit={setLimit}
            query={ideasAsked}
            onOffset={ideas.setOffset}
            toolbar={<FilterBar fields={KEYWORD_FILTERS} query={ideas.applied} onChange={ideas.change} scope={ideas.scope} presets={presets} />}
            extra={
              <Segmented<IdeaKind>
                label={t("keywords.tabIdeas")}
                value={ideaKind}
                onChange={setIdeaKind}
                options={IDEA_KINDS.map((kind) => ({ value: kind, label: t(`keywords.ideas_${kind}`) }))}
              />
            }
          >
            {(data) => {
              const groups = wordGroups(data.keywords, seed, marketOf(project, market).language);
              return (
                <ListSeen seen={ideas.seen} rows={data.keywords.length} total={data.total}>
                  <div className={groups.length > 0 ? "magic" : undefined}>
                    <WordGroups groups={groups} active={typeof pickedGroup?.value === "string" ? pickedGroup.value : null} onPick={pickGroup} />
                    <div className="page-stack">
                      <p className="faint small">{t("keywords.ideasTotal", { shown: data.keywords.length, total: data.total })}</p>
                      <KeywordTable
                        rows={data.keywords}
                        projectId={project.id}
                        fileName={`${seed}-${ideaKind}`}
                        extraFilter={ideaKind === "questions" && !pattern ? questions : undefined}
                        query={{ filters: ideas.local, order: ideas.applied.order }}
                      />
                    </div>
                  </div>
                </ListSeen>
              );
            }}
          </PaidFrame>
        )}
        {tab === "serp" && asked.length > 0 && (
          <div className="page-stack">
          <PaidFrame<SerpPage>
            module="keywords"
            ask={{ project_id: project.id, report: "serp", seed, mobile, ...inMarket }}
            title={t("keywords.serpPageTitle", { keyword: seed })}
            hint={t("keywords.serpPageHint")}
            extra={
              <Segmented<string>
                label={t("keywords.device")}
                value={mobile ? "mobile" : "desktop"}
                onChange={(value) => setMobile(value === "mobile")}
                options={[
                  { value: "desktop", label: t("keywords.desktop") },
                  { value: "mobile", label: t("keywords.mobile") },
                ]}
              />
            }
          >
            {(page) => <SerpView page={page} />}
          </PaidFrame>
          <SerpWeights projectId={project.id} seed={seed} mobile={mobile} market={market} />
          <SerpHistory projectId={project.id} seed={seed} market={market} />
          </div>
        )}
        {tab === "wordstat" && asked.length > 0 && (
          <PaidFrame<KeywordList> module="keywords" ask={{ project_id: project.id, report: "wordstat", seed, ...inMarket }} title={`Wordstat: ${seed}`} hint={t("keywords.wordstatHint")}>
            {(data) => <KeywordTable rows={data.keywords} projectId={project.id} fileName={`${seed}-wordstat`} />}
          </PaidFrame>
        )}
        {tab === "lists" && <ListsTab project={project} onAnalyze={analyze} />}
      </div>
    </div>
  );
}
