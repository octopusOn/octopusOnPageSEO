// The signed-in frame: the sidebar with the account's pages (and the admin pages for admins).

import { LogOut } from "lucide-react";
import Link from "next/link";
import type { ReactNode } from "react";

import { setLanguage, signOut } from "@/app/actions";
import type { Dictionary } from "@/i18n";
import type { SessionUser } from "@/lib/session";

import { BrandMark } from "./BrandMark";
import { Nav, type NavItem } from "./Nav";

export function Shell({ user, t, children }: { user: SessionUser; t: Dictionary; children: ReactNode }) {
  const account: NavItem[] = [
    { href: "/", label: t.nav.dashboard, icon: "dashboard", tone: "violet" },
    { href: "/kredi", label: t.nav.buy, icon: "buy", tone: "amber" },
    { href: "/kullanim", label: t.nav.usage, icon: "usage", tone: "teal" },
    { href: "/jetonlar", label: t.nav.tokens, icon: "tokens", tone: "blue" },
    { href: "/fatura", label: t.nav.invoices, icon: "invoices", tone: "pink" },
    { href: "/hesap", label: t.nav.profile, icon: "profile", tone: "slate" },
  ];
  const admin: NavItem[] = [
    { href: "/admin", label: t.nav.overview, icon: "overview", tone: "violet" },
    { href: "/admin/baglayicilar", label: t.nav.connectors, icon: "connectors", tone: "orange" },
    { href: "/admin/kullanicilar", label: t.nav.users, icon: "users", tone: "cyan" },
    { href: "/admin/siparisler", label: t.nav.orders, icon: "orders", tone: "amber" },
    { href: "/admin/ayarlar", label: t.nav.settings, icon: "settings", tone: "pink" },
    { href: "/admin/raporlar", label: t.nav.reports, icon: "reports", tone: "teal" },
    { href: "/admin/denetim", label: t.nav.audit, icon: "audit", tone: "slate" },
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
        <nav className="rail-legal small">
          <Link href="/yasal/kredi-kosullari">{t.nav.legal}</Link>
        </nav>
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
            <form action={setLanguage}>
              <button className="oc-btn ghost small">{t.nav.language}</button>
            </form>
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
