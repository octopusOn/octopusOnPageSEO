import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Globe2 } from "lucide-react";

import { api } from "../api/commands";
import type { CommandError, GeoUpdateReport } from "../api/types";
import { AccountCard } from "../components/AccountCard";
import { useToast } from "../components/Toasts";
import { Button, Card, ErrorBanner, Segmented } from "../components/ui";
import { formatDateTime, formatNumber } from "../lib/format";
import { useI18n, type Language } from "../i18n";
import { keys } from "../state";

export function SettingsPage() {
  const { t, language, setLanguage } = useI18n();
  const info = useQuery({ queryKey: keys.appInfo, queryFn: api.appInfo });
  return (
    <div className="page">
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
        </div>
      </Card>
      <AccountCard />
      <GeoCard />
      {info.data && (
        <Card>
          <table className="oc-table">
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
