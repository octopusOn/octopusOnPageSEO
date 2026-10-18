import type { CrawlConfig } from "../api/types";
import { useI18n, number } from "../i18n";
import { effectiveViewport } from "../lib/viewport";

export function RepeatSettings({ config }: {config:CrawlConfig}) {
  const {t,lang}=useI18n();
  const viewport=effectiveViewport(config);
  const unlimited=(value:number|null)=>value===null || value===0 ? t("repeatUnlimited") : number(value,lang);
  const fields=[
    [t("agent"),config.agent],
    [t("requestsPerSecond"),unlimited(config.speed.max_requests_per_second)],
    [t("maxUrls"),unlimited(config.limits.max_urls)],
    [t("maxDepth"),config.limits.max_depth===null ? t("repeatUnlimited") : number(config.limits.max_depth,lang)],
    [t("timeout"),`${config.limits.timeout_seconds} s`],
    [t("repeatScopeRules"),`${config.urls.subdomains ? t("subdomains") : t("repeatSameHost")} · ${t("includePatterns")}: ${config.urls.include.join(" · ") || "—"} · ${t("excludePatterns")}: ${config.urls.exclude.join(" · ") || "—"}`],
    [t("stripParams"),config.urls.strip_params.join(" · ") || "—"],
    [lang==="tr"?"Schema alanlarını puanla":"Score Schema properties",t(config.schema_validation?"yes":"no")],
    [t("storeHtml"),t(config.storage.html ? "yes":"no")],
    [t("storeHeaders"),t(config.storage.headers ? "yes":"no")],
    [t("repeatFollow"),`${t("externalCheck")}: ${t(config.follow.check_external ? "yes":"no")} · ${t("imagesCheck")}: ${t(config.follow.images ? "yes":"no")} · ${t("sitemapsCheck")}: ${t(config.follow.sitemaps ? "yes":"no")}`],
    [t("renderWait"),`${config.rendering.wait_ms} ms`],
    [t("renderTimeout"),`${config.rendering.timeout_seconds} s`],
    [t("renderConcurrency"),number(config.rendering.concurrency,lang)],
    [t("renderResourceCache"),t(config.rendering.cache_resources!==false ? "yes":"no")],
    [t("browserPath"),config.rendering.browser_path || t("autoBrowser")],
    [t("renderViewport"),`${!config.rendering.viewport ? `${t("viewportAuto")} · ` : ""}${viewport.width} × ${viewport.height} CSS px · ${t(viewport.mobile ? "viewportMobile" : "viewportDesktop")}`],
  ];
  return <details className="repeat-settings"><summary>{t("repeatSettings")}</summary><dl>{fields.map(([label,value])=><div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl></details>;
}
