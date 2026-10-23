// A keyword, domain or address shown anywhere, with a menu that opens it in the other modules:
// a keyword in the Keyword Explorer or its result page, a domain in the Site Explorer or
// Backlinks, an address's backlinks; also tracking a keyword, adding a competitor, copying.

import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { ChevronDown, Copy, ExternalLink, Globe, KeyRound, Link2, ListOrdered, PenLine, Plus, Search, TrendingUp } from "lucide-react";
import { api } from "../api/client";
import { openExternal } from "../api/transport";
import { useI18n } from "../i18n";
import { useShell } from "./context";
import { hashOf, type RouteChange } from "./route";

export type EntityKind = "keyword" | "domain" | "url";

/** The domain of an address (`https://www.x.com/a` → `x.com`). */
export function domainOf(value: string): string {
  const host = value.trim().replace(/^[a-z]+:\/\//i, "").split(/[/?#]/)[0] ?? "";
  return host.toLowerCase().replace(/^www\./, "");
}

/** Whether `value` names a page rather than a whole site. */
export function isPage(value: string): boolean {
  const rest = value.trim().replace(/^[a-z]+:\/\//i, "");
  const slash = rest.indexOf("/");
  return slash >= 0 && rest.slice(slash).replace(/\/+$/, "") !== "";
}

interface Action {
  key: string;
  label: string;
  icon: ReactNode;
  run: () => void;
}

/** Where a typed search goes: an address or domain to the Site Explorer, words to the Keyword Explorer. */
export function searchTarget(text: string): RouteChange | null {
  const value = text.trim();
  if (!value) return null;
  const looksLikeSite = /^[a-z]+:\/\//i.test(value) || (!/\s/.test(value) && /^[^\s/]+\.[a-z]{2,}(\/.*)?$/i.test(value));
  if (looksLikeSite) {
    return isPage(value) ? { page: "backlinks", params: { url: value } } : { page: "overview", params: { domain: domainOf(value) } };
  }
  return { page: "keywords", params: { keyword: value } };
}

function useActions(kind: EntityKind, value: string, close: () => void): Action[] {
  const { t } = useI18n();
  const { go, project, notify, fail } = useShell();
  const queryClient = useQueryClient();
  const track = useMutation({
    mutationFn: () => api.trackKeywords(project!.id, [{ keyword: value }]),
    onSuccess: (tracked) => {
      void queryClient.invalidateQueries({ queryKey: ["rank"] });
      notify(tracked.added ? t("rank.tracked", { n: tracked.added }) : t("rank.alreadyTracked"));
    },
    onError: (thrown) => fail(thrown),
  });
  const compete = useMutation({
    mutationFn: () =>
      api.updateProject(project!.id, {
        name: project!.name,
        domain: project!.domain,
        country: project!.country,
        language: project!.language,
        competitors: [...project!.competitors, domainOf(value)],
        brands: project!.brands,
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["projects"] });
      notify(t("overview.added"));
    },
    onError: (thrown) => fail(thrown),
  });
  const open = (change: RouteChange) => () => {
    close();
    go({ project: project?.id ?? null, ...change });
  };
  const copy = () => {
    close();
    void navigator.clipboard.writeText(value).then(
      () => notify(t("entity.copied")),
      () => notify(t("entity.copyFailed"), true),
    );
  };
  const visit = () => {
    close();
    const url = /^[a-z]+:\/\//i.test(value) ? value : `https://${value}`;
    openExternal(url, false);
  };
  const actions: Action[] = [];
  if (kind === "keyword") {
    actions.push(
      { key: "analyze", label: t("entity.keywordOverview"), icon: <KeyRound size={14} />, run: open({ page: "keywords", params: { keyword: value } }) },
      { key: "serp", label: t("entity.keywordSerp"), icon: <ListOrdered size={14} />, run: open({ page: "keywords", params: { keyword: value, tab: "serp" } }) },
      { key: "ideas", label: t("entity.keywordIdeas"), icon: <Search size={14} />, run: open({ page: "keywords", params: { keyword: value, tab: "ideas" } }) },
    );
    if (project) {
      actions.push({ key: "brief", label: t("entity.contentBrief"), icon: <PenLine size={14} />, run: open({ page: "content", params: { keyword: value } }) });
      actions.push({
        key: "track",
        label: t("entity.track"),
        icon: <TrendingUp size={14} />,
        run: () => {
          close();
          track.mutate();
        },
      });
    }
  } else {
    const domain = domainOf(value);
    actions.push({ key: "overview", label: t("entity.domainOverview"), icon: <Globe size={14} />, run: open({ page: "overview", params: { domain } }) });
    if (kind === "url") {
      actions.push({ key: "keywords", label: t("entity.pageKeywords"), icon: <KeyRound size={14} />, run: open({ page: "overview", params: { domain, tab: "keywords", url: value } }) });
    }
    actions.push({
      key: "backlinks",
      label: kind === "url" ? t("entity.pageBacklinks") : t("entity.domainBacklinks"),
      icon: <Link2 size={14} />,
      run: open(kind === "url" ? { page: "backlinks", params: { url: value } } : { page: "backlinks", params: { domain } }),
    });
    if (project && kind === "domain" && domain !== project.domain && !project.competitors.includes(domain)) {
      actions.push({
        key: "compete",
        label: t("entity.addCompetitor"),
        icon: <Plus size={14} />,
        run: () => {
          close();
          compete.mutate();
        },
      });
    }
    actions.push({ key: "visit", label: t("entity.visit"), icon: <ExternalLink size={14} />, run: visit });
  }
  actions.push({ key: "copy", label: t("entity.copy"), icon: <Copy size={14} />, run: copy });
  return actions;
}

/** The menu, drawn on the document body under its button. */
function Menu({ anchor, kind, value, onClose }: { anchor: HTMLElement; kind: EntityKind; value: string; onClose: () => void }) {
  const actions = useActions(kind, value, onClose);
  const ref = useRef<HTMLDivElement>(null);
  const [place, setPlace] = useState({ top: 0, left: 0 });
  useLayoutEffect(() => {
    const box = anchor.getBoundingClientRect();
    const width = ref.current?.offsetWidth ?? 220;
    const height = ref.current?.offsetHeight ?? 200;
    const below = box.bottom + 4 + height < window.innerHeight;
    setPlace({
      top: below ? box.bottom + 4 : Math.max(8, box.top - 4 - height),
      left: Math.min(Math.max(8, box.left), window.innerWidth - width - 8),
    });
  }, [anchor]);
  useEffect(() => {
    const away = (event: MouseEvent) => {
      if (!ref.current?.contains(event.target as Node) && !anchor.contains(event.target as Node)) onClose();
    };
    const key = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", key);
    window.addEventListener("scroll", onClose, true);
    ref.current?.querySelector<HTMLButtonElement>("button")?.focus();
    return () => {
      document.removeEventListener("mousedown", away);
      document.removeEventListener("keydown", key);
      window.removeEventListener("scroll", onClose, true);
    };
  }, [anchor, onClose]);
  return createPortal(
    <div ref={ref} className="oc-menu entity-menu" role="menu" style={{ top: place.top, left: place.left }}>
      <div className="oc-menu-heading" title={value}>
        {value}
      </div>
      {actions.map((action) => (
        <button key={action.key} type="button" role="menuitem" className="oc-menu-item" onClick={action.run}>
          {action.icon}
          {action.label}
        </button>
      ))}
    </div>,
    document.body,
  );
}

/** Where clicking the entity itself goes. */
export function primaryTarget(kind: EntityKind, value: string): RouteChange {
  if (kind === "keyword") return { page: "keywords", params: { keyword: value } };
  if (kind === "url") return { page: "backlinks", params: { url: value } };
  return { page: "overview", params: { domain: domainOf(value) } };
}

/**
 * A keyword, domain or address: a link to its main report (a real link, so it opens in a new
 * tab on the web) and a menu of everything else to do with it.
 */
export function Entity({ kind, value, children, className = "" }: { kind: EntityKind; value: string; children?: ReactNode; className?: string }) {
  const { t } = useI18n();
  const { project } = useShell();
  const button = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const close = useCallback(() => setOpen(false), []);
  if (!value) return <>{children}</>;
  const target = primaryTarget(kind, value);
  const href = hashOf({ page: target.page ?? "dashboard", project: project?.id ?? null, params: target.params ?? {} });
  return (
    <span className={`entity ${className}`.trim()}>
      <a className="entity-link" href={href} title={value} onClick={(event) => event.stopPropagation()}>
        {children ?? value}
      </a>
      <button
        ref={button}
        type="button"
        className="entity-caret"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={t("entity.menu", { value })}
        title={t("entity.menu", { value })}
        onClick={(event) => {
          event.stopPropagation();
          setOpen(!open);
        }}
      >
        <ChevronDown size={12} aria-hidden="true" />
      </button>
      {open && button.current && <Menu anchor={button.current} kind={kind} value={value} onClose={close} />}
    </span>
  );
}
