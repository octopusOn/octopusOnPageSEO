import { Check, KeyRound, Play, Sparkles } from "lucide-react";
import type { IndexEngine, ScanConfig } from "../api/types";
import { AccountCard, PaymentModes } from "../components/AccountPanel";
import { useI18n, type Key } from "../i18n";

const ENGINE_CARDS: {
  id: IndexEngine;
  title: string;
  desc: Key;
}[] = [
  {
    id: "google",
    title: "Google Search (site:)",
    desc: "engineDesc_google",
  },
  {
    id: "yandex",
    title: "Yandex Search (site:)",
    desc: "engineDesc_yandex",
  },
  {
    id: "bing",
    title: "Microsoft Bing (site:)",
    desc: "engineDesc_bing",
  },
  {
    id: "duckduckgo",
    title: "DuckDuckGo HTML (site:)",
    desc: "engineDesc_duckduckgo",
  },
];

export function EnginesPage({
  config,
  isScanning = false,
  onChangeConfig,
  onSave,
  onStartScan,
  notify,
}: {
  config: ScanConfig;
  isScanning?: boolean;
  onChangeConfig: (next: ScanConfig) => void;
  onSave: () => void;
  onStartScan: () => void;
  notify: (text: string, error?: boolean) => void;
}) {
  const { t } = useI18n();

  const toggleEngine = (id: IndexEngine) => {
    onChangeConfig({
      ...config,
      engines: config.engines.includes(id)
        ? config.engines.filter((e) => e !== id)
        : [...config.engines, id],
    });
  };

  return (
    <div className="finder-page">
      <div className="oc-pagehead">
        <div className="oc-pagehead-text">
          <h1 className="oc-pagehead-title">{t("engines")}</h1>
          <p className="oc-pagehead-lead">{t("enginesLead")}</p>
        </div>
        <div className="oc-pagehead-actions">
          <button className="oc-btn" onClick={onSave}>
            <Check size={15} />
            {t("saveSettings")}
          </button>
          <button
            className="oc-btn primary"
            onClick={onStartScan}
            disabled={isScanning}
          >
            <Play size={15} />
            {isScanning ? t("scanRunning") : t("newScan")}
          </button>
        </div>
      </div>

      <div className="oc-panel" style={{ padding: 20, marginBottom: 18 }}>
        <div className="row between" style={{ marginBottom: 14 }}>
          <h2 className="type-card-title">{t("freeEnginesTitle")}</h2>
          <span className="oc-badge good">{t("freeNoApiBadge")}</span>
        </div>
        <div className="engine-cards-grid">
          {ENGINE_CARDS.map((card) => {
            const active = config.engines.includes(card.id);
            return (
              <div
                key={card.id}
                className={`oc-card interactive ${active ? "selected-card" : ""}`}
                onClick={() => toggleEngine(card.id)}
              >
                <div className="row between">
                  <b>{card.title}</b>
                  <input
                    type="checkbox"
                    checked={active}
                    onChange={() => {}}
                  />
                </div>
                <p className="muted" style={{ fontSize: 12.5, marginTop: 8 }}>
                  {t(card.desc)}
                </p>
              </div>
            );
          })}
        </div>

        <div style={{ marginTop: 16, display: "grid", gap: 10 }}>
          <label className="oc-check">
            <input
              type="checkbox"
              checked={config.use_wayback}
              onChange={(e) =>
                onChangeConfig({ ...config, use_wayback: e.target.checked })
              }
            />
            <span>
              <Sparkles
                size={14}
                style={{ verticalAlign: "middle", marginRight: 6 }}
              />
              {t("useWayback")}
            </span>
          </label>
          <label className="oc-check">
            <input
              type="checkbox"
              checked={config.use_commoncrawl}
              onChange={(e) =>
                onChangeConfig({
                  ...config,
                  use_commoncrawl: e.target.checked,
                })
              }
            />
            <span>{t("useCommonCrawl")}</span>
          </label>
          <label className="oc-check">
            <input
              type="checkbox"
              checked={config.use_sitemaps}
              onChange={(e) =>
                onChangeConfig({
                  ...config,
                  use_sitemaps: e.target.checked,
                })
              }
            />
            <span>{t("useSitemaps")}</span>
          </label>
          <label className="oc-check">
            <input
              type="checkbox"
              checked={config.expand_subpaths}
              onChange={(e) =>
                onChangeConfig({
                  ...config,
                  expand_subpaths: e.target.checked,
                })
              }
            />
            <span>{t("expandSubpaths")}</span>
          </label>
        </div>
      </div>

      <AccountCard notify={notify} />

      <div className="oc-panel" style={{ padding: 20 }}>
        <div className="row between" style={{ marginBottom: 14 }}>
          <h2 className="type-card-title">
            <KeyRound
              size={18}
              style={{ verticalAlign: "middle", marginRight: 8 }}
            />
            {t("apiSectionTitle")}
          </h2>
          <span className="oc-badge info">{t("optionalBadge")}</span>
        </div>

        <div className="form-two-col">
          <label className="oc-field">
            <span>{t("googleApiKey")}</span>
            <input
              type="password"
              className="oc-input mono"
              value={config.api.google_api_key}
              onChange={(e) =>
                onChangeConfig({
                  ...config,
                  api: { ...config.api, google_api_key: e.target.value },
                })
              }
              placeholder="AIzaSy..."
            />
          </label>
          <label className="oc-field">
            <span>{t("googleCx")}</span>
            <input
              className="oc-input mono"
              value={config.api.google_cx}
              onChange={(e) =>
                onChangeConfig({
                  ...config,
                  api: { ...config.api, google_cx: e.target.value },
                })
              }
              placeholder="a1b2c3d4e5..."
            />
          </label>

          <label className="oc-field">
            <span>{t("yandexApiKey")}</span>
            <input
              type="password"
              className="oc-input mono"
              value={config.api.yandex_api_key}
              onChange={(e) =>
                onChangeConfig({
                  ...config,
                  api: { ...config.api, yandex_api_key: e.target.value },
                })
              }
              placeholder="AQVN..."
            />
          </label>
          <label className="oc-field">
            <span>{t("yandexUser")}</span>
            <input
              className="oc-input mono"
              value={config.api.yandex_user}
              onChange={(e) =>
                onChangeConfig({
                  ...config,
                  api: { ...config.api, yandex_user: e.target.value },
                })
              }
              placeholder="b1g..."
            />
          </label>

          <label className="oc-field">
            <span>{t("serperApiKey")}</span>
            <input
              type="password"
              className="oc-input mono"
              value={config.api.serper_api_key}
              onChange={(e) =>
                onChangeConfig({
                  ...config,
                  api: { ...config.api, serper_api_key: e.target.value },
                })
              }
              placeholder={t("serperPlaceholder")}
            />
          </label>
          <label className="oc-field">
            <span>{t("serpapiKey")}</span>
            <input
              type="password"
              className="oc-input mono"
              value={config.api.serpapi_key}
              onChange={(e) =>
                onChangeConfig({
                  ...config,
                  api: { ...config.api, serpapi_key: e.target.value },
                })
              }
              placeholder={t("serpapiPlaceholder")}
            />
          </label>
        </div>

        <PaymentModes config={config.api} notify={notify} />
      </div>
    </div>
  );
}
