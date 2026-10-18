import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { X, FileSearch, Copy, Check, ArrowDownLeft, ArrowUpRight, Braces } from "lucide-react";
import { api, errorText } from "../api/client";
import { useI18n, number, type Key } from "../i18n";
import type { LinkClass, LinkScope, DuplicateScope } from "../api/types";
import { Button, Loading, ErrorState, Empty } from "./ui";
import { DataTable, columnLabel } from "./DataTable";
import { RedirectChain } from "./RedirectChain";
import { RenderingComparison } from "./RenderingComparison";
import { ContentMatch } from "./ContentMatch";
import { DuplicateGroups } from "./DuplicateGroups";
import { UrlFindings } from "./UrlFindings";
import { InspectorLinks } from "./InspectorLinks";
import { SavedSchemaResult } from "./SavedSchemaResult";
import { inspectorLinkTabs, type InspectorLinkTab } from "../lib/links";

export function Inspector({
  id,
  url,
  onClose,
  onLinks,
  onInspect,
  onDuplicates,
  onIssue,
  excluded = [],
  activeIssueId = "",
  initialTab = "facts",
  onSchema,
}: {
  id: string;
  url: string;
  onClose: () => void;
  onLinks: (scope: LinkScope, classification?: LinkClass) => void;
  onInspect: (url: string) => void;
  onDuplicates: (scope: DuplicateScope) => void;
  onIssue: (id: string) => void;
  excluded?: string[];
  activeIssueId?: string;
  initialTab?: "facts" | "renderComparison" | "contentMatch" | "duplicates" | "urlFindings" | InspectorLinkTab;
  onSchema?: (url:string)=>void;
}) {
  const { t, lang } = useI18n();
  const panel = useRef<HTMLElement>(null);
  const [tab, setTab] = useState<
    "schemaReport" | "facts" | "headers" | "html" | "rawSource" | "redirectChain" | "renderComparison" | "contentMatch" | "duplicates" | "urlFindings" | InspectorLinkTab
  >(initialTab);
  useEffect(() => { panel.current?.scrollIntoView({ block: "nearest", behavior: "smooth" }); }, [url, tab]);
  const [copied, setCopied] = useState(false);
  const linkTab = tab in inspectorLinkTabs ? inspectorLinkTabs[tab as InspectorLinkTab] : null;
  const query = useQuery({
    queryKey: ["detail", id, url],
    queryFn: () => api.detail(id, url),
  });
  return (
    <section className={`inspector oc-panel ${tab === "renderComparison" ? "comparison-open" : tab === "urlFindings" ? "findings-open" : tab === "contentMatch" || tab === "duplicates" ? "content-match-open" : ""}`} ref={panel}>
      <header className="inspector-head row">
        <FileSearch size={17} />
        <span className="inspector-url">{url}</span>
        <Button
          className="icon ghost"
          aria-label={t("copyUrl")}
          onClick={() =>
            void navigator.clipboard
              .writeText(url)
              .then(() => {
                setCopied(true);
                setTimeout(() => setCopied(false), 1500);
              })
              .catch(() => setCopied(false))
          }
        >
          {copied ? <Check size={15} /> : <Copy size={15} />}
        </Button>
        <Button
          className="icon ghost"
          aria-label={t("close")}
          onClick={onClose}
        >
          <X size={17} />
        </Button>
      </header>
      <div className="inspector-actions row">
        {onSchema&&<Button small onClick={()=>onSchema(url)}><Braces size={14}/>{t("testSchema")}</Button>}
        <Button small onClick={() => onLinks({ url, direction: "incoming" })}>
          <ArrowDownLeft size={14} />{t("openIncomingLinks")}
        </Button>
        <Button small onClick={() => onLinks({ url, direction: "outgoing" })}>
          <ArrowUpRight size={14} />{t("openOutgoingLinks")}
        </Button>
      </div>
      <div className="oc-tabs inspector-tabs">
        {(
          [
            "facts",
            "schemaReport",
            "urlFindings",
            "renderComparison",
            "contentMatch",
            "duplicates",
            "internalInlinks",
            "externalInlinks",
            "internalOutlinks",
            "externalOutlinks",
            "redirectChain",
            "headers",
            "html",
            "rawSource",
          ] as const
        ).map((key) => (
          <button
            className={`oc-tab ${tab === key ? "active" : ""}`}
            key={key}
            onClick={() => setTab(key)}
          >
            {t(key as Key)}
            {key === "urlFindings" && query.data?.findings && <small>{number(query.data.findings.total, lang)}</small>}
            {key in inspectorLinkTabs && <small>{query.data?.link_counts ? number(query.data.link_counts[inspectorLinkTabs[key as InspectorLinkTab].direction][inspectorLinkTabs[key as InspectorLinkTab].classification], lang) : "—"}</small>}
          </button>
        ))}
      </div>
      <div className="inspector-body">
        {tab === "duplicates" ? <DuplicateGroups id={id} url={url} onOpen={onDuplicates} /> : tab === "contentMatch" ? <ContentMatch id={id} url={url} onInspect={onInspect} /> : tab === "redirectChain" ? <RedirectChain id={id} url={url} /> : query.isPending ? (
          <Loading />
        ) : query.error ? (
            <ErrorState message={errorText(query.error)} retry={() => void query.refetch()} />
        ) : (
          query.data && (linkTab ? <InspectorLinks id={id} url={url} direction={linkTab.direction} classification={linkTab.classification} available={!!query.data.link_counts} onExplore={onLinks} onInspect={onInspect} /> :
          (tab === "schemaReport" ? <SavedSchemaResult url={url} facts={query.data.facts} report={query.data.schema_report} error={query.data.schema_error}/> : tab === "urlFindings" ? <UrlFindings findings={query.data.findings} recorded={query.data.facts.total > 0} excluded={excluded} activeIssueId={activeIssueId} onIssue={onIssue} /> : tab === "renderComparison" ? <RenderingComparison facts={query.data.facts} /> : tab === "facts" ? (
            <dl className="facts-grid">
              {query.data.facts.columns.map((column, index) => (
                <div key={column.name}>
                  <dt>{columnLabel(column.name, t)}</dt>
                  <dd title={String(query.data.facts.rows[0]?.[index] ?? "")}>
                    {String(query.data.facts.rows[0]?.[index] ?? "—")}
                  </dd>
                </div>
              ))}
            </dl>
          ) : tab === "html" || tab === "rawSource" ? (
            (tab === "rawSource" ? query.data.raw_html : query.data.html) ? (
              <pre className="source-code">
                {tab === "rawSource" ? query.data.raw_html : query.data.html}
              </pre>
            ) : (
              <Empty>
                {t(tab === "rawSource" ? "noRawSource" : "noSource")}
              </Empty>
            )
          ) : (
            <DataTable data={query.data.headers} />
          )))
        )}
      </div>
    </section>
  );
}
