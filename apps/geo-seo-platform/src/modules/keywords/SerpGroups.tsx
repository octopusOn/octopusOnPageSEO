// A list grouped by the result pages its keywords share: each group is what one page can
// target. The latest result page of each keyword is fetched once as a job (price first); the
// groups are drawn from what is kept, so other settings cost nothing.

import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Network, PenLine } from "lucide-react";
import { Button, Empty, Segmented } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { KeywordListRow, ProjectRow, SerpGroup } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { Entity } from "../../lib/Entity";
import { formatNumber } from "../../lib/format";
import { blockedText, CostLabel, refreshSpending } from "../../lib/PaidFrame";
import { shortUrl } from "../../lib/tables";

const THRESHOLDS = ["2", "3", "4", "5"] as const;

function GroupCard({ group }: { group: SerpGroup }) {
  const { t, locale } = useI18n();
  const { go } = useShell();
  return (
    <article className="sub-panel topic">
      <header>
        <Entity kind="keyword" value={group.pillar}>
          <strong className="cell-main">{group.pillar}</strong>
        </Entity>
        <span className="faint small">
          {t("keywords.topicSize", { n: group.keywords.length, volume: formatNumber(group.volume, locale) })}
          {group.difficulty !== null && <> · KD {Math.round(group.difficulty)}</>}
          {group.intent && <> · {t(`overview.intent_${group.intent}`)}</>}
        </span>
      </header>
      <ul>
        {group.keywords.slice(1, 10).map((member) => (
          <li key={member.keyword}>
            <span>{member.keyword}</span>
            <span className="faint">
              {member.volume === null ? "" : formatNumber(member.volume, locale)}
              {member.shared !== null && <small title={t("keywords.sharedResults")}> · {member.shared}/10</small>}
            </span>
          </li>
        ))}
        {group.keywords.length > 10 && <li className="faint">+{group.keywords.length - 10}</li>}
      </ul>
      <Button small className="ghost group-brief" onClick={() => go({ page: "content", params: { keyword: group.pillar } })}>
        <PenLine size={14} />
        {t("keywords.createBrief")}
      </Button>
      {group.pages.length > 0 && (
        <p className="faint small group-pages">
          {t("keywords.rankingPages")}{" "}
          {group.pages.slice(0, 3).map((page) => (
            <Entity key={page} kind="url" value={page}>
              <span className="cell-url">{shortUrl(page)}</span>
            </Entity>
          ))}
        </p>
      )}
    </article>
  );
}

export function SerpGroups({ project, list }: { project: ProjectRow; list: KeywordListRow }) {
  const { t, locale } = useI18n();
  const { jobs, fail, go } = useShell();
  const queryClient = useQueryClient();
  const [threshold, setThreshold] = useState<(typeof THRESHOLDS)[number]>("3");
  const [mode, setMode] = useState<"soft" | "hard">("soft");
  const estimate = useQuery({ queryKey: ["serp-clusters", "estimate", list.id, list.keywords], queryFn: () => api.serpClusterEstimate(list.id) });
  const groups = useQuery({ queryKey: ["serp-clusters", list.id, list.keywords, threshold, mode], queryFn: () => api.serpClusters(list.id, Number(threshold), mode) });
  const running = jobs.find((job) => job.kind === "serp_clusters" && job.project_id === project.id && job.status === "running");
  // When the job ends, what it kept is grouped again.
  const was = useRef(running?.id);
  useEffect(() => {
    if (was.current && !running) void queryClient.invalidateQueries({ queryKey: ["serp-clusters"] });
    was.current = running?.id;
  }, [running, queryClient]);
  const start = useMutation({
    mutationFn: () => api.startSerpClustering(list.id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
      refreshSpending(queryClient);
    },
    onError: (thrown) => fail(thrown),
  });
  const price = estimate.data?.estimate ?? null;
  const missing = estimate.data ? estimate.data.keywords - estimate.data.kept : 0;
  const blocked = blockedText(price ?? undefined, t);
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("keywords.serpGroupsTitle")}</h3>
          <span className="faint">{t("keywords.serpGroupsHint")}</span>
        </div>
        <div className="report-actions">
          <Segmented<(typeof THRESHOLDS)[number]> label={t("keywords.sharedResults")} value={threshold} onChange={setThreshold} options={THRESHOLDS.map((value) => ({ value, label: `${value}/10` }))} />
          <Segmented<"soft" | "hard">
            label={t("keywords.groupMode")}
            value={mode}
            onChange={setMode}
            options={[
              { value: "soft", label: t("keywords.groupSoft") },
              { value: "hard", label: t("keywords.groupHard") },
            ]}
          />
        </div>
      </header>
      {estimate.data && (
        <div className="oc-banner info" role="status">
          <span>
            {missing > 0
              ? t("keywords.pagesMissing", { kept: formatNumber(estimate.data.kept, locale), total: formatNumber(estimate.data.keywords, locale) })
              : t("keywords.pagesAllKept", { total: formatNumber(estimate.data.keywords, locale) })}
          </span>
          {running && <span className="faint small">{t("keywords.pagesRunning", { done: running.done, total: running.total })}</span>}
          {missing > 0 && !running && price && (
            <>
              {!blocked && <CostLabel estimate={price} />}
              <Button small primary busy={start.isPending} disabled={Boolean(blocked)} title={blocked ?? undefined} onClick={() => start.mutate()}>
                {t("keywords.fetchPages", { n: formatNumber(missing, locale) })}
              </Button>
              {blocked && (
                <Button small onClick={() => go({ page: "sources" })}>
                  {t("common.goToSources")}
                </Button>
              )}
            </>
          )}
        </div>
      )}
      {groups.data && groups.data.groups.length === 0 && (
        <Empty icon={<Network size={26} />} title={t("keywords.serpGroupsNone")}>
          <p>{t("keywords.serpGroupsNoneText")}</p>
        </Empty>
      )}
      {groups.data && groups.data.groups.length > 0 && (
        <div className="topic-grid">
          {groups.data.groups.map((group) => (
            <GroupCard key={group.pillar} group={group} />
          ))}
        </div>
      )}
      {groups.data && groups.data.without_page.length > 0 && groups.data.groups.length > 0 && (
        <p className="faint small">{t("keywords.withoutPage", { n: groups.data.without_page.length, keywords: groups.data.without_page.slice(0, 8).join(", ") })}</p>
      )}
    </section>
  );
}
