import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { AppWindow, Coins, KeyRound, LogIn, LogOut, MonitorSmartphone, RotateCcw, ShieldAlert } from "lucide-react";
import { api, errorText } from "../api/client";
import type { Engine, Job, ProviderInfo, Settings, SourceMode } from "../api/types";
import { JobCard } from "../components/JobCard";
import { Button, Check, Field, Loading } from "../components/ui";
import { number, usd, useI18n, type Key } from "../i18n";

const KEYED = ["wordstat", "dataforseo", "serpapi", "serper", "yandex-cloud"];

function KeyEditor({ provider, signedIn, notify }: { provider: ProviderInfo; signedIn: boolean; notify: (text: string, error?: boolean) => void }) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const [values, setValues] = useState<Record<string, string>>({});
  const [budget, setBudget] = useState(provider.budget_usd === null ? "" : String(provider.budget_usd));
  useEffect(() => setBudget(provider.budget_usd === null ? "" : String(provider.budget_usd)), [provider.budget_usd]);
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["providers"] });
  const save = useMutation({
    mutationFn: async (name: string) => api.setKey(provider.id, name, values[name] ?? ""),
    onSuccess: (_, name) => {
      setValues((old) => ({ ...old, [name]: "" }));
      notify(t("keySaved"));
      refresh();
    },
    onError: (error) => notify(errorText(error), true),
  });
  const remove = useMutation({
    mutationFn: (name: string) => api.removeKey(provider.id, name),
    onSuccess: refresh,
    onError: (error) => notify(errorText(error), true),
  });
  const saveBudget = useMutation({
    mutationFn: () => api.setBudget(provider.id, budget.trim() === "" ? null : Number(budget)),
    onSuccess: () => {
      notify(t("saved"));
      refresh();
    },
    onError: (error) => notify(errorText(error), true),
  });
  const toggle = useMutation({
    mutationFn: (enabled: boolean) => api.setEnabled(provider.id, enabled),
    onSuccess: refresh,
    onError: (error) => notify(errorText(error), true),
  });
  const mode = useMutation({
    mutationFn: (next: SourceMode) => api.setSourceMode(provider.id, next),
    onSuccess: refresh,
    onError: (error) => notify(errorText(error), true),
  });
  const factor = provider.credit ? number(provider.credit.price_multiplier, lang, 2) : "";
  return (
    <article className="oc-card provider-card">
      <header className="row between">
        <h3 className="oc-card-title">{provider.name}</h3>
        <span className={`oc-badge ${provider.via === "credit" ? "accent" : provider.ready ? "good" : ""}`}>
          {provider.via === "credit" ? t("viaCredit", { factor }) : provider.ready ? t("ready") : t("notSetUp")}
        </span>
      </header>
      <p className="faint small">{provider.offers.map((offer) => t(`offer.${offer}` as Key)).join(" · ")}</p>
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
      {provider.secrets.map((secret) => (
        <div key={secret.name} className="key-row">
          <Field label={`${t(`secret.${secret.name}` as Key)} · ${t(`secretState.${secret.state}` as Key)}`}>
            <div className="row">
              <input
                className="oc-input"
                type="password"
                autoComplete="off"
                value={values[secret.name] ?? ""}
                onChange={(e) => setValues((old) => ({ ...old, [secret.name]: e.target.value }))}
                placeholder={secret.state === "saved" ? "••••••••" : t(`secretPlaceholder.${secret.name}` as Key)}
              />
              <Button small disabled={!(values[secret.name] ?? "").trim()} busy={save.isPending} onClick={() => save.mutate(secret.name)}>
                {t("save")}
              </Button>
              {secret.state === "saved" && (
                <Button small className="ghost" onClick={() => remove.mutate(secret.name)}>
                  {t("remove")}
                </Button>
              )}
            </div>
          </Field>
        </div>
      ))}
      <div className="row wrap">
        <Field label={t("monthlyBudget")}>
          <div className="row">
            <input className="oc-input narrow" type="number" min={0} step={0.5} value={budget} onChange={(e) => setBudget(e.target.value)} placeholder={t("noBudget")} />
            <Button small onClick={() => saveBudget.mutate()}>
              {t("save")}
            </Button>
          </div>
        </Field>
        <Check checked={provider.enabled} onChange={(enabled) => toggle.mutate(enabled)}>
          {t("enabled")}
        </Check>
      </div>
      <footer className="oc-card-foot row between small faint">
        <span>{t("spentThisMonth", { usd: usd(provider.spent_usd, "en") })}</span>
        {provider.signup && (
          <a href={provider.signup} target="_blank" rel="noreferrer">
            {t("getKey")}
          </a>
        )}
      </footer>
    </article>
  );
}

