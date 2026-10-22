import { useState, type FormEvent } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, CheckCircle2, ChevronDown, CircleAlert, Coins, ExternalLink, KeyRound, LogOut, PlugZap, Wallet } from "lucide-react";
import { Button, ErrorState, Field, Loading, Segmented } from "@octopusseo/ui";
import { api } from "../../api/client";
import { openExternal } from "../../api/transport";
import type { Job, ProviderInfo, SourceMode } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { explain } from "../../lib/errors";
import { creditsOf, formatCredits, formatNumber, formatUsd } from "../../lib/format";

function useOpen() {
  const { fail } = useShell();
  return (opening: Promise<{ url: string; opened: boolean }>) =>
    opening.then((opened) => openExternal(opened.url, opened.opened)).catch((thrown) => fail(thrown));
}

function AccountCard({ jobs }: { jobs: Job[] }) {
  const { t, locale } = useI18n();
  const { mode, notify, fail } = useShell();
  const queryClient = useQueryClient();
  const open = useOpen();
  const status = useQuery({ queryKey: ["account"], queryFn: api.account, refetchInterval: 30_000, refetchOnWindowFocus: true });
  const connecting = jobs.some((job) => job.kind === "account-login" && job.status === "running");
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["account"] });
    void queryClient.invalidateQueries({ queryKey: ["providers"] });
  };
  const connect = useMutation({
    mutationFn: api.connect,
    onSuccess: () => {
      notify(t("sources.connecting"));
      void queryClient.invalidateQueries({ queryKey: ["jobs"] });
    },
    onError: (thrown) => fail(thrown),
  });
  const disconnect = useMutation({ mutationFn: api.disconnect, onSuccess: refresh, onError: (thrown) => fail(thrown) });
  const account = status.data?.account ?? null;
  const web = status.data?.host === "web" || mode === "web";
  return (
    <section className="oc-card account-card choice recommended">
      <header className="card-head">
        <span className="oc-tile amber large">
          <Wallet size={20} />
        </span>
        <div>
          <span className="oc-badge good">{t("sources.recommended")}</span>
          <h2 className="oc-card-title">{t("sources.accountTitle")}</h2>
          <p className="faint">{t("sources.accountLead")}</p>
        </div>
      </header>
      <ul className="choice-points">
        {(["accountPoint1", "accountPoint2", "accountPoint3"] as const).map((point) => (
          <li key={point}>
            <Check size={15} aria-hidden="true" />
            {t(`sources.${point}`)}
          </li>
        ))}
      </ul>
      {status.isLoading && <Loading label={t("common.loading")} />}
      {status.data && (
        <div className="account-body">
          <div className="account-who">
            {account ? <CheckCircle2 size={16} className="good" aria-hidden="true" /> : <CircleAlert size={16} aria-hidden="true" />}
            <span>{account ? t("sources.signedInAs", { email: account.email ?? "" }) : status.data.signed_in ? status.data.error : t("sources.notSignedIn")}</span>
            {web && account && <span className="faint">{t("sources.webAccount")}</span>}
          </div>
          {account && (
            <div className="account-balance">
              <span className="faint">{t("sources.balance")}</span>
              <strong>{t("common.credits", { n: formatCredits(creditsOf(account.available_micros, account.credit_unit_micros), locale) })}</strong>
              {account.low_balance && <span className="oc-badge warning">{t("sources.lowBalance")}</span>}
            </div>
          )}
          <div className="account-actions">
            {!account && !web && (
              <Button primary busy={connect.isPending || connecting} onClick={() => connect.mutate()}>
                <PlugZap size={16} />
                {connecting ? t("sources.connecting") : t("sources.connect")}
              </Button>
            )}
            {account && (
              <>
                <Button primary onClick={() => open(api.accountPage("credits"))}>
                  <Coins size={16} />
                  {t("sources.buy")}
                </Button>
                <Button onClick={() => open(api.accountPage("usage"))}>
                  <ExternalLink size={15} />
                  {t("sources.usagePage")}
                </Button>
                {!web && (
                  <Button busy={disconnect.isPending} onClick={() => disconnect.mutate()}>
                    <LogOut size={15} />
                    {t("sources.disconnect")}
                  </Button>
                )}
              </>
            )}
          </div>
        </div>
      )}
    </section>
  );
}

