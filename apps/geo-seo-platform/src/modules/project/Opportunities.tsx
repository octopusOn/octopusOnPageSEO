// Opportunities: what to do next, the most valuable for the least work first, each with what
// it would bring, how much work it is and where to do it; marked done, snoozed for a week or
// ignored.

import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlarmClock, ArrowRight, Check, EyeOff, Lightbulb, PenLine, RotateCcw } from "lucide-react";
import { Button, Empty, Loading, Segmented } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { Effort, Opportunity, OpportunitySources, OpportunityState, ProjectRow } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { explain } from "../../lib/errors";
import { formatNumber } from "../../lib/format";
import { moduleOf } from "../registry";
import { ALL_OPPORTUNITIES, AREAS, BRIEF_RULES, filterOpportunities, textValues, titleKey, type Area, type OpportunityFilter } from "./opportunity";

/** Gain or impact, as one short line. */
function Worth({ item }: { item: Opportunity }) {
  const { t, locale } = useI18n();
  return (
    <span className="opportunity-worth">
      {item.gain !== null && item.gain > 0 ? (
        <strong title={t("opportunities.gainHint")}>{t("opportunities.gain", { n: formatNumber(Math.round(item.gain), locale) })}</strong>
      ) : (
        <span className="impact-bar" title={t("opportunities.impactHint", { n: Math.round(item.impact) })} aria-label={t("opportunities.impactHint", { n: Math.round(item.impact) })}>
          <span style={{ width: `${item.impact}%` }} />
        </span>
      )}
      <span className={`oc-chip small effort-${item.effort}`}>{t(`opportunities.effort_${item.effort}`)}</span>
    </span>
  );
}

export function OpportunityRow({ project, item, compact = false }: { project: ProjectRow; item: Opportunity; compact?: boolean }) {
  const { t, locale } = useI18n();
  const { go, fail } = useShell();
  const queryClient = useQueryClient();
  const decide = useMutation({
    mutationFn: ({ state, days }: { state: OpportunityState; days?: number }) => api.setOpportunity(project.id, item.key, state, days),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["opportunities"] });
      void queryClient.invalidateQueries({ queryKey: ["project-dashboard"] });
    },
    onError: (thrown) => fail(thrown),
  });
  const entry = moduleOf(item.page);
  const Icon = entry.icon;
  const values = textValues(item, locale);
  const keyword = item.params.keyword ?? (item.rule === "low_content" ? String(item.detail.keyword ?? "") : "");
  return (
    <li className={`opportunity${item.state !== "open" ? ` state-${item.state}` : ""}`}>
      <span className={`oc-tile small ${entry.tone}`} aria-hidden="true">
        <Icon size={14} />
      </span>
      <div className="opportunity-body">
        <strong className="opportunity-title">{t(titleKey(item), values)}</strong>
        {!compact && <span className="faint small">{t(`opportunities.how_${item.rule}`, values)}</span>}
        <span className="opportunity-meta">
          <Worth item={item} />
          <span className="faint small">{t(entry.label)}</span>
          {item.returned && <span className="oc-chip small warning">{t("opportunities.returned")}</span>}
          {item.state === "snoozed" && item.until && <span className="faint small">{t("opportunities.snoozedUntil", { day: new Date(item.until).toLocaleDateString(locale) })}</span>}
          {item.state !== "open" && item.state !== "snoozed" && <span className="oc-chip small">{t(`opportunities.state_${item.state}`)}</span>}
        </span>
      </div>
      <div className="opportunity-actions">
        <Button small onClick={() => go({ page: item.page, params: item.params })} title={t("opportunities.open")}>
          <ArrowRight size={14} />
          {!compact && t("opportunities.open")}
        </Button>
        {BRIEF_RULES.includes(item.rule) && keyword && !compact && (
          <Button small className="ghost" onClick={() => go({ page: "content", params: { keyword } })} title={t("keywords.createBrief")}>
            <PenLine size={14} />
          </Button>
        )}
        {item.state === "open" ? (
          <>
            <Button small className="ghost" busy={decide.isPending && decide.variables?.state === "done"} onClick={() => decide.mutate({ state: "done" })} aria-label={t("opportunities.markDone")} title={t("opportunities.markDone")}>
              <Check size={14} />
            </Button>
            {!compact && (
              <>
                <Button small className="ghost" busy={decide.isPending && decide.variables?.state === "snoozed"} onClick={() => decide.mutate({ state: "snoozed", days: 7 })} aria-label={t("opportunities.snooze")} title={t("opportunities.snooze")}>
                  <AlarmClock size={14} />
                </Button>
                <Button small className="ghost" busy={decide.isPending && decide.variables?.state === "ignored"} onClick={() => decide.mutate({ state: "ignored" })} aria-label={t("opportunities.ignore")} title={t("opportunities.ignore")}>
                  <EyeOff size={14} />
                </Button>
              </>
            )}
          </>
        ) : (
          <Button small className="ghost" busy={decide.isPending} onClick={() => decide.mutate({ state: "open" })} aria-label={t("opportunities.reopen")} title={t("opportunities.reopen")}>
            <RotateCcw size={14} />
          </Button>
        )}
      </div>
    </li>
  );
}

