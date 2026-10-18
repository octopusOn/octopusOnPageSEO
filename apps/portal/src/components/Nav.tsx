"use client";

import {
  Activity,
  Coins,
  FileClock,
  Gauge,
  LayoutDashboard,
  LineChart,
  MonitorSmartphone,
  Receipt,
  ShoppingCart,
  Plug,
  SlidersHorizontal,
  UserCog,
  Users,
} from "lucide-react";
import Link from "next/link";
import { usePathname } from "next/navigation";

const ICONS = {
  dashboard: LayoutDashboard,
  buy: Coins,
  usage: Activity,
  tokens: MonitorSmartphone,
  invoices: Receipt,
  orders: ShoppingCart,
  profile: UserCog,
  overview: Gauge,
  connectors: Plug,
  users: Users,
  settings: SlidersHorizontal,
  reports: LineChart,
  audit: FileClock,
};

/** `exact` items (the dashboards) are active only on their own page, the others on their subpages too. */
export type NavItem = { href: string; label: string; icon: keyof typeof ICONS; tone: string; exact?: boolean };

export function Nav({ items, heading }: { items: NavItem[]; heading?: string }) {
  const path = usePathname();
  return (
    <nav className="oc-nav">
      {heading ? <div className="nav-heading type-label">{heading}</div> : null}
      {items.map((item) => {
        const Icon = ICONS[item.icon];
        const active = item.exact ? path === item.href : path === item.href || path.startsWith(`${item.href}/`);
        return (
          <Link key={item.href} href={item.href} className={`oc-nav-item ${active ? "active" : ""}`} aria-current={active ? "page" : undefined}>
            <span className={`oc-tile ${item.tone}`}>
              <Icon size={16} />
            </span>
            {item.label}
          </Link>
        );
      })}
    </nav>
  );
}
