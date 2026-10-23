import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Layers, ListPlus, Network, Pencil, Sparkles, Trash2 } from "lucide-react";
import { Button, DataTable, Empty, Loading, type Column } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { ListKeyword, ProjectRow, Topic } from "../../api/types";
import { useI18n } from "../../i18n";
import { Entity } from "../../lib/Entity";
import { useGlossary } from "../../lib/glossary";
import { useShell } from "../../lib/context";
import { formatNumber, formatUsd } from "../../lib/format";
import { download, toCsv } from "../../lib/report";
import { featureName, tableLabels } from "../../lib/tables";
import { Difficulty, Sparkline } from "./KeywordTable";
import { SerpGroups } from "./SerpGroups";

function Topics({ topics, onAnalyze }: { topics: Topic[]; onAnalyze: (keyword: string) => void }) {
  const { t, locale } = useI18n();
  return (
    <div className="topic-grid">
      {topics.map((topic) => (
        <article key={topic.pillar} className="sub-panel topic">
          <header>
            <button type="button" className="link-button cell-main" onClick={() => onAnalyze(topic.pillar)}>
              {topic.pillar}
            </button>
            <span className="faint small">
              {t("keywords.topicSize", { n: topic.keywords.length, volume: formatNumber(topic.volume, locale) })}
            </span>
          </header>
          <ul>
            {topic.keywords.slice(1, 8).map((member) => (
              <li key={member.keyword}>
                <span>{member.keyword}</span>
                <span className="faint">{member.volume === null ? "" : formatNumber(member.volume, locale)}</span>
              </li>
            ))}
            {topic.keywords.length > 8 && <li className="faint">+{topic.keywords.length - 8}</li>}
          </ul>
        </article>
      ))}
    </div>
  );
}

