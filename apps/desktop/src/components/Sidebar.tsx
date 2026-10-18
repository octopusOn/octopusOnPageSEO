// The sidebar: brand, the site switcher and the modules of the selected site.

import { useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { ArchiveRestore, Building2, Check, ChevronsUpDown, Globe, Plus, Search, Trash2 } from "lucide-react";

import { api } from "../api/commands";
import type { CommandError, SiteInfo } from "../api/types";
import { useI18n } from "../i18n";
import { keys, useAppState } from "../state";
import { BrandMark } from "./BrandMark";
import { ConfirmDialog } from "./Modal";
import { SETTINGS_PAGE, SITE_PAGES, type NavItem } from "./nav";
import { ClientDialog, SiteDialog } from "./SiteDialogs";
import { useToast } from "./Toasts";
import { Button } from "./ui";

export function Sidebar() {
  const { t } = useI18n();
  const { site, page, setPage } = useAppState();

  const item = (nav: NavItem, disabled = false) => {
    const Icon = nav.icon;
    return (
      <button
        key={nav.page}
        type="button"
        className={`oc-nav-item ${page === nav.page ? "active" : ""}`}
        disabled={disabled}
        aria-current={page === nav.page ? "page" : undefined}
        onClick={() => setPage(nav.page)}
      >
        <span className={`oc-tile ${nav.tile}`}>
          <Icon size={16} strokeWidth={2.2} />
        </span>
        {t(nav.label)}
      </button>
    );
  };

  return (
    <aside className="oc-rail">
      <div className="oc-titlebar" data-tauri-drag-region />
      <div className="oc-brand" data-tauri-drag-region>
        <BrandMark />
        <div>
          <div className="oc-brand-name">OctopusOn SEO</div>
          <div className="oc-brand-product">Log Analyser</div>
        </div>
      </div>
      <SiteSwitcher />
      <nav className="oc-nav" aria-label={t("nav.modules")}>
        {SITE_PAGES.map((nav) => item(nav, !site))}
      </nav>
      <div className="oc-rail-foot">{item(SETTINGS_PAGE)}</div>
    </aside>
  );
}

/** The selected site; opens a menu of every client's sites and the actions to add more. */
function SiteSwitcher() {
  const { t } = useI18n();
  const notify = useToast();
  const queryClient = useQueryClient();
  const { site, setSite, page, setPage } = useAppState();
  const clients = useQuery({ queryKey: keys.clients, queryFn: api.listClients });
  const sites = useQuery({ queryKey: keys.sites, queryFn: api.listSites });
  const [openMenu, setOpenMenu] = useState(false);
  const [filter, setFilter] = useState("");
  const [dialog, setDialog] = useState<{ kind: "client" } | { kind: "site"; client?: string } | null>(null);
  const [removing, setRemoving] = useState<string | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const current = sites.data?.find((s) => s.name === site) ?? null;

  useEffect(() => {
    if (!openMenu) return undefined;
    const onKey = (event: KeyboardEvent) => event.key === "Escape" && setOpenMenu(false);
    const onPointer = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpenMenu(false);
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onPointer);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("pointerdown", onPointer);
    };
  }, [openMenu]);

  const importing = useMutation<SiteInfo | null, CommandError>({
    mutationFn: async () => {
      const path = await open({ multiple: false, filters: [{ name: "OctopusOn SEO site", extensions: ["snxsite"] }] });
      return typeof path === "string" ? api.importSite(path, null, null) : null;
    },
    onSuccess: async (imported) => {
      if (!imported) return;
      await queryClient.invalidateQueries({ queryKey: keys.clients });
      await queryClient.invalidateQueries({ queryKey: keys.sites });
      setSite(imported.name);
      notify("success", t("site.imported", { name: imported.name }));
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

  const choose = (name: string) => {
    setSite(name);
    if (page === "settings") setPage("overview");
    setOpenMenu(false);
    setFilter("");
  };
  const act = (action: () => void) => () => {
    setOpenMenu(false);
    action();
  };

  return (
    <div ref={root} className="switcher-root">
      <button
        type="button"
        className="oc-switcher"
        aria-haspopup="menu"
        aria-expanded={openMenu}
        onClick={() => setOpenMenu(!openMenu)}
      >
        <span className="oc-switcher-text">
          <small>{current?.client ?? t("sidebar.clients")}</small>
          <b>{current?.name ?? t("sidebar.chooseSite")}</b>
        </span>
        <ChevronsUpDown size={16} />
      </button>
      {openMenu && (
        <div className="oc-menu switcher-menu" role="menu">
          {(sites.data ?? []).length > 6 && (
            <label className="oc-search switcher-search">
              <Search size={15} />
              <input autoFocus value={filter} placeholder={t("sidebar.filter")} onChange={(e) => setFilter(e.target.value)} />
            </label>
          )}
          <div className="switcher-list">
            {grouped.length === 0 && <p className="switcher-empty">{t("sidebar.noClients")}</p>}
            {grouped.map(({ client, sites: clientSites }) => (
              <div key={client.id} className="client-row-group">
                <div className="oc-menu-heading client-row">
                  <Building2 size={13} />
                  <span className="grow">{client.name}</span>
                  <span className="actions">
                    <Button
                      variant="ghost"
                      small
                      title={t("sidebar.addSite")}
                      icon={<Plus size={13} />}
                      onClick={act(() => setDialog({ kind: "site", client: client.name }))}
                    />
                    {clientSites.length === 0 && !filter && (
                      <Button
                        variant="ghost"
                        small
                        title={t("client.remove")}
                        icon={<Trash2 size={13} />}
                        onClick={act(() => setRemoving(client.name))}
                      />
                    )}
                  </span>
                </div>
                {clientSites.length === 0 && <p className="switcher-empty">{t("sidebar.noSites")}</p>}
                {clientSites.map((s) => (
                  <button
                    key={s.id}
                    type="button"
                    role="menuitemradio"
                    aria-checked={s.name === site}
                    className={`oc-menu-item ${s.name === site ? "active" : ""}`}
                    onClick={() => choose(s.name)}
                  >
                    <Globe size={15} />
                    <span className="grow">{s.name}</span>
                    {s.name === site && <Check size={15} />}
                  </button>
                ))}
              </div>
            ))}
          </div>
          <div className="oc-menu-sep" />
          <button type="button" className="oc-menu-item" onClick={act(() => setDialog({ kind: "client" }))}>
            <Plus size={15} />
            {t("sidebar.addClient")}
          </button>
          <button
            type="button"
            className="oc-menu-item"
            disabled={(clients.data ?? []).length === 0}
            onClick={act(() => setDialog({ kind: "site" }))}
          >
            <Plus size={15} />
            {t("sidebar.addSite")}
          </button>
          <button type="button" className="oc-menu-item" onClick={act(() => importing.mutate())}>
            <ArchiveRestore size={15} />
            {t("site.importArchive")}
          </button>
        </div>
      )}
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
    </div>
  );
}
