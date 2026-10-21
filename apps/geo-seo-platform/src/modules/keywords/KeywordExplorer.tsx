import { useMemo, useState, type FormEvent } from "react";
import { useQuery } from "@tanstack/react-query";
import { ExternalLink, KeyRound, Search, Sparkles } from "lucide-react";
import { Button, Empty, Segmented, Stat } from "@octopusseo/ui";
import { api } from "../../api/client";
import { openExternal } from "../../api/transport";
import type { KeywordData, KeywordList, ProjectRow, SerpPage } from "../../api/types";
import { useI18n } from "../../i18n";
import { axisStyle, EChart, useChartTokens, type ChartOption } from "../../lib/EChart";
import { formatCompact, formatNumber, formatUsd } from "../../lib/format";
import { PaidFrame } from "../../lib/PaidFrame";
import { isQuestion } from "../../lib/questions";
import { featureName } from "../../lib/tables";
import { Difficulty, KeywordTable } from "./KeywordTable";
import { ListsTab } from "./ListsTab";

const TABS = ["overview", "ideas", "serp", "wordstat", "lists"] as const;
type Tab = (typeof TABS)[number];
type IdeaKind = "suggestions" | "questions" | "related" | "ideas";

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
  const percent = (value: number | null) => (value === null ? "—" : `${value > 0 ? "+" : ""}${value}%`);
  return (
    <div className="page-stack">
      <div className="oc-stats">
        <Stat label={t("overview.colVolume")} value={keyword.volume === null ? "—" : formatNumber(keyword.volume, locale)} hint={t("keywords.perMonth")} />
        <Stat
          label={t("keywords.difficulty")}
          value={
            <span className="kd-big">
              <Difficulty value={keyword.difficulty} />
            </span>
          }
          hint={difficultyLabel(keyword.difficulty, t)}
        />
        <Stat label={t("overview.colCpc")} value={keyword.cpc === null ? "—" : formatUsd(keyword.cpc, locale)} hint={keyword.competition_level ? t(`keywords.competition_${keyword.competition_level.toLowerCase()}`) : undefined} />
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

function Autocomplete({ projectId, seed, onAnalyze }: { projectId: number; seed: string; onAnalyze: (keyword: string) => void }) {
  const { t } = useI18n();
  const [engine, setEngine] = useState<"google" | "yandex">("google");
  const suggestions = useQuery({ queryKey: ["autocomplete", projectId, seed, engine], queryFn: () => api.keywordAutocomplete(projectId, seed, engine), staleTime: 3_600_000 });
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
  const [input, setInput] = useState("");
  const [asked, setAsked] = useState<string[]>([]);
  const [tab, setTab] = useState<Tab>("overview");
  const [ideaKind, setIdeaKind] = useState<IdeaKind>("suggestions");
  const [limit, setLimit] = useState(100);
  const [mobile, setMobile] = useState(false);
  const seed = asked[0] ?? "";
  const analyze = (keyword: string) => {
    setInput(keyword);
    setAsked([keyword]);
    setTab("overview");
  };
  const submit = (event: FormEvent) => {
    event.preventDefault();
    const list = words(input);
    if (list.length > 0) setAsked(list);
  };
  const tabLabels: Record<Tab, string> = {
    overview: t("keywords.tabOverview"),
    ideas: t("keywords.tabIdeas"),
    serp: t("keywords.tabSerp"),
    wordstat: "Yandex Wordstat",
    lists: t("keywords.tabLists"),
  };
  const visibleTabs = TABS.filter((entry) => entry !== "wordstat" || YANDEX_COUNTRIES.includes(project.country));
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
                const list = words(input);
                if (list.length > 0) setAsked(list);
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
              ask={{ project_id: project.id, report: "overview", keywords: asked }}
              title={asked.length === 1 ? seed : t("keywords.bulkTitle", { n: asked.length })}
              hint={t("keywords.overviewHint")}
            >
              {(data) =>
                asked.length === 1 && data.keywords[0] ? (
                  <KeywordCard keyword={data.keywords[0]} />
                ) : (
                  <KeywordTable rows={data.keywords} projectId={project.id} fileName={`${project.domain}-keywords`} onAnalyze={analyze} />
                )
              }
            </PaidFrame>
            {asked.length === 1 && <Autocomplete projectId={project.id} seed={seed} onAnalyze={analyze} />}
          </div>
        )}
        {tab === "ideas" && asked.length > 0 && (
          <PaidFrame<KeywordList>
            module="keywords"
            ask={{
              project_id: project.id,
              report: ideaKind === "questions" ? "suggestions" : ideaKind,
              ...(ideaKind === "ideas" ? { keywords: asked } : { seed }),
              ...(ideaKind === "related" ? { depth: 2 } : {}),
            }}
            title={t(`keywords.ideas_${ideaKind}`)}
            hint={t(`keywords.ideasHint_${ideaKind}`)}
            limit={limit}
            onLimit={setLimit}
            extra={
              <Segmented<IdeaKind>
                label={t("keywords.tabIdeas")}
                value={ideaKind}
                onChange={setIdeaKind}
                options={(["suggestions", "questions", "related", "ideas"] as const).map((kind) => ({ value: kind, label: t(`keywords.ideas_${kind}`) }))}
              />
            }
          >
            {(data) => (
              <div className="page-stack">
                <p className="faint small">{t("keywords.ideasTotal", { shown: data.keywords.length, total: data.total })}</p>
                <KeywordTable
                  rows={data.keywords}
                  projectId={project.id}
                  fileName={`${seed}-${ideaKind}`}
                  onAnalyze={analyze}
                  extraFilter={ideaKind === "questions" ? questions : undefined}
                />
              </div>
            )}
          </PaidFrame>
        )}
        {tab === "serp" && asked.length > 0 && (
          <PaidFrame<SerpPage>
            module="keywords"
            ask={{ project_id: project.id, report: "serp", seed, mobile }}
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
        )}
        {tab === "wordstat" && asked.length > 0 && (
          <PaidFrame<KeywordList> module="keywords" ask={{ project_id: project.id, report: "wordstat", seed }} title={`Wordstat: ${seed}`} hint={t("keywords.wordstatHint")}>
            {(data) => <KeywordTable rows={data.keywords} projectId={project.id} fileName={`${seed}-wordstat`} onAnalyze={analyze} />}
          </PaidFrame>
        )}
        {tab === "lists" && <ListsTab project={project} onAnalyze={analyze} />}
      </div>
    </div>
  );
}