export function ListsTab({ project, onAnalyze }: { project: ProjectRow; onAnalyze: (keyword: string) => void }) {
  const { t, locale } = useI18n();
  const gloss = useGlossary();
  const { fail } = useShell();
  const queryClient = useQueryClient();
  const lists = useQuery({ queryKey: ["keyword-lists", project.id], queryFn: () => api.keywordLists(project.id) });
  const [listId, setListId] = useState<number | null>(null);
  const [topics, setTopics] = useState<Topic[] | null>(null);
  const [serpOpen, setSerpOpen] = useState(false);
  useEffect(() => {
    if (lists.data && (listId === null || !lists.data.some((list) => list.id === listId))) setListId(lists.data[0]?.id ?? null);
  }, [lists.data, listId]);
  useEffect(() => {
    setTopics(null);
    setSerpOpen(false);
  }, [listId]);
  const keywords = useQuery({ queryKey: ["list-keywords", listId], queryFn: () => api.listKeywords(listId ?? 0), enabled: listId !== null });
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["keyword-lists", project.id] });
    void queryClient.invalidateQueries({ queryKey: ["list-keywords"] });
  };
  const failed = (thrown: unknown) => fail(thrown);
  const create = useMutation({
    mutationFn: (name: string) => api.createKeywordList(project.id, name),
    onSuccess: (list) => {
      refresh();
      setListId(list.id);
    },
    onError: failed,
  });
  const rename = useMutation({ mutationFn: ({ id, name }: { id: number; name: string }) => api.renameKeywordList(id, name), onSuccess: refresh, onError: failed });
  const remove = useMutation({ mutationFn: (id: number) => api.deleteKeywordList(id), onSuccess: refresh, onError: failed });
  const removeKeyword = useMutation({ mutationFn: (keyword: string) => api.removeFromKeywordList(listId ?? 0, [keyword]), onSuccess: refresh, onError: failed });
  const cluster = useMutation({ mutationFn: () => api.clusterKeywordList(listId ?? 0), onSuccess: setTopics, onError: failed });
  const current = lists.data?.find((list) => list.id === listId) ?? null;
  const number = (entry: ListKeyword, field: "volume" | "cpc" | "difficulty") => {
    const value = entry.data[field];
    return typeof value === "number" ? value : null;
  };
  const columns: Column<ListKeyword>[] = [
    {
      key: "keyword",
      label: t("overview.colKeyword"),
      render: (entry) => (
        <Entity kind="keyword" value={entry.keyword}>
          <strong className="cell-main">{entry.keyword}</strong>
        </Entity>
      ),
      sort: (entry) => entry.keyword,
    },
    { key: "volume", label: t("overview.colVolume"), help: gloss("volume"), render: (entry) => (number(entry, "volume") === null ? "—" : formatNumber(number(entry, "volume") ?? 0, locale)), sort: (entry) => number(entry, "volume"), numeric: true },
    { key: "difficulty", label: t("overview.colKd"), help: gloss("kd"), render: (entry) => <Difficulty value={number(entry, "difficulty")} />, sort: (entry) => number(entry, "difficulty"), numeric: true },
    { key: "cpc", label: t("overview.colCpc"), help: gloss("cpc"), render: (entry) => (number(entry, "cpc") === null ? "—" : formatUsd(number(entry, "cpc") ?? 0, locale)), sort: (entry) => number(entry, "cpc"), numeric: true },
    { key: "intent", label: t("overview.colIntent"), help: gloss("intent"), optional: true, render: (entry) => (entry.data.intent ? t(`overview.intent_${entry.data.intent}`) : ""), sort: (entry) => entry.data.intent ?? null },
    { key: "trend", label: t("keywords.colTrend"), optional: true, render: (entry) => <Sparkline values={(entry.data.monthly ?? []).slice(-12).map((month) => month.volume)} />, sort: (entry) => entry.data.trend?.yearly ?? null },
    {
      key: "features",
      label: t("overview.colFeatures"),
      help: gloss("serp_features"),
      optional: true,
      render: (entry) => {
        const features = entry.data.serp_features ?? [];
        const names = features.map((feature) => featureName(feature, t)).join(", ");
        return features.includes("ai_overview") ? (
          <span className="feature-ai" title={names}>
            <Sparkles size={13} aria-hidden="true" /> AI
          </span>
        ) : (
          <span className="faint" title={names}>
            {features.length || ""}
          </span>
        );
      },
      sort: (entry) => (entry.data.serp_features ?? []).length,
      numeric: true,
    },
    {
      key: "remove",
      label: "",
      render: (entry) => (
        <Button small className="icon ghost" aria-label={t("keywords.removeKeyword", { keyword: entry.keyword })} onClick={() => removeKeyword.mutate(entry.keyword)}>
          <Trash2 size={14} />
        </Button>
      ),
    },
  ];
  if (lists.isLoading) return <Loading label={t("common.loading")} />;
  return (
    <div className="page-stack">
      <div className="oc-card list-bar">
        <select className="oc-select" value={listId ?? ""} onChange={(event) => setListId(Number(event.target.value))} aria-label={t("keywords.list")}>
          {lists.data?.length === 0 && <option value="">{t("keywords.noLists")}</option>}
          {lists.data?.map((list) => (
            <option key={list.id} value={list.id}>
              {list.name} ({list.keywords})
            </option>
          ))}
        </select>
        <Button
          small
          onClick={() => {
            const name = window.prompt(t("keywords.newListName"), t("keywords.defaultListName"));
            if (name) create.mutate(name);
          }}
        >
          <ListPlus size={14} />
          {t("keywords.newList")}
        </Button>
        {current && (
          <>
            <Button
              small
              className="ghost"
              onClick={() => {
                const name = window.prompt(t("keywords.renameList"), current.name);
                if (name) rename.mutate({ id: current.id, name });
              }}
            >
              <Pencil size={14} />
              {t("common.edit")}
            </Button>
            <Button
              small
              className="ghost"
              onClick={() => {
                if (window.confirm(t("keywords.deleteListConfirm", { name: current.name }))) remove.mutate(current.id);
              }}
            >
              <Trash2 size={14} />
              {t("common.delete")}
            </Button>
            <span className="spacer" />
            <Button small disabled={current.keywords === 0} busy={cluster.isPending} onClick={() => cluster.mutate()}>
              <Layers size={14} />
              {t("keywords.cluster")}
            </Button>
            <Button small disabled={current.keywords === 0} aria-pressed={serpOpen} onClick={() => setSerpOpen(!serpOpen)}>
              <Network size={14} />
              {t("keywords.clusterSerp")}
            </Button>
            <Button
              small
              disabled={!keywords.data?.length}
              onClick={() =>
                download(
                  `${current.name}.csv`,
                  toCsv(
                    [t("overview.colKeyword"), t("overview.colVolume"), t("overview.colKd"), t("overview.colCpc"), t("overview.colIntent"), t("keywords.colTrendYear"), t("overview.colFeatures")],
                    (keywords.data ?? []).map((entry) => [entry.keyword, number(entry, "volume"), number(entry, "difficulty"), number(entry, "cpc"), entry.data.intent ?? "", entry.data.trend?.yearly ?? "", (entry.data.serp_features ?? []).join(" ")]),
                  ),
                )
              }
            >
              {t("overview.export")}
            </Button>
          </>
        )}
      </div>
      {!current && (
        <div className="oc-card">
          <Empty icon={<ListPlus size={28} />} title={t("keywords.listsEmptyTitle")}>
            <p>{t("keywords.listsEmptyText")}</p>
          </Empty>
        </div>
      )}
      {serpOpen && current && <SerpGroups project={project} list={current} />}
      {topics && (
        <section className="oc-panel report">
          <header className="report-head">
            <div className="report-title">
              <h3 className="oc-panel-title">{t("keywords.topicsTitle", { n: topics.length })}</h3>
              <span className="faint">{t("keywords.topicsHint")}</span>
            </div>
          </header>
          <Topics topics={topics} onAnalyze={onAnalyze} />
        </section>
      )}
      {current && keywords.data && (
        <DataTable rows={keywords.data} columns={columns} rowKey={(entry) => entry.keyword} initialSort={{ key: "volume", direction: "desc" }} tableId="keyword-list" empty={t("keywords.listEmpty")} labels={tableLabels(t, locale)} />
      )}
    </div>
  );
}