/** The OctopusOn SEO account: connect it, see the balance, buy credits. */
function AccountPanel({ jobs, notify }: { jobs: Job[]; notify: (text: string, error?: boolean) => void }) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  // Another OctopusOn SEO program may connect or disconnect the shared account meanwhile.
  const status = useQuery({ queryKey: ["cloud"], queryFn: api.cloudStatus, refetchInterval: 30_000, refetchOnWindowFocus: true });
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["cloud"] });
    void queryClient.invalidateQueries({ queryKey: ["providers"] });
  };
  const login = useMutation({ mutationFn: api.accountLogin, onError: (error) => notify(errorText(error), true) });
  const logout = useMutation({ mutationFn: api.accountLogout, onSuccess: refresh, onError: (error) => notify(errorText(error), true) });
  const open = useMutation({
    mutationFn: (page: "credits" | "devices") => api.openAccountPage(page),
    onError: (error) => notify(errorText(error), true),
  });
  const loginJob = jobs.find((job) => job.kind === "account-login");
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
      <h2 className="oc-panel-title">
        <Coins size={16} />
        {t("accountTitle")}
      </h2>
      <p className="faint small">{t("accountLead")}</p>
      {loginJob && loginJob.status !== "done" && <JobCard job={loginJob} onStop={(id) => void api.cancel(id)} />}
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
            <b>{t("googleNotConnected")}</b>
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

