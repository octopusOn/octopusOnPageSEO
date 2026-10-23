// The title bar's bell: how many alerts are unread, and the latest of every project in a list
// that opens each one where it happened and marks it read.

import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Bell, CheckCheck, Settings2 } from "lucide-react";
import { Button, Empty } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { AlertRow } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { relativeTime } from "../../lib/format";
import { moduleOf } from "../registry";
import { alertTarget, alertValues } from "./alert";

/** Alerts listed in the bell. */
const BELL_ROWS = 30;

/** How often the bell looks for alerts found in the background. */
const BELL_EVERY = 60_000;

/** One alert: its module, title, text, project and time. */
export function AlertLine({ alert, showProject, onOpen }: { alert: AlertRow; showProject: boolean; onOpen: (alert: AlertRow) => void }) {
  const { t, locale } = useI18n();
  const entry = moduleOf(alert.page);
  const Icon = entry.icon;
  const values = alertValues(alert, locale);
  return (
    <li className={`alert-line severity-${alert.severity}${alert.read_at ? "" : " unread"}`}>
      <button type="button" className="alert-open" onClick={() => onOpen(alert)}>
        <span className={`oc-tile small ${entry.tone}`} aria-hidden="true">
          <Icon size={14} />
        </span>
        <span className="alert-body">
          <strong>{t(`alerts.title_${alert.kind}`, values)}</strong>
          <span className="faint small">{t(`alerts.text_${alert.kind}`, values)}</span>
          <span className="alert-meta faint small">
            {showProject && <span>{alert.project}</span>}
            <time dateTime={alert.created_at}>{relativeTime(alert.created_at, locale)}</time>
          </span>
        </span>
        {!alert.read_at && <span className="alert-dot" aria-label={t("alerts.unread")} />}
      </button>
    </li>
  );
}

/** Marks alerts read and keeps every alert list current. */
export function useMarkRead() {
  const queryClient = useQueryClient();
  const { fail } = useShell();
  return useMutation({
    mutationFn: ({ projectId, ids }: { projectId: number | null; ids?: number[] }) => api.markAlertsRead(projectId, ids),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["alerts"] }),
    onError: (thrown) => fail(thrown),
  });
}

export function AlertBell({ className = "" }: { className?: string }) {
  const { t } = useI18n();
  const { go, project } = useShell();
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  const list = useQuery({ queryKey: ["alerts", "bell"], queryFn: () => api.listAlerts(null, BELL_ROWS), refetchInterval: BELL_EVERY });
  const read = useMarkRead();
  useEffect(() => {
    if (!open) return undefined;
    const away = (event: MouseEvent) => {
      if (!box.current?.contains(event.target as Node)) setOpen(false);
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("mousedown", away);
      document.removeEventListener("keydown", key);
    };
  }, [open]);
  const unread = list.data?.unread ?? 0;
  const alerts = list.data?.alerts ?? [];
  const projects = new Set(alerts.map((alert) => alert.project_id));
  const openAlert = (alert: AlertRow) => {
    setOpen(false);
    if (!alert.read_at) read.mutate({ projectId: alert.project_id, ids: [alert.id] });
    go(alertTarget(alert));
  };
  const label = unread > 0 ? t("alerts.bellUnread", { n: unread }) : t("alerts.bell");
  return (
    <div className={`alert-bell ${className}`} ref={box}>
      <button type="button" className="alert-bell-button" aria-haspopup="dialog" aria-expanded={open} aria-label={label} title={label} onClick={() => setOpen(!open)}>
        <Bell size={16} aria-hidden="true" />
        {unread > 0 && <span className="alert-count">{unread > 99 ? "99+" : unread}</span>}
      </button>
      {open && (
        <div className="oc-menu alert-centre" role="dialog" aria-label={t("alerts.centre")}>
          <header className="alert-centre-head">
            <strong>{t("alerts.centre")}</strong>
            {unread > 0 && (
              <Button small className="ghost" busy={read.isPending && !read.variables?.ids} onClick={() => read.mutate({ projectId: null })}>
                <CheckCheck size={14} />
                {t("alerts.markAllRead")}
              </Button>
            )}
          </header>
          {alerts.length === 0 ? (
            <Empty icon={<Bell size={22} />} title={t("alerts.noneTitle")}>
              <p className="small">{t("alerts.noneText")}</p>
            </Empty>
          ) : (
            <ul className="alert-list">
              {alerts.map((alert) => (
                <AlertLine key={alert.id} alert={alert} showProject={projects.size > 1} onOpen={openAlert} />
              ))}
            </ul>
          )}
          {project && (
            <footer className="alert-centre-foot">
              <Button
                small
                className="ghost"
                onClick={() => {
                  setOpen(false);
                  go({ page: "project", project: project.id, params: { tab: "alerts" } });
                }}
              >
                <Settings2 size={14} />
                {t("alerts.settingsOf", { project: project.name })}
              </Button>
            </footer>
          )}
        </div>
      )}
    </div>
  );
}