function keyState(state: string, t: (key: string) => string): string {
  switch (state) {
    case "saved":
      return t("sources.keySaved");
    case "rebind":
      return t("sources.keyRebind");
    case "environment":
      return t("sources.keyEnvironment");
    default:
      return t("sources.keyUnset");
  }
}

function ProviderCard({ provider, connected }: { provider: ProviderInfo; connected: boolean }) {
  const { t, locale } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const open = useOpen();
  const [value, setValue] = useState("");
  const [budget, setBudget] = useState(provider.budget_usd === null ? "" : String(provider.budget_usd));
  const refresh = () => {
    void queryClient.invalidateQueries({ queryKey: ["providers"] });
    void queryClient.invalidateQueries({ queryKey: ["overview-estimate"] });
  };
  const failed = (thrown: unknown) => fail(thrown);
  const key = provider.keys[0];
  const saveKey = useMutation({
    mutationFn: () => api.setKey(provider.id, key?.name ?? "", value),
    onSuccess: () => {
      setValue("");
      notify(t("sources.keySavedToast"));
      refresh();
    },
    onError: failed,
  });
  const removeKey = useMutation({
    mutationFn: () => api.removeKey(provider.id, key?.name ?? ""),
    onSuccess: () => {
      notify(t("sources.keyRemovedToast"));
      refresh();
    },
    onError: failed,
  });
  const setMode = useMutation({ mutationFn: (mode: SourceMode) => api.setMode(provider.id, mode), onSuccess: refresh, onError: failed });
  const saveBudget = useMutation({
    mutationFn: () => api.setBudget(provider.id, budget.trim() === "" ? null : Number(budget.replace(",", "."))),
    onSuccess: () => {
      notify(t("sources.budgetSaved"));
      refresh();
    },
    onError: failed,
  });
  const submitKey = (event: FormEvent) => {
    event.preventDefault();
    if (value.trim()) saveKey.mutate();
  };
  // An AI assistant's own key: no credits, and without it AI visibility asks through DataForSEO.
  const assistant = provider.modules.length === 1 && provider.modules[0] === "ai";
  const status =
    provider.via === "own" ? t("sources.viaOwn") : provider.via === "credit" ? t("sources.viaCredit") : assistant ? t("sources.viaNoneAssistant") : t("sources.viaNone");
  // Credits already cover this source: the own key and the payment choice are an option, folded away.
  const [ownOpen, setOwnOpen] = useState(!(connected && provider.via === "credit"));
  return (
    <article className="oc-card provider-card">
      <header className="card-head">
        <span className="oc-tile blue">
          <KeyRound size={16} />
        </span>
        <div>
          <h3 className="oc-card-title">{provider.name}</h3>
          <p className={provider.via ? "good" : "warn"}>{status}</p>
        </div>
        <span className="provider-spent">{t("sources.spentMonth", { amount: formatUsd(provider.spent_usd, locale) })}</span>
      </header>
      <div className="provider-body">
        {!ownOpen ? (
          <button type="button" className="link-button" onClick={() => setOwnOpen(true)}>
            <ChevronDown size={14} aria-hidden="true" />
            {t("sources.useOwnKey")}
          </button>
        ) : (
          <>
          {!assistant && (
            <Field label={t("sources.mode")} hint={t("sources.modeHint")}>
              <Segmented<SourceMode>
                label={t("sources.mode")}
                value={provider.mode}
                onChange={(mode) => setMode.mutate(mode)}
                disabled={setMode.isPending}
                options={[
                  { value: "auto", label: t("sources.modeAuto") },
                  { value: "own", label: t("sources.modeOwn") },
                  { value: "credit", label: t("sources.modeCredit"), disabled: !provider.credit_connector && provider.mode !== "credit" },
                ]}
              />
            </Field>
          )}
          <p className="faint small">
            {provider.credits_per_usd !== null
              ? t("sources.creditOffer", { credits: formatNumber(provider.credits_per_usd, locale) })
              : assistant
                ? t("sources.assistantKey")
                : t("sources.noCreditOffer")}
          </p>
          {key && (
            <form className="key-form" onSubmit={submitKey}>
              <Field label={provider.id === "dataforseo" ? t("sources.keyDataforseo") : t("sources.key")} hint={keyState(key.state, t)}>
                <input
                  className="oc-input"
                  type="password"
                  value={value}
                  onChange={(event) => setValue(event.target.value)}
                  autoComplete="off"
                  spellCheck={false}
                  placeholder={key.state === "saved" ? "••••••••" : provider.id === "dataforseo" ? "login:password" : undefined}
                />
              </Field>
              <div className="key-actions">
                <Button type="submit" primary small busy={saveKey.isPending} disabled={!value.trim()}>
                  {t("sources.saveKey")}
                </Button>
                {key.state === "saved" && (
                  <Button small busy={removeKey.isPending} onClick={() => removeKey.mutate()}>
                    {t("sources.removeKey")}
                  </Button>
                )}
                <Button small className="ghost" onClick={() => open(api.openUrl(provider.signup_url))}>
                  <ExternalLink size={14} />
                  {t("sources.getKey")}
                </Button>
              </div>
            </form>
          )}
          </>
        )}
        <form
          className="budget-form"
          onSubmit={(event) => {
            event.preventDefault();
            saveBudget.mutate();
          }}
        >
          <Field label={t("sources.budget")} hint={t("sources.budgetHint")}>
            <input className="oc-input" inputMode="decimal" value={budget} onChange={(event) => setBudget(event.target.value)} />
          </Field>
          <Button type="submit" small busy={saveBudget.isPending}>
            {t("common.save")}
          </Button>
        </form>
      </div>
    </article>
  );
}

