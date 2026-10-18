// User-defined bots (custom user agents with a category and verification), shared with the
// CLI (`octopuson-log bots custom`). Saving re-classifies stored hits the next time a site is read.

import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open, save } from "@tauri-apps/plugin-dialog";
import { FileDown, FileUp, Pencil, Plus, Trash2 } from "lucide-react";

import { api } from "../api/commands";
import type { CommandError, CustomBot } from "../api/types";
import { useI18n } from "../i18n";
import type { MessageKey } from "../i18n/en";
import { keys } from "../state";
import { ConfirmDialog, Modal } from "./Modal";
import { useToast } from "./Toasts";
import { Badge, Button, Card, ErrorBanner, Field, Select, TextInput } from "./ui";

const CATEGORIES = ["search", "ai-training", "ai-search", "ai-assistant", "ads", "seo", "social", "monitoring", "other"];

const EMPTY: CustomBot = {
  id: "",
  name: "",
  operator: "",
  category: "other",
  patterns: [],
  ip_ranges: [],
  ip_list_url: null,
  dns_suffixes: [],
};

const lines = (text: string) =>
  text
    .split(/\r?\n|,/)
    .map((line) => line.trim())
    .filter(Boolean);

export function CustomBotsCard() {
  const { t } = useI18n();
  const notify = useToast();
  const queryClient = useQueryClient();
  const custom = useQuery({ queryKey: keys.customBots, queryFn: api.listCustomBots });
  const [editing, setEditing] = useState<{ bot: CustomBot; original: string | null } | null>(null);
  const [removing, setRemoving] = useState<string | null>(null);

  const saveAll = useMutation<CustomBot[], CommandError, CustomBot[]>({
    mutationFn: (bots) => api.saveCustomBots(bots),
    onSuccess: async (saved) => {
      queryClient.setQueryData(keys.customBots, saved);
      setEditing(null);
      setRemoving(null);
      notify("success", t("customBots.saved"));
      // Bot lists, suggestions and every site's classification may change.
      await queryClient.invalidateQueries({ queryKey: keys.bots });
      await queryClient.invalidateQueries({ queryKey: ["site"] });
      await queryClient.invalidateQueries({ queryKey: keys.ranges });
    },
  });

  const list = custom.data ?? [];
  const upsert = (bot: CustomBot, original: string | null) =>
    saveAll.mutate([...list.filter((b) => b.id !== (original ?? bot.id.trim().toLowerCase())), bot]);

  const exportBots = async () => {
    const path = await save({ defaultPath: "octopuson-custom-bots.json", filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!path) return;
    try {
      const count = await api.exportCustomBots(path);
      notify("success", t("customBots.exported", { count, path }));
    } catch (error) {
      notify("error", (error as CommandError).message);
    }
  };

  const importBots = async () => {
    const path = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
    if (typeof path !== "string") return;
    try {
      const saved = await api.importCustomBots(path);
      queryClient.setQueryData(keys.customBots, saved);
      await queryClient.invalidateQueries({ queryKey: keys.bots });
      await queryClient.invalidateQueries({ queryKey: ["site"] });
      notify("success", t("customBots.imported", { count: saved.length }));
    } catch (error) {
      notify("error", (error as CommandError).message);
    }
  };

  return (
    <Card
      title={t("customBots.title")}
      actions={
        <div className="row">
          <Button small variant="ghost" icon={<FileUp size={14} />} onClick={() => void importBots()}>
            {t("customBots.import")}
          </Button>
          <Button small variant="ghost" icon={<FileDown size={14} />} disabled={list.length === 0} onClick={() => void exportBots()}>
            {t("customBots.export")}
          </Button>
          <Button small variant="primary" icon={<Plus size={14} />} onClick={() => setEditing({ bot: EMPTY, original: null })}>
            {t("customBots.add")}
          </Button>
        </div>
      }
    >
      <p className="muted small">{t("customBots.intro")}</p>
      <ErrorBanner error={custom.error as CommandError | null} />
      {list.length === 0 ? (
        <p className="muted">{t("customBots.none")}</p>
      ) : (
        <div className="oc-table-wrap">
          <table className="oc-table">
            <thead>
              <tr>
                <th />
                <th>id</th>
                <th>{t("bots.name")}</th>
                <th>{t("bots.category")}</th>
                <th>{t("customBots.patterns")}</th>
                <th>{t("bots.verifiedBy")}</th>
              </tr>
            </thead>
            <tbody>
              {list.map((bot) => (
                <tr key={bot.id}>
                  <td className="cell-action">
                    <div className="row" style={{ gap: 2, flexWrap: "nowrap" }}>
                      <Button small variant="ghost" title={t("customBots.edit")} icon={<Pencil size={13} />} onClick={() => setEditing({ bot, original: bot.id })} />
                      <Button small variant="ghost" title={t("common.delete")} icon={<Trash2 size={13} />} onClick={() => setRemoving(bot.id)} />
                    </div>
                  </td>
                  <td className="mono">{bot.id}</td>
                  <td>{bot.name}</td>
                  <td>{t(`category.${bot.category}` as MessageKey)}</td>
                  <td className="mono small">{bot.patterns.join(" · ")}</td>
                  <td>
                    <div className="row" style={{ gap: 4 }}>
                      {bot.ip_ranges.length > 0 && <Badge tone="accent">{t("customBots.ranges", { count: bot.ip_ranges.length })}</Badge>}
                      {bot.ip_list_url && <Badge tone="accent">{t("customBots.list")}</Badge>}
                      {bot.dns_suffixes.length > 0 && <Badge tone="accent">dns</Badge>}
                      {bot.ip_ranges.length === 0 && !bot.ip_list_url && bot.dns_suffixes.length === 0 && <Badge>none</Badge>}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {editing && (
        <CustomBotDialog
          initial={editing.bot}
          busy={saveAll.isPending}
          error={saveAll.error}
          onSave={(bot) => upsert(bot, editing.original)}
          onClose={() => {
            saveAll.reset();
            setEditing(null);
          }}
        />
      )}
      {removing && (
        <ConfirmDialog
          title={t("customBots.remove")}
          message={t("customBots.removeConfirm", { id: removing })}
          confirmLabel={t("common.delete")}
          busy={saveAll.isPending}
          onConfirm={() => saveAll.mutate(list.filter((b) => b.id !== removing))}
          onClose={() => setRemoving(null)}
        />
      )}
    </Card>
  );
}

function CustomBotDialog({
  initial,
  busy,
  error,
  onSave,
  onClose,
}: {
  initial: CustomBot;
  busy: boolean;
  error: CommandError | null;
  onSave: (bot: CustomBot) => void;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const [id, setId] = useState(initial.id);
  const [name, setName] = useState(initial.name);
  const [operator, setOperator] = useState(initial.operator);
  const [category, setCategory] = useState(initial.category);
  const [patterns, setPatterns] = useState(initial.patterns.join("\n"));
  const [ranges, setRanges] = useState(initial.ip_ranges.join("\n"));
  const [url, setUrl] = useState(initial.ip_list_url ?? "");
  const [suffixes, setSuffixes] = useState(initial.dns_suffixes.join("\n"));

  const submit = () =>
    onSave({
      id: id.trim(),
      name: name.trim(),
      operator: operator.trim(),
      category,
      patterns: patterns.split(/\r?\n/).map((p) => p.trim()).filter(Boolean),
      ip_ranges: lines(ranges),
      ip_list_url: url.trim() || null,
      dns_suffixes: lines(suffixes),
    });

  return (
    <Modal
      title={initial.id ? t("customBots.edit") : t("customBots.add")}
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" busy={busy} disabled={!id.trim() || !patterns.trim()} onClick={submit}>
            {t("common.save")}
          </Button>
        </>
      }
    >
      <div className="stack">
        <div className="grid-2">
          <Field label={t("customBots.id")}>
            <TextInput className="oc-input mono" value={id} placeholder="acme-monitor" onChange={(e) => setId(e.target.value)} />
          </Field>
          <Field label={t("bots.name")}>
            <TextInput value={name} placeholder="Acme Monitor" onChange={(e) => setName(e.target.value)} />
          </Field>
          <Field label={t("bots.operator")}>
            <TextInput value={operator} onChange={(e) => setOperator(e.target.value)} />
          </Field>
          <Field label={t("bots.category")}>
            <Select value={category} onChange={(e) => setCategory(e.target.value)}>
              {CATEGORIES.map((c) => (
                <option key={c} value={c}>
                  {t(`category.${c}` as MessageKey)}
                </option>
              ))}
            </Select>
          </Field>
        </div>
        <Field label={t("customBots.patterns")}>
          <textarea className="oc-input mono" rows={2} value={patterns} placeholder={"AcmeMonitor\nacme-crawler/"} onChange={(e) => setPatterns(e.target.value)} />
          <small className="oc-field-hint">{t("customBots.patternsHint")}</small>
        </Field>
        <div className="grid-2">
          <Field label={t("customBots.rangesLabel")}>
            <textarea className="oc-input mono" rows={3} value={ranges} placeholder={"203.0.113.0/24\n2001:db8::/32"} onChange={(e) => setRanges(e.target.value)} />
          </Field>
          <Field label={t("customBots.suffixes")}>
            <textarea className="oc-input mono" rows={3} value={suffixes} placeholder="crawler.acme.com" onChange={(e) => setSuffixes(e.target.value)} />
          </Field>
        </div>
        <Field label={t("customBots.url")}>
          <TextInput className="oc-input mono" value={url} placeholder="https://acme.com/crawler-ips.json" onChange={(e) => setUrl(e.target.value)} />
          <small className="oc-field-hint">{t("customBots.urlHint")}</small>
        </Field>
        <ErrorBanner error={error} />
      </div>
    </Modal>
  );
}