/** What to connect or run for more opportunities. */
export function MissingSources({ sources }: { sources: OpportunitySources }) {
  const { t } = useI18n();
  const { go } = useShell();
  const missing = (
    [
      ["rank", "rank"],
      ["gsc", "gsc"],
      ["audit", "audit"],
      ["ai", "ai"],
      ["vitals", "audit"],
      ["content", "content"],
    ] as const
  ).filter(([source]) => !sources[source]);
  if (missing.length === 0) return null;
  return (
    <div className="missing-sources">
      <span className="faint small">{t("opportunities.moreFrom")}</span>
      {missing.map(([source, page]) => (
        <Button key={source} small className="ghost" onClick={() => go({ page, params: source === "vitals" ? { tab: "vitals" } : {} })}>
          {t(`opportunities.source_${source}`)}
        </Button>
      ))}
    </div>
  );
}

export function OpportunitiesTab({ project }: { project: ProjectRow }) {
  const { t, lang, locale } = useI18n();
  const [filter, setFilter] = useState<OpportunityFilter>(ALL_OPPORTUNITIES);
  const [all, setAll] = useState(false);
  const list = useQuery({ queryKey: ["opportunities", project.id, lang, all], queryFn: () => api.opportunities(project.id, lang, all) });
  if (list.isLoading) return <Loading label={t("opportunities.working")} />;
  if (list.error || !list.data) return <p className="warn">{explain(list.error, t).message}</p>;
  const data = list.data;
  const shown = filterOpportunities(data.items, filter);
  const hidden = Object.values(data.hidden).reduce((sum, n) => sum + (n ?? 0), 0);
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("opportunities.title")}</h3>
          <span className="faint">
            {t("opportunities.summary", { n: formatNumber(data.open, locale), gain: formatNumber(Math.round(data.gain), locale) })}
          </span>
        </div>
        <div className="report-actions">
          <label className="oc-check">
            <input type="checkbox" checked={all} onChange={(event) => setAll(event.target.checked)} />
            <span>{t("opportunities.showHidden", { n: hidden })}</span>
          </label>
        </div>
      </header>
      <div className="filters">
        <select className="oc-select" aria-label={t("opportunities.area")} value={filter.area} onChange={(event) => setFilter({ ...filter, area: event.target.value as Area })}>
          {AREAS.map((area) => (
            <option key={area} value={area}>
              {area === "all" ? t("opportunities.areaAll") : t(moduleOf(area).label)}
            </option>
          ))}
        </select>
        <Segmented<Effort | "all">
          label={t("opportunities.effort")}
          value={filter.effort}
          onChange={(effort) => setFilter({ ...filter, effort })}
          options={[
            { value: "all", label: t("opportunities.effortAll") },
            { value: "low", label: t("opportunities.effort_low") },
            { value: "medium", label: t("opportunities.effort_medium") },
            { value: "high", label: t("opportunities.effort_high") },
          ]}
        />
      </div>
      <MissingSources sources={data.sources} />
      {shown.length === 0 ? (
        <Empty icon={<Lightbulb size={26} />} title={t("opportunities.noneTitle")}>
          <p>{data.items.length ? t("opportunities.noneFiltered") : t("opportunities.noneText")}</p>
        </Empty>
      ) : (
        <ul className="opportunity-list">
          {shown.map((item) => (
            <OpportunityRow key={item.key} project={project} item={item} />
          ))}
        </ul>
      )}
      <p className="faint small">{t("opportunities.howWorked")}</p>
    </section>
  );
}
