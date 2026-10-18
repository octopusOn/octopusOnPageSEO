import Link from "next/link";
import type { ReactNode } from "react";

import { BrandMark } from "@/components/BrandMark";
import { LanguageMenu } from "@/components/LanguageMenu";
import { getDictionary } from "@/i18n";
import { legalTexts } from "@/legal/cms";

export default async function AuthLayout({ children }: { children: ReactNode }) {
  const { locale, url } = await getDictionary();
  const legal = await legalTexts(locale);
  return (
    <div className="oc-ground auth-ground">
      <div className="auth-box">
        <div className="oc-brand auth-brand">
          <BrandMark />
          <div className="oc-brand-name">
            OctopusOn <span>SEO</span>
          </div>
        </div>
        <div className="oc-card auth-card">{children}</div>
        <nav className="auth-legal">
          {legal.map((text) => (
            <Link key={text.key} href={url("legal/[slug]", { slug: text.slug })}>
              {text.label}
            </Link>
          ))}
        </nav>
        <LanguageMenu className="auth-lang up" />
      </div>
    </div>
  );
}
