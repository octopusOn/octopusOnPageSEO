import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AlertTriangle, Code2, Download, FlaskConical, Pencil, Plus, Trash2, Upload } from "lucide-react";
import { api, errorText } from "../api/client";
import { engines, type Engine, type ParsedPage, type ParserInfo, type ProviderSpec } from "../api/types";
import { ParsedResults } from "./ParsedResults";
import { date, number, percent, useI18n } from "../i18n";
import { Button, Modal } from "./ui";

/** A spec to start from: a JSON API with results and People also ask. */
export const specTemplate = {
  version: 1,
  id: "my-serp-api",
  name: "My SERP API",
  engines: ["google"],
  secrets: [{ name: "token" }],
  request: {
    method: "POST",
    url: "https://api.example.com/v1/search?page={page}",
    headers: { Authorization: "Bearer {secret:token}" },
    body: { q: "{keyword}", gl: "{gl}", hl: "{hl}", device: "{device}", num: "{num:int}" },
  },
  paging: { per_page: 10, first_page: 1, max_pages: 2 },
  response: {
    format: "json",
    error: "$.error.message",
    total: "$.info.total",
    results: { items: "$.organic[*]", fields: { url: "$.link", title: "$.title", snippet: "$.snippet", position: "$.position" } },
    features: [{ kind: "people-also-ask", items: "$.related_questions[*]", fields: { text: "$.question" } }],
  },
  cost: { per_request_usd: 0.002 },
  rate_limit: { per_minute: 60 },
};

function parseJson<T>(text: string): { value?: T; error?: string } {
  try {
    return { value: JSON.parse(text) as T };
  } catch (error) {
    return { error: errorText(error) };
  }
}

/** What a parser read from a page: counts, coverage, problems and the results. */
export function TestResult({ result }: { result: ParsedPage }) {
  const { t, lang } = useI18n();
  const coverage = result.coverage.matched > 0 ? result.coverage.usable / result.coverage.matched : null;
  return (
    <div className="test-result">
      <div className="row test-stats">
        <span className={`oc-badge ${result.organic.length > 0 ? "good" : "critical"}`}>
          <span className="oc-dot" />
          {t("testResults", { count: number(result.organic.length, lang, 0) })}
        </span>
        {coverage !== null && (
          <span className={`oc-badge ${coverage >= 0.8 ? "good" : "warning"}`}>
            {t("testCoverage", { share: percent(coverage, lang) })}
          </span>
        )}
        <span className="oc-badge">{t("testFeatures", { count: number(result.features.length, lang, 0) })}</span>
        {result.total_results ? <span className="faint">{t("totalResults", { count: number(result.total_results, lang, 0) })}</span> : null}
      </div>
      {result.error && <div className="oc-banner error">{result.error}</div>}
      {result.challenge && (
        <div className="oc-banner warning">
          <AlertTriangle size={16} />
          {t("testChallenge", { kind: result.challenge.kind, reason: result.challenge.reason })}
        </div>
      )}
      <ParsedResults organic={result.organic.slice(0, 10)} features={result.features} />
    </div>
  );
}

