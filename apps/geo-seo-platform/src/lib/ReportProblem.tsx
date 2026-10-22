import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { Send } from "lucide-react";
import { Button, Field, Modal } from "@octopusseo/ui";
import { api } from "../api/client";
import { useI18n } from "../i18n";
import { explain } from "./errors";

/** Sends an error the person saw, with an optional note, to OctopusOn support. */
export function ReportProblem({ detail, page, onClose, onSent, onFailed }: { detail: string; page: string; onClose: () => void; onSent: () => void; onFailed: (message: string) => void }) {
  const { t } = useI18n();
  const [note, setNote] = useState("");
  const send = useMutation({
    mutationFn: () => api.reportProblem(note.trim(), page, detail),
    onSuccess: () => {
      onSent();
      onClose();
    },
    onError: (thrown) => onFailed(explain(thrown, t).message),
  });
  return (
    <Modal
      title={t("common.reportTitle")}
      closeLabel={t("common.close")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button primary busy={send.isPending} onClick={() => send.mutate()}>
            <Send size={15} />
            {t("common.reportSend")}
          </Button>
        </>
      }
    >
      <div className="form-grid">
        <p className="faint">{t("common.reportLead")}</p>
        <Field label={t("common.reportError")}>
          <p className="report-detail mono small">{detail}</p>
        </Field>
        <Field label={t("common.reportNote")}>
          <textarea className="oc-input report-note" value={note} maxLength={2000} onChange={(event) => setNote(event.target.value)} autoFocus />
        </Field>
      </div>
    </Modal>
  );
}
