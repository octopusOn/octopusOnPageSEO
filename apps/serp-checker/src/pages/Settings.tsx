import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Download, KeyRound, RotateCcw } from "lucide-react";
import { api, errorText } from "../api/client";
import type { AppInfo, Device, Settings as SettingsData } from "../api/types";
import { Button, Loading } from "../components/ui";
import { date, number, usd, useI18n } from "../i18n";
import { KeyDialog } from "./Providers";

export function Settings({
  settings,
  info,
  notify,
}: {
  settings?: SettingsData;
  info?: AppInfo;
  notify: (text: string, error?: boolean) => void;
}) {
  const { t, lang } = useI18n();
  const queryClient = useQueryClient();
  const [draft, setDraft] = useState<SettingsData | undefined>(settings);
  const [keyOpen, setKeyOpen] = useState(false);
  const [accepted, setAccepted] = useState(false);
  useEffect(() => setDraft(settings), [settings]);
  const usage = useQuery({ queryKey: ["usage"], queryFn: api.usage });
  const save = useMutation({
    mutationFn: api.saveSettings,
    onSuccess: () => {
      notify(t("saved"));
      void queryClient.invalidateQueries({ queryKey: ["settings"] });
      void queryClient.invalidateQueries({ queryKey: ["estimate"] });
    },
    onError: (error) => notify(errorText(error), true),
  });
  const reset = useMutation({
    mutationFn: api.resetBrowser,
    onSuccess: () => notify(t("resetProfileDone")),
    onError: (error) => notify(errorText(error), true),
  });
  const tranco = useMutation({
    mutationFn: () => api.updateTranco(accepted),
    onSuccess: (rows) => {
      notify(t("trancoImported", { rows: number(rows, lang, 0) }));
      void queryClient.invalidateQueries({ queryKey: ["info"] });
    },
    onError: (error) => notify(errorText(error), true),
  });
  if (!draft || !info) return <Loading />;
  const update = (change: (old: SettingsData) => SettingsData) => setDraft((old) => (old ? change(old) : old));
  const changed = JSON.stringify(draft) !== JSON.stringify(settings);
  const integer = (value: string, fallback: number) => {
    const parsed = Number.parseInt(value, 10);
    return Number.isFinite(parsed) ? parsed : fallback;
  };
  const store = t(info.secret_store === "file" ? "file" : "keychain");

  return (
    <div className="settings">
      <form
        className="settings-grid"
        onSubmit={(event) => {
          event.preventDefault();
          save.mutate(draft);
        }}
      >
        <section className="oc-panel">
          <div className="oc-panel-head">
            <h2>{t("defaults")}</h2>
          </div>
          <div className="field-row">
            <label className="oc-field">
              <span>{t("country")}</span>
              <input
                className="oc-input"
                maxLength={2}
                value={draft.defaults.country}
                onChange={(event) => update((old) => ({ ...old, defaults: { ...old.defaults, country: event.target.value.toLowerCase() } }))}
              />
            </label>
            <label className="oc-field">
              <span>{t("languageCode")}</span>
              <input
                className="oc-input"
                maxLength={2}
                value={draft.defaults.language}
                onChange={(event) => update((old) => ({ ...old, defaults: { ...old.defaults, language: event.target.value.toLowerCase() } }))}
              />
            </label>
            <label className="oc-field">
              <span>{t("device")}</span>
              <select
                className="oc-select"
                value={draft.defaults.device}
                onChange={(event) => update((old) => ({ ...old, defaults: { ...old.defaults, device: event.target.value as Device } }))}
              >
                <option value="desktop">{t("desktop")}</option>
                <option value="mobile">{t("mobile")}</option>
              </select>
            </label>
            <label className="oc-field">
              <span>{t("depth")}</span>
              <select
                className="oc-select"
                value={draft.defaults.depth}
                onChange={(event) => update((old) => ({ ...old, defaults: { ...old.defaults, depth: Number(event.target.value) } }))}
              >
                {[10, 20, 30, 50, 100].map((depth) => (
                  <option key={depth} value={depth}>
                    {depth}
                  </option>
                ))}
              </select>
            </label>
          </div>
        </section>

        <section className="oc-panel">
          <div className="oc-panel-head">
            <h2>{t("browserMode")}</h2>
          </div>
          <p className="panel-lead">{t("browserModeLead")}</p>
          <div className="field-row">
            <label className="oc-field">
              <span>{t("delaySecs")}</span>
              <input
                className="oc-input"
                type="number"
                min={10}
                value={draft.browser.delay_secs}
                onChange={(event) =>
                  update((old) => ({ ...old, browser: { ...old.browser, delay_secs: integer(event.target.value, old.browser.delay_secs) } }))
                }
              />
            </label>
            <label className="oc-field">
              <span>{t("dailyCap")}</span>
              <input
                className="oc-input"
                type="number"
                min={1}
                value={draft.browser.daily_cap}
                onChange={(event) =>
                  update((old) => ({ ...old, browser: { ...old.browser, daily_cap: integer(event.target.value, old.browser.daily_cap) } }))
                }
              />
            </label>
          </div>
          <label className="oc-field">
            <span>{t("browserPath")}</span>
            <input
              className="oc-input"
              value={draft.browser.executable ?? ""}
              onChange={(event) =>
                update((old) => {
                  const { executable: _old, ...browser } = old.browser;
                  return { ...old, browser: event.target.value.trim() ? { ...browser, executable: event.target.value } : browser };
                })
              }
            />
          </label>
          <Button small type="button" busy={reset.isPending} onClick={() => reset.mutate()}>
            <RotateCcw size={14} />
            {t("resetProfile")}
          </Button>
        </section>

        <section className="oc-panel">
          <div className="oc-panel-head">
            <h2>{t("network")}</h2>
          </div>
          <label className="oc-field">
            <span>{t("proxy")}</span>
            <input
              className="oc-input"
              placeholder="http://127.0.0.1:8080"
              value={draft.proxy ?? ""}
              onChange={(event) =>
                update((old) => {
                  const { proxy: _old, ...rest } = old;
                  return event.target.value.trim() ? { ...rest, proxy: event.target.value.trim() } : rest;
                })
              }
            />
          </label>
          <div className="field-row">
            <label className="oc-field">
              <span>{t("timeout")}</span>
              <input
                className="oc-input"
                type="number"
                min={5}
                max={600}
                value={draft.timeout_secs}
                onChange={(event) => update((old) => ({ ...old, timeout_secs: integer(event.target.value, old.timeout_secs) }))}
              />
            </label>
            <label className="oc-field">
              <span>{t("confirmAbove")}</span>
              <input
                className="oc-input"
                inputMode="decimal"
                value={draft.confirm_above_usd}
                onChange={(event) => {
                  const value = Number(event.target.value.replace(",", "."));
                  if (Number.isFinite(value)) update((old) => ({ ...old, confirm_above_usd: value }));
                }}
              />
            </label>
          </div>
        </section>

        <div className="settings-actions">
          <Button primary type="submit" disabled={!changed} busy={save.isPending}>
            {t("save")}
          </Button>
        </div>
      </form>

      <section className="oc-panel">
        <div className="oc-panel-head">
          <h2>{t("data")}</h2>
        </div>
        <dl className="facts">
          <dt>{t("dataDir")}</dt>
          <dd>
            <code className="oc-code">{info.data_dir}</code>
          </dd>
          <dt>{t("secretStore")}</dt>
          <dd>{store}</dd>
        </dl>
        <div className="data-source">
          <div>
            <h3>{t("openPageRank")}</h3>
            <p className="panel-lead">{t("openPageRankLead")}</p>
          </div>
          <Button small onClick={() => setKeyOpen(true)} disabled={info.open_pagerank === "environment"}>
            <KeyRound size={14} />
            {info.open_pagerank === "unset" ? t("setKey") : t("replaceKey")}
          </Button>
        </div>
        <div className="data-source">
          <div>
            <h3>{t("tranco")}</h3>
            <p className="panel-lead">{t("trancoLead")}</p>
            {info.tranco ? (
              <p className="faint">
                {t("trancoImported", { rows: number(info.tranco.rows, lang, 0) })} · {date(info.tranco.imported_at, lang)}
              </p>
            ) : (
              <>
                <p className="licence">{info.tranco_notice}</p>
                <label className="oc-check">
                  <input type="checkbox" checked={accepted} onChange={(event) => setAccepted(event.target.checked)} />
                  {t("trancoAccept")}
                </label>
              </>
            )}
          </div>
          <Button small disabled={!accepted && !info.tranco} busy={tranco.isPending} onClick={() => tranco.mutate()}>
            <Download size={14} />
            {t("trancoDownload")}
          </Button>
        </div>
      </section>

      <section className="oc-panel">
        <div className="oc-panel-head">
          <h2>{t("usage")}</h2>
        </div>
        <p className="panel-lead">{t("usageLead")}</p>
        <div className="oc-table-wrap">
          <table className="oc-table">
            <thead>
              <tr>
                <th>{t("providers")}</th>
                <th>{t("month")}</th>
                <th className="num">{t("requests")}</th>
                <th className="num">{t("failures")}</th>
                <th className="num">{t("spent")}</th>
                {(usage.data ?? []).some((row) => row.credit_usd > 0) && <th className="num">{t("onCredits")}</th>}
              </tr>
            </thead>
            <tbody>
              {(usage.data ?? []).map((row) => (
                <tr key={`${row.provider}-${row.month}`}>
                  <td>{row.provider}</td>
                  <td>{row.month}</td>
                  <td className="num oc-num">{number(row.requests, lang, 0)}</td>
                  <td className="num oc-num">{number(row.failures, lang, 0)}</td>
                  <td className="num oc-num">{usd(row.cost_usd, lang)}</td>
                  {(usage.data ?? []).some((other) => other.credit_usd > 0) && <td className="num oc-num">{usd(row.credit_usd, lang)}</td>}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>

      {keyOpen && (
        <KeyDialog
          provider={{ id: "open-pagerank", name: "Open PageRank" }}
          secret={{ name: "api_key", label: "API key" }}
          store={store}
          onClose={() => setKeyOpen(false)}
          onSaved={() => {
            setKeyOpen(false);
            notify(t("keySaved", { store }));
            void queryClient.invalidateQueries({ queryKey: ["info"] });
          }}
          notify={notify}
        />
      )}
    </div>
  );
}