export function CustomProviders({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const specs = useQuery({ queryKey: ["specs"], queryFn: api.customSpecs });
  const [editing, setEditing] = useState<ProviderSpec | "new" | null>(null);
  const [importing, setImporting] = useState(false);
  const [exported, setExported] = useState<string | null>(null);
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["specs"] });
    void queryClient.invalidateQueries({ queryKey: ["providers"] });
    void queryClient.invalidateQueries({ queryKey: ["settings"] });
  };
  const remove = useMutation({
    mutationFn: api.removeSpec,
    onSuccess: refresh,
    onError: (error) => notify(errorText(error), true),
  });
  const exportAll = async () => {
    try {
      setExported(await api.exportSpecs((specs.data ?? []).map((spec) => spec.id)));
    } catch (error) {
      notify(errorText(error), true);
    }
  };
  return (
    <section className="oc-panel">
      <div className="oc-panel-head row between">
        <h2>{t("customProviders")}</h2>
        <div className="row">
          <Button small className="ghost" onClick={() => setImporting(true)}>
            <Upload size={14} />
            {t("import")}
          </Button>
          <Button small className="ghost" disabled={!specs.data?.length} onClick={() => void exportAll()}>
            <Download size={14} />
            {t("export")}
          </Button>
          <Button small onClick={() => setEditing("new")}>
            <Plus size={14} />
            {t("addProvider")}
          </Button>
        </div>
      </div>
      <p className="panel-lead">{t("customProvidersLead")}</p>
      {specs.data?.length ? (
        <ul className="spec-list">
          {specs.data.map((spec) => (
            <li key={spec.id}>
              <Code2 size={15} />
              <b>{spec.name}</b>
              <code className="oc-code">{spec.id}</code>
              <span className="faint">{(spec.engines ?? []).map((engine) => t(engine)).join(" · ")}</span>
              <Button small className="icon ghost" aria-label={t("edit")} onClick={() => setEditing(spec)}>
                <Pencil size={14} />
              </Button>
              <Button small className="icon ghost" aria-label={t("remove")} onClick={() => remove.mutate(spec.id)}>
                <Trash2 size={14} />
              </Button>
            </li>
          ))}
        </ul>
      ) : null}
      {editing && (
        <SpecEditor
          spec={editing === "new" ? null : editing}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            refresh();
            notify(t("providerSaved"));
          }}
        />
      )}
      {importing && (
        <ImportDialog
          onClose={() => setImporting(false)}
          onImported={(ids) => {
            setImporting(false);
            refresh();
            notify(t("providersImported", { count: ids.length }));
          }}
        />
      )}
      {exported !== null && (
        <Modal title={t("export")} onClose={() => setExported(null)} footer={<CopyButton text={exported} />}>
          <p className="oc-field-hint">{t("exportSpecsHint")}</p>
          <textarea className="oc-textarea code" rows={14} readOnly value={exported} />
        </Modal>
      )}
    </section>
  );
}

function CopyButton({ text }: { text: string }) {
  const { t } = useI18n();
  const [copied, setCopied] = useState(false);
  return (
    <Button
      primary
      onClick={() =>
        void navigator.clipboard?.writeText(text).then(() => {
          setCopied(true);
          setTimeout(() => setCopied(false), 1500);
        })
      }
    >
      {copied ? t("copied") : t("copy")}
    </Button>
  );
}

function ImportDialog({ onClose, onImported }: { onClose: () => void; onImported: (ids: string[]) => void }) {
  const { t } = useI18n();
  const [text, setText] = useState("");
  const [problem, setProblem] = useState("");
  const run = useMutation({
    mutationFn: () => api.importSpecs(text),
    onSuccess: onImported,
    onError: (error) => setProblem(errorText(error)),
  });
  return (
    <Modal
      title={t("import")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("cancel")}</Button>
          <Button primary busy={run.isPending} disabled={!text.trim()} onClick={() => run.mutate()}>
            {t("import")}
          </Button>
        </>
      }
    >
      <p className="oc-field-hint">{t("importSpecsHint")}</p>
      <textarea className="oc-textarea code" rows={12} value={text} onChange={(event) => setText(event.target.value)} spellCheck={false} />
      {problem && <div className="oc-banner error">{problem}</div>}
    </Modal>
  );
}

