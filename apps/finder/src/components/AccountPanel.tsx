import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Coins, LogIn, LogOut, MonitorSmartphone, X } from "lucide-react";
import { api, errorText, isDemo } from "../api/client";
import type { ApiConfig, CreditSource, Lang, SourceMode } from "../api/types";
import { useI18n, type Key } from "../i18n";

const CLOUD = ["finder-cloud"];
const LOGIN = ["finder-login"];
const SOURCES = "finder-credit-sources";

type Notify = (text: string, error?: boolean) => void;

function decimal(value: number, lang: Lang, digits = 2): string {
  return new Intl.NumberFormat(lang === "tr" ? "tr-TR" : "en-US", {
    maximumFractionDigits: digits,
  }).format(value);
}

/** The OctopusOn SEO account: connect it, see the balance, buy credits. */
export function AccountCard({ notify }: { notify: Notify }) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  // Another OctopusOn SEO program may connect or disconnect the shared account meanwhile.
  const status = useQuery({
    queryKey: CLOUD,
    queryFn: api.cloudStatus,
    refetchInterval: 30_000,
    refetchOnWindowFocus: true,
  });
  const login = useQuery({
    queryKey: LOGIN,
    queryFn: api.accountLoginState,
    refetchInterval: (query) =>
      query.state.data?.status === "waiting" ? 1000 : false,
  });

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void api
      .watchAccount((next) => {
        queryClient.setQueryData(LOGIN, next);
        if (next.status === "approved") notify(t("accountConnected"));
        else if (next.status === "failed" && next.error) notify(next.error, true);
        void queryClient.invalidateQueries({ queryKey: CLOUD });
        void queryClient.invalidateQueries({ queryKey: [SOURCES] });
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [queryClient, notify, t]);

  const cloud = status.data;
  const account = cloud?.account;
  useEffect(() => {
    // A connection made or ended elsewhere changes which sources run on credits.
    void queryClient.invalidateQueries({ queryKey: [SOURCES] });
  }, [queryClient, cloud?.signed_in]);

  const start = useMutation({
    mutationFn: api.accountLogin,
    onSuccess: (next) => queryClient.setQueryData(LOGIN, next),
    onError: (error) => notify(errorText(error), true),
  });
  const cancel = useMutation({
    mutationFn: api.cancelAccountLogin,
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: LOGIN }),
  });
  const logout = useMutation({
    mutationFn: api.accountLogout,
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: CLOUD });
      void queryClient.invalidateQueries({ queryKey: [SOURCES] });
    },
    onError: (error) => notify(errorText(error), true),
  });
  const open = useMutation({
    mutationFn: (page: "credits" | "devices" | "usage") =>
      api.openAccountPage(page),
    onError: (error) => notify(errorText(error), true),
  });
  const waiting = login.data?.status === "waiting" ? login.data : null;

  return (
    <section className="oc-panel account-card">
      <div className="row between" style={{ marginBottom: 10 }}>
        <h2 className="type-card-title">
          <Coins size={18} style={{ verticalAlign: "middle", marginRight: 8 }} />
          {t("accountTitle")}
        </h2>
        <span className={`oc-badge ${cloud?.signed_in ? "good" : ""}`}>
          <span className="oc-dot" />
          {cloud?.signed_in
            ? (account?.email ?? t("accountTitle"))
            : t("accountNotConnected")}
        </span>
      </div>
      <p className="muted account-lead">{t("accountLead")}</p>
      {cloud?.error && <div className="oc-banner warning">{cloud.error}</div>}
      {account?.low_balance && (
        <div className="oc-banner warning">{t("accountLow")}</div>
      )}
      {waiting && <div className="oc-banner info">{t("accountWaiting")}</div>}
      <div className="row between account-foot">
        <div>
          {account ? (
            <p className="account-balance">
              {decimal(
                account.available_micros / Math.max(account.credit_unit_micros, 1),
                lang,
              )}{" "}
              <span className="muted">{t("creditsAvailable")}</span>
            </p>
          ) : (
            <small className="faint">
              {isDemo ? t("accountPreview") : t("accountShared")}
            </small>
          )}
        </div>
        <div className="row">
          {cloud?.signed_in ? (
            <>
              <button
                className="oc-btn primary"
                disabled={open.isPending}
                onClick={() => open.mutate("credits")}
              >
                <Coins size={14} />
                {t("buyCredits")}
              </button>
              <button className="oc-btn" onClick={() => open.mutate("devices")}>
                <MonitorSmartphone size={14} />
                {t("accountDevices")}
              </button>
              <button
                className="oc-btn"
                disabled={logout.isPending}
                onClick={() => logout.mutate()}
              >
                <LogOut size={14} />
                {t("accountSignOut")}
              </button>
            </>
          ) : waiting ? (
            <button
              className="oc-btn"
              disabled={cancel.isPending}
              onClick={() => cancel.mutate()}
            >
              <X size={14} />
              {t("accountCancel")}
            </button>
          ) : (
            <button
              className="oc-btn primary"
              disabled={start.isPending}
              onClick={() => start.mutate()}
            >
              <LogIn size={14} />
              {t("accountConnect")}
            </button>
          )}
        </div>
      </div>
    </section>
  );
}

