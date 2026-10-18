import Link from "next/link";
import type { ReactNode } from "react";

import { BrandMark } from "@/components/BrandMark";

export default function AuthLayout({ children }: { children: ReactNode }) {
  return (
    <div className="oc-ground auth-ground">
      <div className="auth-box">
        <div className="oc-brand auth-brand">
          <BrandMark />
          <div className="oc-brand-name">
            octopus<span>SEO</span>
          </div>
        </div>
        <div className="oc-card auth-card">{children}</div>
        <nav className="auth-legal">
          <Link href="/yasal/kredi-kosullari">Kredi koşulları</Link>
          <Link href="/yasal/kvkk">KVKK</Link>
          <Link href="/yasal/cerezler">Çerezler</Link>
        </nav>
      </div>
    </div>
  );
}
