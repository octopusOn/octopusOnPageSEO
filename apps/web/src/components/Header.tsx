import Link from "next/link";
import { Download } from "lucide-react";

import type { Locale } from "@/i18n/config";
import type { Dictionary } from "@/i18n";
import { githubUrl } from "@/lib/site";
import { GithubIcon } from "./icons";
import { LanguageSwitcher } from "./LanguageSwitcher";
import { Logo } from "./Logo";
import { MobileMenu } from "./MobileMenu";

export function Header({ lang, dict }: { lang: Locale; dict: Dictionary }) {
  const github = githubUrl();
  return (
    <header className="header">
      <nav className="container nav" aria-label="Main">
        <Link href={`/${lang}`} className="nav-home" aria-label="octopusSEO">
          <Logo />
        </Link>
        <MobileMenu label={dict.nav.menu}>
          <Link href={`/${lang}/features`}>{dict.nav.features}</Link>
          <Link href={`/${lang}/docs`}>{dict.nav.docs}</Link>
          <Link href={`/${lang}/changelog`}>{dict.nav.changelog}</Link>
          {github && (
            <a href={github} rel="noopener" className="nav-icon">
              <GithubIcon size={16} /> {dict.nav.github}
            </a>
          )}
        </MobileMenu>
        <div className="nav-end">
          <LanguageSwitcher current={lang} label={dict.nav.language} />
          <Link href={`/${lang}/download`} className="button button-small button-primary nav-cta">
            <Download size={16} aria-hidden="true" />
            <span className="nav-cta-label">{dict.nav.download}</span>
          </Link>
        </div>
      </nav>
    </header>
  );
}
