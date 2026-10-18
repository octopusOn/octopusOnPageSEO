import { Check } from "lucide-react";
import type { ScanConfig } from "../api/types";
import { useI18n } from "../i18n";

export function Settings({
  config,
  onChangeConfig,
  onSave,
}: {
  config: ScanConfig;
  onChangeConfig: (next: ScanConfig) => void;
  onSave: () => void;
}) {
  const { t, lang, setLang } = useI18n();

  return (
    <div className="finder-page">
      <div className="oc-pagehead">
        <div className="oc-pagehead-text">
          <h1 className="oc-pagehead-title">{t("settings")}</h1>
          <p className="oc-pagehead-lead">{t("settingsLead")}</p>
        </div>
        <div className="oc-pagehead-actions">
          <button className="oc-btn primary" onClick={onSave}>
            <Check size={15} />
            {t("saveSettings")}
          </button>
        </div>
      </div>

      <div className="oc-panel" style={{ padding: 20, maxWidth: 680 }}>
        <div className="form-two-col">
          <label className="oc-field">
            <span>{t("language")}</span>
            <select
              className="oc-select"
              value={lang}
              onChange={(e) => setLang(e.target.value === "en" ? "en" : "tr")}
            >
              <option value="tr">Türkçe (TR)</option>
              <option value="en">English (EN)</option>
            </select>
          </label>

          <label className="oc-field">
            <span>{t("minWordCount")}</span>
            <input
              type="number"
              min={5}
              max={2000}
              className="oc-input"
              value={config.min_word_count}
              onChange={(e) =>
                onChangeConfig({
                  ...config,
                  min_word_count: Number(e.target.value) || 60,
                })
              }
            />
          </label>

          <label className="oc-field">
            <span>{t("concurrencyLabel")}</span>
            <input
              type="number"
              min={1}
              max={32}
              className="oc-input"
              value={config.concurrency}
              onChange={(e) =>
                onChangeConfig({
                  ...config,
                  concurrency: Number(e.target.value) || 8,
                })
              }
            />
          </label>

          <label className="oc-field">
            <span>{t("timeoutLabel")}</span>
            <input
              type="number"
              min={3}
              max={60}
              className="oc-input"
              value={config.timeout_seconds}
              onChange={(e) =>
                onChangeConfig({
                  ...config,
                  timeout_seconds: Number(e.target.value) || 15,
                })
              }
            />
          </label>

          <label className="oc-field">
            <span>{t("maxRedirectsLabel")}</span>
            <input
              type="number"
              min={0}
              max={10}
              className="oc-input"
              value={config.max_redirects}
              onChange={(e) =>
                onChangeConfig({
                  ...config,
                  max_redirects: Math.min(
                    10,
                    Math.max(0, Number(e.target.value) || 0),
                  ),
                })
              }
            />
          </label>

          <div className="oc-field" style={{ gridColumn: "1 / -1" }}>
            <span>User-Agent</span>
            <input
              className="oc-input mono"
              value={config.user_agent}
              onChange={(e) =>
                onChangeConfig({ ...config, user_agent: e.target.value })
              }
            />
            <div
              className="row"
              style={{ gap: 6, marginTop: 8, flexWrap: "wrap" }}
            >
              {[
                {
                  label: t("uaChromeDesktop"),
                  ua: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 (compatible; octopusSEO-Finder/0.1)",
                },
                {
                  label: "Googlebot Desktop",
                  ua: "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
                },
                {
                  label: "Googlebot Smartphone",
                  ua: "Mozilla/5.0 (Linux; Android 6.0.1; Nexus 5X Build/MMB29P) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
                },
                {
                  label: "YandexBot",
                  ua: "Mozilla/5.0 (compatible; YandexBot/3.0; +http://yandex.com/bots)",
                },
                {
                  label: "Bingbot",
                  ua: "Mozilla/5.0 (compatible; bingbot/2.0; +http://www.bing.com/bingbot.htm)",
                },
              ].map((preset) => (
                <button
                  key={preset.label}
                  type="button"
                  className={`oc-chip ${config.user_agent === preset.ua ? "active-chip" : ""}`}
                  onClick={() =>
                    onChangeConfig({ ...config, user_agent: preset.ua })
                  }
                >
                  {preset.label}
                </button>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
