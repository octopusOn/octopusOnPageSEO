import type { CrawlConfig } from "../api/types";
import { useI18n, type Key } from "../i18n";
import { Info } from "lucide-react";
import { ViewportSettings } from "./ViewportSettings";
import { ResourceCacheSettings } from "./ResourceCacheSettings";

export function RenderingSettings({
  config,
  onChange,
}: {
  config: CrawlConfig;
  onChange: (config: CrawlConfig) => void;
}) {
  const { t } = useI18n();
  const rendering = config.rendering;
  function numeric(
    field: "wait_ms" | "timeout_seconds",
    label: Key,
    min: number,
    max: number,
  ) {
    return (
      <label className="oc-field">
        {t(label)}
        <input
          className="oc-input"
          type="number"
          min={min}
          max={max}
          value={rendering[field]}
          onChange={(e) =>
            onChange({
              ...config,
              rendering: { ...rendering, [field]: Number(e.target.value) },
            })
          }
        />
      </label>
    );
  }
  return (
    <>
      <label className="oc-field">
        {t("renderMode")}
        <select
          className="oc-select"
          value={rendering.mode}
          onChange={(e) =>
            onChange({
              ...config,
              rendering: {
                ...rendering,
                mode: e.target.value as "html" | "javascript",
              },
            })
          }
        >
          <option value="html">{t("rawHtml")}</option>
          <option value="javascript">{t("javascript")}</option>
        </select>
      </label>
      {rendering.mode === "javascript" && (
        <>
          <div className="field-grid rendering-fields">
            {numeric("wait_ms", "renderWait", 0, 119999)}
            {numeric("timeout_seconds", "renderTimeout", 1, 120)}
          </div>
          <ViewportSettings config={config} onChange={onChange} />
          <ResourceCacheSettings config={config} onChange={onChange} />
          <label className="oc-field">
            {t("browserPath")}
            <input
              className="oc-input mono"
              value={rendering.browser_path ?? ""}
              placeholder={t("autoBrowser")}
              onChange={(e) =>
                onChange({
                  ...config,
                  rendering: {
                    ...rendering,
                    browser_path: e.target.value.trim() || null,
                  },
                })
              }
            />
          </label>
          <div className="oc-banner info">
            <Info size={18} />
            <p>{t("renderingNote")}</p>
          </div>
        </>
      )}
    </>
  );
}
