import { useState } from "react";
import type { CrawlConfig } from "../api/types";
import { useI18n } from "../i18n";
import { desktopViewport, mobileViewport, effectiveViewport, viewportPreset, type ViewportPreset } from "../lib/viewport";

export function ViewportSettings({ config, onChange, disabled = false }: {
  config: CrawlConfig;
  onChange: (config: CrawlConfig) => void;
  disabled?: boolean;
}) {
  const { t } = useI18n();
  const rendering = config.rendering;
  const [customViewport, setCustomViewport] = useState(false);
  const viewport = effectiveViewport(config);
  const preset = customViewport && rendering.viewport ? "custom" : viewportPreset(rendering.viewport);
  return <>
          <label className="oc-field">
            {t("renderViewport")}
            <select disabled={disabled} className="oc-select" value={preset} onChange={event => {
              const selected = event.target.value as ViewportPreset;
              setCustomViewport(selected === "custom");
              onChange({ ...config, rendering: { ...rendering, viewport: selected === "auto" ? null
                : { ...(selected === "desktop" ? desktopViewport : selected === "mobile" ? mobileViewport : viewport) } } });
            }}>
              <option value="auto">{t("viewportAuto")}</option>
              <option value="desktop">{t("viewportDesktop")} · 1280 × 800</option>
              <option value="mobile">{t("viewportMobile")} · 390 × 844</option>
              <option value="custom">{t("viewportCustom")}</option>
            </select>
          </label>
          {preset === "custom" && <>
            <div className="field-grid">
              {(["width", "height"] as const).map(field => <label className="oc-field" key={field}>
                {t(field === "width" ? "viewportWidth" : "viewportHeight")}
                <input disabled={disabled} className="oc-input" type="number" min={240} max={3840} step={1} value={viewport[field]}
                  onChange={event => onChange({ ...config, rendering: { ...rendering, viewport: { ...viewport, [field]: Number(event.target.value) } } })} />
              </label>)}
            </div>
            <label className="viewport-mobile-check"><input disabled={disabled} type="checkbox" checked={viewport.mobile}
              onChange={event => onChange({ ...config, rendering: { ...rendering, viewport: { ...viewport, mobile: event.target.checked } } })} />{t("viewportMobileLayout")}</label>
          </>}
          <p className="render-viewport-note">{t("viewportNote")}</p>
  </>;
}
