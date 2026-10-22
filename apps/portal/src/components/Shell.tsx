// The signed-in frame: the sidebar with the account's pages (and the admin pages for admins).

import { LogOut } from "lucide-react";
import Link from "next/link";
import type { ReactNode } from "react";

import { signOut } from "@/app/actions";
import { getDictionary } from "@/i18n";
import { legalTexts } from "@/legal/cms";
import type { SessionUser } from "@/lib/session";

import { BrandMark } from "./BrandMark";
import { LanguageMenu } from "./LanguageMenu";
import { Nav, type NavItem } from "./Nav";

export async function Shell({ user, children }: { user: SessionUser; children: ReactNode }) {
  const { t, locale, url } = await getDictionary();
  const legal = await legalTexts(locale);
  const account: NavItem[] = [
    { href: url(""), label: t.nav.dashboard, icon: "dashboard", tone: "violet", exact: true },
    { href: url("credits"), label: t.nav.buy, icon: "buy", tone: "amber" },
    { href: url("usage"), label: t.nav.usage, icon: "usage", tone: "teal" },
    { href: url("tokens"), label: t.nav.tokens, icon: "tokens", tone: "blue" },
    { href: url("invoices"), label: t.nav.invoices, icon: "invoices", tone: "pink" },
    { href: url("account"), label: t.nav.profile, icon: "profile", tone: "slate" },
  ];
  const admin: NavItem[] = [
    { href: url("admin"), label: t.nav.overview, icon: "overview", tone: "violet", exact: true },
    { href: url("admin/connectors"), label: t.nav.connectors, icon: "connectors", tone: "orange" },
    { href: url("admin/users"), label: t.nav.users, icon: "users", tone: "cyan" },
    { href: url("admin/orders"), label: t.nav.orders, icon: "orders", tone: "amber" },
    { href: url("admin/settings"), label: t.nav.settings, icon: "settings", tone: "pink" },
    { href: url("admin/reports"), label: t.nav.reports, icon: "reports", tone: "teal" },
    { href: url("admin/support"), label: t.nav.support, icon: "support", tone: "orange" },
    { href: url("admin/audit"), label: t.nav.audit, icon: "audit", tone: "slate" },
  ];
  return (
    <div className="oc-shell oc-ground portal-shell">
      <aside className="oc-rail">
        <div className="oc-brand">
          <BrandMark />
          <div>
            <div className="oc-brand-name">
              OctopusOn <span>SEO</span>
            </div>
            <div className="oc-brand-product">{t.nav.account}</div>
          </div>
        </div>
        <Nav items={account} />
        {user.role === "admin" ? <Nav items={admin} heading={t.nav.admin} /> : null}
        {legal[0] ? (
          <nav className="rail-legal small">
            <Link href={url("legal/[slug]", { slug: legal[0].slug })}>{t.nav.legal}</Link>
          </nav>
        ) : null}
        <div className="oc-rail-foot">
          <div className="rail-user">
            <strong>{user.name}</strong>
            <span>{user.email}</span>
          </div>
          <div className="rail-actions">
            <form action={signOut}>
              <button className="oc-btn ghost small">
                <LogOut size={14} />
                {t.nav.signOut}
              </button>
            </form>
            <LanguageMenu className="up" />
          </div>
        </div>
      </aside>
      <main className="oc-work">
        <div className="page">{children}</div>
      </main>
    </div>
  );
}

export function PageHead({ title, lead, actions }: { title: string; lead?: string; actions?: ReactNode }) {
  return (
    <header className="oc-pagehead">
      <div className="oc-pagehead-text">
        <h1 className="oc-pagehead-title">{title}</h1>
        {lead ? <p className="oc-pagehead-lead">{lead}</p> : null}
      </div>
      {actions ? <div className="oc-pagehead-actions">{actions}</div> : null}
    </header>
  );
}
