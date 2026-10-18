import type { CrawlConfig } from "../api/types";
import { defaultConfig } from "../api/types";
import { useI18n, type Key } from "../i18n";
import { RenderingSettings } from "../components/RenderingSettings";
import { CrawlPerformanceSettings } from "../components/CrawlPerformanceSettings";
import { StartFolderSettings } from "../components/StartFolderSettings";
import { SchemaCrawlSettings } from "../components/SchemaCrawlSettings";
import { AccountCard } from "../components/AccountCard";
import { Button } from "../components/ui";
import { Save, RotateCcw } from "lucide-react";

export function Settings({
  config,
  onChange,
  onSave,
}: {
  config: CrawlConfig;
  onChange: (config: CrawlConfig) => void;
  onSave: () => void;
}) {
  const { t, lang, setLang } = useI18n();
  function numeric(
    section: "speed" | "limits",
    field: string,
    label: Key,
    min: number,
    max: number,
    hint?: Key,
  ) {
    const value = (config[section] as Record<string, number | null>)[field];
    return (
      <label className="oc-field">
        {t(label)}
        <input
          className="oc-input"
          type="number"
          min={min}
          max={max}
          value={value ?? ""}
          onChange={(e) =>
            onChange({
              ...config,
              [section]: {
                ...config[section],
                [field]:
                  e.target.value === "" && field === "max_depth"
                    ? null
                    : Number(e.target.value),
              },
            })
          }
        />
        {hint && <small>{t(hint)}</small>}
      </label>
    );
  }
  function check(
    section: "follow" | "urls" | "storage",
    field: string,
    label: Key,
  ) {
    return (
      <label className="setting-switch">
        <span>{t(label)}</span>
        <input
          type="checkbox"
          role="switch"
          checked={Boolean((config[section] as Record<string, unknown>)[field])}
          onChange={(e) =>
            onChange({
              ...config,
              [section]: { ...config[section], [field]: e.target.checked },
            })
          }
        />
      </label>
    );
  }
  function patterns(
    field: "include" | "exclude" | "strip_params",
    label: Key,
    hint: Key,
  ) {
    return (
      <label className="oc-field">
        {t(label)}
        <textarea
          className="oc-textarea mono"
          rows={field === "exclude" ? 6 : 3}
          value={config.urls[field].join("\n")}
          onChange={(e) =>
            onChange({
              ...config,
              urls: { ...config.urls, [field]: e.target.value.split("\n") },
            })
          }
        />
        <small>{t(hint)}</small>
      </label>
    );
  }
  return (
    <div className="settings-page">
      <div className="settings-grid">
        <section className="oc-panel settings-card">
          <h2>{t("general")}</h2>
          <label className="oc-field">
            {t("agent")}
            <select
              className="oc-select"
              value={config.agent}
              onChange={(e) => onChange({ ...config, agent: e.target.value })}
            >
              {[
                "octopus",
                "googlebot-smartphone",
                "googlebot-desktop",
                "bingbot",
                "chrome",
              ].map((v) => (
                <option key={v} value={v}>
                  {v === "octopus" ? "OctopusOn SEO Crawler" : v}
                </option>
              ))}
            </select>
          </label>
          <label className="oc-field">
            {t("robotsPolicy")}
            <select
              className="oc-select"
              value={config.robots}
              onChange={(e) => onChange({ ...config, robots: e.target.value })}
            >
              <option value="respect">{t("respectRobots")}</option>
              <option value="ignore-but-report">{t("ignoreReport")}</option>
              <option value="ignore">{t("ignoreRobots")}</option>
            </select>
          </label>
          <label className="oc-field">
            {t("language")}
            <select
              className="oc-select"
              value={lang}
              onChange={(e) => setLang(e.target.value as "en" | "tr")}
            >
              <option value="tr">Türkçe</option>
              <option value="en">English</option>
            </select>
          </label>
        </section>
        <section className="oc-panel settings-card">
          <h2>{t("performance")}</h2>
          <CrawlPerformanceSettings config={config} onChange={onChange}/>
          <div className="field-grid">
            {numeric(
              "limits",
              "max_urls",
              "maxUrls",
              0,
              4294967295,
              "unlimitedHint",
            )}
            {numeric("limits", "max_depth", "maxDepth", 0, 10000, "depthHint")}
            {numeric("limits", "timeout_seconds", "timeout", 1, 600)}
          </div>
          <p className="setting-hint">{t("cpuNote")}</p>
        </section>
        <section className="oc-panel settings-card">
          <h2>{t("scope")}</h2>
          <StartFolderSettings config={config} onChange={onChange} />
          <SchemaCrawlSettings config={config} onChange={onChange} />
          {check("follow", "check_external", "externalCheck")}
          {check("follow", "images", "imagesCheck")}
          {check("follow", "sitemaps", "sitemapsCheck")}
          {check("urls", "subdomains", "subdomains")}
          {patterns("include", "includePatterns", "patternsHint")}
          {patterns("exclude", "excludePatterns", "patternsHint")}
        </section>
        <section className="oc-panel settings-card">
          <h2>{t("storage")}</h2>
          {check("storage", "headers", "storeHeaders")}
          {check("storage", "html", "storeHtml")}
          {patterns("strip_params", "stripParams", "stripHint")}
          <h3>{t("htmlRendering")}</h3>
          <RenderingSettings config={config} onChange={onChange} />
        </section>
        <AccountCard />
      </div>
      <div className="settings-actions row end">
        <Button onClick={() => onChange(structuredClone(defaultConfig))}>
          <RotateCcw size={15} />
          {t("resetConfig")}
        </Button>
        <Button primary onClick={onSave}>
          <Save size={16} />
          {t("saveConfig")}
        </Button>
      </div>
    </div>
  );
}
