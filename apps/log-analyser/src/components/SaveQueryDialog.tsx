// Saving the current query under a name; asks before replacing an existing one.

import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import { api } from "../api/commands";
import type { CommandError, SavedQueryInfo } from "../api/types";
import { useI18n } from "../i18n";
import { keys } from "../state";
import { Modal } from "./Modal";
import { Button, ErrorBanner, Field, TextInput } from "./ui";

export function SaveQueryDialog({
  query,
  initialName,
  initialDescription,
  onSaved,
  onClose,
}: {
  /** Full query text to save (quick filters included). */
  query: string;
  initialName: string;
  initialDescription: string;
  onSaved: (saved: SavedQueryInfo) => void;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [name, setName] = useState(initialName);
  const [description, setDescription] = useState(initialDescription);
  const [confirmReplace, setConfirmReplace] = useState(false);

  const save = useMutation<SavedQueryInfo, CommandError, boolean>({
    // Re-saving the query that is open updates it without asking.
    mutationFn: (overwrite) => api.saveQuery(name, description, query, overwrite || name.trim() === initialName.trim()),
    onSuccess: async (saved) => {
      await queryClient.invalidateQueries({ queryKey: keys.saved });
      onSaved(saved);
      onClose();
    },
    onError: (error) => {
      if (error.kind === "duplicate") setConfirmReplace(true);
    },
  });

  const empty = !query.trim();
  return (
    <Modal
      title={t("saved.dialogTitle")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          {confirmReplace ? (
            <Button variant="danger-solid" busy={save.isPending} onClick={() => save.mutate(true)}>
              {t("saved.replace")}
            </Button>
          ) : (
            <Button variant="primary" busy={save.isPending} disabled={!name.trim() || empty} onClick={() => save.mutate(false)}>
              {t("common.save")}
            </Button>
          )}
        </>
      }
    >
      <Field label={t("saved.name")}>
        <TextInput
          autoFocus
          value={name}
          onChange={(e) => {
            setName(e.target.value);
            setConfirmReplace(false);
          }}
          onKeyDown={(e) => e.key === "Enter" && name.trim() && !empty && save.mutate(confirmReplace)}
        />
      </Field>
      <Field label={t("saved.description")}>
        <TextInput value={description} onChange={(e) => setDescription(e.target.value)} />
      </Field>
      <Field label={t("saved.preview")}>
        <pre className="query-preview">{empty ? "–" : query}</pre>
      </Field>
      <p className="muted small">{empty ? t("saved.emptyQuery") : t("saved.presetsNote")}</p>
      {confirmReplace && <div className="oc-banner info">{t("saved.exists", { name: name.trim() })}</div>}
      <ErrorBanner error={save.error?.kind === "duplicate" ? null : save.error} />
    </Modal>
  );
}
