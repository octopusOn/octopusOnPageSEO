import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Layers, RefreshCw, Trash2 } from "lucide-react";
import { api, errorText } from "../api/client";
import { engines, type Engine, type SnapshotSummary } from "../api/types";
import { ParsedResults } from "../components/ParsedResults";
import { Button, Empty, ErrorState, Loading, Modal } from "../components/ui";
import { date, number, usd, useI18n } from "../i18n";

export function Snapshots({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t, lang } = useI18n();
  const [keyword, setKeyword] = useState("");
  const [engine, setEngine] = useState<Engine | "">("");
  const [selected, setSelected] = useState<number | null>(null);
  const list = useQuery({
    queryKey: ["snapshots", keyword.trim(), engine],
    queryFn: () => api.snapshots({ keyword: keyword.trim() || undefined, engine: engine || undefined, limit: 300 }),
  });
  const items = list.data ?? [];
  const current = items.find((item) => item.id === selected) ?? items[0];
  return (
    <div className="snapshots">
      <div className="snapshot-filters">
        <input className="oc-input" placeholder={t("filterKeywords")} value={keyword} onChange={(event) => setKeyword(event.target.value)} />
        <select className="oc-select" value={engine} onChange={(event) => setEngine(event.target.value as Engine | "")}>
          <option value="">{t("allEngines")}</option>
          {engines.map((item) => (
            <option key={item} value={item}>
              {t(item)}
            </option>
          ))}
        </select>
      </div>
      {list.isLoading && <Loading />}
      {list.isError && <ErrorState message={errorText(list.error)} retry={() => void list.refetch()} />}
      {list.isSuccess && items.length === 0 && <Empty icon={<Layers size={28} />}>{t("noSnapshots")}</Empty>}
      {items.length > 0 && (
        <div className="snapshot-layout">
          <aside className="oc-panel snapshot-list">
            <ul>
              {items.map((item) => (
                <li key={item.id}>
                  <button className={item.id === current?.id ? "active" : ""} aria-current={item.id === current?.id} onClick={() => setSelected(item.id)}>
                    <b>{item.keyword}</b>
                    <small>
                      {t(item.engine)} · {item.country.toUpperCase()} · {item.provider} · {date(item.fetched_at, lang)}
                    </small>
                  </button>
                </li>
              ))}
            </ul>
          </aside>
          {current && <SnapshotView summary={current} notify={notify} onDeleted={() => setSelected(null)} />}
        </div>
      )}
    </div>
  );
}

function SnapshotView({
  summary,
  notify,
  onDeleted,
}: {
  summary: SnapshotSummary;
  notify: (text: string, error?: boolean) => void;
  onDeleted: () => void;
}) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<"parsed" | "raw">("parsed");
  const [deleting, setDeleting] = useState(false);
  const snapshot = useQuery({ queryKey: ["snapshot", summary.id], queryFn: () => api.snapshot(summary.id) });
  const pages = useQuery({ queryKey: ["stored-pages", summary.id], queryFn: () => api.storedPages(summary.id), enabled: tab === "raw" });
  const reparse = useMutation({
    mutationFn: () => api.reparse(summary.id),
    onSuccess: (fresh) => {
      queryClient.setQueryData(["snapshot", summary.id], fresh);
      void queryClient.invalidateQueries({ queryKey: ["snapshots"] });
      void queryClient.invalidateQueries({ queryKey: ["rank-report"] });
      notify(t("reparsed", { count: number(fresh.organic.length, lang, 0) }));
    },
    onError: (error) => notify(errorText(error), true),
  });
  const remove = useMutation({
    mutationFn: () => api.deleteSnapshot(summary.id),
    onSuccess: () => {
      setDeleting(false);
      onDeleted();
      void queryClient.invalidateQueries({ queryKey: ["snapshots"] });
    },
    onError: (error) => notify(errorText(error), true),
  });
  return (
    <section className="oc-panel snapshot-view">
      <div className="result-head">
        <div>
          <h2>{summary.keyword}</h2>
          <p className="faint">
            {t(summary.engine)} · {summary.country.toUpperCase()}-{summary.language} · {t(summary.device)} · {summary.provider} ·{" "}
            {date(summary.fetched_at, lang)} · {usd(summary.cost_usd, lang)}
          </p>
        </div>
        <div className="row">
          <Button small busy={reparse.isPending} onClick={() => reparse.mutate()} title={t("reparseHelp")}>
            <RefreshCw size={14} />
            {t("reparse")}
          </Button>
          <Button small className="icon ghost" aria-label={t("deleteSnapshot")} onClick={() => setDeleting(true)}>
            <Trash2 size={15} />
          </Button>
        </div>
      </div>
      <div className="oc-tabs" role="tablist">
        {(["parsed", "raw"] as const).map((item) => (
          <button key={item} role="tab" aria-selected={tab === item} className={`oc-tab ${tab === item ? "active" : ""}`} onClick={() => setTab(item)}>
            {t(item === "parsed" ? "parsedView" : "rawView")}
          </button>
        ))}
      </div>
      {tab === "parsed" && (
        <>
          {snapshot.isLoading && <Loading />}
          {snapshot.isError && <ErrorState message={errorText(snapshot.error)} />}
          {snapshot.data && (
            <>
              {snapshot.data.warnings?.map((warning) => (
                <div className="oc-banner warning" key={warning}>
                  {warning}
                </div>
              ))}
              <ParsedResults organic={snapshot.data.organic} features={snapshot.data.features} />
              {snapshot.data.parser_rev && (
                <p className="faint parser-rev">
                  {t("parserUsed")} <code className="oc-code">{snapshot.data.parser_rev}</code>
                </p>
              )}
            </>
          )}
        </>
      )}
      {tab === "raw" && (
        <>
          {pages.isLoading && <Loading />}
          {pages.isError && <ErrorState message={errorText(pages.error)} />}
          {pages.data?.length === 0 && <p className="panel-lead">{t("noRawPages")}</p>}
          {pages.data?.map((page) => (
            <div className="raw-page" key={page.page}>
              <p className="faint">
                {t("pageNumber", { page: page.page })} · {page.content_type ?? "—"} · {number(page.size / 1024, lang, 0)} KB ·{" "}
                <code className="oc-code">{page.url}</code>
              </p>
              <pre>{page.body.length > 200_000 ? `${page.body.slice(0, 200_000)}\n…` : page.body}</pre>
            </div>
          ))}
        </>
      )}
      {deleting && (
        <Modal
          title={t("deleteSnapshot")}
          onClose={() => setDeleting(false)}
          footer={
            <>
              <Button onClick={() => setDeleting(false)}>{t("cancel")}</Button>
              <Button className="danger solid" busy={remove.isPending} onClick={() => remove.mutate()}>
                {t("deleteSnapshot")}
              </Button>
            </>
          }
        >
          <p>{t("deleteSnapshotConfirm")}</p>
        </Modal>
      )}
    </section>
  );
}
