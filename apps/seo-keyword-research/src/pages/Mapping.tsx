import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Map, RefreshCw } from "lucide-react";
import { api, errorText } from "../api/client";
import type { ClusterRecord, Job, ProjectRow } from "../api/types";
import { JobCard } from "../components/JobCard";
import { Button, Check, Empty, Field, Loading } from "../components/ui";
import { number, useI18n, type Key } from "../i18n";

type View = "gaps" | "competing" | "mapped";

function SearchConsoleImport({ project, notify }: { project: ProjectRow; notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n();
  const google = useQuery({ queryKey: ["google"], queryFn: api.googleStatus });
  const connected = google.data?.connected && google.data.scopes.some((scope) => scope.includes("webmasters"));
  const properties = useQuery({ queryKey: ["properties"], queryFn: api.properties, enabled: Boolean(connected) });
  const [site, setSite] = useState("");
  const [days, setDays] = useState(90);
  const [countryOnly, setCountryOnly] = useState(true);
  const start = useMutation({
    mutationFn: () =>
      api.importSearchConsole(project.id, {
        site_url: site || properties.data?.[0]?.site_url || "",
        days,
        country_only: countryOnly,
        max_rows: 50000,
      }),
    onError: (error) => notify(errorText(error), true),
  });
  return (
    <section className="oc-panel">
      <h2 className="oc-panel-title">{t("gscTitle")}</h2>
      {!connected ? (
        <p className="faint">{t("gscConnectFirst")}</p>
      ) : (
        <>
          <p className="faint small">{t("gscLead")}</p>
          <div className="form-row four">
            <Field label={t("property")}>
              <select className="oc-select" value={site} onChange={(e) => setSite(e.target.value)}>
                {(properties.data ?? []).map((property) => (
                  <option key={property.site_url} value={property.site_url}>
                    {property.site_url}
                  </option>
                ))}
              </select>
            </Field>
            <Field label={t("days")}>
              <input className="oc-input" type="number" min={7} max={480} value={days} onChange={(e) => setDays(Number(e.target.value) || 90)} />
            </Field>
          </div>
          <Check checked={countryOnly} onChange={setCountryOnly}>
            {t("countryOnly", { country: project.country.toUpperCase() })}
          </Check>
          <div className="row end">
            <Button primary busy={start.isPending} disabled={(properties.data?.length ?? 0) === 0} onClick={() => start.mutate()}>
              {t("importQueries")}
            </Button>
          </div>
        </>
      )}
    </section>
  );
}

export function Mapping({ project, jobs, notify }: { project: ProjectRow; jobs: Job[]; notify: (text: string, error?: boolean) => void }) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const [view, setView] = useState<View>("gaps");
  const [dismissed, setDismissed] = useState<string[]>([]);
  const clusterings = useQuery({ queryKey: ["clusterings", project.id], queryFn: () => api.clusterings(project.id) });
  const clustering = clusterings.data?.[0];
  const clusters = useQuery({
    queryKey: ["clusters", clustering?.id, "mapping"],
    queryFn: () =>
      api.clusters(clustering?.id ?? 0, { sort: "volume", descending: true, offset: 0, limit: 10000 }),
    enabled: clustering !== undefined,
  });
  const remap = useMutation({
    mutationFn: () => api.remap(project.id),
    onSuccess: () => {
      notify(t("remapped"));
      void queryClient.invalidateQueries({ queryKey: ["clusters"] });
    },
    onError: (error) => notify(errorText(error), true),
  });
  const own = jobs.filter((job) => job.project_id === project.id && job.kind === "search-console" && !dismissed.includes(job.id));
  const rows = clusters.data?.rows ?? [];
  const gaps = rows.filter((row) => row.gap);
  const competing = rows.filter((row) => row.competing.length > 0);
  const mapped = rows.filter((row) => row.target_url);
  const shown: ClusterRecord[] = view === "gaps" ? gaps : view === "competing" ? competing : mapped;
  return (
    <div className="stack">
      {own.slice(0, 2).map((job) => (
        <JobCard key={job.id} job={job} onStop={(id) => void api.cancel(id)} onDismiss={() => setDismissed((old) => [...old, job.id])} />
      ))}
      {!project.site && <div className="oc-banner warning">{t("mappingNeedsSite")}</div>}
      <SearchConsoleImport project={project} notify={notify} />
      {clusterings.isLoading || clusters.isLoading ? (
        <Loading />
      ) : !clustering ? (
        <Empty icon={<Map size={28} />}>
          <b>{t("noClusterings")}</b>
          <p>{t("mappingNoClusters")}</p>
        </Empty>
      ) : (
        <>
          <div className="oc-stats">
            <div className="oc-stat">
              <span className="oc-stat-label">{t("mappedClusters")}</span>
              <span className="oc-stat-value">{number(mapped.length, lang, 0)}</span>
            </div>
            <div className="oc-stat">
              <span className="oc-stat-label">{t("gaps")}</span>
              <span className="oc-stat-value">{number(gaps.length, lang, 0)}</span>
              <span className="oc-stat-hint">{t("gapsHint")}</span>
            </div>
            <div className="oc-stat">
              <span className="oc-stat-label">{t("cannibalisation")}</span>
              <span className="oc-stat-value">{number(competing.length, lang, 0)}</span>
              <span className="oc-stat-hint">{t("cannibalisationHint")}</span>
            </div>
          </div>
          <div className="row between">
            <div className="oc-tabs" role="tablist">
              {(["gaps", "competing", "mapped"] as View[]).map((id) => (
                <button key={id} role="tab" aria-selected={view === id} className={`oc-tab ${view === id ? "active" : ""}`} onClick={() => setView(id)}>
                  {t(`view.${id}` as Key)}
                </button>
              ))}
            </div>
            <Button busy={remap.isPending} onClick={() => remap.mutate()}>
              <RefreshCw size={14} />
              {t("remap")}
            </Button>
          </div>
          {shown.length === 0 ? (
            <Empty>
              <p>{t(`viewEmpty.${view}` as Key)}</p>
            </Empty>
          ) : (
            <div className="oc-table-wrap">
              <table className="oc-table">
                <thead>
                  <tr>
                    <th>{t("cluster")}</th>
                    <th className="num">{t("volume")}</th>
                    <th className="num">{t("keywords")}</th>
                    <th>{view === "gaps" ? t("topCompetitors") : t("targetPage")}</th>
                    {view === "competing" && <th>{t("competingPages")}</th>}
                  </tr>
                </thead>
                <tbody>
                  {shown.slice(0, 1000).map((row) => (
                    <tr key={row.number}>
                      <td>
                        <b>{row.label}</b>
                        {row.intent && <span className={`intent ${row.intent}`}>{t(`intent.${row.intent}` as Key)}</span>}
                      </td>
                      <td className="num">{number(row.volume, lang, 0)}</td>
                      <td className="num">{row.size}</td>
                      <td className="clip mono small">
                        {view === "gaps"
                          ? row.competitors.map((competitor) => `${competitor.domain} (${competitor.keywords})`).join(", ")
                          : row.target_url}
                      </td>
                      {view === "competing" && <td className="clip mono small warning-text">{row.competing.join(", ")}</td>}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </>
      )}
    </div>
  );
}