function SpecEditor({ spec, onClose, onSaved }: { spec: ProviderSpec | null; onClose: () => void; onSaved: () => void }) {
  const { t } = useI18n();
  const [text, setText] = useState(() => JSON.stringify(spec ?? specTemplate, null, 2));
  const [problem, setProblem] = useState("");
  const [engine, setEngine] = useState<Engine>(spec?.engines?.[0] ?? "google");
  const [body, setBody] = useState("");
  const [url, setUrl] = useState("");
  const [result, setResult] = useState<ParsedPage | null>(null);
  const parsed = useMemo(() => parseJson<ProviderSpec>(text), [text]);
  const save = useMutation({
    mutationFn: (value: ProviderSpec) => api.saveSpec(value),
    onSuccess: onSaved,
    onError: (error) => setProblem(errorText(error)),
  });
  const test = useMutation({
    mutationFn: (value: ProviderSpec) => api.testSpec(value, engine, body, url.trim() || undefined),
    onSuccess: (page) => {
      setProblem("");
      setResult(page);
    },
    onError: (error) => {
      setResult(null);
      setProblem(errorText(error));
    },
  });
  return (
    <Modal
      title={spec ? spec.name : t("addProvider")}
      className="editor-modal"
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("cancel")}</Button>
          <Button primary busy={save.isPending} disabled={!parsed.value} onClick={() => parsed.value && save.mutate(parsed.value)}>
            {t("save")}
          </Button>
        </>
      }
    >
      <div className="editor-grid">
        <div className="editor-pane">
          <span className="type-label">{t("providerSpec")}</span>
          <textarea className="oc-textarea code" rows={22} value={text} spellCheck={false} onChange={(event) => setText(event.target.value)} />
          {parsed.error ? <p className="oc-field-hint critical">{parsed.error}</p> : <p className="oc-field-hint">{t("specHint")}</p>}
        </div>
        <div className="editor-pane">
          <span className="type-label">{t("testWithResponse")}</span>
          <div className="row">
            <select className="oc-select" aria-label={t("engine")} value={engine} onChange={(event) => setEngine(event.target.value as Engine)}>
              {engines.map((item) => (
                <option key={item} value={item}>
                  {t(item)}
                </option>
              ))}
            </select>
            <input className="oc-input" placeholder={t("responseUrl")} value={url} onChange={(event) => setUrl(event.target.value)} />
          </div>
          <textarea
            className="oc-textarea code"
            rows={8}
            placeholder={t("pasteResponse")}
            value={body}
            spellCheck={false}
            onChange={(event) => setBody(event.target.value)}
          />
          <Button small busy={test.isPending} disabled={!parsed.value || !body.trim()} onClick={() => parsed.value && test.mutate(parsed.value)}>
            <FlaskConical size={14} />
            {t("test")}
          </Button>
          <p className="oc-field-hint">{t("testOffline")}</p>
          {result && <TestResult result={result} />}
        </div>
      </div>
      {problem && <div className="oc-banner error">{problem}</div>}
    </Modal>
  );
}

