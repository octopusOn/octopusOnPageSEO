import { useState, useCallback } from "react";
import { ArrowRight, Globe2, List, ShieldCheck } from "lucide-react";
import type { CrawlConfig, CrawlMeta } from "../api/types";
import { useI18n } from "../i18n";
import { isDemo } from "../api/client";
import { Modal, Button } from "./ui";
import { normalizeUrls, validConfig } from "../lib/config";
import { RepeatSettings } from "./RepeatSettings";
import { UrlListImporter } from "./UrlListImporter";
import { mergeImportedUrls } from "../lib/urlListImport";
import { ViewportSettings } from "./ViewportSettings";
import { validViewport } from "../lib/viewport";
import { ResourceCacheSettings } from "./ResourceCacheSettings";
import { CrawlPerformanceSettings } from "./CrawlPerformanceSettings";
import { StartFolderSettings } from "./StartFolderSettings";
import { SchemaCrawlSettings } from "./SchemaCrawlSettings";

export function NewCrawl({
  config,
  initialUrl,
  onClose,
  onStart,
  retryRendering = false,
  repeatSource,
}: {
  config: CrawlConfig;
  initialUrl: string;
  onClose: () => void;
  onStart: (config: CrawlConfig) => Promise<void>;
  retryRendering?: boolean;
  repeatSource?: Pick<CrawlMeta,"name"|"created_at_ms">;
}) {
  const { t, lang } = useI18n();
  const [mode, setMode] = useState<"spider" | "list">(retryRendering ? "list" : repeatSource ? config.mode : "spider");
  const multipleSeeds=!!repeatSource && config.start_urls.length>1;
  const [spiderUrls, setSpiderUrls] = useState(() => multipleSeeds ? initialUrl : initialUrl.split(/\r\n|\n|\r/).map(url=>url.trim()).find(Boolean) ?? "");
  const [listUrls, setListUrls] = useState(initialUrl);
  const urls = mode === "list" ? listUrls : spiderUrls;
  const setUrls = mode === "list" ? setListUrls : setSpiderUrls;
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [importPending,setImportPending]=useState(false);
  const [importNotice,setImportNotice]=useState("");
  const [renderMode, setRenderMode] = useState(config.rendering.mode);
  const [speed, setSpeed] = useState(config.speed);
  const [renderConcurrency,setRenderConcurrency]=useState(config.rendering.concurrency);
  const [viewport, setViewport] = useState(config.rendering.viewport ?? null);
  const [cacheResources,setCacheResources]=useState(config.rendering.cache_resources!==false);
  const [startFolderOnly,setStartFolderOnly]=useState(config.urls.start_folder_only);
  const [schemaValidation,setSchemaValidation]=useState(config.schema_validation===true);
  const draftConfig = { ...config, schema_validation:schemaValidation, urls: { ...config.urls, start_folder_only:startFolderOnly }, rendering: { ...config.rendering, mode: renderMode, viewport, cache_resources:cacheResources, concurrency:renderConcurrency }, speed };
  const close = useCallback(() => {
    if (!busy) onClose();
  }, [busy, onClose]);
  function changeMode(next: "spider" | "list") {
    if (next === mode) return;
    setMode(next);
    setImportPending(false);
    setImportNotice("");
    setError("");
  }
  async function submit() {
    if(busy||importPending)return;
    setError("");
    if (!validViewport(viewport)) { setError(t("invalidViewport")); return; }
    if (!validConfig(draftConfig)) { setError(t("invalidNumbers")); return; }
    let start_urls: string[];
    try {
      start_urls = normalizeUrls(urls, mode==="list" || multipleSeeds ? "list" : "spider");
    } catch {
      setError(t("invalidUrl"));
      return;
    }
    setBusy(true);
    try {
      await onStart({
        ...draftConfig,
        mode,
        start_urls,
      });
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }
  return (
    <Modal
      className="crawl-start-modal"
      title={t(retryRendering ? "retryRendering" : repeatSource ? "repeatCrawl" : "newCrawl")}
      onClose={close}
      footer={
        <>
          {error && <div className="oc-banner error crawl-start-error" role="alert">{error}</div>}
          <Button disabled={busy} onClick={onClose}>
            {t("cancel")}
          </Button>
          <Button primary busy={busy} disabled={importPending} onClick={() => void submit()}>
            {t("start")}
            <ArrowRight size={16} />
          </Button>
        </>
      }
    >
      <p className="muted">{t(retryRendering ? "retryRenderingDescription" : repeatSource ? "repeatCrawlDescription" : mode==="list" ? "listCrawlScope" : "newCrawlDescription")}</p>
      {repeatSource && <div className="oc-banner info">{repeatSource.name} · {new Intl.DateTimeFormat(lang==="tr"?"tr-TR":"en-US",{dateStyle:"medium",timeStyle:"short"}).format(repeatSource.created_at_ms)}</div>}
      {retryRendering && <div className="oc-banner info">{t("listCrawlScope")}</div>}
      <div className="oc-segmented crawl-modes">
        <button
          className={mode === "spider" ? "active" : ""}
          disabled={busy}
          onClick={() => changeMode("spider")}
        >
          <Globe2 size={16} />
          {t("spider")}
        </button>
        <button
          className={mode === "list" ? "active" : ""}
          disabled={busy}
          onClick={() => changeMode("list")}
        >
          <List size={16} />
          {t("list")}
        </button>
      </div>
      {mode==="list"&&<UrlListImporter disabled={busy} onPending={setImportPending} onApply={(imported,append)=>{
        const merged=mergeImportedUrls(urls,imported,append);
        setUrls(merged.join("\n"));setError("");setImportNotice(`${t("urlImportApplied")} · ${merged.length.toLocaleString(lang)} URL`);
      }}/>}
      {importNotice&&<div className="oc-banner info" role="status">{importNotice}</div>}
      <label className="oc-field">
        {t(mode === "list" ? "listUrls" : multipleSeeds ? "spiderSeedUrls" : "crawlUrlLabel")}
        {mode === "list" || multipleSeeds ? (
          <textarea
            autoFocus
            className="oc-textarea mono"
            rows={6}
            disabled={busy||importPending}
            value={urls}
            onChange={(e) => {setUrls(e.target.value);setImportNotice("");}}
            placeholder="https://example.com/"
          />
        ) : (
          <input
            autoFocus
            className="oc-input mono"
            value={urls}
            onChange={(e) => {setUrls(e.target.value);setImportNotice("");}}
            onKeyDown={(e) => e.key === "Enter" && void submit()}
            placeholder="https://example.com/"
          />
        )}
        {(mode === "list" || multipleSeeds) && <small>{t("listHint")}</small>}
      </label>
      {mode === "spider" && <StartFolderSettings config={draftConfig} disabled={busy || importPending} onChange={next => setStartFolderOnly(next.urls.start_folder_only)} />}
      <label className="oc-field">
        {t("renderMode")}
        <select
          className="oc-select"
          disabled={busy || importPending}
          value={renderMode}
          onChange={(e) =>
            setRenderMode(e.target.value as "html" | "javascript")
          }
        >
          <option value="html">{t("rawHtml")}</option>
          <option value="javascript">{t("javascript")}</option>
        </select>
      </label>
      <CrawlPerformanceSettings config={draftConfig} disabled={busy || importPending} onChange={next=>{setSpeed(next.speed);setRenderConcurrency(next.rendering.concurrency);setError("");}}/>
      <SchemaCrawlSettings config={draftConfig} disabled={busy || importPending} onChange={next=>setSchemaValidation(next.schema_validation===true)}/>
      {renderMode === "javascript" && <ViewportSettings config={draftConfig} disabled={busy || importPending} onChange={next => setViewport(next.rendering.viewport ?? null)} />}
      {renderMode === "javascript" && <ResourceCacheSettings config={draftConfig} disabled={busy || importPending} onChange={next=>setCacheResources(next.rendering.cache_resources!==false)}/>}
      <div className="row crawl-options">
        <ShieldCheck size={16} />
        <span>
          {t(config.robots === "respect" ? "respectRobots" : config.robots === "ignore" ? "ignoreRobots" : "ignoreReport")}
        </span>
      </div>
      {repeatSource && <RepeatSettings config={draftConfig} />}
      {isDemo && <div className="oc-banner info">{t("demoNote")}</div>}
    </Modal>
  );
}
