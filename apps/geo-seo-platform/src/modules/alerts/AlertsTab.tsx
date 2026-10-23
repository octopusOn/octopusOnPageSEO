// A project's alerts: what was found, newest first, and what to be told about, with each
// rule's threshold, the e-mails and where notifications show. The latest few also show on the
// project's dashboard.

import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowRight, Bell, BellRing, CheckCheck, Link2, Mail, MonitorSmartphone, RefreshCw, Save } from "lucide-react";
import { Button, Empty, Field, Loading, Segmented } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { AlertEmail, AlertRow, AlertRules, AlertSettings, AlertSetup, ProjectRow } from "../../api/types";
import { LANGUAGES, useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { explain } from "../../lib/errors";
import { formatUsd, relativeTime } from "../../lib/format";
import { blockedText } from "../../lib/PaidFrame";
import { MAX_RECIPIENTS } from "../reports/report";
import { moduleOf } from "../registry";
import { AlertLine, useMarkRead } from "./AlertBell";
import { alertTarget, RULE_GROUPS, rulesOn, settingsToSave, type RuleField } from "./alert";

/** Alerts listed on the tab. */
const HISTORY_ROWS = 200;

/** Alerts on the project's dashboard. */
const RECENT_ROWS = 5;

function RuleRow({ rule, rules, onChange, setup }: { rule: RuleField; rules: AlertRules; onChange: (rules: AlertRules) => void; setup: AlertSetup }) {
  const { t, locale } = useI18n();
  const value = rules[rule.key];
  const on = typeof value === "number" ? value > 0 : value;
  const set = (next: number | boolean) => onChange({ ...rules, [rule.key]: next });
  const blocked = rule.key === "links" ? blockedText(setup.links_estimate ?? undefined, t) : null;
  return (
    <li className="alert-rule">
      <label className="oc-check">
        <input type="checkbox" checked={on} onChange={(event) => set(rule.kind === "number" ? (event.target.checked ? rule.fallback : 0) : event.target.checked)} />
        <span>{t(`alerts.rule_${rule.key}`)}</span>
      </label>
      {rule.kind === "number" && on && (
        <span className="alert-threshold">
          <input
            className="oc-input"
            type="number"
            min={1}
            max={rule.unit === "places" ? 100 : 99}
            step={rule.unit === "places" ? 1 : 0.5}
            value={Number(value)}
            aria-label={t(`alerts.rule_${rule.key}`)}
            onChange={(event) => {
              const typed = Number(event.target.value);
              if (Number.isFinite(typed) && typed > 0) set(rule.unit === "places" ? Math.round(typed) : typed);
            }}
          />
          <span className="faint small">{t(`alerts.unit_${rule.unit}`)}</span>
        </span>
      )}
      {rule.key === "links" && (
        <span className="faint small alert-rule-hint">
          {t("alerts.linksCost", { usd: formatUsd(setup.links_usd, locale) })}
          {setup.settings.links_last_run && ` · ${t("alerts.linksLast", { when: relativeTime(setup.settings.links_last_run, locale) })}`}
          {blocked && ` · ${blocked}`}
        </span>
      )}
    </li>
  );
}

function AlertForm({ project, setup }: { project: ProjectRow; setup: AlertSetup }) {
  const { t } = useI18n();
  const { notify, fail } = useShell();
  const queryClient = useQueryClient();
  const [settings, setSettings] = useState<AlertSettings>(setup.settings);
  const [recipients, setRecipients] = useState(setup.settings.recipients.join("\n"));
  const checked = settingsToSave(settings, recipients);
  const problem = "problem" in checked ? t(checked.problem, checked.values) : null;
  const save = useMutation({
    mutationFn: () => {
      if (!("settings" in checked)) throw new Error(problem ?? "");
      return api.saveAlertSettings(project.id, checked.settings);
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["alerts"] });
      notify(t("alerts.saved"));
    },
    onError: (thrown) => fail(thrown),
  });
  const check = useMutation({
    mutationFn: () => api.checkAlerts(project.id),
    onSuccess: (found) => {
      void queryClient.invalidateQueries({ queryKey: ["alerts"] });
      notify(found > 0 ? t("alerts.checkedFound", { n: found }) : t("alerts.checkedNone"));
    },
    onError: (thrown) => fail(thrown),
  });
  const links = useMutation({
    mutationFn: () => api.startLinkWatch(project.id),
    onSuccess: () => notify(t("alerts.linksStarted")),
    onError: (thrown) => fail(thrown),
  });
  const changed = JSON.stringify({ ...settings, recipients }) !== JSON.stringify({ ...setup.settings, recipients: setup.settings.recipients.join("\n") });
  return (
    <section className="oc-panel report alert-settings">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">{t("alerts.settings")}</h3>
          <span className="faint">{settings.enabled ? t("alerts.rulesOn", { n: rulesOn(settings.rules) }) : t("alerts.off")}</span>
        </div>
      </header>
      {!setup.evaluates_here && <p className="alert-note small">{t("alerts.webLooks")}</p>}
      <label className="oc-check alert-enabled">
        <input type="checkbox" checked={settings.enabled} onChange={(event) => setSettings({ ...settings, enabled: event.target.checked })} />
        <span>{t("alerts.enabled")}</span>
      </label>
      <fieldset className="alert-groups" disabled={!settings.enabled}>
        {RULE_GROUPS.map((group) => {
          const entry = moduleOf(group.page);
          const Icon = entry.icon;
          return (
            <div key={group.page} className="alert-group">
              <h4>
                <span className={`oc-tile small ${entry.tone}`} aria-hidden="true">
                  <Icon size={13} />
                </span>
                {t(entry.label)}
              </h4>
              <ul>
                {group.rules.map((rule) => (
                  <RuleRow key={rule.key} rule={rule} rules={settings.rules} setup={setup} onChange={(rules) => setSettings({ ...settings, rules })} />
                ))}
              </ul>
            </div>
          );
        })}
      </fieldset>
      <div className="alert-delivery">
        <h4>{t("alerts.delivery")}</h4>
        <p className="alert-channel small">
          <Bell size={14} aria-hidden="true" /> {t("alerts.channelApp")}
        </p>
        <p className="alert-channel small">
          <MonitorSmartphone size={14} aria-hidden="true" /> {setup.can_notify ? t("alerts.channelNotify") : t("alerts.channelNotifyDesktop")}
        </p>
        <Field label={t("alerts.email")} hint={setup.can_email ? t(`alerts.emailHint_${settings.email}`) : t("alerts.emailWeb")}>
          <Segmented<AlertEmail>
            label={t("alerts.email")}
            value={settings.email}
            onChange={(email) => setSettings({ ...settings, email })}
            options={[
              { value: "off", label: t("alerts.email_off") },
              { value: "instant", label: t("alerts.email_instant") },
              { value: "daily", label: t("alerts.email_daily") },
            ]}
          />
        </Field>
        {settings.email !== "off" && (
          <>
            <Field label={t("alerts.recipients")} hint={t("alerts.recipientsHint", { n: MAX_RECIPIENTS })}>
              <textarea className="oc-input recipients-input" value={recipients} onChange={(event) => setRecipients(event.target.value)} placeholder="ekip@example.com" spellCheck={false} />
            </Field>
            <Field label={t("alerts.language")}>
              <select className="oc-select" value={settings.lang} onChange={(event) => setSettings({ ...settings, lang: event.target.value })}>
                <option value="">{t("alerts.languageProject", { lang: project.language })}</option>
                {LANGUAGES.map((language) => (
                  <option key={language.code} value={language.code}>
                    {language.name}
                  </option>
                ))}
              </select>
            </Field>
          </>
        )}
      </div>
      <footer className="alert-actions">
        {problem && <span className="faint small dialog-problem">{problem}</span>}
        {setup.evaluates_here && (
          <Button small className="ghost" busy={check.isPending} disabled={changed || !settings.enabled} title={changed ? t("alerts.saveFirst") : undefined} onClick={() => check.mutate()}>
            <RefreshCw size={14} />
            {t("alerts.checkNow")}
          </Button>
        )}
        {setup.evaluates_here && settings.rules.links && (
          <Button small className="ghost" busy={links.isPending} disabled={changed || Boolean(setup.links_estimate?.blocked)} title={t("alerts.linksCost", { usd: formatUsd(setup.links_usd, "en") })} onClick={() => links.mutate()}>
            <Link2 size={14} />
            {t("alerts.linksNow")}
          </Button>
        )}
        <Button primary small busy={save.isPending} disabled={!changed || problem !== null} onClick={() => save.mutate()}>
          <Save size={14} />
          {t("alerts.save")}
        </Button>
      </footer>
    </section>
  );
}

