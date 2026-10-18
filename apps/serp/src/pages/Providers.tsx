import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowDown, ArrowUp, Coins, KeyRound, LogIn, LogOut, MonitorSmartphone, Trash2 } from "lucide-react";
import { api, errorText } from "../api/client";
import { engines, type AppInfo, type Engine, type Job, type ProviderInfo, type SecretInfo, type Settings, type SourceMode } from "../api/types";
import { CustomProviders, ParserOverrides } from "../components/Editors";
import { Button, ErrorState, Loading, Modal } from "../components/ui";
import { number, usd, useI18n, type Key } from "../i18n";
import { JobCard } from "../components/JobCard";

/** The OctopusOn SEO account: connect it, see the balance, buy credits. */
function AccountPanel({ jobs, notify }: { jobs: Job[]; notify: (text: string, error?: boolean) => void }) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  // Another OctopusOn SEO program may connect or disconnect the shared account meanwhile.
  const status = useQuery({ queryKey: ["cloud"], queryFn: api.cloudStatus, refetchInterval: 30_000, refetchOnWindowFocus: true });
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["cloud"] });
    void queryClient.invalidateQueries({ queryKey: ["providers"] });
    void queryClient.invalidateQueries({ queryKey: ["estimate"] });
  };
  const login = useMutation({ mutationFn: api.accountLogin, onError: (error) => notify(errorText(error), true) });
  const logout = useMutation({ mutationFn: api.accountLogout, onSuccess: refresh, onError: (error) => notify(errorText(error), true) });
  const open = useMutation({
    mutationFn: (page: "credits" | "devices") => api.openAccountPage(page),
    onError: (error) => notify(errorText(error), true),
  });
  const loginJob = jobs.find((job) => job.kind === "account");
  useEffect(() => {
    if (loginJob && loginJob.status !== "running") refresh();
    // Refresh once the sign-in ends.
  }, [loginJob?.status]);
  const cloud = status.data;
  const account = cloud?.account;
  useEffect(() => {
    // A connection made or ended elsewhere changes which sources run on credits.
    void queryClient.invalidateQueries({ queryKey: ["providers"] });
    void queryClient.invalidateQueries({ queryKey: ["estimate"] });
  }, [cloud?.signed_in]);
  return (
    <section className="oc-panel account-panel">
      <div className="oc-panel-head">
        <h2>
          <Coins size={16} /> {t("accountTitle")}
        </h2>
      </div>
      <p className="panel-lead">{t("accountLead")}</p>
      {loginJob && loginJob.status === "running" && <JobCard job={loginJob} onStop={(id) => void api.cancel(id)} />}
      {cloud?.error && <div className="oc-banner warning">{cloud.error}</div>}
      {account?.low_balance && <div className="oc-banner warning">{t("accountLow")}</div>}
      <div className="row between wrap">
        <div>
          {cloud?.signed_in ? (
            <>
              <b>{account?.email ?? t("ready")}</b>
              {account && (
                <p className="account-balance">
                  {t("accountBalance", { credits: number(account.available_micros / account.credit_unit_micros, lang, 2) })}
                </p>
              )}
            </>
          ) : (
            <b>{t("accountNotConnected")}</b>
          )}
        </div>
        <div className="row">
          {cloud?.signed_in ? (
            <>
              <Button primary busy={open.isPending} onClick={() => open.mutate("credits")}>
                <Coins size={14} />
                {t("buyCredits")}
              </Button>
              <Button className="ghost" onClick={() => open.mutate("devices")}>
                <MonitorSmartphone size={14} />
                {t("accountDevices")}
              </Button>
              <Button className="ghost" busy={logout.isPending} onClick={() => logout.mutate()}>
                <LogOut size={14} />
                {t("accountSignOut")}
              </Button>
            </>
          ) : (
            <Button primary busy={login.isPending || loginJob?.status === "running"} onClick={() => login.mutate()}>
              <LogIn size={14} />
              {t("accountConnect")}
            </Button>
          )}
        </div>
      </div>
    </section>
  );
}

/** Own key, credits or automatic for a built-in source. */
function PayWith({ provider, signedIn, notify }: { provider: ProviderInfo; signedIn: boolean; notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const mode = useMutation({
    mutationFn: (next: SourceMode) => api.setSourceMode(provider.id, next),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["providers"] });
      void queryClient.invalidateQueries({ queryKey: ["estimate"] });
    },
    onError: (error) => notify(errorText(error), true),
  });
  return (
    <div className="pay-with">
      <span className="type-label">{t("payWith")}</span>
      <div className="oc-segmented" role="radiogroup" aria-label={t("payWith")}>
        {(["auto", "own", "credit"] as const).map((option) => (
          <button
            key={option}
            type="button"
            role="radio"
            aria-checked={provider.mode === option}
            className={provider.mode === option ? "active" : undefined}
            disabled={mode.isPending || (option === "credit" && !provider.credit && provider.mode !== "credit")}
            onClick={() => mode.mutate(option)}
          >
            {t(`mode.${option}` as Key)}
          </button>
        ))}
      </div>
      {!provider.credit && <small className="faint">{signedIn ? t("creditUnavailable") : t("creditSignIn")}</small>}
    </div>
  );
}

