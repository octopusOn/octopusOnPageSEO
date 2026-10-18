import { useEffect, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api, errorText } from "../api/client";
import type { AppInfo, Engine, Settings as SettingsShape } from "../api/types";
import { Button, Field, Loading } from "../components/ui";
import { useI18n } from "../i18n";

export function Settings({
  settings,
  info,
  notify,
}: {
  settings: SettingsShape | undefined;
  info: AppInfo | undefined;
  notify: (text: string, error?: boolean) => void;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [draft, setDraft] = useState<SettingsShape | undefined>(settings);
  const [chains, setChains] = useState<Record<string, string>>({});
  useEffect(() => {
    setDraft(settings);
    setChains(Object.fromEntries(Object.entries(settings?.serp.chains ?? {}).map(([engine, ids]) => [engine, ids.join(", ")])));
  }, [settings]);
  const save = useMutation({
    mutationFn: (next: SettingsShape) => api.saveSettings(next),
    onSuccess: () => {
      notify(t("saved"));
      void queryClient.invalidateQueries({ queryKey: ["settings"] });
      void queryClient.invalidateQueries({ queryKey: ["browser"] });
    },
    onError: (error) => notify(errorText(error), true),
  });
  if (!draft) return <Loading />;
  const set = (patch: Partial<SettingsShape>) => setDraft({ ...draft, ...patch });
  const submit = () =>
    save.mutate({
      ...draft,
      serp: {
        ...draft.serp,
        chains: Object.fromEntries(
          Object.entries(chains).map(([engine, text]) => [engine, text.split(",").map((id) => id.trim()).filter(Boolean)]),
        ),
      },
    });
  return (
    <div className="stack settings-page">
      <section className="oc-panel">
        <h2 className="oc-panel-title">{t("dataTitle")}</h2>
        <p className="small">
          {t("dataFolder")}: <span className="mono">{info?.data_dir}</span>
        </p>
        <p className="small faint">{t("secretStore", { store: info?.secret_store ?? "" })}</p>
      </section>
      <section className="oc-panel">
        <h2 className="oc-panel-title">{t("defaultsTitle")}</h2>
        <div className="form-row four">
          <Field label={t("country")}>
            <input className="oc-input" maxLength={2} value={draft.projects.country} onChange={(e) => set({ projects: { ...draft.projects, country: e.target.value.toLowerCase() } })} />
          </Field>
          <Field label={t("languageCode")}>
            <input className="oc-input" maxLength={2} value={draft.projects.language} onChange={(e) => set({ projects: { ...draft.projects, language: e.target.value.toLowerCase() } })} />
          </Field>
          <Field label={t("primaryEngine")}>
            <select className="oc-select" value={draft.projects.engines[0] ?? "google"} onChange={(e) => set({ projects: { ...draft.projects, engines: [e.target.value as Engine] } })}>
              <option value="google">{t("google")}</option>
              <option value="yandex">{t("yandex")}</option>
              <option value="bing">{t("bing")}</option>
            </select>
          </Field>
        </div>
        <div className="form-row four">
          <Field label={t("confirmAbove")} hint={t("confirmAboveHint")}>
            <input className="oc-input" type="number" min={0} step={0.5} value={draft.confirm_above_usd} onChange={(e) => set({ confirm_above_usd: Number(e.target.value) })} />
          </Field>
          <Field label={t("wordstatQuota")} hint={t("wordstatQuotaHint")}>
            <input className="oc-input" type="number" min={1} value={draft.wordstat_hourly_quota} onChange={(e) => set({ wordstat_hourly_quota: Number(e.target.value) })} />
          </Field>
          <Field label={t("serpMaxAge")} hint={t("serpMaxAgeHint")}>
            <input className="oc-input" type="number" min={1} max={365} value={draft.serp.max_age_days} onChange={(e) => set({ serp: { ...draft.serp, max_age_days: Number(e.target.value) } })} />
          </Field>
        </div>
      </section>
      <section className="oc-panel">
        <h2 className="oc-panel-title">{t("chainsTitle")}</h2>
        <p className="faint small">{t("chainsLead")}</p>
        {(["google", "yandex", "bing"] as Engine[]).map((engine) => (
          <Field key={engine} label={t(engine)}>
            <input className="oc-input" value={chains[engine] ?? ""} onChange={(e) => setChains({ ...chains, [engine]: e.target.value })} />
          </Field>
        ))}
      </section>
      <section className="oc-panel">
        <h2 className="oc-panel-title">{t("browserSettingsTitle")}</h2>
        <p className="faint small">{t("browserSettingsLead")}</p>
        <div className="form-row">
          <Field label={t("browserExecutable")} hint={t("browserExecutableHint")}>
            <input
              className="oc-input"
              value={draft.browser.executable ?? ""}
              onChange={(e) => set({ browser: { ...draft.browser, executable: e.target.value.trim() || null } })}
              placeholder="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
            />
          </Field>
          <Field label={t("browserProxy")} hint={t("browserProxyHint")}>
            <input className="oc-input" value={draft.browser.proxy ?? ""} onChange={(e) => set({ browser: { ...draft.browser, proxy: e.target.value.trim() || null } })} placeholder="http://127.0.0.1:8080" />
          </Field>
        </div>
        <div className="form-row four">
          <Field label={t("browserDelay")} hint={t("browserDelayHint")}>
            <input className="oc-input" type="number" min={10} value={draft.browser.delay_secs} onChange={(e) => set({ browser: { ...draft.browser, delay_secs: Number(e.target.value) } })} />
          </Field>
          <Field label={t("browserCap")}>
            <input className="oc-input" type="number" min={1} value={draft.browser.daily_cap} onChange={(e) => set({ browser: { ...draft.browser, daily_cap: Number(e.target.value) } })} />
          </Field>
        </div>
      </section>
      <section className="oc-panel">
        <h2 className="oc-panel-title">{t("networkTitle")}</h2>
        <div className="form-row">
          <Field label={t("proxy")} hint={t("proxyHint")}>
            <input className="oc-input" value={draft.network.proxy ?? ""} onChange={(e) => set({ network: { ...draft.network, proxy: e.target.value.trim() || null } })} placeholder="http://127.0.0.1:8080" />
          </Field>
          <Field label={t("timeout")}>
            <input className="oc-input" type="number" min={5} max={300} value={draft.network.timeout_secs} onChange={(e) => set({ network: { ...draft.network, timeout_secs: Number(e.target.value) } })} />
          </Field>
        </div>
      </section>
      <div className="row end">
        <Button primary busy={save.isPending} onClick={submit}>
          {t("save")}
        </Button>
      </div>
    </div>
  );
}
