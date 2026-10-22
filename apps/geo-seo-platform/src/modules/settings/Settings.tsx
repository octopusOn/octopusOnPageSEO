import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { LogOut } from "lucide-react";
import { Button, Field, Segmented } from "@octopusseo/ui";
import { api } from "../../api/client";
import { signOut } from "../../api/transport";
import type { AppInfo, Settings as SettingsData } from "../../api/types";
import { LANGUAGES, useI18n, type Lang } from "../../i18n";
import { useShell } from "../../lib/context";
import { Assistants } from "./Assistants";
import { SyncCard } from "./SyncCard";

function Proxy({ settings }: { settings: SettingsData }) {
  const { t } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const [proxy, setProxy] = useState(settings.proxy ?? "");
  const save = useMutation({
    mutationFn: () => api.saveSettings({ ...settings, proxy: proxy.trim() || undefined }),
    onSuccess: () => {
      notify(t("settings.proxySaved"));
      void queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
    onError: (thrown) => fail(thrown),
  });
  return (
    <form
      className="inline-form"
      onSubmit={(event) => {
        event.preventDefault();
        save.mutate();
      }}
    >
      <Field label={t("settings.proxy")} hint={t("settings.proxyHint")}>
        <input className="oc-input" value={proxy} onChange={(event) => setProxy(event.target.value)} spellCheck={false} placeholder="http://127.0.0.1:3128" />
      </Field>
      <Button type="submit" small busy={save.isPending}>
        {t("common.save")}
      </Button>
    </form>
  );
}

export function Settings({ info }: { info: AppInfo | undefined }) {
  const { t, lang, setLang } = useI18n();
  const { mode } = useShell();
  const settings = useQuery({ queryKey: ["settings"], queryFn: api.settings });
  const web = info?.host === "web" || mode === "web";
  return (
    <div className="page-stack settings-page">
      {mode !== "demo" && <SyncCard web={web} />}
      <section className="oc-card">
        <h2 className="oc-card-title">{t("settings.language")}</h2>
        <Segmented<Lang>
          label={t("settings.language")}
          value={lang}
          onChange={setLang}
          options={LANGUAGES.map((language) => ({ value: language.code, label: language.name }))}
        />
      </section>
      {!web && settings.data && (
        <section className="oc-card">
          <Proxy key={settings.data.proxy ?? ""} settings={settings.data} />
        </section>
      )}
      <Assistants web={web} command={info?.mcp_command} />
      <section className="oc-card">
        <h2 className="oc-card-title">{t("settings.about")}</h2>
        <dl className="facts">
          <div>
            <dt>{t("settings.version")}</dt>
            <dd>{info?.version ?? "—"}</dd>
          </div>
          <div>
            <dt>{t("settings.host")}</dt>
            <dd>{web ? t("settings.hostWeb") : t("settings.hostDesktop")}</dd>
          </div>
          {info?.data_dir && (
            <div>
              <dt>{t("settings.dataDir")}</dt>
              <dd className="mono">{info.data_dir}</dd>
            </div>
          )}
        </dl>
      </section>
      {web && (
        <section className="oc-card">
          <h2 className="oc-card-title">{t("settings.session")}</h2>
          <p className="faint">{t("settings.signOutHint")}</p>
          <Button onClick={() => void signOut()}>
            <LogOut size={15} />
            {t("common.signOut")}
          </Button>
        </section>
      )}
    </div>
  );
}