export function ParserOverrides({ notify }: { notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n();
  const parsers = useQuery({ queryKey: ["parsers"], queryFn: api.parsers });
  const [editing, setEditing] = useState<ParserInfo | null>(null);
  const queryClient = useQueryClient();
  return (
    <section className="oc-panel">
      <div className="oc-panel-head">
        <h2>{t("parserSelectors")}</h2>
      </div>
      <p className="panel-lead">{t("parserSelectorsLead")}</p>
      <ul className="spec-list">
        {(parsers.data ?? []).map((info) => (
          <li key={info.engine}>
            <Code2 size={15} />
            <b>{t(info.engine)}</b>
            <code className="oc-code">{info.key}</code>
            {info.patched && <span className="oc-badge accent">{t("patched")}</span>}
            {info.stale_base && <span className="oc-badge warning" title={t("staleBaseHelp", { base: info.stale_base })}>{t("staleBase")}</span>}
            {info.error && <span className="oc-badge critical" title={info.error}>{t("overrideInvalid")}</span>}
            <span className="spacer" />
            <Button small onClick={() => setEditing(info)}>
              <Pencil size={14} />
              {t("editSelectors")}
            </Button>
          </li>
        ))}
      </ul>
      {editing && (
        <ParserEditor
          info={editing}
          onClose={() => setEditing(null)}
          onSaved={(text) => {
            setEditing(null);
            notify(text);
            void queryClient.invalidateQueries({ queryKey: ["parsers"] });
          }}
          notify={notify}
        />
      )}
    </section>
  );
}

function ParserEditor({
  info,
  onClose,
  onSaved,
  notify,
}: {
  info: ParserInfo;
  onClose: () => void;
  onSaved: (text: string) => void;
  notify: (text: string, error?: boolean) => void;
}) {
  const { t, lang } = useI18n();
  const [text, setText] = useState(() => JSON.stringify(info.patch ?? {}, null, 2));
  const [snapshotId, setSnapshotId] = useState<number | null>(null);
  const [result, setResult] = useState<ParsedPage | null>(null);
  const [problem, setProblem] = useState("");
  const parsed = useMemo(() => parseJson<unknown>(text), [text]);
  const snapshots = useQuery({
    queryKey: ["snapshots", "", info.engine, "browser"],
    queryFn: async () => (await api.snapshots({ engine: info.engine, limit: 100 })).filter((item) => item.provider === "browser"),
  });
  const chosen = snapshotId ?? snapshots.data?.[0]?.id ?? null;
  const patch = () => ({ engine: info.engine, base: info.base, mapping: parsed.value, enabled: true });
  const test = useMutation({
    mutationFn: () => api.testParser(chosen as number, patch()),
    onSuccess: (page) => {
      setProblem("");
      setResult(page);
    },
    onError: (error) => {
      setResult(null);
      setProblem(errorText(error));
    },
  });
  const save = useMutation({
    mutationFn: () => api.saveParserOverride(patch()),
    onSuccess: () => onSaved(t("overrideSaved")),
    onError: (error) => setProblem(errorText(error)),
  });
  const remove = useMutation({
    mutationFn: () => api.removeParserOverride(info.engine),
    onSuccess: () => onSaved(t("overrideRemoved")),
    onError: (error) => notify(errorText(error), true),
  });
  return (
    <Modal
      title={`${t(info.engine)} · ${t("parserSelectors")}`}
      className="editor-modal"
      onClose={onClose}
      footer={
        <>
          {info.patch !== undefined && (
            <Button className="danger" busy={remove.isPending} onClick={() => remove.mutate()}>
              {t("removeOverride")}
            </Button>
          )}
          <span className="spacer" />
          <Button onClick={onClose}>{t("cancel")}</Button>
          <Button primary busy={save.isPending} disabled={parsed.value === undefined} onClick={() => save.mutate()}>
            {t("save")}
          </Button>
        </>
      }
    >
      <div className="editor-grid">
        <div className="editor-pane">
          <span className="type-label">{t("overridePatch")}</span>
          <textarea className="oc-textarea code" rows={16} value={text} spellCheck={false} onChange={(event) => setText(event.target.value)} />
          {parsed.error ? <p className="oc-field-hint critical">{parsed.error}</p> : <p className="oc-field-hint">{t("overrideHint")}</p>}
          <details className="mapping-source">
            <summary>{t("currentMapping", { key: info.key })}</summary>
            <pre>{JSON.stringify(info.mapping, null, 2)}</pre>
          </details>
        </div>
        <div className="editor-pane">
          <span className="type-label">{t("testOnStoredPage")}</span>
          {snapshots.data?.length ? (
            <>
              <select className="oc-select" value={chosen ?? ""} onChange={(event) => setSnapshotId(Number(event.target.value))}>
                {snapshots.data.map((item) => (
                  <option key={item.id} value={item.id}>
                    {item.keyword} · {date(item.fetched_at, lang)} · {number(item.organic_count, lang, 0)}
                  </option>
                ))}
              </select>
              <Button small busy={test.isPending} disabled={parsed.value === undefined || chosen === null} onClick={() => test.mutate()}>
                <FlaskConical size={14} />
                {t("test")}
              </Button>
            </>
          ) : (
            <p className="oc-field-hint">{t("noBrowserSnapshots")}</p>
          )}
          {result && <TestResult result={result} />}
        </div>
      </div>
      {problem && <div className="oc-banner error">{problem}</div>}
    </Modal>
  );
}