const CREDITABLE = ["serper", "serpapi", "dataforseo", "yandex-cloud"];

export function Providers({
  settings,
  info,
  jobs,
  notify,
}: {
  settings?: Settings;
  info?: AppInfo;
  jobs: Job[];
  notify: (text: string, error?: boolean) => void;
}) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const providers = useQuery({ queryKey: ["providers"], queryFn: api.providers });
  const cloud = useQuery({ queryKey: ["cloud"], queryFn: api.cloudStatus });
  const [keyFor, setKeyFor] = useState<{ provider: ProviderInfo; secret: SecretInfo } | null>(null);
  const saveSettings = useMutation({
    mutationFn: api.saveSettings,
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["settings"] });
      void queryClient.invalidateQueries({ queryKey: ["providers"] });
      void queryClient.invalidateQueries({ queryKey: ["estimate"] });
    },
    onError: (error) => notify(errorText(error), true),
  });
  const removeKey = useMutation({
    mutationFn: ({ provider, name }: { provider: string; name: string }) => api.removeSecret(provider, name),
    onSuccess: () => {
      notify(t("keyRemoved"));
      void queryClient.invalidateQueries({ queryKey: ["providers"] });
      void queryClient.invalidateQueries({ queryKey: ["estimate"] });
    },
    onError: (error) => notify(errorText(error), true),
  });
  if (providers.isLoading || !settings) return <Loading />;
  if (providers.isError) return <ErrorState message={errorText(providers.error)} retry={() => void providers.refetch()} />;
  const list = providers.data ?? [];
  const store = t(info?.secret_store === "file" ? "file" : "keychain");

  const nameOf = (provider: ProviderInfo) => (provider.kind === "browser" ? t("browserMode") : provider.name);
  const toggle = (provider: ProviderInfo) => {
    const disabled = new Set(settings.disabled ?? []);
    if (disabled.has(provider.id)) disabled.delete(provider.id);
    else disabled.add(provider.id);
    saveSettings.mutate({ ...settings, disabled: [...disabled] });
  };
  const setBudget = (provider: ProviderInfo, value: string) => {
    const budgets = { ...(settings.budgets ?? {}) };
    const amount = Number(value.replace(",", "."));
    if (!value.trim() || !Number.isFinite(amount)) delete budgets[provider.id];
    else budgets[provider.id] = Math.max(0, amount);
    saveSettings.mutate({ ...settings, budgets });
  };
  const move = (engine: Engine, index: number, by: number) => {
    const chain = [...(settings.chains[engine] ?? [])];
    const [item] = chain.splice(index, 1);
    if (!item) return;
    chain.splice(Math.max(0, Math.min(chain.length, index + by)), 0, item);
    saveSettings.mutate({ ...settings, chains: { ...settings.chains, [engine]: chain } });
  };

  return (
    <div className="providers">
      <AccountPanel jobs={jobs} notify={notify} />
      <section className="oc-panel">
        <div className="oc-panel-head">
          <h2>{t("sourcesChain")}</h2>
        </div>
        <p className="panel-lead">{t("sourcesChainLead")}</p>
        <div className="chains">
          {engines.map((engine) => (
            <div key={engine} className="chain">
              <h3>{t(engine)}</h3>
              <ol>
                {(settings.chains[engine] ?? []).map((id, index, chain) => {
                  const provider = list.find((item) => item.id === id);
                  return (
                    <li key={id}>
                      <span className={`oc-dot ${provider?.ready && provider.enabled ? "good" : ""}`} />
                      <span className="chain-name">{provider ? nameOf(provider) : id}</span>
                      <Button small className="icon ghost" aria-label="↑" disabled={index === 0} onClick={() => move(engine, index, -1)}>
                        <ArrowUp size={13} />
                      </Button>
                      <Button small className="icon ghost" aria-label="↓" disabled={index === chain.length - 1} onClick={() => move(engine, index, 1)}>
                        <ArrowDown size={13} />
                      </Button>
                    </li>
                  );
                })}
              </ol>
            </div>
          ))}
        </div>
      </section>

      <div className="provider-cards">
        {list.map((provider) => (
          <section key={provider.id} className="oc-card provider-card">
            <div className="row between">
              <div>
                <h3>{nameOf(provider)}</h3>
                <small className="faint">
                  {provider.engines.map((engine) => t(engine)).join(" · ")}
                  {provider.per_request_usd > 0 ? ` · ${t("perRequest", { usd: usd(provider.per_request_usd, lang) })}` : ""}
                </small>
              </div>
              <span className={`oc-badge ${!provider.enabled ? "" : provider.via === "credit" ? "accent" : provider.ready ? "good" : "warning"}`}>
                <span className="oc-dot" />
                {!provider.enabled
                  ? t("disabled")
                  : provider.via === "credit"
                    ? t("viaCredit", { factor: number(provider.credit?.price_multiplier ?? 1, lang, 2) })
                    : provider.ready
                      ? t("ready")
                      : t("needsKey")}
              </span>
            </div>
            {provider.kind === "builtin" && CREDITABLE.includes(provider.id) && (
              <PayWith provider={provider} signedIn={cloud.data?.signed_in ?? false} notify={notify} />
            )}
            {provider.kind === "browser" && <p className="provider-note">{t("browserModeLead")}</p>}
            {provider.error && <div className="oc-banner error">{provider.error}</div>}
            {provider.secrets.map((secret) => (
              <div key={secret.name} className="secret-row">
                <KeyRound size={14} />
                <span className="secret-name">{secret.label ?? secret.name}</span>
                <span className="faint">
                  {secret.state === "saved"
                    ? store
                    : secret.state === "environment"
                      ? t("keyFromEnvironment")
                      : secret.state === "rebind"
                        ? t("keyRebind")
                        : ""}
                </span>
                <Button small onClick={() => setKeyFor({ provider, secret })} disabled={secret.state === "environment"}>
                  {secret.state === "unset" ? t("setKey") : t("replaceKey")}
                </Button>
                {secret.state === "saved" && (
                  <Button
                    small
                    className="icon ghost"
                    aria-label={t("remove")}
                    onClick={() => removeKey.mutate({ provider: provider.id, name: secret.name })}
                  >
                    <Trash2 size={14} />
                  </Button>
                )}
              </div>
            ))}
            <div className="provider-foot">
              {provider.per_request_usd > 0 && (
                <label className="oc-field budget">
                  <span>{t("budget")} (USD)</span>
                  <input
                    className="oc-input"
                    inputMode="decimal"
                    placeholder={t("noBudget")}
                    defaultValue={provider.budget_usd ?? ""}
                    onBlur={(event) => {
                      if (event.target.value !== String(provider.budget_usd ?? "")) setBudget(provider, event.target.value);
                    }}
                  />
                </label>
              )}
              <Button small className="ghost" onClick={() => toggle(provider)}>
                {provider.enabled ? t("disable") : t("enable")}
              </Button>
            </div>
          </section>
        ))}
      </div>

      <CustomProviders notify={notify} />
      <ParserOverrides notify={notify} />

      {keyFor && (
        <KeyDialog
          provider={keyFor.provider}
          secret={keyFor.secret}
          store={store}
          onClose={() => setKeyFor(null)}
          onSaved={() => {
            setKeyFor(null);
            notify(t("keySaved", { store }));
            void queryClient.invalidateQueries({ queryKey: ["providers"] });
            void queryClient.invalidateQueries({ queryKey: ["estimate"] });
          }}
          notify={notify}
        />
      )}
    </div>
  );
}

