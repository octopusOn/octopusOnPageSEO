import { useRef, useState } from "react";
import { Upload, X } from "lucide-react";
import { extractUrlsFromText } from "../api/demo";
import type { IndexEngine, ScanConfig } from "../api/types";
import { useI18n } from "../i18n";

const list = (text: string) =>
  text
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean);

const FREE_ENGINES: { id: IndexEngine; label: string }[] = [
  { id: "google", label: "Google (site:)" },
  { id: "yandex", label: "Yandex (site:)" },
  { id: "bing", label: "Bing (site:)" },
  { id: "duckduckgo", label: "DuckDuckGo (site:)" },
];

export function NewScanModal({
  initialConfig,
  isScanning = false,
  onClose,
  onStart,
}: {
  initialConfig: ScanConfig;
  isScanning?: boolean;
  onClose: () => void;
  onStart: (config: ScanConfig) => void;
}) {
  const { t } = useI18n();
  const [config, setConfig] = useState<ScanConfig>(initialConfig);
  const [seedsText, setSeedsText] = useState(
    initialConfig.seed_queries.join(", "),
  );
  const [extraUrlsText, setExtraUrlsText] = useState(
    initialConfig.extra_urls.join("\n"),
  );
  const [importedCount, setImportedCount] = useState<number>(0);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  const toggleEngine = (eng: IndexEngine) => {
    setConfig((prev) => ({
      ...prev,
      engines: prev.engines.includes(eng)
        ? prev.engines.filter((e) => e !== eng)
        : [...prev.engines, eng],
    }));
  };

  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    if (isScanning) return;
    const seed_queries = seedsText
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    const extra_urls = extraUrlsText
      .split("\n")
      .map((s) => s.trim())
      .filter(Boolean);
    onStart({
      ...config,
      seed_queries,
      extra_urls,
    });
  };

  return (
    <div className="oc-scrim" onClick={onClose}>
      <div
        className="oc-modal"
        style={{ maxWidth: 680, width: "92vw", maxHeight: "88vh", overflowY: "auto" }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="row between" style={{ marginBottom: 16 }}>
          <h2 className="type-card-title">{t("newScan")}</h2>
          <button className="oc-btn ghost icon" onClick={onClose} aria-label={t("close")}>
            <X size={18} />
          </button>
        </div>

        <form onSubmit={submit} className="scan-form">
          <label className="oc-field">
            <span>{t("domainLabel")}</span>
            <input
              className="oc-input mono"
              required
              value={config.domain}
              onChange={(e) => setConfig({ ...config, domain: e.target.value })}
              placeholder={t("domainPlaceholder")}
            />
          </label>

          <div className="oc-panel" style={{ padding: 16, marginTop: 14 }}>
            <div className="type-label" style={{ marginBottom: 10 }}>
              {t("freeEnginesTitle")}
            </div>
            <div className="engine-grid">
              {FREE_ENGINES.map((eng) => (
                <label key={eng.id} className="oc-check">
                  <input
                    type="checkbox"
                    checked={config.engines.includes(eng.id)}
                    onChange={() => toggleEngine(eng.id)}
                  />
                  <span>{eng.label}</span>
                </label>
              ))}
            </div>
            <div style={{ marginTop: 12, display: "grid", gap: 8 }}>
              <label className="oc-check">
                <input
                  type="checkbox"
                  checked={config.use_wayback}
                  onChange={(e) =>
                    setConfig({ ...config, use_wayback: e.target.checked })
                  }
                />
                <span>{t("useWayback")}</span>
              </label>
              <label className="oc-check">
                <input
                  type="checkbox"
                  checked={config.use_commoncrawl}
                  onChange={(e) =>
                    setConfig({ ...config, use_commoncrawl: e.target.checked })
                  }
                />
                <span>{t("useCommonCrawl")}</span>
              </label>
              <label className="oc-check">
                <input
                  type="checkbox"
                  checked={config.use_sitemaps}
                  onChange={(e) =>
                    setConfig({ ...config, use_sitemaps: e.target.checked })
                  }
                />
                <span>{t("useSitemaps")}</span>
              </label>
              <label className="oc-check">
                <input
                  type="checkbox"
                  checked={config.expand_subpaths}
                  onChange={(e) =>
                    setConfig({ ...config, expand_subpaths: e.target.checked })
                  }
                />
                <span>{t("expandSubpaths")}</span>
              </label>
              <label className="oc-check">
                <input
                  type="checkbox"
                  checked={config.include_subdomains}
                  onChange={(e) =>
                    setConfig({
                      ...config,
                      include_subdomains: e.target.checked,
                    })
                  }
                />
                <span>{t("includeSubdomains")}</span>
              </label>
            </div>
          </div>

          <div className="oc-panel" style={{ padding: 16, marginTop: 14 }}>
            <div className="type-label" style={{ marginBottom: 10 }}>
              {t("detectionRulesTitle")}
            </div>
            <div style={{ display: "grid", gap: 8 }}>
              <label className="oc-check">
                <input
                  type="checkbox"
                  checked={config.detect_all_4xx}
                  onChange={(e) =>
                    setConfig({ ...config, detect_all_4xx: e.target.checked })
                  }
                />
                <span>{t("detectAll4xx")}</span>
              </label>
              <label className="oc-check">
                <input
                  type="checkbox"
                  checked={config.detect_5xx}
                  onChange={(e) =>
                    setConfig({ ...config, detect_5xx: e.target.checked })
                  }
                />
                <span>{t("detect5xx")}</span>
              </label>
              <label className="oc-check">
                <input
                  type="checkbox"
                  checked={config.detect_thin_200}
                  onChange={(e) =>
                    setConfig({ ...config, detect_thin_200: e.target.checked })
                  }
                />
                <span>{t("detectThin200")}</span>
              </label>
            </div>

            <div className="form-two-col" style={{ marginTop: 12 }}>
              <label className="oc-field">
                <span>{t("minWordCount")}</span>
                <input
                  type="number"
                  min={5}
                  max={2000}
                  className="oc-input"
                  value={config.min_word_count}
                  onChange={(e) =>
                    setConfig({
                      ...config,
                      min_word_count: Number(e.target.value) || 60,
                    })
                  }
                />
              </label>
              <label className="oc-field">
                <span>{t("pagesPerQuery")}</span>
                <input
                  type="number"
                  min={1}
                  max={10}
                  className="oc-input"
                  value={config.pages_per_query}
                  onChange={(e) =>
                    setConfig({
                      ...config,
                      pages_per_query: Number(e.target.value) || 3,
                    })
                  }
                />
              </label>
            </div>
          </div>

          <div style={{ marginTop: 14, display: "grid", gap: 12 }}>
            <div className="oc-field">
              <span>{t("seedQueriesLabel")}</span>
              <input
                className="oc-input mono"
                value={seedsText}
                onChange={(e) => setSeedsText(e.target.value)}
                placeholder={t("seedPlaceholder")}
              />
              <div
                className="row"
                style={{ gap: 6, marginTop: 6, flexWrap: "wrap" }}
              >
                {[
                  {
                    label: t("seedPresetShop"),
                    items: list(t("seedPresetShopItems")),
                  },
                  {
                    label: t("seedPresetBlog"),
                    items: list(t("seedPresetBlogItems")),
                  },
                  {
                    label: t("seedPresetDork"),
                    items: list(t("seedPresetDorkItems")),
                  },
                  {
                    label: t("seedPresetParams"),
                    items: ["inurl:page", "inurl:sort", "inurl:filter"],
                  },
                ].map((preset) => (
                  <button
                    key={preset.label}
                    type="button"
                    className="oc-chip"
                    onClick={() => {
                      const existing = list(seedsText);
                      const merged = Array.from(
                        new Set([...existing, ...preset.items]),
                      );
                      setSeedsText(merged.join(", "));
                    }}
                  >
                    {preset.label}
                  </button>
                ))}
              </div>
            </div>
            <div className="oc-field">
              <div className="row between" style={{ marginBottom: 4 }}>
                <span>{t("extraUrlsLabel")}</span>
                <div className="row" style={{ gap: 8 }}>
                  {importedCount > 0 && (
                    <span className="oc-badge good">
                      +{importedCount} {t("importedUrlsToast")}
                    </span>
                  )}
                  <input
                    ref={fileInputRef}
                    type="file"
                    accept=".csv,.txt,.tsv,.json,.xml"
                    style={{ display: "none" }}
                    onChange={(e) => {
                      const file = e.target.files?.[0];
                      if (!file) return;
                      void file.text().then((raw) => {
                        const extracted = extractUrlsFromText(raw);
                        if (extracted.length > 0) {
                          const existing = extractUrlsFromText(extraUrlsText);
                          const merged = Array.from(
                            new Set([...existing, ...extracted]),
                          );
                          setExtraUrlsText(merged.join("\n"));
                          setImportedCount(extracted.length);
                        }
                      });
                    }}
                  />
                  <button
                    type="button"
                    className="oc-btn small ghost"
                    onClick={() => fileInputRef.current?.click()}
                  >
                    <Upload size={13} />
                    {t("importUrlsBtn")}
                  </button>
                </div>
              </div>
              <textarea
                className="oc-textarea mono"
                rows={3}
                value={extraUrlsText}
                onChange={(e) => setExtraUrlsText(e.target.value)}
                placeholder={t("extraUrlsPlaceholder")}
              />
            </div>
          </div>

          <div className="row end" style={{ marginTop: 20, gap: 10 }}>
            <button type="button" className="oc-btn ghost" onClick={onClose}>
              {t("cancel")}
            </button>
            <button type="submit" className="oc-btn primary" disabled={isScanning}>
              {isScanning ? t("scanRunning") : t("startScan")}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