function Usage() {
  const { t, locale } = useI18n();
  const usage = useQuery({ queryKey: ["usage"], queryFn: api.usage });
  return (
    <section className="oc-panel">
      <header className="oc-panel-head">
        <h3 className="oc-panel-title">{t("sources.usageTitle")}</h3>
      </header>
      {usage.data && usage.data.length === 0 && <p className="faint">{t("sources.noUsage")}</p>}
      {usage.data && usage.data.length > 0 && (
        <div className="oc-table-wrap">
          <table className="oc-table">
            <thead>
              <tr>
                <th>{t("sources.month")}</th>
                <th>{t("sources.provider")}</th>
                <th>{t("sources.payment")}</th>
                <th className="number">{t("sources.requests")}</th>
                <th className="number">{t("sources.cost")}</th>
              </tr>
            </thead>
            <tbody>
              {usage.data.map((row) => (
                <tr key={`${row.month}-${row.provider}-${row.via}`}>
                  <td>{row.month}</td>
                  <td>{row.provider}</td>
                  <td>{row.via === "credit" ? t("common.viaCredit") : t("common.viaOwn")}</td>
                  <td className="number">{formatNumber(row.requests, locale)}</td>
                  <td className="number">{formatUsd(row.cost_usd, locale)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}

/** The second way: the user's own keys, optional; explains that one way is enough. */
function OwnKeysCard({ connected, onShow }: { connected: boolean; onShow: () => void }) {
  const { t } = useI18n();
  return (
    <section className="oc-card choice">
      <header className="card-head">
        <span className="oc-tile blue large">
          <KeyRound size={20} />
        </span>
        <div>
          <span className="oc-badge">{t("sources.optional")}</span>
          <h2 className="oc-card-title">{t("sources.ownTitle")}</h2>
          <p className="faint">{t("sources.ownLead")}</p>
        </div>
      </header>
      <ul className="choice-points">
        {(["ownPoint1", "ownPoint2", "ownPoint3"] as const).map((point) => (
          <li key={point}>
            <Check size={15} aria-hidden="true" />
            {t(`sources.${point}`)}
          </li>
        ))}
      </ul>
      <div className="account-actions">
        <Button onClick={onShow}>
          <KeyRound size={15} />
          {connected ? t("sources.ownShowConnected") : t("sources.ownShow")}
        </Button>
      </div>
    </section>
  );
}

/** Whether answers the gateway keeps for every account may come at a discount (ADR 0079). */
function SharedCacheOption() {
  const { t } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const settings = useQuery({ queryKey: ["settings"], queryFn: api.settings });
  const save = useMutation({
    mutationFn: (freshOnly: boolean) => api.saveSettings({ ...settings.data!, fresh_only: freshOnly }),
    onSuccess: () => {
      notify(t("sources.sharedCacheSaved"));
      void queryClient.invalidateQueries({ queryKey: ["settings"] });
    },
    onError: (thrown) => fail(thrown),
  });
  if (!settings.data) return null;
  return (
    <section className="oc-card shared-cache">
      <label className="oc-check">
        <input type="checkbox" checked={!settings.data.fresh_only} disabled={save.isPending} onChange={(event) => save.mutate(!event.target.checked)} />
        <span>
          <strong>{t("sources.sharedCache")}</strong>
          <span className="faint small">{t("sources.sharedCacheHint")}</span>
        </span>
      </label>
    </section>
  );
}

export function Sources({ jobs }: { jobs: Job[] }) {
  const { t } = useI18n();
  const providers = useQuery({ queryKey: ["providers"], queryFn: api.providers });
  const status = useQuery({ queryKey: ["account"], queryFn: api.account, refetchInterval: 30_000 });
  const connected = Boolean(status.data?.account);
  // With the account connected, the own keys are an extra: folded until asked for.
  const [keysOpen, setKeysOpen] = useState<boolean | null>(null);
  const showKeys = keysOpen ?? !connected;
  const openKeys = () => {
    setKeysOpen(true);
    window.requestAnimationFrame(() => document.getElementById("own-keys")?.scrollIntoView({ behavior: "smooth", block: "start" }));
  };
  return (
    <div className="page-stack">
      <p className="choice-lead">{t("sources.choiceLead")}</p>
      <div className="choice-grid">
        <AccountCard jobs={jobs} />
        <OwnKeysCard connected={connected} onShow={openKeys} />
      </div>
      {connected && <SharedCacheOption />}
      <section className="section" id="own-keys">
        <button type="button" className="section-head section-toggle" aria-expanded={showKeys} onClick={() => setKeysOpen(!showKeys)}>
          <h2>{t("sources.providersTitle")}</h2>
          <span className="faint small">{connected ? t("sources.providersConnected") : t("sources.providersHint")}</span>
          <ChevronDown size={18} className={showKeys ? "turned" : undefined} aria-hidden="true" />
        </button>
        {showKeys && (
          <>
            {providers.isLoading && <Loading label={t("common.loading")} />}
            {providers.error && <ErrorState title={t("common.loadFailed")} message={explain(providers.error, t).message} retry={() => void providers.refetch()} retryLabel={t("common.retry")} />}
            <div className="provider-grid">
              {providers.data?.map((provider) => (
                <ProviderCard key={`${provider.id}-${provider.budget_usd ?? ""}`} provider={provider} connected={connected} />
              ))}
            </div>
          </>
        )}
      </section>
      <Usage />
      <p className="faint small privacy-note">{t("sources.privacy")}</p>
    </div>
  );
}