function GooglePanel({ jobs, notify }: { jobs: Job[]; notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const status = useQuery({ queryKey: ["google"], queryFn: api.googleStatus });
  const [clientId, setClientId] = useState("");
  const [secret, setSecret] = useState("");
  const [customer, setCustomer] = useState("");
  const [manager, setManager] = useState("");
  const [ads, setAds] = useState(false);
  useEffect(() => {
    if (status.data?.client_id) setClientId(status.data.client_id);
    if (status.data?.ads_customer_id) setCustomer(status.data.ads_customer_id);
  }, [status.data?.client_id, status.data?.ads_customer_id]);
  const refresh = () => void queryClient.invalidateQueries({ queryKey: ["google"] });
  const saveClient = useMutation({
    mutationFn: () => api.setGoogleClient(clientId.trim(), secret.trim()),
    onSuccess: () => {
      setSecret("");
      notify(t("saved"));
      refresh();
    },
    onError: (error) => notify(errorText(error), true),
  });
  const login = useMutation({
    mutationFn: () => api.googleLogin({ search_console: true, ads }),
    onError: (error) => notify(errorText(error), true),
  });
  const logout = useMutation({
    mutationFn: api.googleLogout,
    onSuccess: refresh,
    onError: (error) => notify(errorText(error), true),
  });
  const saveAds = useMutation({
    mutationFn: () => api.setAdsAccount(customer.trim() || null, manager.trim() || null),
    onSuccess: () => {
      notify(t("saved"));
      refresh();
    },
    onError: (error) => notify(errorText(error), true),
  });
  const loginJob = jobs.find((job) => job.kind === "google-login");
  useEffect(() => {
    if (loginJob && loginJob.status !== "running") refresh();
    // Refresh once the sign-in ends.
  }, [loginJob?.status]);
  const google = status.data;
  return (
    <section className="oc-panel">
      <h2 className="oc-panel-title">{t("googleTitle")}</h2>
      <p className="faint small">{t("googleLead")}</p>
      {loginJob && <JobCard job={loginJob} onStop={(id) => void api.cancel(id)} />}
      <div className="form-row">
        <Field label={t("clientId")} hint={t("clientIdHint")}>
          <input className="oc-input" value={clientId} onChange={(e) => setClientId(e.target.value)} placeholder="123-abc.apps.googleusercontent.com" />
        </Field>
        <Field label={`${t("clientSecret")} · ${google?.client_secret ? t("secretState.saved") : t("secretState.unset")}`}>
          <input className="oc-input" type="password" autoComplete="off" value={secret} onChange={(e) => setSecret(e.target.value)} />
        </Field>
      </div>
      <div className="row end">
        <Button disabled={!clientId.trim() || !secret.trim()} busy={saveClient.isPending} onClick={() => saveClient.mutate()}>
          <KeyRound size={14} />
          {t("saveClient")}
        </Button>
      </div>
      <div className="row between wrap">
        <div>
          <b>{google?.connected ? t("googleConnected") : t("googleNotConnected")}</b>
          {google?.connected && <p className="faint small">{google.scopes.map((scope) => (scope.includes("adwords") ? "Google Ads" : "Search Console")).join(" · ")}</p>}
        </div>
        <div className="row">
          <Check checked={ads} onChange={setAds}>
            {t("alsoAds")}
          </Check>
          <Button primary disabled={!google?.client_id || !google.client_secret} busy={login.isPending} onClick={() => login.mutate()}>
            <LogIn size={14} />
            {t("signIn")}
          </Button>
          {google?.connected && (
            <Button className="ghost" onClick={() => logout.mutate()}>
              <LogOut size={14} />
              {t("signOut")}
            </Button>
          )}
        </div>
      </div>
      <p className="faint small">{t("adsAccess")}</p>
      <div className="form-row four">
        <Field label={t("adsCustomer")}>
          <input className="oc-input" value={customer} onChange={(e) => setCustomer(e.target.value)} placeholder="123-456-7890" />
        </Field>
        <Field label={t("adsManager")}>
          <input className="oc-input" value={manager} onChange={(e) => setManager(e.target.value)} placeholder={t("optional")} />
        </Field>
      </div>
      <div className="row end">
        <Button busy={saveAds.isPending} onClick={() => saveAds.mutate()}>
          {t("save")}
        </Button>
      </div>
    </section>
  );
}

function FreeSuggest({ settings, notify }: { settings: Settings; notify: (text: string, error?: boolean) => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [accepted, setAccepted] = useState(settings.suggest.accepted_at !== null);
  const save = useMutation({
    mutationFn: (suggest: Settings["suggest"]) => api.saveSettings({ ...settings, suggest }),
    onSuccess: () => {
      notify(t("saved"));
      void queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
    onError: (error) => notify(errorText(error), true),
  });
  const suggest = settings.suggest;
  const change = (patch: Partial<Settings["suggest"]>) =>
    save.mutate({ ...suggest, accepted_at: suggest.accepted_at ?? (accepted ? new Date().toISOString().replace(/\.\d+Z$/, "Z") : null), ...patch });
  return (
    <section className="oc-panel">
      <h2 className="oc-panel-title">
        <ShieldAlert size={16} />
        {t("freeTitle")}
      </h2>
      <div className="oc-banner warning">{t("freeSuggestTerms")}</div>
      <Check checked={accepted} disabled={suggest.accepted_at !== null} onChange={setAccepted}>
        {t("acceptTerms")}
      </Check>
      <div className="row wrap">
        <Check checked={suggest.google} disabled={!accepted} onChange={(google) => change({ google })}>
          {t("source.google-suggest")}
        </Check>
        <Check checked={suggest.yandex} disabled={!accepted} onChange={(yandex) => change({ yandex })}>
          {t("source.yandex-suggest")}
        </Check>
      </div>
      <p className="faint small">{t("freePace", { seconds: suggest.pause_ms / 1000, limit: suggest.daily_limit })}</p>
    </section>
  );
}

const ENGINES: Engine[] = ["google", "yandex", "bing"];

/** The attended browser mode: its warning, where it is used and today's pages. */
function BrowserPanel({ settings, provider, notify }: { settings: Settings; provider: ProviderInfo | undefined; notify: (text: string, error?: boolean) => void }) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const status = useQuery({ queryKey: ["browser"], queryFn: api.browserStatus });
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["browser"] });
    void queryClient.invalidateQueries({ queryKey: ["settings"] });
    void queryClient.invalidateQueries({ queryKey: ["providers"] });
  };
  const chain = useMutation({
    mutationFn: ({ engine, on }: { engine: Engine; on: boolean }) => {
      const rest = (settings.serp.chains[engine] ?? []).filter((id) => id !== "browser");
      const chains = { ...settings.serp.chains, [engine]: on ? [...rest, "browser"] : rest };
      return api.saveSettings({ ...settings, serp: { ...settings.serp, chains } });
    },
    onSuccess: () => {
      notify(t("saved"));
      refresh();
    },
    onError: (error) => notify(errorText(error), true),
  });
  const toggle = useMutation({
    mutationFn: (enabled: boolean) => api.setEnabled("browser", enabled),
    onSuccess: refresh,
    onError: (error) => notify(errorText(error), true),
  });
  const reset = useMutation({
    mutationFn: api.resetBrowserProfile,
    onSuccess: () => notify(t("browserResetDone")),
    onError: (error) => notify(errorText(error), true),
  });
  const browser = status.data;
  return (
    <section className="oc-panel browser-panel">
      <h2 className="oc-panel-title">
        <AppWindow size={16} />
        {t("browserTitle")}
      </h2>
      <div className="oc-banner warning">
        <ShieldAlert size={18} />
        <div>
          <p>{t("browserWarning")}</p>
          <p>{t("browserGoogleSites")}</p>
        </div>
      </div>
      {browser && (
        <p className={`small ${browser.executable ? "faint mono" : "warning-text"}`}>
          {browser.executable ? t("browserFound", { path: browser.executable }) : t("browserMissing")}
        </p>
      )}
      <div className="browser-engines" role="group" aria-label={t("browserChains")}>
        <span className="small">{t("browserChains")}</span>
        <div className="row wrap">
          {ENGINES.map((engine) => (
            <Check key={engine} checked={browser?.engines.includes(engine) ?? false} disabled={!browser || chain.isPending} onChange={(on) => chain.mutate({ engine, on })}>
              {t(engine)}
              <small className="faint"> · {t("browserToday", { used: number(browser?.used_today[engine] ?? 0, lang, 0), cap: number(settings.browser.daily_cap, lang, 0) })}</small>
            </Check>
          ))}
        </div>
        <small className="faint">{t("browserChainsHint")}</small>
      </div>
      <p className="faint small">{t("browserPace", { seconds: settings.browser.delay_secs, cap: number(settings.browser.daily_cap, lang, 0) })}</p>
      <div className="row between wrap">
        <Check checked={provider?.enabled ?? true} onChange={(enabled) => toggle.mutate(enabled)}>
          {t("enabled")}
        </Check>
        <Button small className="ghost" busy={reset.isPending} onClick={() => reset.mutate()} title={t("browserResetHint")}>
          <RotateCcw size={14} />
          {t("browserReset")}
        </Button>
      </div>
    </section>
  );
}

