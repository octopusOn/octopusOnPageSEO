import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Globe2 } from "lucide-react";

import { api } from "../api/commands";
import type { CommandError, GeoUpdateReport } from "../api/types";
import { useToast } from "../components/Toasts";
import { Button, Card, ErrorBanner, Segmented } from "../components/ui";
import { formatDateTime, formatNumber } from "../lib/format";
import { useStored } from "../lib/storage";
import { useI18n, type Language } from "../i18n";
import { keys } from "../state";

export type Theme = "system" | "light" | "dark";

/** Applies the stored theme to the document. */
export function useTheme(): [Theme, (theme: Theme) => void] {
  const [theme, setTheme] = useStored<Theme>("theme", "system");
  useEffect(() => {
    if (theme === "system") document.documentElement.removeAttribute("data-theme");
    else document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);
  return [theme, setTheme];
}

export function SettingsPage({ theme, setTheme }: { theme: Theme; setTheme: (theme: Theme) => void }) {
  const { t, language, setLanguage } = useI18n();
  const info = useQuery({ queryKey: keys.appInfo, queryFn: api.appInfo });
  return (
    <div className="page">
      <h1>{t("settings.title")}</h1>
      <Card>
        <div className="stack">
          <div className="row between">
            <strong>{t("settings.language")}</strong>
            <Segmented<Language>
              value={language}
              onChange={setLanguage}
              options={[
                { value: "tr", label: "Türkçe" },
                { value: "en", label: "English" },
              ]}
            />
          </div>
          <div className="row between">
            <strong>{t("settings.theme")}</strong>
            <Segmented<Theme>
              value={theme}
              onChange={setTheme}
              options={[
                { value: "system", label: t("settings.themeSystem") },
                { value: "light", label: t("settings.themeLight") },
                { value: "dark", label: t("settings.themeDark") },
              ]}
            />
          </div>
        </div>
      </Card>
      <GeoCard />
      {info.data && (
        <Card>
          <table className="table-simple">
            <tbody>
              <tr>
                <td>{t("settings.version")}</td>
                <td className="mono">{info.data.version}</td>
              </tr>
              <tr>
                <td>{t("settings.dataDir")}</td>
                <td className="mono">{info.data.dataDir}</td>
              </tr>
              <tr>
                <td>{t("settings.memory")}</td>
                <td className="mono">{info.data.memoryLimitMb} MiB</td>
              </tr>
            </tbody>
          </table>
          <p className="muted small" style={{ marginTop: 10 }}>
            {t("settings.memoryHint")}
          </p>
        </Card>
      )}
    </div>
  );
}

/** IP geolocation data (DB-IP Lite): country and network of client IPs in reports. */
function GeoCard() {
  const { t, locale } = useI18n();
  const notify = useToast();
  const queryClient = useQueryClient();
  const status = useQuery({ queryKey: keys.geo, queryFn: api.geoStatus });
  const update = useMutation<GeoUpdateReport, CommandError>({
    mutationFn: api.updateGeoData,
    onSuccess: async (report) => {
      notify(
        "success",
        t("geo.updated", {
          release: report.release.country,
          ranges: formatNumber(report.country_ranges + report.asn_ranges, locale),
        }),
      );
      await queryClient.invalidateQueries({ queryKey: keys.geo });
      await queryClient.invalidateQueries({ queryKey: ["site"] });
    },
  });
  const release = status.data?.release;
  return (
    <Card title={t("geo.title")}>
      <div className="stack">
        <p className="muted small">{t("geo.intro")}</p>
        <div className="row between">
          <span>
            {release
              ? t("geo.installed", { release: release.country, date: formatDateTime(release.fetched_at, locale) })
              : t("geo.missing")}
          </span>
          <Button icon={<Globe2 size={15} />} busy={update.isPending} onClick={() => update.mutate()}>
            {release ? t("geo.update") : t("geo.download")}
          </Button>
        </div>
        <ErrorBanner error={update.error} />
        <p className="small muted">
          <a href="https://db-ip.com" target="_blank" rel="noreferrer">
            IP Geolocation by DB-IP
          </a>{" "}
          · {t("geo.licence")}
        </p>
      </div>
    </Card>
  );
}