const MODES: { mode: SourceMode; label: Key }[] = [
  { mode: "auto", label: "modeAuto" },
  { mode: "own", label: "modeOwn" },
  { mode: "credit", label: "modeCredit" },
];

function PaymentRow({
  source,
  signedIn,
  notify,
}: {
  source: CreditSource;
  signedIn: boolean;
  notify: Notify;
}) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const mode = useMutation({
    mutationFn: (next: SourceMode) => api.setSourceMode(source.id, next),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: [SOURCES] }),
    onError: (error) => notify(errorText(error), true),
  });
  const badge =
    source.via === "credit"
      ? {
          className: "accent",
          text: `${t("viaCredit")} ×${decimal(source.credit?.price_multiplier ?? 1, lang)}`,
        }
      : source.via === "own"
        ? { className: "good", text: t("viaOwn") }
        : { className: "", text: t("notRunning") };
  const note = !source.creditable
    ? t("ownKeyOnly")
    : source.credit
      ? null
      : signedIn
        ? t("creditUnavailable")
        : t("creditSignIn");

  return (
    <div className="payment-row">
      <div className="payment-name">
        <b>{source.name}</b>
        {note && <small className="faint">{note}</small>}
      </div>
      {source.creditable && (
        <div
          className="oc-segmented"
          role="radiogroup"
          aria-label={`${t("payWith")}: ${source.name}`}
        >
          {MODES.map((option) => (
            <button
              key={option.mode}
              type="button"
              role="radio"
              aria-checked={source.mode === option.mode}
              className={source.mode === option.mode ? "active" : undefined}
              disabled={
                mode.isPending ||
                (option.mode === "credit" &&
                  !source.credit &&
                  source.mode !== "credit")
              }
              onClick={() => mode.mutate(option.mode)}
            >
              {t(option.label)}
            </button>
          ))}
        </div>
      )}
      <span className={`oc-badge ${badge.className}`}>
        <span className="oc-dot" />
        {badge.text}
      </span>
    </div>
  );
}

/** How each paid search API is paid in the next scan: own key, credits or automatic. */
export function PaymentModes({
  config,
  notify,
}: {
  config: ApiConfig;
  notify: Notify;
}) {
  const { t } = useI18n();
  const cloud = useQuery({ queryKey: CLOUD, queryFn: api.cloudStatus });
  const signedIn = cloud.data?.signed_in ?? false;
  // Only whether each key is filled in matters, so the keys stay out of the cache key.
  const keys = [
    config.serper_api_key.trim() !== "",
    config.serpapi_key.trim() !== "",
    config.google_api_key.trim() !== "" && config.google_cx.trim() !== "",
    config.yandex_api_key.trim() !== "",
  ];
  const sources = useQuery({
    queryKey: [SOURCES, signedIn, ...keys],
    queryFn: () => api.creditSources(config),
  });

  return (
    <div className="payment-modes">
      <h3 className="type-card-title">{t("paymentTitle")}</h3>
      <p className="muted">{t("paymentLead")}</p>
      {sources.isError && (
        <div className="oc-banner error">{errorText(sources.error)}</div>
      )}
      {(sources.data ?? []).map((source) => (
        <PaymentRow
          key={source.id}
          source={source}
          signedIn={signedIn}
          notify={notify}
        />
      ))}
    </div>
  );
}
