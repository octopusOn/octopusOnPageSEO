// Clients and their sites.

import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { ArchiveRestore, Building2, Globe, Plus, Settings, Trash2 } from "lucide-react";

import { api } from "../api/commands";
import type { CommandError, SiteInfo } from "../api/types";
import { useI18n } from "../i18n";
import { keys, useAppState } from "../state";
import { BrandMark } from "./BrandMark";
import { ConfirmDialog } from "./Modal";
import { ClientDialog, SiteDialog } from "./SiteDialogs";
import { useToast } from "./Toasts";
import { Button } from "./ui";

export function Sidebar() {
  const { t } = useI18n();
  const notify = useToast();
  const queryClient = useQueryClient();
  const { site, setSite, page, setPage } = useAppState();
  const clients = useQuery({ queryKey: keys.clients, queryFn: api.listClients });
  const sites = useQuery({ queryKey: keys.sites, queryFn: api.listSites });
  const [filter, setFilter] = useState("");
  const [dialog, setDialog] = useState<{ kind: "client" } | { kind: "site"; client?: string } | null>(null);
  const [removing, setRemoving] = useState<string | null>(null);

  const importing = useMutation<SiteInfo | null, CommandError>({
    mutationFn: async () => {
      const path = await open({ multiple: false, filters: [{ name: "octopusSEO site", extensions: ["snxsite"] }] });
      return typeof path === "string" ? api.importSite(path, null, null) : null;
    },
    onSuccess: async (site) => {
      if (!site) return;
      await queryClient.invalidateQueries({ queryKey: keys.clients });
      await queryClient.invalidateQueries({ queryKey: keys.sites });
      setSite(site.name);
      notify("success", t("site.imported", { name: site.name }));
    },
    onError: (error) => notify("error", error.kind === "duplicate" ? t("site.importDuplicate") : error.message),
  });

  const removeClient = useMutation<void, CommandError, string>({
    mutationFn: (name) => api.removeClient(name),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: keys.clients }),
    onError: (error) => notify("error", error.message),
    onSettled: () => setRemoving(null),
  });

  const grouped = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    return (clients.data ?? []).map((client) => ({
      client,
      sites: (sites.data ?? []).filter(
        (s) => s.client === client.name && (!needle || s.name.toLowerCase().includes(needle)),
      ),
    }));
  }, [clients.data, sites.data, filter]);

  return (
    <aside className="sidebar">
      <div className="brand">
        <BrandMark />
        <div>
          <div className="brand-name">
            octopus<b>SEO</b>
          </div>
          <div className="brand-sub">Log Analyser</div>
        </div>
      </div>
      <div className="sidebar-search">
        <input value={filter} placeholder={t("sidebar.filter")} onChange={(e) => setFilter(e.target.value)} />
      </div>
      <nav className="sidebar-tree">
        {grouped.length === 0 && <div className="sidebar-empty">{t("sidebar.noClients")}</div>}
        {grouped.map(({ client, sites: clientSites }) => (
          <div className="client" key={client.id}>
            <div className="client-row">
              <Building2 size={13} />
              <span>{client.name}</span>
              <span className="actions">
                <Button
                  variant="ghost"
                  small
                  title={t("sidebar.addSite")}
                  icon={<Plus size={13} />}
                  onClick={() => setDialog({ kind: "site", client: client.name })}
                />
                {clientSites.length === 0 && (
                  <Button
                    variant="ghost"
                    small
                    title={t("client.remove")}
                    icon={<Trash2 size={13} />}
                    onClick={() => setRemoving(client.name)}
                  />
                )}
              </span>
            </div>
            {clientSites.length === 0 && <div className="sidebar-empty">{t("sidebar.noSites")}</div>}
            {clientSites.map((s) => (
              <button
                key={s.id}
                type="button"
                className={`site-item ${s.name === site && page !== "settings" ? "active" : ""}`}
                onClick={() => {
                  setSite(s.name);
                  if (page === "settings") setPage("overview");
                }}
              >
                <span className="dot" />
                <Globe size={14} />
                <span className="grow">{s.name}</span>
              </button>
            ))}
          </div>
        ))}
      </nav>
      <div className="sidebar-footer">
        <Button variant="ghost" icon={<Plus size={15} />} onClick={() => setDialog({ kind: "client" })}>
          {t("sidebar.addClient")}
        </Button>
        <Button
          variant="ghost"
          icon={<Plus size={15} />}
          disabled={(clients.data ?? []).length === 0}
          onClick={() => setDialog({ kind: "site" })}
        >
          {t("sidebar.addSite")}
        </Button>
        <Button variant="ghost" title={t("site.importArchive")} icon={<ArchiveRestore size={15} />} busy={importing.isPending} onClick={() => importing.mutate()} />
        <Button variant="ghost" title={t("nav.settings")} icon={<Settings size={15} />} onClick={() => setPage("settings")} />
      </div>
      {dialog?.kind === "client" && <ClientDialog onClose={() => setDialog(null)} />}
      {dialog?.kind === "site" && <SiteDialog client={dialog.client} onClose={() => setDialog(null)} />}
      {removing && (
        <ConfirmDialog
          title={t("client.remove")}
          message={t("client.removeConfirm", { name: removing })}
          confirmLabel={t("common.delete")}
          busy={removeClient.isPending}
          onConfirm={() => removeClient.mutate(removing)}
          onClose={() => setRemoving(null)}
        />
      )}
    </aside>
  );
}