export function Sources({ settings, jobs, notify }: { settings: Settings | undefined; jobs: Job[]; notify: (text: string, error?: boolean) => void }) {
  const { t, lang } = useI18n();
  const providers = useQuery({ queryKey: ["providers"], queryFn: api.providers });
  const usage = useQuery({ queryKey: ["usage"], queryFn: api.usage });
  const cloud = useQuery({ queryKey: ["cloud"], queryFn: api.cloudStatus });
  if (providers.isLoading || !settings) return <Loading />;
  const credit = (usage.data ?? []).some((row) => row.credit_usd > 0);
  return (
    <div className="stack">
      <AccountPanel jobs={jobs} notify={notify} />
      <div className="oc-cards provider-cards">
        {(providers.data ?? [])
          .filter((provider) => KEYED.includes(provider.id))
          .map((provider) => (
            <KeyEditor key={provider.id} provider={provider} signedIn={cloud.data?.signed_in ?? false} notify={notify} />
          ))}
      </div>
      <GooglePanel jobs={jobs} notify={notify} />
      <BrowserPanel settings={settings} provider={providers.data?.find((provider) => provider.id === "browser")} notify={notify} />
      <FreeSuggest settings={settings} notify={notify} />
      {(usage.data?.length ?? 0) > 0 && (
        <section className="oc-panel">
          <h2 className="oc-panel-title">{t("usage")}</h2>
          <div className="oc-table-wrap">
            <table className="oc-table">
              <thead>
                <tr>
                  <th>{t("month")}</th>
                  <th>{t("provider")}</th>
                  <th className="num">{t("requests")}</th>
                  <th className="num">{t("items")}</th>
                  <th className="num">{t("failures")}</th>
                  <th className="num">{t("spent")}</th>
                  {credit && <th className="num">{t("onCredits")}</th>}
                </tr>
              </thead>
              <tbody>
                {(usage.data ?? []).map((row) => (
                  <tr key={`${row.month}-${row.provider}`}>
                    <td>{row.month}</td>
                    <td>{row.provider}</td>
                    <td className="num">{number(row.requests, lang, 0)}</td>
                    <td className="num">{number(row.items, lang, 0)}</td>
                    <td className="num">{number(row.failures, lang, 0)}</td>
                    <td className="num">{usd(row.cost_usd, lang)}</td>
                    {credit && <td className="num">{usd(row.credit_usd, lang)}</td>}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      )}
    </div>
  );
}
