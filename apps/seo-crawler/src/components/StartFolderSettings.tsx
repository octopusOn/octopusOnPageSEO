import type { CrawlConfig } from "../api/types";
import { useI18n } from "../i18n";

export function StartFolderSettings({ config, onChange, disabled = false }: {
  config: CrawlConfig; onChange: (config: CrawlConfig) => void; disabled?: boolean;
}) {
  const { t } = useI18n();
  return <div className="oc-field">
    <label className="setting-switch"><span>{t("startFolderOnly")}</span>
      <input type="checkbox" role="switch" disabled={disabled} checked={config.urls.start_folder_only === true}
        onChange={event => onChange({ ...config, urls: { ...config.urls, start_folder_only: event.target.checked } })} />
    </label>
    <small>{t("startFolderHint")}</small>
  </div>;
}
