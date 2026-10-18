// Dialogs for creating clients and sites and for changing a site's settings.

import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";

import { api } from "../api/commands";
import type { ArchiveReport, CommandError, FormatInput, SiteInfo } from "../api/types";
import { formatBytes } from "../lib/format";
import { formatInputFromSpec } from "../lib/formatSpec";
import { useI18n } from "../i18n";
import { keys, useAppState } from "../state";
import { ConfirmDialog, Modal } from "./Modal";
import { useToast } from "./Toasts";
import { Button, ErrorBanner, Field, Select, TextInput } from "./ui";

export function ClientDialog({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [name, setName] = useState("");
  const create = useMutation<unknown, CommandError>({
    mutationFn: () => api.addClient(name),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: keys.clients });
      onClose();
    },
  });
  return (
    <Modal
      title={t("client.dialogTitle")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" busy={create.isPending} disabled={!name.trim()} onClick={() => create.mutate()}>
            {t("common.create")}
          </Button>
        </>
      }
    >
      <Field label={t("client.name")}>
        <TextInput
          autoFocus
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && name.trim() && create.mutate()}
        />
      </Field>
      <ErrorBanner error={create.error} />
    </Modal>
  );
}

type FormatKind = FormatInput["kind"];

function FormatPicker({ value, onChange }: { value: FormatInput; onChange: (value: FormatInput) => void }) {
  const { t } = useI18n();
  const formats = useQuery({ queryKey: keys.formats, queryFn: api.listFormats, staleTime: Infinity });
  const selectValue = value.kind === "builtin" ? `builtin:${value.id}` : value.kind;
  const pick = (choice: string) => {
    if (choice.startsWith("builtin:")) onChange({ kind: "builtin", id: choice.slice(8) });
    else if (choice === "nginx") onChange({ kind: "nginx", format: "" });
    else if (choice === "apache") onChange({ kind: "apache", format: "" });
    else onChange({ kind: "auto" });
  };
  return (
    <>
      <Field label={t("site.format")}>
        <Select value={selectValue} onChange={(e) => pick(e.target.value)}>
          <option value="auto">{t("site.formatAuto")}</option>
          {formats.data?.map((format) => (
            <option key={format.id} value={`builtin:${format.id}`}>
              {format.name}
            </option>
          ))}
          <option value="nginx">{t("site.formatNginx")}</option>
          <option value="apache">{t("site.formatApache")}</option>
        </Select>
      </Field>
      {(value.kind === "nginx" || value.kind === "apache") && (
        <Field label={t("site.customFormat")}>
          <textarea
            className="oc-textarea"
            rows={3}
            value={value.format}
            placeholder={
              value.kind === "nginx"
                ? '$remote_addr - $remote_user [$time_local] "$request" $status $body_bytes_sent "$http_referer" "$http_user_agent"'
                : '%h %l %u %t "%r" %>s %b "%{Referer}i" "%{User-Agent}i"'
            }
            onChange={(e) => onChange({ kind: value.kind as Exclude<FormatKind, "auto" | "builtin">, format: e.target.value })}
          />
        </Field>
      )}
    </>
  );
}

export function SiteDialog({ client, onClose }: { client?: string; onClose: () => void }) {
  const { t } = useI18n();
  const { setSite, setPage } = useAppState();
  const queryClient = useQueryClient();
  const clients = useQuery({ queryKey: keys.clients, queryFn: api.listClients });
  const [owner, setOwner] = useState(client ?? "");
  const [name, setName] = useState("");
  const [format, setFormat] = useState<FormatInput>({ kind: "auto" });
  const selectedOwner = owner || clients.data?.[0]?.name || "";
  const create = useMutation<SiteInfo, CommandError>({
    mutationFn: () => api.addSite(selectedOwner, name, format),
    onSuccess: async (site) => {
      await queryClient.invalidateQueries({ queryKey: keys.sites });
      setSite(site.name);
      setPage("import");
      onClose();
    },
  });
  return (
    <Modal
      title={t("site.dialogTitle")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button
            variant="primary"
            busy={create.isPending}
            disabled={!name.trim() || !selectedOwner}
            onClick={() => create.mutate()}
          >
            {t("common.create")}
          </Button>
        </>
      }
    >
      <Field label={t("site.client")}>
        <Select value={selectedOwner} onChange={(e) => setOwner(e.target.value)}>
          {clients.data?.map((c) => (
            <option key={c.id} value={c.name}>
              {c.name}
            </option>
          ))}
        </Select>
      </Field>
      <Field label={t("site.name")}>
        <TextInput autoFocus value={name} placeholder="example.com" onChange={(e) => setName(e.target.value)} />
      </Field>
      <FormatPicker value={format} onChange={setFormat} />
      <ErrorBanner error={create.error} />
    </Modal>
  );
}

export function SiteSettingsDialog({ site, onClose }: { site: SiteInfo; onClose: () => void }) {
  const { t, locale } = useI18n();
  const notify = useToast();
  const { setSite } = useAppState();
  const queryClient = useQueryClient();
  const [format, setFormat] = useState<FormatInput>(formatInputFromSpec(site.format));
  const [confirming, setConfirming] = useState(false);
  const save = useMutation<SiteInfo, CommandError>({
    mutationFn: () => api.setSiteFormat(site.name, format),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: keys.sites });
      await queryClient.invalidateQueries({ queryKey: keys.site(site.name) });
      onClose();
    },
  });
  const remove = useMutation<void, CommandError>({
    mutationFn: () => api.removeSite(site.name),
    onSuccess: async () => {
      setSite(null);
      queryClient.removeQueries({ queryKey: keys.site(site.name) });
      await queryClient.invalidateQueries({ queryKey: keys.sites });
      notify("success", site.name);
      onClose();
    },
    onError: (error) => {
      setConfirming(false);
      notify("error", error.kind === "busy" ? t("site.busy") : error.message);
    },
  });
  const exporting = useMutation<ArchiveReport | null, CommandError>({
    mutationFn: async () => {
      const path = await saveDialog({ defaultPath: `${site.name}.snxsite`, filters: [{ name: "OctopusOn SEO site", extensions: ["snxsite"] }] });
      return path ? api.exportSite(site.name, path) : null;
    },
    onSuccess: (report) => {
      if (report) notify("success", t("site.exported", { path: report.path, size: formatBytes(report.bytes, locale) }));
    },
    onError: (error) => notify("error", error.kind === "busy" ? t("site.busy") : error.message),
  });
  if (confirming) {
    return (
      <ConfirmDialog
        title={t("site.remove")}
        message={t("site.removeConfirm", { name: site.name })}
        confirmLabel={t("common.delete")}
        busy={remove.isPending}
        onConfirm={() => remove.mutate()}
        onClose={() => setConfirming(false)}
      />
    );
  }
  return (
    <Modal
      title={`${t("site.settings")} · ${site.name}`}
      onClose={onClose}
      footer={
        <>
          <Button variant="danger" onClick={() => setConfirming(true)}>
            {t("site.remove")}
          </Button>
          <Button busy={exporting.isPending} onClick={() => exporting.mutate()}>
            {t("site.export")}
          </Button>
          <span className="grow" />
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" busy={save.isPending} onClick={() => save.mutate()}>
            {t("common.save")}
          </Button>
        </>
      }
    >
      <FormatPicker value={format} onChange={setFormat} />
      <ErrorBanner error={save.error} />
    </Modal>
  );
}