function History({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const { go } = useShell();
  const [unreadOnly, setUnreadOnly] = useState(false);
  const list = useQuery({ queryKey: ["alerts", "project", project.id, unreadOnly], queryFn: () => api.listAlerts(project.id, HISTORY_ROWS, unreadOnly) });
  const read = useMarkRead();
  const unread = list.data?.by_project[String(project.id)] ?? 0;
  const open = (alert: AlertRow) => {
    if (!alert.read_at) read.mutate({ projectId: project.id, ids: [alert.id] });
    go(alertTarget(alert));
  };
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">
            <BellRing size={15} aria-hidden="true" /> {t("alerts.history")}
          </h3>
          <span className="faint">{t("alerts.unreadCount", { n: unread })}</span>
        </div>
        <div className="report-actions">
          <label className="oc-check">
            <input type="checkbox" checked={unreadOnly} onChange={(event) => setUnreadOnly(event.target.checked)} />
            <span>{t("alerts.unreadOnly")}</span>
          </label>
          {unread > 0 && (
            <Button small className="ghost" busy={read.isPending && !read.variables?.ids} onClick={() => read.mutate({ projectId: project.id })}>
              <CheckCheck size={14} />
              {t("alerts.markAllRead")}
            </Button>
          )}
        </div>
      </header>
      {list.isLoading ? (
        <Loading label={t("common.loading")} />
      ) : list.error || !list.data ? (
        <p className="warn">{explain(list.error, t).message}</p>
      ) : list.data.alerts.length === 0 ? (
        <Empty icon={<Bell size={24} />} title={t("alerts.noneTitle")}>
          <p>{unreadOnly ? t("alerts.noneUnread") : t("alerts.noneText")}</p>
        </Empty>
      ) : (
        <ul className="alert-list wide">
          {list.data.alerts.map((alert) => (
            <AlertLine key={alert.id} alert={alert} showProject={false} onOpen={open} />
          ))}
        </ul>
      )}
    </section>
  );
}