/** Asks for one secret. The value goes to the secret store and is never shown again. */
export function KeyDialog({
  provider,
  secret,
  store,
  onClose,
  onSaved,
  notify,
}: {
  provider: { id: string; name: string };
  secret: { name: string; label?: string };
  store: string;
  onClose: () => void;
  onSaved: () => void;
  notify: (text: string, error?: boolean) => void;
}) {
  const { t } = useI18n();
  const [value, setValue] = useState("");
  const save = useMutation({
    mutationFn: () => api.setSecret(provider.id, secret.name, value),
    onSuccess: onSaved,
    onError: (error) => notify(errorText(error), true),
  });
  return (
    <Modal
      title={`${provider.name} · ${secret.label ?? secret.name}`}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("cancel")}</Button>
          <Button primary busy={save.isPending} disabled={!value.trim()} onClick={() => save.mutate()}>
            {t("save")}
          </Button>
        </>
      }
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (value.trim()) save.mutate();
        }}
      >
        <label className="oc-field">
          <span>{secret.label ?? secret.name}</span>
          <input
            className="oc-input"
            type="password"
            autoComplete="off"
            spellCheck={false}
            value={value}
            onChange={(event) => setValue(event.target.value)}
          />
        </label>
        <p className="oc-field-hint">{t("pasteKey", { store })}</p>
      </form>
    </Modal>
  );
}
