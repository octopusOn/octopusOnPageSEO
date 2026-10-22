import { useMutation, useQueryClient } from "@tanstack/react-query";
import { CircleAlert, CloudCog, RefreshCw } from "lucide-react";
import { Button, Modal } from "@octopusseo/ui";
import { api } from "../../api/client";
import type { SyncAnswer, SyncStatus } from "../../api/types";
import { useI18n } from "../../i18n";
import { useShell } from "../../lib/context";
import { relativeTime } from "../../lib/format";
import { useSync } from "../../lib/sync";

/** Where syncing stands, in words. */
function stateOf(sync: SyncStatus): { key: string; tone: "good" | "warn" | "muted" } {
  if (!sync.signed_in) return { key: "sync.stateSignedOut", tone: "muted" };
  if (!sync.enabled) return { key: "sync.stateOff", tone: "muted" };
  if (sync.choice) return { key: "sync.stateChoice", tone: "warn" };
  if (sync.syncing) return { key: "sync.stateSyncing", tone: "good" };
  if (sync.last_error) return { key: "sync.stateError", tone: "warn" };
  if (sync.active) return { key: "sync.stateOn", tone: "good" };
  return { key: "sync.stateWaiting", tone: "muted" };
}

/** Syncing on the Settings page: the desktop's switch and state, the web's explanation. */
export function SyncCard({ web }: { web: boolean }) {
  const { t, locale } = useI18n();
  const { notify, fail, go } = useShell();
  const queryClient = useQueryClient();
  const sync = useSync();
  const update = (status: SyncStatus) => queryClient.setQueryData(["sync"], status);
  const toggle = useMutation({
    mutationFn: (enabled: boolean) => api.setSync(enabled),
    onSuccess: (status) => {
      update(status);
      notify(status.enabled ? t("sync.turnedOn") : t("sync.turnedOff"));
    },
    onError: (thrown) => fail(thrown),
  });
  const now = useMutation({
    mutationFn: () => api.syncNow(),
    onSuccess: (status) => {
      update(status);
      void queryClient.invalidateQueries();
      notify(t("sync.synced"));
    },
    onError: (thrown) => {
      void queryClient.invalidateQueries({ queryKey: ["sync"] });
      fail(thrown);
    },
  });
  if (web) {
    return (
      <section className="oc-card">
        <h2 className="oc-card-title">
          <CloudCog size={17} aria-hidden="true" /> {t("sync.title")}
        </h2>
        <p className="faint">{t("sync.webLead")}</p>
      </section>
    );
  }
  if (!sync) return null;
  const state = stateOf(sync);
  return (
    <section className="oc-card sync-card">
      <div className="card-head">
        <h2 className="oc-card-title">
          <CloudCog size={17} aria-hidden="true" /> {t("sync.title")}
        </h2>
        <span className={`oc-badge sync-state ${state.tone}`}>{t(state.key)}</span>
      </div>
      <p className="faint">{t("sync.lead", { url: sync.url.replace(/^https?:\/\//, "") })}</p>
      {!sync.signed_in ? (
        <div className="sync-actions">
          <Button primary onClick={() => go({ page: "sources" })}>
            {t("sync.connect")}
          </Button>
        </div>
      ) : (
        <>
          <dl className="facts">
            <div>
              <dt>{t("sync.lastSync")}</dt>
              <dd>{sync.last_at ? relativeTime(new Date(sync.last_at).toISOString(), locale) : t("sync.never")}</dd>
            </div>
            <div>
              <dt>{t("sync.unsent")}</dt>
              <dd>{sync.unsent}</dd>
            </div>
          </dl>
          {sync.last_error && sync.enabled && (
            <div className="oc-banner error" role="status">
              <CircleAlert size={16} aria-hidden="true" />
              <span>{t("sync.error", { error: sync.last_error })}</span>
            </div>
          )}
          {sync.enabled && <p className="faint">{t("sync.scheduleNote")}</p>}
          <div className="sync-actions">
            {sync.enabled && (
              <Button primary small busy={now.isPending || sync.syncing} onClick={() => now.mutate()}>
                <RefreshCw size={14} aria-hidden="true" />
                {t("sync.now")}
              </Button>
            )}
            <Button small busy={toggle.isPending} onClick={() => toggle.mutate(!sync.enabled)}>
              {sync.enabled ? t("sync.turnOff") : t("sync.turnOn")}
            </Button>
          </div>
        </>
      )}
    </section>
  );
}

const ANSWERS: { value: Exclude<SyncAnswer, "pause">; title: string; text: string }[] = [
  { value: "merge", title: "sync.merge", text: "sync.mergeText" },
  { value: "upload", title: "sync.upload", text: "sync.uploadText" },
  { value: "download", title: "sync.download", text: "sync.downloadText" },
];

/** The choice a sync waits for, asked wherever the person is. */
export function SyncChoiceDialog() {
  const { t } = useI18n();
  const { fail, notify } = useShell();
  const queryClient = useQueryClient();
  const sync = useSync();
  const choose = useMutation({
    mutationFn: (answer: SyncAnswer) => api.syncChoose(answer),
    onSuccess: (status, answer) => {
      queryClient.setQueryData(["sync"], status);
      void queryClient.invalidateQueries();
      notify(answer === "pause" ? t("sync.turnedOff") : t("sync.chosen"));
    },
    onError: (thrown) => fail(thrown),
  });
  const choice = sync?.choice;
  if (!choice) return null;
  return (
    <Modal
      title={t("sync.choiceTitle")}
      closeLabel={t("sync.later")}
      onClose={() => choose.mutate("pause")}
      footer={
        <Button busy={choose.isPending} onClick={() => choose.mutate("pause")}>
          {t("sync.later")}
        </Button>
      }
    >
      <p>{t(`sync.reason_${choice.reason}`, { here: choice.here, web: choice.web })}</p>
      <div className="sync-answers">
        {ANSWERS.map((answer) => (
          <button
            key={answer.value}
            type="button"
            className={`sync-answer ${answer.value === "merge" ? "recommended" : ""}`}
            disabled={choose.isPending}
            onClick={() => {
              if (answer.value !== "merge" && !window.confirm(t(`${answer.text}Confirm`))) return;
              choose.mutate(answer.value);
            }}
          >
            <strong>
              {t(answer.title)}
              {answer.value === "merge" && <span className="oc-badge">{t("sync.recommended")}</span>}
            </strong>
            <span>{t(answer.text, { here: choice.here, web: choice.web })}</span>
          </button>
        ))}
      </div>
    </Modal>
  );
}