export function AlertsTab({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const setup = useQuery({ queryKey: ["alerts", "setup", project.id], queryFn: () => api.alertSetup(project.id) });
  return (
    <div className="alerts-layout">
      <History project={project} />
      {setup.isLoading ? <Loading label={t("common.loading")} /> : setup.error || !setup.data ? <p className="warn">{explain(setup.error, t).message}</p> : <AlertForm key={JSON.stringify(setup.data.settings)} project={project} setup={setup.data} />}
    </div>
  );
}

/** The project's latest alerts, for its dashboard. */
export function RecentAlerts({ project }: { project: ProjectRow }) {
  const { t } = useI18n();
  const { go } = useShell();
  const list = useQuery({ queryKey: ["alerts", "project", project.id, "recent"], queryFn: () => api.listAlerts(project.id, RECENT_ROWS) });
  const read = useMarkRead();
  const unread = list.data?.by_project[String(project.id)] ?? 0;
  return (
    <section className="oc-panel report">
      <header className="report-head">
        <div className="report-title">
          <h3 className="oc-panel-title">
            <BellRing size={15} aria-hidden="true" /> {t("project.alerts")}
          </h3>
          {unread > 0 && <span className="faint">{t("alerts.unreadCount", { n: unread })}</span>}
        </div>
        <div className="report-actions">
          <Button small className="ghost" onClick={() => go({ params: { tab: "alerts" } })}>
            {t("alerts.all")}
            <ArrowRight size={14} />
          </Button>
        </div>
      </header>
      {list.data && list.data.alerts.length > 0 ? (
        <ul className="alert-list">
          {list.data.alerts.map((alert) => (
            <AlertLine
              key={alert.id}
              alert={alert}
              showProject={false}
              onOpen={(opened) => {
                if (!opened.read_at) read.mutate({ projectId: project.id, ids: [opened.id] });
                go(alertTarget(opened));
              }}
            />
          ))}
        </ul>
      ) : (
        <p className="faint small">
          <Mail size={13} aria-hidden="true" /> {list.isLoading ? t("common.loading") : t("alerts.noneDashboard")}
        </p>
      )}
    </section>
  );
}
